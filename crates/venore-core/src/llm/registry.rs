//! Provider and Model Registry
//!
//! Centralizes information about supported providers and their models.

use crate::traits::LlmProviderType;

/// Get available models for a provider
pub fn get_provider_models(provider: LlmProviderType) -> Vec<String> {
    match provider {
        LlmProviderType::Anthropic => vec![
            "claude-sonnet-4-5".into(),
            "claude-haiku-4-5".into(),
            "claude-opus-4-6".into(),
        ],
        LlmProviderType::OpenAI => vec![
            "gpt-4.1".into(),
            "gpt-4.1-mini".into(),
            "gpt-4.1-nano".into(),
            "o3".into(),
            "o4-mini".into(),
        ],
        LlmProviderType::Gemini => vec![
            "gemini-2.5-flash".into(),
            "gemini-2.5-pro".into(),
        ],
        LlmProviderType::Ollama => vec![
            "qwen3:8b".into(),
            "qwen3:4b".into(),
            "deepseek-r1:8b".into(),
            "gemma3:12b".into(),
            "mistral:7b".into(),
        ],
        LlmProviderType::Tavily => vec![],
    }
}

/// Get default model for a provider
pub fn get_default_model(provider: LlmProviderType) -> String {
    match provider {
        LlmProviderType::Anthropic => "claude-sonnet-4-5".into(),
        LlmProviderType::OpenAI => "gpt-4.1".into(),
        LlmProviderType::Gemini => "gemini-2.5-flash".into(),
        LlmProviderType::Ollama => "qwen3:8b".into(),
        LlmProviderType::Tavily => String::new(),
    }
}

/// Check if a model is supported by a provider
pub fn is_model_supported(provider: LlmProviderType, model: &str) -> bool {
    get_provider_models(provider).contains(&model.to_string())
}

/// Model information
#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub provider: LlmProviderType,
    pub context_window: Option<u32>,
    pub max_output_tokens: Option<u32>,
}

/// Get detailed information about a model
pub fn get_model_info(provider: LlmProviderType, model: &str) -> Option<ModelInfo> {
    if !is_model_supported(provider, model) {
        return None;
    }

    Some(match (provider, model) {
        // Anthropic models
        (LlmProviderType::Anthropic, "claude-sonnet-4-5") => ModelInfo {
            id: model.into(),
            name: "Claude Sonnet 4.5".into(),
            provider,
            context_window: Some(200_000),
            max_output_tokens: Some(64_000),
        },
        (LlmProviderType::Anthropic, "claude-haiku-4-5") => ModelInfo {
            id: model.into(),
            name: "Claude Haiku 4.5".into(),
            provider,
            context_window: Some(200_000),
            max_output_tokens: Some(64_000),
        },
        (LlmProviderType::Anthropic, "claude-opus-4-6") => ModelInfo {
            id: model.into(),
            name: "Claude Opus 4.6".into(),
            provider,
            context_window: Some(200_000),
            max_output_tokens: Some(128_000),
        },

        // OpenAI models
        (LlmProviderType::OpenAI, "gpt-4.1") => ModelInfo {
            id: model.into(),
            name: "GPT-4.1".into(),
            provider,
            context_window: Some(1_000_000),
            max_output_tokens: Some(32_768),
        },
        (LlmProviderType::OpenAI, "gpt-4.1-mini") => ModelInfo {
            id: model.into(),
            name: "GPT-4.1 mini".into(),
            provider,
            context_window: Some(1_000_000),
            max_output_tokens: Some(32_768),
        },
        (LlmProviderType::OpenAI, "gpt-4.1-nano") => ModelInfo {
            id: model.into(),
            name: "GPT-4.1 nano".into(),
            provider,
            context_window: Some(1_000_000),
            max_output_tokens: Some(32_768),
        },
        (LlmProviderType::OpenAI, "o3") => ModelInfo {
            id: model.into(),
            name: "o3".into(),
            provider,
            context_window: Some(200_000),
            max_output_tokens: Some(100_000),
        },
        (LlmProviderType::OpenAI, "o4-mini") => ModelInfo {
            id: model.into(),
            name: "o4-mini".into(),
            provider,
            context_window: Some(200_000),
            max_output_tokens: Some(100_000),
        },

        // Gemini models
        (LlmProviderType::Gemini, "gemini-2.5-flash") => ModelInfo {
            id: model.into(),
            name: "Gemini 2.5 Flash".into(),
            provider,
            context_window: Some(1_000_000),
            max_output_tokens: Some(65_000),
        },
        (LlmProviderType::Gemini, "gemini-2.5-pro") => ModelInfo {
            id: model.into(),
            name: "Gemini 2.5 Pro".into(),
            provider,
            context_window: Some(1_000_000),
            max_output_tokens: Some(65_000),
        },

        // Default for any other model
        _ => ModelInfo {
            id: model.into(),
            name: model.into(),
            provider,
            context_window: None,
            max_output_tokens: None,
        },
    })
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_anthropic_models() {
        let models = get_provider_models(LlmProviderType::Anthropic);
        assert!(!models.is_empty());
        assert!(models.contains(&"claude-sonnet-4-5".to_string()));
        assert!(models.contains(&"claude-haiku-4-5".to_string()));
        assert!(models.contains(&"claude-opus-4-6".to_string()));
    }

    #[test]
    fn test_get_openai_models() {
        let models = get_provider_models(LlmProviderType::OpenAI);
        assert!(!models.is_empty());
        assert!(models.contains(&"gpt-4.1".to_string()));
        assert!(models.contains(&"gpt-4.1-mini".to_string()));
        assert!(models.contains(&"o3".to_string()));
    }

    #[test]
    fn test_get_default_model() {
        let model = get_default_model(LlmProviderType::Anthropic);
        assert_eq!(model, "claude-sonnet-4-5");
    }

    #[test]
    fn test_is_model_supported() {
        assert!(is_model_supported(
            LlmProviderType::Anthropic,
            "claude-sonnet-4-5"
        ));

        assert!(!is_model_supported(
            LlmProviderType::Anthropic,
            "nonexistent-model"
        ));
    }

    #[test]
    fn test_get_model_info() {
        let info = get_model_info(
            LlmProviderType::Anthropic,
            "claude-sonnet-4-5"
        );
        assert!(info.is_some());

        let info = info.unwrap();
        assert_eq!(info.id, "claude-sonnet-4-5");
        assert_eq!(info.name, "Claude Sonnet 4.5");
        assert_eq!(info.provider, LlmProviderType::Anthropic);
        assert_eq!(info.context_window, Some(200_000));
    }

    #[test]
    fn test_get_model_info_unsupported() {
        let info = get_model_info(LlmProviderType::Anthropic, "fake-model");
        assert!(info.is_none());
    }
}
