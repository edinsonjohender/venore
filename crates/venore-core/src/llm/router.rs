//! LLM Router
//!
//! Handles provider selection, retry logic, and fallback mechanisms.
//! This is the orchestration layer between the gateway and providers.

use crate::traits::{ApiKeyStore, LlmProviderType, LlmTask};
use crate::{Result, VenoreError};

use super::config::TaskConfig;
use super::error::{should_retry, should_fallback};
use super::types::{LlmRequest, LlmResponse, LlmStream};

// ============================================================================
// ROUTER OPTIONS
// ============================================================================

/// Options for routing LLM requests
#[derive(Debug, Clone)]
pub struct RouterOptions {
    /// Task context
    pub task: LlmTask,

    /// Override provider (if None, uses default from config)
    pub provider: Option<LlmProviderType>,

    /// Override model (if None, uses default from config)
    pub model: Option<String>,

    /// Task configuration
    pub config: TaskConfig,
}

// ============================================================================
// ROUTING FUNCTIONS
// ============================================================================

/// Route a completion request through providers with retry/fallback
///
/// # Flow
///
/// 1. Build provider list (primary + fallbacks)
/// 2. For each provider:
///    a. Get API key
///    b. Execute with retry logic
///    c. If fails with fallback-able error, try next provider
/// 3. Return result or error
pub async fn route_completion(
    options: RouterOptions,
    request: LlmRequest,
    key_store: &dyn ApiKeyStore,
) -> Result<LlmResponse> {
    // Build ordered list of providers to try
    let providers = build_provider_list(&options, key_store).await?;

    if providers.is_empty() {
        return Err(VenoreError::LlmNoApiKey(
            "No AI providers configured. Add an API key in Settings → AI Configuration.".into(),
        ));
    }

    let mut last_error: Option<VenoreError> = None;

    // Try each provider in order
    for (provider, model) in providers {
        tracing::info!(
            "Attempting completion with provider: {} (model: {})",
            provider.as_str(),
            model
        );

        // Get API key (Ollama doesn't need one - it's a local service)
        let api_key = if provider == LlmProviderType::Ollama {
            String::new()
        } else {
            match key_store.get_api_key(provider).await? {
                Some(key) => key,
                None => {
                    tracing::warn!("No API key for provider: {}", provider.as_str());
                    continue;
                }
            }
        };

        // Prepare request with correct model
        let mut provider_request = request.clone();
        provider_request.model = model.clone();

        // Execute with retry
        match execute_with_retry(provider, &api_key, provider_request, &options.config).await {
            Ok(response) => {
                tracing::info!(
                    "Successfully completed request with provider: {}",
                    provider.as_str()
                );
                return Ok(response);
            }
            Err(e) => {
                tracing::warn!(
                    "Provider {} failed: {}",
                    provider.as_str(),
                    e
                );

                let should_try_fallback = should_fallback(&e);
                last_error = Some(e);

                if !should_try_fallback {
                    // Error is not fallback-able, return immediately
                    break;
                }

                // Continue to next provider
            }
        }
    }

    // All providers failed
    Err(last_error.unwrap_or_else(|| {
        VenoreError::LlmProviderError(
            "No AI provider could fulfill this request. Check your provider configuration.".into(),
        )
    }))
}

/// Route a streaming request through providers
pub async fn route_stream(
    options: RouterOptions,
    request: LlmRequest,
    key_store: &dyn ApiKeyStore,
) -> Result<LlmStream> {
    // For now, streaming doesn't support fallback
    // (would require buffering and replay)

    let provider = options.provider.unwrap_or(options.config.default_provider);
    let model = options.model.unwrap_or(options.config.default_model.clone());

    // Get API key (Ollama doesn't need one - it's a local service)
    let api_key = if provider == LlmProviderType::Ollama {
        String::new()
    } else {
        key_store.get_api_key(provider).await?
            .ok_or_else(|| VenoreError::LlmNoApiKey(provider.as_str().to_string()))?
    };

    // Prepare request
    let mut provider_request = request;
    provider_request.model = model;

    // Get provider implementation
    let provider_impl = super::providers::factory::get_provider(provider)?;

    // Execute stream
    provider_impl.stream(&api_key, provider_request).await
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Build ordered list of (provider, model) to try
async fn build_provider_list(
    options: &RouterOptions,
    key_store: &dyn ApiKeyStore,
) -> Result<Vec<(LlmProviderType, String)>> {
    let mut providers = Vec::new();

    // If provider is explicitly specified, use only that one
    if let Some(provider) = options.provider {
        let model = options.model.clone()
            .unwrap_or_else(|| super::registry::get_default_model(provider));

        providers.push((provider, model));
        return Ok(providers);
    }

    // Otherwise, use default + fallbacks from config
    let primary_provider = options.config.default_provider;
    let primary_model = options.model.clone()
        .unwrap_or_else(|| options.config.default_model.clone());

    // Add primary provider if it has a key
    if key_store.has_api_key(primary_provider).await? {
        providers.push((primary_provider, primary_model));
    }

    // Add fallback providers
    for fallback in &options.config.fallbacks {
        if key_store.has_api_key(*fallback).await? {
            let model = super::registry::get_default_model(*fallback);
            providers.push((*fallback, model));
        }
    }

    Ok(providers)
}

/// Execute request with exponential backoff retry
async fn execute_with_retry(
    provider: LlmProviderType,
    api_key: &str,
    request: LlmRequest,
    config: &TaskConfig,
) -> Result<LlmResponse> {
    let mut attempts = 0;
    let max_retries = config.max_retries;

    loop {
        attempts += 1;

        tracing::debug!(
            "Attempt {}/{} for provider {}",
            attempts,
            max_retries,
            provider.as_str()
        );

        // Get provider implementation
        let provider_impl = super::providers::factory::get_provider(provider)?;

        // Execute request
        match provider_impl.complete(api_key, request.clone()).await {
            Ok(response) => return Ok(response),
            Err(e) => {
                if !should_retry(&e) || attempts >= max_retries {
                    return Err(e);
                }

                // Extract Retry-After from error if available
                let retry_after = super::error::extract_retry_after(&e);

                // Calculate backoff delay
                let backoff_config = super::utils::backoff::BackoffConfig {
                    base_delay_ms: config.retry_delay_ms,
                    max_delay_ms: 60_000, // 60s cap
                    retry_after_secs: retry_after,
                };
                let delay = super::utils::backoff::exponential_backoff_with_config(
                    attempts - 1, // 0-indexed
                    &backoff_config,
                );

                if let Some(retry_secs) = retry_after {
                    tracing::info!(
                        "Retrying after {}s (server Retry-After)",
                        retry_secs
                    );
                } else {
                    tracing::info!(
                        "Retrying after {}ms (exponential backoff, attempt {}/{})",
                        delay,
                        attempts,
                        max_retries
                    );
                }

                tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
            }
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::RwLock;

    struct MockKeyStore {
        keys: RwLock<HashMap<LlmProviderType, String>>,
    }

    impl MockKeyStore {
        fn new() -> Self {
            Self {
                keys: RwLock::new(HashMap::new()),
            }
        }

        fn with_key(mut self, provider: LlmProviderType, key: &str) -> Self {
            self.keys.get_mut().unwrap().insert(provider, key.to_string());
            self
        }
    }

    #[async_trait::async_trait]
    impl ApiKeyStore for MockKeyStore {
        async fn store_api_key(&self, provider: LlmProviderType, key: String) -> Result<()> {
            self.keys.write().unwrap().insert(provider, key);
            Ok(())
        }

        async fn get_api_key(&self, provider: LlmProviderType) -> Result<Option<String>> {
            Ok(self.keys.read().unwrap().get(&provider).cloned())
        }

        async fn has_api_key(&self, provider: LlmProviderType) -> Result<bool> {
            Ok(self.keys.read().unwrap().contains_key(&provider))
        }

        async fn remove_api_key(&self, provider: LlmProviderType) -> Result<()> {
            self.keys.write().unwrap().remove(&provider);
            Ok(())
        }

        async fn list_configured_providers(&self) -> Result<Vec<LlmProviderType>> {
            Ok(self.keys.read().unwrap().keys().copied().collect())
        }
    }

    #[tokio::test]
    async fn test_build_provider_list_with_explicit_provider() {
        let store = MockKeyStore::new()
            .with_key(LlmProviderType::Anthropic, "key1");

        let options = RouterOptions {
            task: LlmTask::Chat,
            provider: Some(LlmProviderType::Anthropic),
            model: Some("custom-model".into()),
            config: super::super::config::get_task_config(LlmTask::Chat),
        };

        let providers = build_provider_list(&options, &store).await.unwrap();

        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].0, LlmProviderType::Anthropic);
        assert_eq!(providers[0].1, "custom-model");
    }

    #[tokio::test]
    async fn test_build_provider_list_with_defaults() {
        let store = MockKeyStore::new()
            .with_key(LlmProviderType::Anthropic, "key1")
            .with_key(LlmProviderType::OpenAI, "key2");

        let mut config = super::super::config::get_task_config(LlmTask::Chat);
        config.fallbacks = vec![LlmProviderType::OpenAI];

        let options = RouterOptions {
            task: LlmTask::Chat,
            provider: None,
            model: None,
            config,
        };

        let providers = build_provider_list(&options, &store).await.unwrap();

        // Should have primary + fallback
        assert!(!providers.is_empty());
        assert_eq!(providers[0].0, LlmProviderType::Anthropic);
    }

    #[tokio::test]
    async fn test_build_provider_list_no_keys() {
        let store = MockKeyStore::new();

        let options = RouterOptions {
            task: LlmTask::Chat,
            provider: None,
            model: None,
            config: super::super::config::get_task_config(LlmTask::Chat),
        };

        let providers = build_provider_list(&options, &store).await.unwrap();
        assert_eq!(providers.len(), 0);
    }
}
