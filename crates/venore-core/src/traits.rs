//! Traits for LLM integration and configuration
//!
//! Core traits for:
//! - LLM provider implementations (Anthropic, OpenAI, Gemini)
//! - Configuration storage (API keys, task settings)

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::Result;

// ============================================================================
// LLM CONFIGURATION TRAITS
// ============================================================================

/// Provider types soportados
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LlmProviderType {
    Anthropic,
    OpenAI,
    Gemini,
    Ollama,
    /// Tavily web search API (not an LLM, but uses the same keyring pattern)
    Tavily,
}

impl LlmProviderType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::OpenAI => "openai",
            Self::Gemini => "gemini",
            Self::Ollama => "ollama",
            Self::Tavily => "tavily",
        }
    }
}

impl std::str::FromStr for LlmProviderType {
    type Err = crate::VenoreError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "anthropic" => Ok(Self::Anthropic),
            "openai" => Ok(Self::OpenAI),
            "gemini" => Ok(Self::Gemini),
            "ollama" => Ok(Self::Ollama),
            "tavily" => Ok(Self::Tavily),
            _ => Err(crate::VenoreError::LlmInvalidProvider(s.to_string())),
        }
    }
}

/// Task types for LLM configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LlmTask {
    /// Onboarding: generates .context.md
    Onboarding,
    /// Chat: general conversation
    Chat,
    /// Analysis: code analysis
    Analysis,
    /// Embeddings: RAG vector search (different API than completion).
    /// Stored here so the user can pick provider+model from the same
    /// config UI; the actual embedding endpoint is invoked elsewhere.
    Embeddings,
}

/// Store for API keys (secure storage)
#[async_trait]
pub trait ApiKeyStore: Send + Sync {
    /// Store an API key securely
    async fn store_api_key(&self, provider: LlmProviderType, key: String) -> Result<()>;

    /// Retrieve a decrypted API key
    async fn get_api_key(&self, provider: LlmProviderType) -> Result<Option<String>>;

    /// Check whether an API key exists
    async fn has_api_key(&self, provider: LlmProviderType) -> Result<bool>;

    /// Remove an API key
    async fn remove_api_key(&self, provider: LlmProviderType) -> Result<()>;

    /// List providers that have an API key configured
    async fn list_configured_providers(&self) -> Result<Vec<LlmProviderType>>;
}

/// Store for per-task configuration
#[async_trait]
pub trait TaskConfigStore: Send + Sync {
    /// Get the configuration for a task
    async fn get_task_settings(&self, task: LlmTask) -> Result<crate::core::config::TaskSettings>;

    /// Save the configuration for a task
    async fn set_task_settings(&self, task: LlmTask, settings: crate::core::config::TaskSettings) -> Result<()>;

    /// Reset a task to its defaults
    async fn reset_task_settings(&self, task: LlmTask) -> Result<()>;

    /// Check whether a task has custom settings
    async fn has_custom_settings(&self, task: LlmTask) -> Result<bool>;
}

/// Composite store (combines API keys + task config)
#[async_trait]
pub trait ConfigStore: ApiKeyStore + TaskConfigStore + Send + Sync {
    /// Initialize storage (create directories, DB, etc.)
    async fn initialize(&self) -> Result<()>;

    /// Validate the configuration
    async fn validate(&self) -> Result<()>;
}

// ============================================================================
// EMBEDDING PROVIDER TRAIT
// ============================================================================

/// Trait for embedding providers (separate from LlmProvider because
/// Anthropic has no embeddings and the interface is fundamentally different).
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Provider name (e.g. "openai", "gemini", "ollama")
    fn provider_name(&self) -> &str;

    /// Embedding dimensions (e.g. 1536 for OpenAI, 768 for Gemini/Ollama)
    fn dimensions(&self) -> u32;

    /// Model name (e.g. "text-embedding-3-small")
    fn model(&self) -> &str;

    /// Embed a batch of texts. Returns one vector per input text.
    async fn embed_batch(&self, api_key: &str, texts: &[String]) -> Result<Vec<Vec<f32>>>;
}

// ============================================================================
// LLM PROVIDER TRAIT
// ============================================================================

/// Trait every LLM provider must implement
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Provider name
    fn provider_name(&self) -> &str;

    /// Supported model list
    fn supported_models(&self) -> Vec<String>;

    /// Default model
    fn default_model(&self) -> String;

    /// Complete a request (non-streaming)
    async fn complete(
        &self,
        api_key: &str,
        request: crate::llm::types::LlmRequest,
    ) -> Result<crate::llm::types::LlmResponse>;

    /// Complete a request with streaming
    async fn stream(
        &self,
        api_key: &str,
        request: crate::llm::types::LlmRequest,
    ) -> Result<crate::llm::types::LlmStream>;

    /// Test the connection with the provider
    async fn test(
        &self,
        api_key: &str,
        model: &str,
    ) -> Result<crate::llm::types::ProviderTestResult>;
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_type_as_str() {
        assert_eq!(LlmProviderType::Anthropic.as_str(), "anthropic");
        assert_eq!(LlmProviderType::OpenAI.as_str(), "openai");
        assert_eq!(LlmProviderType::Gemini.as_str(), "gemini");
    }

    #[test]
    fn test_provider_type_from_str() {
        use std::str::FromStr;

        assert_eq!(LlmProviderType::from_str("anthropic").unwrap(), LlmProviderType::Anthropic);
        assert_eq!(LlmProviderType::from_str("ANTHROPIC").unwrap(), LlmProviderType::Anthropic);
        assert_eq!(LlmProviderType::from_str("openai").unwrap(), LlmProviderType::OpenAI);
        assert_eq!(LlmProviderType::from_str("gemini").unwrap(), LlmProviderType::Gemini);

        assert!(LlmProviderType::from_str("invalid").is_err());
    }
}
