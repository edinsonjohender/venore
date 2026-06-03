//! LLM Gateway
//!
//! Main entry point for LLM operations. Provides a unified interface for:
//! - Text completion (synchronous)
//! - Streaming responses
//! - Connection testing
//! - Provider management

use std::sync::Arc;

use crate::traits::{ApiKeyStore, LlmProviderType, LlmTask, TaskConfigStore};
use crate::{Result, VenoreError};

use super::types::{LlmRequest, LlmResponse, LlmStream, ProviderTestResult};

// ============================================================================
// GATEWAY OPTIONS
// ============================================================================

/// Options for gateway operations
#[derive(Debug, Clone)]
pub struct GatewayOptions {
    /// Task context (determines default config)
    pub task: LlmTask,

    /// Override default provider
    pub provider: Option<LlmProviderType>,

    /// Override default model
    pub model: Option<String>,

    /// Override temperature
    pub temperature: Option<f32>,

    /// Override max tokens
    pub max_tokens: Option<u32>,
}

impl GatewayOptions {
    /// Create options for a task with defaults
    pub fn for_task(task: LlmTask) -> Self {
        Self {
            task,
            provider: None,
            model: None,
            temperature: None,
            max_tokens: None,
        }
    }

    /// Set provider override
    pub fn with_provider(mut self, provider: LlmProviderType) -> Self {
        self.provider = Some(provider);
        self
    }

    /// Set model override
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set temperature override
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set max tokens override
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }
}

// ============================================================================
// GATEWAY
// ============================================================================

/// LLM Gateway - Main entry point for LLM operations
///
/// Provides a unified interface for interacting with multiple LLM providers.
/// Handles provider selection, retry logic, and fallback mechanisms.
///
/// # Example
///
/// ```ignore
/// use venore_core::llm::{LlmGateway, GatewayOptions, LlmRequest, LlmMessage, MessageRole};
/// use venore_core::traits::LlmTask;
///
/// let gateway = LlmGateway::new(api_key_store);
///
/// let request = LlmRequest {
///     model: "claude-sonnet-4-5".into(),
///     messages: vec![
///         LlmMessage {
///             role: MessageRole::User,
///             content: "Hello!".into(),
///             tool_call_id: None,
///             tool_calls: None,
///             content_parts: None,
///         }
///     ],
///     temperature: None,
///     max_tokens: None,
///     tools: None,
///     json_schema: None,
///     timeout_secs: None,
/// };
///
/// let options = GatewayOptions::for_task(LlmTask::Chat);
/// let response = gateway.complete(request, options).await?;
/// ```
pub struct LlmGateway {
    key_store: Box<dyn ApiKeyStore>,
    task_config_store: Option<Arc<dyn TaskConfigStore>>,
}

impl LlmGateway {
    /// Create a new gateway with the given API key storage.
    /// Uses hardcoded defaults as fallback (tests, CLI).
    pub fn new(key_store: Box<dyn ApiKeyStore>) -> Self {
        Self { key_store, task_config_store: None }
    }

    /// Create a gateway that reads user's DB-persisted task settings (production).
    pub fn with_config_store(
        key_store: Box<dyn ApiKeyStore>,
        task_config_store: Arc<dyn TaskConfigStore>,
    ) -> Self {
        Self { key_store, task_config_store: Some(task_config_store) }
    }

    /// Complete a request (non-streaming)
    ///
    /// This is the main method for getting LLM completions. It:
    /// 1. Loads task configuration
    /// 2. Applies any overrides from options
    /// 3. Delegates to router for execution (with retry/fallback)
    /// 4. Returns the complete response
    ///
    /// # Arguments
    ///
    /// * `request` - The LLM request to execute
    /// * `options` - Gateway options (task, provider overrides, etc.)
    ///
    /// # Returns
    ///
    /// * `Ok(LlmResponse)` - Successful response from LLM
    /// * `Err(VenoreError)` - If all attempts fail
    ///
    /// # Errors
    ///
    /// * `LlmNoApiKey` - No API key configured for provider
    /// * `LlmProviderError` - Provider-specific error
    /// * `LlmRateLimit` - Rate limit exceeded (after retries)
    /// * `Timeout` - Request timed out
    pub async fn complete(
        &self,
        request: LlmRequest,
        options: GatewayOptions,
    ) -> Result<LlmResponse> {
        // Resolve config from DB → overrides → hardcoded fallback
        let (provider, model, config) = self.resolve_config(&options).await;
        self.validate_model(provider, &model)?;

        // Build router options — always Some after resolution
        let router_options = super::router::RouterOptions {
            task: options.task,
            provider: Some(provider),
            model: Some(model),
            config,
        };

        // Apply overrides to request
        let mut request = request;
        if let Some(temp) = options.temperature {
            request.temperature = Some(temp);
        }
        if let Some(tokens) = options.max_tokens {
            request.max_tokens = Some(tokens);
        }

        // Delegate to router
        super::router::route_completion(router_options, request, self.key_store.as_ref()).await
    }

    /// Stream a request
    ///
    /// Similar to `complete()` but returns a stream of chunks instead of
    /// a single response. Useful for real-time UI updates.
    ///
    /// # Arguments
    ///
    /// * `request` - The LLM request to execute
    /// * `options` - Gateway options
    ///
    /// # Returns
    ///
    /// * `Ok(LlmStream)` - Stream of response chunks
    /// * `Err(VenoreError)` - If stream cannot be initiated
    pub async fn stream(
        &self,
        request: LlmRequest,
        options: GatewayOptions,
    ) -> Result<LlmStream> {
        // Resolve config from DB → overrides → hardcoded fallback
        let (provider, model, config) = self.resolve_config(&options).await;
        self.validate_model(provider, &model)?;

        // Build router options — always Some after resolution
        let router_options = super::router::RouterOptions {
            task: options.task,
            provider: Some(provider),
            model: Some(model),
            config,
        };

        // Apply overrides to request
        let mut request = request;
        if let Some(temp) = options.temperature {
            request.temperature = Some(temp);
        }
        if let Some(tokens) = options.max_tokens {
            request.max_tokens = Some(tokens);
        }

        // Delegate to router
        super::router::route_stream(router_options, request, self.key_store.as_ref()).await
    }

    /// Resolve the provider and model that will be used for a given set of options.
    ///
    /// Resolution order: DB-persisted settings → GatewayOptions overrides → hardcoded fallback.
    /// Use this when you need to know the model before calling `stream()` or `complete()`.
    pub async fn resolve_model(&self, options: &GatewayOptions) -> (LlmProviderType, String) {
        let (provider, model, _) = self.resolve_config(options).await;
        (provider, model)
    }

    /// Resolve provider/model from: DB settings → GatewayOptions overrides → hardcoded fallback.
    async fn resolve_config(&self, options: &GatewayOptions)
        -> (LlmProviderType, String, super::config::TaskConfig)
    {
        #[allow(deprecated)]
        let mut config = super::config::get_task_config(options.task);

        // Read from DB if available
        if let Some(ref store) = self.task_config_store {
            if let Ok(settings) = store.get_task_settings(options.task).await {
                config.default_provider = settings.provider;
                config.default_model = settings.model;
                if let Some(t) = settings.temperature { config.temperature = t; }
                if let Some(t) = settings.max_tokens { config.max_tokens = t; }
                if let Some(t) = settings.timeout_ms { config.timeout_ms = t; }
            }
        }

        // GatewayOptions overrides take priority (for callers that truly need to override)
        let provider = options.provider.unwrap_or(config.default_provider);
        let model = options.model.clone().unwrap_or_else(|| config.default_model.clone());

        (provider, model, config)
    }

    /// Validate that the resolved model is known to the registry.
    /// Ollama models are user-managed, so skip validation for them.
    fn validate_model(&self, provider: LlmProviderType, model: &str) -> Result<()> {
        if provider == LlmProviderType::Ollama {
            return Ok(());
        }
        if !super::registry::is_model_supported(provider, model) {
            return Err(VenoreError::LlmModelNotAvailable {
                provider: provider.as_str().to_string(),
                model: model.to_string(),
            });
        }
        Ok(())
    }

    /// Test connection to a provider
    ///
    /// Sends a minimal request to verify:
    /// 1. API key is valid
    /// 2. Provider is reachable
    /// 3. Model is accessible
    ///
    /// # Arguments
    ///
    /// * `provider` - Provider to test
    /// * `model` - Optional model to test (uses default if None)
    ///
    /// # Returns
    ///
    /// * `Ok(ProviderTestResult)` - Test result with latency
    pub async fn test_connection(
        &self,
        provider: LlmProviderType,
        model: Option<String>,
    ) -> Result<ProviderTestResult> {
        use std::time::Instant;

        // Ollama doesn't need API key (local service)
        let api_key = if provider == LlmProviderType::Ollama {
            String::new() // Empty key for Ollama
        } else {
            // Check if API key exists for cloud providers
            if !self.has_api_key(provider) {
                return Ok(ProviderTestResult {
                    success: false,
                    latency_ms: 0,
                    model: model.unwrap_or_else(|| super::registry::get_default_model(provider)),
                    error: Some(format!("No API key configured for {}", provider.as_str())),
                });
            }

            // Get API key
            self.key_store.get_api_key(provider).await?
                .ok_or_else(|| VenoreError::LlmNoApiKey(provider.as_str().to_string()))?
        };

        // Get or use default model
        let model = model.unwrap_or_else(|| super::registry::get_default_model(provider));

        // Get provider implementation
        let provider_impl = super::providers::factory::get_provider(provider)?;

        // Test connection
        let start = Instant::now();
        let result = provider_impl.test(&api_key, &model).await;
        let latency = start.elapsed().as_millis() as u64;

        match result {
            Ok(mut test_result) => {
                test_result.latency_ms = latency;
                Ok(test_result)
            }
            Err(e) => Ok(ProviderTestResult {
                success: false,
                latency_ms: latency,
                model,
                error: Some(e.to_string()),
            }),
        }
    }

    /// Check if an API key is configured for a provider
    pub fn has_api_key(&self, provider: LlmProviderType) -> bool {
        // This is a sync wrapper around the async method
        // In production, this should be called from an async context
        // or the ApiKeyStore trait should have a sync has_api_key method
        futures::executor::block_on(async {
            self.key_store.has_api_key(provider).await.unwrap_or(false)
        })
    }

    /// Get list of providers that have API keys configured
    pub fn configured_providers(&self) -> Vec<LlmProviderType> {
        // Sync wrapper around async method
        futures::executor::block_on(async {
            self.key_store.list_configured_providers().await.unwrap_or_default()
        })
    }

    /// Get reference to key store (for advanced usage)
    pub fn key_store(&self) -> &dyn ApiKeyStore {
        self.key_store.as_ref()
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::RwLock;

    // Simple mock for testing
    struct MockKeyStore {
        keys: RwLock<HashMap<LlmProviderType, String>>,
    }

    impl MockKeyStore {
        fn new() -> Self {
            Self {
                keys: RwLock::new(HashMap::new()),
            }
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

    #[test]
    fn test_gateway_options_builder() {
        let options = GatewayOptions::for_task(LlmTask::Chat)
            .with_provider(LlmProviderType::Anthropic)
            .with_model("claude-haiku-4-5")
            .with_temperature(0.5)
            .with_max_tokens(1000);

        assert_eq!(options.task, LlmTask::Chat);
        assert_eq!(options.provider, Some(LlmProviderType::Anthropic));
        assert_eq!(options.model, Some("claude-haiku-4-5".to_string()));
        assert_eq!(options.temperature, Some(0.5));
        assert_eq!(options.max_tokens, Some(1000));
    }

    #[tokio::test]
    async fn test_gateway_creation() {
        let key_store = Box::new(MockKeyStore::new());
        let gateway = LlmGateway::new(key_store);

        assert_eq!(gateway.configured_providers().len(), 0);
    }

    #[tokio::test]
    async fn test_has_api_key() {
        let key_store = Box::new(MockKeyStore::new());
        let gateway = LlmGateway::new(key_store);

        // Store a key via the gateway's key_store
        gateway.key_store()
            .store_api_key(LlmProviderType::Anthropic, "test-key".to_string())
            .await
            .unwrap();

        assert!(gateway.has_api_key(LlmProviderType::Anthropic));
        assert!(!gateway.has_api_key(LlmProviderType::OpenAI));
    }

    #[tokio::test]
    async fn test_configured_providers() {
        let key_store = Box::new(MockKeyStore::new());
        let gateway = LlmGateway::new(key_store);

        // Add some keys
        gateway.key_store()
            .store_api_key(LlmProviderType::Anthropic, "key1".to_string())
            .await
            .unwrap();
        gateway.key_store()
            .store_api_key(LlmProviderType::OpenAI, "key2".to_string())
            .await
            .unwrap();

        let providers = gateway.configured_providers();
        assert_eq!(providers.len(), 2);
        assert!(providers.contains(&LlmProviderType::Anthropic));
        assert!(providers.contains(&LlmProviderType::OpenAI));
    }
}
