//! Types for LLM module
//!
//! Defines request/response structures, enums, and other types used throughout the LLM module.

use serde::{Deserialize, Serialize};
use crate::traits::LlmProviderType;

// ============================================================================
// MESSAGE TYPES
// ============================================================================

/// Message role in conversation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    #[serde(rename = "tool")]
    Tool,
}

/// Multimodal content part (text or image)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text { text: String },
    ImageBase64 { media_type: String, data: String },
}

/// Message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role: MessageRole,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Tool calls made by the assistant (for multi-turn tool use)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<LlmToolCall>>,
    /// Multimodal content parts (images + text) — when present, providers
    /// should use these instead of the plain `content` string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_parts: Option<Vec<ContentPart>>,
}

// ============================================================================
// TOOL CALLING
// ============================================================================

/// Tool definition (provider-agnostic)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmTool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Tool call in response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

// ============================================================================
// STRUCTURED OUTPUT
// ============================================================================

/// JSON Schema for structured output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonSchema {
    pub name: String,
    pub schema: serde_json::Value,
    #[serde(default)]
    pub strict: bool,
}

// ============================================================================
// REQUEST / RESPONSE
// ============================================================================

/// Unified request (provider-agnostic)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    /// Model to use
    pub model: String,

    /// Messages
    pub messages: Vec<LlmMessage>,

    /// Temperature (0.0 - 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Max tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// Tools for function calling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<LlmTool>>,

    /// JSON schema for structured output
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_schema: Option<JsonSchema>,

    /// Timeout in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,

    /// Enable provider-native web search (e.g. Gemini Grounding with Google
    /// Search). Each provider decides how to interpret it; providers without
    /// a native search ignore the flag. When `true` the model can ground its
    /// answer against fresh web results without the agent having to call a
    /// `web_search` function tool, and citations come back via
    /// `LlmResponse::sources`. Default `false` to keep existing callers
    /// (Onboarding, embeddings, etc.) opt-in.
    #[serde(default)]
    pub web_search: bool,
}

/// Token usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Web search source returned alongside a grounded response.
///
/// Populated when the provider's native search ran during the LLM call (e.g.
/// Gemini Grounding with Google Search). Empty for plain non-grounded
/// responses or for providers that don't support native search yet.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlmSource {
    /// URL the model cited.
    pub uri: String,
    /// Page title (when the provider gives one).
    pub title: String,
}

/// Unified response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    /// Generated content
    pub content: String,

    /// Tool calls (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<LlmToolCall>>,

    /// Token usage
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,

    /// Provider used
    pub provider: LlmProviderType,

    /// Model used
    pub model: String,

    /// Citations from provider-native web search (when `LlmRequest::web_search`
    /// was on and the provider returned grounding metadata). Empty otherwise.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<LlmSource>,
}

// ============================================================================
// STREAMING
// ============================================================================

/// Stream chunk types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum LlmStreamChunk {
    Text { content: String },
    ToolCall { call: LlmToolCall },
    Done {
        #[serde(skip_serializing_if = "Option::is_none")]
        usage: Option<TokenUsage>,
        /// Web search citations emitted when the provider grounded this
        /// turn (e.g. Gemini Grounding with Google Search). Empty for
        /// non-grounded turns or providers without native search.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        sources: Vec<LlmSource>,
    },
    Error { error: String },
}

/// Stream type (alias for clarity)
pub type LlmStream = Box<dyn futures::Stream<Item = crate::Result<LlmStreamChunk>> + Send + Unpin>;

// ============================================================================
// PROVIDER INFO
// ============================================================================

/// Provider test result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderTestResult {
    pub success: bool,
    pub latency_ms: u64,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_role_serialization() {
        let role = MessageRole::User;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, r#""user""#);
    }

    #[test]
    fn test_llm_message() {
        let msg = LlmMessage {
            role: MessageRole::User,
            content: "Hello".into(),
            tool_call_id: None,
            tool_calls: None,
            content_parts: None,
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("user"));
        assert!(json.contains("Hello"));
        assert!(!json.contains("tool_call_id"));
    }

    #[test]
    fn test_llm_request() {
        let request = LlmRequest {
            model: "claude-sonnet-4-5".into(),
            messages: vec![
                LlmMessage {
                    role: MessageRole::User,
                    content: "Test".into(),
                    tool_call_id: None,
                    tool_calls: None,
                    content_parts: None,
                }
            ],
            temperature: Some(0.7),
            max_tokens: Some(100),
            tools: None,
            json_schema: None,
            timeout_secs: Some(30),
            web_search: false,
        };

        assert_eq!(request.model, "claude-sonnet-4-5");
        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.temperature, Some(0.7));
    }

    #[test]
    fn test_token_usage() {
        let usage = TokenUsage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
        };

        assert_eq!(usage.total_tokens, usage.prompt_tokens + usage.completion_tokens);
    }
}
