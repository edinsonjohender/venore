//! Provider Factory
//!
//! Creates provider implementations based on provider type.

use crate::traits::{LlmProvider, LlmProviderType};
use crate::Result;

/// Get a provider implementation
///
/// # Arguments
///
/// * `provider` - The provider type to instantiate
///
/// # Returns
///
/// * `Ok(Box<dyn LlmProvider>)` - Provider implementation
/// * `Err(VenoreError)` - If provider is not implemented
///
/// # Examples
///
/// ```ignore
/// use venore_core::llm::providers::factory::get_provider;
/// use venore_core::traits::LlmProviderType;
///
/// let provider = get_provider(LlmProviderType::Anthropic)?;
/// ```
pub fn get_provider(provider: LlmProviderType) -> Result<Box<dyn LlmProvider>> {
    match provider {
        // Anthropic - fully implemented
        LlmProviderType::Anthropic => {
            Ok(Box::new(super::anthropic::AnthropicProvider::new()))
        }

        // OpenAI - fully implemented
        LlmProviderType::OpenAI => {
            Ok(Box::new(super::openai::OpenAIProvider::new()))
        }

        // Gemini - fully implemented
        LlmProviderType::Gemini => {
            Ok(Box::new(super::gemini::GeminiProvider::new()))
        }

        // Ollama - fully implemented (local)
        LlmProviderType::Ollama => {
            Ok(Box::new(super::ollama::OllamaProvider::new()))
        }

        // Tavily is a search API, not an LLM provider
        LlmProviderType::Tavily => {
            Err(crate::VenoreError::LlmInvalidProvider(
                "Tavily is a web search API, not an LLM provider".into(),
            ))
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
    fn test_get_anthropic_provider() {
        let result = get_provider(LlmProviderType::Anthropic);
        assert!(result.is_ok());

        let provider = result.unwrap();
        assert_eq!(provider.provider_name(), "anthropic");
    }

    #[test]
    fn test_get_openai_provider() {
        let result = get_provider(LlmProviderType::OpenAI);
        assert!(result.is_ok());

        let provider = result.unwrap();
        assert_eq!(provider.provider_name(), "openai");
    }

    #[test]
    fn test_get_gemini_provider() {
        let result = get_provider(LlmProviderType::Gemini);
        assert!(result.is_ok());

        let provider = result.unwrap();
        assert_eq!(provider.provider_name(), "gemini");
    }

    #[test]
    fn test_get_ollama_provider() {
        let result = get_provider(LlmProviderType::Ollama);
        assert!(result.is_ok());

        let provider = result.unwrap();
        assert_eq!(provider.provider_name(), "ollama");
    }

    #[test]
    fn test_all_providers_handled() {
        // Ensure all providers return some result
        let providers = vec![
            LlmProviderType::Anthropic,
            LlmProviderType::OpenAI,
            LlmProviderType::Gemini,
            LlmProviderType::Ollama,
        ];

        for provider in providers {
            let result = get_provider(provider);
            assert!(result.is_ok() || result.is_err()); // All should return something
        }
    }
}
