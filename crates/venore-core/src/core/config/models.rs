//! Configuration Models
//!
//! Defines configuration structures for LLM tasks.

use serde::{Deserialize, Serialize};
use crate::traits::{LlmProviderType, LlmTask};
use crate::{Result, VenoreError};

// ============================================================================
// TASK SETTINGS
// ============================================================================

/// Settings for a specific LLM task
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskSettings {
    /// Provider to use
    pub provider: LlmProviderType,

    /// Model to use
    pub model: String,

    /// Temperature (0.0-2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Maximum tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// Timeout in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,

    /// Enable streaming
    #[serde(skip_serializing_if = "Option::is_none")]
    pub streaming: Option<bool>,
}

impl TaskSettings {
    /// Validate settings
    pub fn validate(&self) -> Result<()> {
        // Validate temperature
        if let Some(temp) = self.temperature {
            crate::llm::utils::validation::validate_temperature(temp)?;
        }

        // Validate max_tokens
        if let Some(tokens) = self.max_tokens {
            crate::llm::utils::validation::validate_max_tokens(tokens)?;
        }

        // Validate timeout
        if let Some(timeout_ms) = self.timeout_ms {
            let timeout_secs = timeout_ms / 1000;
            if timeout_secs == 0 || timeout_secs > 600 {
                return Err(VenoreError::LlmInvalidRequest(format!(
                    "Timeout must be between 1s and 600s, got {}ms",
                    timeout_ms
                )));
            }
        }

        // Validate model name
        crate::llm::utils::validation::validate_model_name(&self.model)?;

        Ok(())
    }

    /// Get temperature or default
    pub fn temperature_or(&self, default: f32) -> f32 {
        self.temperature.unwrap_or(default)
    }

    /// Get max_tokens or default
    pub fn max_tokens_or(&self, default: u32) -> u32 {
        self.max_tokens.unwrap_or(default)
    }

    /// Get timeout_ms or default
    pub fn timeout_ms_or(&self, default: u64) -> u64 {
        self.timeout_ms.unwrap_or(default)
    }

    /// Get streaming or default
    pub fn streaming_or(&self, default: bool) -> bool {
        self.streaming.unwrap_or(default)
    }
}

// ============================================================================
// LLM CONFIG
// ============================================================================

/// Complete LLM configuration for all tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Onboarding task settings
    pub onboarding: TaskSettings,

    /// Chat task settings
    pub chat: TaskSettings,

    /// Analysis task settings
    pub analysis: TaskSettings,

    /// Embeddings task settings (RAG vector search)
    #[serde(default = "default_embeddings_settings")]
    pub embeddings: TaskSettings,
}

fn default_embeddings_settings() -> TaskSettings {
    super::defaults::TaskDefaults::embeddings()
}

impl LlmConfig {
    /// Get settings for a specific task
    pub fn get_task_settings(&self, task: LlmTask) -> &TaskSettings {
        match task {
            LlmTask::Onboarding => &self.onboarding,
            LlmTask::Chat => &self.chat,
            LlmTask::Analysis => &self.analysis,
            LlmTask::Embeddings => &self.embeddings,
        }
    }

    /// Get mutable settings for a specific task
    pub fn get_task_settings_mut(&mut self, task: LlmTask) -> &mut TaskSettings {
        match task {
            LlmTask::Onboarding => &mut self.onboarding,
            LlmTask::Chat => &mut self.chat,
            LlmTask::Analysis => &mut self.analysis,
            LlmTask::Embeddings => &mut self.embeddings,
        }
    }

    /// Validate all settings
    pub fn validate(&self) -> Result<()> {
        self.onboarding.validate()?;
        self.chat.validate()?;
        self.analysis.validate()?;
        self.embeddings.validate()?;
        Ok(())
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_settings_validation() {
        let valid = TaskSettings {
            provider: LlmProviderType::Anthropic,
            model: "claude-sonnet-4-5".into(),
            temperature: Some(0.7),
            max_tokens: Some(4096),
            timeout_ms: Some(60000),
            streaming: Some(false),
        };

        assert!(valid.validate().is_ok());

        // Invalid temperature
        let invalid_temp = TaskSettings {
            temperature: Some(5.0),
            ..valid.clone()
        };
        assert!(invalid_temp.validate().is_err());

        // Invalid max_tokens
        let invalid_tokens = TaskSettings {
            max_tokens: Some(0),
            ..valid.clone()
        };
        assert!(invalid_tokens.validate().is_err());

        // Invalid timeout
        let invalid_timeout = TaskSettings {
            timeout_ms: Some(0),
            ..valid.clone()
        };
        assert!(invalid_timeout.validate().is_err());
    }

    #[test]
    fn test_task_settings_defaults() {
        let settings = TaskSettings {
            provider: LlmProviderType::Anthropic,
            model: "test-model".into(),
            temperature: None,
            max_tokens: Some(100),
            timeout_ms: None,
            streaming: None,
        };

        assert_eq!(settings.temperature_or(0.5), 0.5);
        assert_eq!(settings.max_tokens_or(200), 100); // Has value
        assert_eq!(settings.timeout_ms_or(30000), 30000);
        assert!(!settings.streaming_or(false));
    }

    #[test]
    fn test_llm_config_get_task() {
        let config = LlmConfig {
            onboarding: TaskSettings {
                provider: LlmProviderType::Anthropic,
                model: "model-onboarding".into(),
                temperature: Some(0.3),
                max_tokens: None,
                timeout_ms: None,
                streaming: None,
            },
            chat: TaskSettings {
                provider: LlmProviderType::Anthropic,
                model: "model-chat".into(),
                temperature: Some(0.7),
                max_tokens: None,
                timeout_ms: None,
                streaming: None,
            },
            analysis: TaskSettings {
                provider: LlmProviderType::Anthropic,
                model: "model-analysis".into(),
                temperature: Some(0.2),
                max_tokens: None,
                timeout_ms: None,
                streaming: None,
            },
            embeddings: TaskSettings {
                provider: LlmProviderType::Gemini,
                model: "text-embedding-004".into(),
                temperature: None,
                max_tokens: None,
                timeout_ms: None,
                streaming: None,
            },
        };

        assert_eq!(config.get_task_settings(LlmTask::Onboarding).model, "model-onboarding");
        assert_eq!(config.get_task_settings(LlmTask::Chat).model, "model-chat");
        assert_eq!(config.get_task_settings(LlmTask::Analysis).model, "model-analysis");
    }

    #[test]
    fn test_llm_config_validation() {
        let valid_config = LlmConfig {
            onboarding: TaskSettings {
                provider: LlmProviderType::Anthropic,
                model: "model-1".into(),
                temperature: Some(0.3),
                max_tokens: Some(100),
                timeout_ms: Some(30000),
                streaming: None,
            },
            chat: TaskSettings {
                provider: LlmProviderType::Anthropic,
                model: "model-2".into(),
                temperature: Some(0.7),
                max_tokens: Some(200),
                timeout_ms: Some(60000),
                streaming: None,
            },
            analysis: TaskSettings {
                provider: LlmProviderType::Anthropic,
                model: "model-3".into(),
                temperature: Some(0.2),
                max_tokens: Some(300),
                timeout_ms: Some(90000),
                streaming: None,
            },
            embeddings: TaskSettings {
                provider: LlmProviderType::Gemini,
                model: "text-embedding-004".into(),
                temperature: None,
                max_tokens: None,
                timeout_ms: Some(30000),
                streaming: None,
            },
        };

        assert!(valid_config.validate().is_ok());
    }

    #[test]
    fn test_task_settings_serialization() {
        let settings = TaskSettings {
            provider: LlmProviderType::Anthropic,
            model: "test-model".into(),
            temperature: Some(0.7),
            max_tokens: None,
            timeout_ms: Some(30000),
            streaming: None,
        };

        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("anthropic"));
        assert!(json.contains("test-model"));
        assert!(!json.contains("max_tokens")); // Should be omitted when None
    }
}
