//! LLM Configuration
//!
//! Defines task-specific configurations and defaults.

use crate::traits::{LlmTask, LlmProviderType};

/// Task configuration
#[derive(Debug, Clone)]
pub struct TaskConfig {
    /// Default provider for this task
    pub default_provider: LlmProviderType,

    /// Default model
    pub default_model: String,

    /// Fallback providers (in order)
    pub fallbacks: Vec<LlmProviderType>,

    /// Max retries per provider
    pub max_retries: u32,

    /// Base retry delay in milliseconds
    pub retry_delay_ms: u64,

    /// Temperature
    pub temperature: f32,

    /// Max tokens
    pub max_tokens: u32,

    /// Timeout in milliseconds
    pub timeout_ms: u64,
}

impl Default for TaskConfig {
    fn default() -> Self {
        Self {
            default_provider: LlmProviderType::Anthropic,
            default_model: "claude-sonnet-4-5".into(),
            fallbacks: vec![],
            max_retries: 3,
            retry_delay_ms: 1000,
            temperature: 0.7,
            max_tokens: 4096,
            timeout_ms: 60000,
        }
    }
}

/// Get task-specific configuration
#[deprecated(note = "Use LlmGateway::resolve_config() which reads from TaskConfigStore")]
pub fn get_task_config(task: LlmTask) -> TaskConfig {
    match task {
        LlmTask::Onboarding => TaskConfig {
            default_provider: LlmProviderType::Anthropic,
            default_model: "claude-sonnet-4-5".into(),
            fallbacks: vec![],
            max_retries: 3,
            retry_delay_ms: 1000,
            temperature: 0.3, // Lower for consistency
            max_tokens: 8000,
            timeout_ms: 90000,
        },
        LlmTask::Chat => TaskConfig {
            default_provider: LlmProviderType::Anthropic,
            default_model: "claude-sonnet-4-5".into(),
            fallbacks: vec![],
            max_retries: 2,
            retry_delay_ms: 500,
            temperature: 0.7,
            max_tokens: 4096,
            timeout_ms: 120000,
        },
        LlmTask::Analysis => TaskConfig {
            default_provider: LlmProviderType::Anthropic,
            default_model: "claude-sonnet-4-5".into(),
            fallbacks: vec![],
            max_retries: 3,
            retry_delay_ms: 1000,
            temperature: 0.2, // Lower for precision
            max_tokens: 8000,
            timeout_ms: 90000,
        },
        LlmTask::Embeddings => TaskConfig {
            // Embeddings hit a different API; the legacy TaskConfig
            // shape is approximated here just so all match arms compile
            // and old callers receive sensible numbers if they hit this
            // path. Real embedding routing reads from TaskSettings.
            default_provider: LlmProviderType::Gemini,
            default_model: "text-embedding-004".into(),
            fallbacks: vec![],
            max_retries: 2,
            retry_delay_ms: 500,
            temperature: 0.0,
            max_tokens: 0,
            timeout_ms: 30000,
        },
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;

    #[test]
    fn test_onboarding_config() {
        let config = get_task_config(LlmTask::Onboarding);
        assert_eq!(config.default_provider, LlmProviderType::Anthropic);
        assert_eq!(config.temperature, 0.3);
        assert_eq!(config.max_tokens, 8000);
        assert!(config.max_retries > 0);
    }

    #[test]
    fn test_chat_config() {
        let config = get_task_config(LlmTask::Chat);
        assert_eq!(config.temperature, 0.7);
        assert_eq!(config.max_tokens, 4096);
    }

    #[test]
    fn test_analysis_config() {
        let config = get_task_config(LlmTask::Analysis);
        assert_eq!(config.temperature, 0.2);
        assert_eq!(config.max_tokens, 8000);
    }

    #[test]
    fn test_default_config() {
        let config = TaskConfig::default();
        assert_eq!(config.default_provider, LlmProviderType::Anthropic);
        assert_eq!(config.max_retries, 3);
        assert!(config.retry_delay_ms > 0);
    }
}
