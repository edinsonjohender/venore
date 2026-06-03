//! Default Configuration Values
//!
//! Provides default task settings for each LLM task type.

use super::models::TaskSettings;
use crate::traits::{LlmProviderType, LlmTask};

/// Default configuration provider
pub struct TaskDefaults;

impl TaskDefaults {
    /// Get default settings for Onboarding task
    ///
    /// Onboarding generates .context.md files, requiring:
    /// - Lower temperature for consistency
    /// - Higher max_tokens for comprehensive output
    /// - Longer timeout for complex projects
    ///
    /// NOTE: Defaults match llm/config.rs::get_task_config(LlmTask::Onboarding)
    pub fn onboarding() -> TaskSettings {
        TaskSettings {
            provider: LlmProviderType::Anthropic,
            model: "claude-sonnet-4-5".into(),
            temperature: Some(0.3), // Lower for consistency
            max_tokens: Some(8000),
            timeout_ms: Some(90_000), // 90 seconds
            streaming: Some(false),
        }
    }

    /// Get default settings for Chat task
    ///
    /// Chat is interactive conversation with user, requiring:
    /// - Moderate temperature for natural responses
    /// - Standard max_tokens for conversation
    /// - Longer timeout for complex queries
    ///
    /// NOTE: Defaults match llm/config.rs::get_task_config(LlmTask::Chat)
    pub fn chat() -> TaskSettings {
        TaskSettings {
            provider: LlmProviderType::Anthropic,
            model: "claude-sonnet-4-5".into(),
            temperature: Some(0.7), // Moderate for natural conversation
            max_tokens: Some(4096),
            timeout_ms: Some(120_000), // 120 seconds
            streaming: Some(true), // Better UX for chat
        }
    }

    /// Get default settings for Analysis task
    ///
    /// Analysis examines code and provides insights, requiring:
    /// - Low temperature for precise analysis
    /// - Higher max_tokens for detailed reports
    /// - Longer timeout for thorough analysis
    ///
    /// NOTE: Defaults match llm/config.rs::get_task_config(LlmTask::Analysis)
    pub fn analysis() -> TaskSettings {
        TaskSettings {
            provider: LlmProviderType::Anthropic,
            model: "claude-sonnet-4-5".into(),
            temperature: Some(0.2), // Lower for precision
            max_tokens: Some(8000),
            timeout_ms: Some(90_000), // 90 seconds
            streaming: Some(false),
        }
    }

    /// Get default settings for Embeddings task
    ///
    /// Embeddings is a different API than chat completion: short input,
    /// fixed-dimension output, deterministic. We default to Gemini's
    /// text-embedding-004 because it's what most users start with on
    /// the free tier; users can switch to OpenAI text-embedding-3-small
    /// or an Ollama model from the UI.
    pub fn embeddings() -> TaskSettings {
        TaskSettings {
            provider: LlmProviderType::Gemini,
            model: "text-embedding-004".into(),
            // Embedding APIs ignore temperature / max_tokens / streaming.
            temperature: None,
            max_tokens: None,
            timeout_ms: Some(30_000),
            streaming: Some(false),
        }
    }

    /// Get default settings for a specific task
    pub fn get(task: LlmTask) -> TaskSettings {
        match task {
            LlmTask::Onboarding => Self::onboarding(),
            LlmTask::Chat => Self::chat(),
            LlmTask::Analysis => Self::analysis(),
            LlmTask::Embeddings => Self::embeddings(),
        }
    }

    /// Get all default settings as LlmConfig
    pub fn all() -> super::models::LlmConfig {
        super::models::LlmConfig {
            onboarding: Self::onboarding(),
            chat: Self::chat(),
            analysis: Self::analysis(),
            embeddings: Self::embeddings(),
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_onboarding_defaults() {
        let settings = TaskDefaults::onboarding();

        assert_eq!(settings.provider, LlmProviderType::Anthropic);
        assert_eq!(settings.model, "claude-sonnet-4-5");
        assert_eq!(settings.temperature, Some(0.3));
        assert_eq!(settings.max_tokens, Some(8000));
        assert_eq!(settings.timeout_ms, Some(90_000));
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn test_chat_defaults() {
        let settings = TaskDefaults::chat();

        assert_eq!(settings.temperature, Some(0.7));
        assert_eq!(settings.max_tokens, Some(4096));
        assert_eq!(settings.streaming, Some(true));
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn test_analysis_defaults() {
        let settings = TaskDefaults::analysis();

        assert_eq!(settings.temperature, Some(0.2));
        assert_eq!(settings.max_tokens, Some(8000));
        assert_eq!(settings.streaming, Some(false));
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn test_get_by_task() {
        let onboarding = TaskDefaults::get(LlmTask::Onboarding);
        assert_eq!(onboarding.temperature, Some(0.3));

        let chat = TaskDefaults::get(LlmTask::Chat);
        assert_eq!(chat.temperature, Some(0.7));

        let analysis = TaskDefaults::get(LlmTask::Analysis);
        assert_eq!(analysis.temperature, Some(0.2));
    }

    #[test]
    fn test_all_defaults() {
        let config = TaskDefaults::all();

        assert!(config.validate().is_ok());
        assert_eq!(config.onboarding.temperature, Some(0.3));
        assert_eq!(config.chat.temperature, Some(0.7));
        assert_eq!(config.analysis.temperature, Some(0.2));
    }

    #[test]
    fn test_temperatures_are_different() {
        // Ensure each task has a distinct temperature
        let onboarding_temp = TaskDefaults::onboarding().temperature.unwrap();
        let chat_temp = TaskDefaults::chat().temperature.unwrap();
        let analysis_temp = TaskDefaults::analysis().temperature.unwrap();

        assert_ne!(onboarding_temp, chat_temp);
        assert_ne!(chat_temp, analysis_temp);
        assert_ne!(onboarding_temp, analysis_temp);
    }
}
