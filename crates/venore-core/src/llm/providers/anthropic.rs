//! Anthropic Provider Implementation
//!
//! Implements the LlmProvider trait for Anthropic's Claude models.
//! Supports text generation, streaming, and tool calling.
//!
//! ## API Documentation
//! - API: https://api.anthropic.com/v1/messages
//! - Docs: https://docs.anthropic.com/

use crate::traits::{LlmProvider, LlmProviderType};
use crate::{Result, VenoreError};

use super::super::types::{
    LlmRequest, LlmResponse, LlmMessage, LlmToolCall, MessageRole,
    LlmStream, LlmStreamChunk, TokenUsage, ProviderTestResult,
};
use super::super::registry;

use serde::{Deserialize, Serialize};
use reqwest::Client;
use futures::StreamExt;

// ============================================================================
// CONSTANTS
// ============================================================================

const API_BASE_URL: &str = "https://api.anthropic.com/v1";
const API_VERSION: &str = "2023-06-01";

// ============================================================================
// ANTHROPIC REQUEST/RESPONSE TYPES
// ============================================================================

/// Anthropic API message format — content can be a string or array of blocks
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: serde_json::Value,
}

/// Tool definition for Anthropic API
#[derive(Debug, Clone, Serialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

/// Anthropic API request
#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AnthropicTool>>,
}

/// Anthropic API response
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AnthropicResponse {
    id: String,
    #[serde(rename = "type")]
    response_type: String,
    role: String,
    content: Vec<AnthropicContentBlock>,
    model: String,
    stop_reason: Option<String>,
    usage: AnthropicUsage,
}

/// Content block in response — can be text or tool_use
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
enum AnthropicContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

#[derive(Debug, Deserialize, Clone)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

/// Anthropic error response
#[derive(Debug, Deserialize)]
struct AnthropicError {
    #[serde(rename = "type")]
    error_type: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicErrorResponse {
    error: AnthropicError,
}

/// Anthropic streaming event
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AnthropicStreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    message: Option<AnthropicResponse>,
    #[serde(default)]
    delta: Option<AnthropicDelta>,
    #[serde(default)]
    usage: Option<AnthropicUsage>,
    #[serde(default)]
    content_block: Option<AnthropicStreamContentBlock>,
    #[serde(default)]
    index: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AnthropicDelta {
    #[serde(rename = "type", default)]
    delta_type: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    partial_json: Option<String>,
    #[serde(default)]
    stop_reason: Option<String>,
}

/// Content block start info in streaming
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AnthropicStreamContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    text: Option<String>,
}

// ============================================================================
// PROVIDER IMPLEMENTATION
// ============================================================================

/// Anthropic provider
pub struct AnthropicProvider {
    client: Client,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .connect_timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    /// Build Anthropic request from generic request
    fn build_request(&self, req: &LlmRequest) -> Result<AnthropicRequest> {
        // Separate system message from conversation messages
        let mut system_message: Option<String> = None;
        let mut messages = Vec::new();

        for msg in &req.messages {
            match msg.role {
                MessageRole::System => {
                    // Anthropic uses a separate "system" field
                    system_message = Some(msg.content.clone());
                }
                MessageRole::User => {
                    // Check for multimodal content_parts (images + text)
                    let content = if let Some(ref parts) = msg.content_parts {
                        let mut blocks = Vec::new();
                        for part in parts {
                            match part {
                                super::super::types::ContentPart::ImageBase64 { media_type, data } => {
                                    blocks.push(serde_json::json!({
                                        "type": "image",
                                        "source": {
                                            "type": "base64",
                                            "media_type": media_type,
                                            "data": data,
                                        }
                                    }));
                                }
                                super::super::types::ContentPart::Text { text } => {
                                    blocks.push(serde_json::json!({
                                        "type": "text",
                                        "text": text,
                                    }));
                                }
                            }
                        }
                        // Always append the main content as text if non-empty
                        if !msg.content.is_empty() {
                            blocks.push(serde_json::json!({
                                "type": "text",
                                "text": msg.content,
                            }));
                        }
                        serde_json::Value::Array(blocks)
                    } else {
                        serde_json::Value::String(msg.content.clone())
                    };
                    messages.push(AnthropicMessage {
                        role: "user".into(),
                        content,
                    });
                }
                MessageRole::Assistant => {
                    // If the assistant message has tool_calls, build content blocks
                    if let Some(ref tool_calls) = msg.tool_calls {
                        let mut blocks = Vec::new();

                        // Add text block if there's text content
                        if !msg.content.is_empty() {
                            blocks.push(serde_json::json!({
                                "type": "text",
                                "text": msg.content,
                            }));
                        }

                        // Add tool_use blocks
                        for tc in tool_calls {
                            blocks.push(serde_json::json!({
                                "type": "tool_use",
                                "id": tc.id,
                                "name": tc.name,
                                "input": tc.arguments,
                            }));
                        }

                        messages.push(AnthropicMessage {
                            role: "assistant".into(),
                            content: serde_json::Value::Array(blocks),
                        });
                    } else {
                        messages.push(AnthropicMessage {
                            role: "assistant".into(),
                            content: serde_json::Value::String(msg.content.clone()),
                        });
                    }
                }
                MessageRole::Tool => {
                    // Anthropic tool results go as user messages with tool_result content blocks
                    let tool_result_block = serde_json::json!([{
                        "type": "tool_result",
                        "tool_use_id": msg.tool_call_id.as_deref().unwrap_or(""),
                        "content": msg.content,
                    }]);
                    messages.push(AnthropicMessage {
                        role: "user".into(),
                        content: tool_result_block,
                    });
                }
            }
        }

        // Anthropic requires at least one message
        if messages.is_empty() {
            return Err(VenoreError::LlmInvalidRequest(
                "At least one user or assistant message required".into(),
            ));
        }

        // If JSON schema provided, augment system message
        let system_message = if let Some(schema) = &req.json_schema {
            let schema_prompt = format!(
                "\n\nIMPORTANT: Respond with valid JSON matching this schema:\n{}",
                serde_json::to_string_pretty(&schema.schema).unwrap_or_default()
            );

            Some(match system_message {
                Some(existing) => format!("{}{}", existing, schema_prompt),
                None => schema_prompt,
            })
        } else {
            system_message
        };

        // Convert tools
        let tools = req.tools.as_ref().map(|tools| {
            tools.iter().map(|t| AnthropicTool {
                name: t.name.clone(),
                description: t.description.clone(),
                input_schema: t.parameters.clone(),
            }).collect()
        });

        Ok(AnthropicRequest {
            model: req.model.clone(),
            messages,
            max_tokens: req.max_tokens.unwrap_or(4096),
            temperature: req.temperature,
            system: system_message,
            stream: None,
            tools,
        })
    }

    /// Convert Anthropic response to generic response
    fn parse_response(&self, resp: AnthropicResponse) -> Result<LlmResponse> {
        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();

        for block in &resp.content {
            match block {
                AnthropicContentBlock::Text { text } => {
                    text_parts.push(text.as_str());
                }
                AnthropicContentBlock::ToolUse { id, name, input } => {
                    tool_calls.push(LlmToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        arguments: input.clone(),
                    });
                }
            }
        }

        let content = text_parts.join("");

        Ok(LlmResponse {
            content,
            tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
            usage: Some(TokenUsage {
                prompt_tokens: resp.usage.input_tokens,
                completion_tokens: resp.usage.output_tokens,
                total_tokens: resp.usage.input_tokens + resp.usage.output_tokens,
            }),
            provider: LlmProviderType::Anthropic,
            model: resp.model,
            sources: Vec::new(),
        })
    }

    /// Map HTTP error to VenoreError
    fn map_error(&self, status: reqwest::StatusCode, body: &str, retry_after: Option<&str>) -> VenoreError {
        // Try to parse Anthropic error format
        if let Ok(error_resp) = serde_json::from_str::<AnthropicErrorResponse>(body) {
            let error = error_resp.error;

            return match error.error_type.as_str() {
                "authentication_error" => VenoreError::LlmNoApiKey("anthropic".into()),
                "permission_error" => VenoreError::LlmNoApiKey("anthropic".into()),
                "rate_limit_error" => {
                    let retry_after_secs = retry_after.and_then(|h| h.parse::<u64>().ok());
                    VenoreError::LlmRateLimit { retry_after_secs }
                }
                "invalid_request_error" => VenoreError::LlmInvalidRequest(error.message),
                _ => VenoreError::LlmProviderError(format!(
                    "Anthropic error ({}): {}",
                    error.error_type, error.message
                )),
            };
        }

        // Fallback to status code mapping
        super::base::map_http_error(status.as_u16(), body)
    }
}

impl Default for AnthropicProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl LlmProvider for AnthropicProvider {
    fn provider_name(&self) -> &str {
        "anthropic"
    }

    fn supported_models(&self) -> Vec<String> {
        registry::get_provider_models(LlmProviderType::Anthropic)
    }

    fn default_model(&self) -> String {
        registry::get_default_model(LlmProviderType::Anthropic)
    }

    async fn complete(
        &self,
        api_key: &str,
        request: LlmRequest,
    ) -> Result<LlmResponse> {
        // Validate model
        super::base::validate_model(&request.model, &self.supported_models())?;

        // Build Anthropic request
        let anthropic_req = self.build_request(&request)?;

        // Make HTTP request
        let response = self.client
            .post(format!("{}/messages", API_BASE_URL))
            .header("x-api-key", api_key)
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .json(&anthropic_req)
            .send()
            .await
            .map_err(|e| VenoreError::LlmProviderError(format!("HTTP error: {}", e)))?;

        let status = response.status();

        // Extract Retry-After header before consuming body
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string());

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(self.map_error(status, &body, retry_after.as_deref()));
        }

        // Parse response
        let anthropic_resp = response
            .json::<AnthropicResponse>()
            .await
            .map_err(|e| VenoreError::LlmInvalidResponse(format!("Failed to parse response: {}", e)))?;

        self.parse_response(anthropic_resp)
    }

    async fn stream(
        &self,
        api_key: &str,
        request: LlmRequest,
    ) -> Result<LlmStream> {
        // Validate model
        super::base::validate_model(&request.model, &self.supported_models())?;

        // Build Anthropic request with streaming enabled
        let mut anthropic_req = self.build_request(&request)?;
        anthropic_req.stream = Some(true);

        // Make HTTP request
        let response = self.client
            .post(format!("{}/messages", API_BASE_URL))
            .header("x-api-key", api_key)
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .json(&anthropic_req)
            .send()
            .await
            .map_err(|e| VenoreError::LlmProviderError(format!("HTTP error: {}", e)))?;

        let status = response.status();

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(self.map_error(status, &body, None));
        }

        // Create stream from response body
        let byte_stream = response.bytes_stream();

        // Buffer for incomplete lines + tool call accumulation state
        let mut buffer = String::new();
        let mut tool_call_id: Option<String> = None;
        let mut tool_call_name: Option<String> = None;
        let mut tool_call_json = String::new();
        let mut consecutive_parse_failures: u32 = 0;

        let stream = byte_stream.flat_map(move |chunk_result| {
            let events: Vec<crate::Result<LlmStreamChunk>> = match chunk_result {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    buffer.push_str(&text);

                    // Process complete lines
                    let mut events = Vec::new();
                    while let Some(newline_pos) = buffer.find('\n') {
                        let line = buffer.drain(..=newline_pos).collect::<String>();
                        let line = line.trim().to_string();

                        if line.is_empty() || line.starts_with(':') {
                            continue; // Skip empty lines and comments
                        }

                        // SSE "event:" lines are just labels — we parse the "data:" payload
                        if line.starts_with("event:") {
                            continue;
                        }

                        // Parse SSE format: "data: {...}"
                        if let Some(data) = line.strip_prefix("data: ") {
                            if data == "[DONE]" {
                                events.push(Ok(LlmStreamChunk::Done { usage: None, sources: Vec::new() }));
                                break;
                            }

                            match serde_json::from_str::<AnthropicStreamEvent>(data) {
                                Ok(event) => {
                                    consecutive_parse_failures = 0;
                                    match event.event_type.as_str() {
                                        "content_block_start" => {
                                            // Check if this is a tool_use block
                                            if let Some(ref cb) = event.content_block {
                                                if cb.block_type == "tool_use" {
                                                    tool_call_id = cb.id.clone();
                                                    tool_call_name = cb.name.clone();
                                                    tool_call_json.clear();
                                                }
                                            }
                                        }
                                        "content_block_delta" => {
                                            if let Some(ref delta) = event.delta {
                                                if delta.delta_type == "input_json_delta" {
                                                    // Accumulate tool call JSON
                                                    if let Some(ref partial) = delta.partial_json {
                                                        tool_call_json.push_str(partial);
                                                    }
                                                } else if let Some(ref text) = delta.text {
                                                    // Normal text delta
                                                    events.push(Ok(LlmStreamChunk::Text {
                                                        content: text.clone(),
                                                    }));
                                                }
                                            }
                                        }
                                        "content_block_stop" => {
                                            // If we were accumulating a tool call, emit it
                                            if let (Some(ref id), Some(ref name)) = (&tool_call_id, &tool_call_name) {
                                                let arguments = serde_json::from_str(&tool_call_json)
                                                    .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

                                                events.push(Ok(LlmStreamChunk::ToolCall {
                                                    call: LlmToolCall {
                                                        id: id.clone(),
                                                        name: name.clone(),
                                                        arguments,
                                                    },
                                                }));

                                                // Reset tool call state
                                                tool_call_id = None;
                                                tool_call_name = None;
                                                tool_call_json.clear();
                                            }
                                        }
                                        "message_delta" => {
                                            // Extract usage from message_delta
                                            if let Some(ref usage) = event.usage {
                                                // Store usage — will be emitted with message_stop
                                                // For now, usage in message_delta is the output tokens
                                                let _ = usage;
                                            }
                                        }
                                        "message_stop" => {
                                            let usage = event.usage.map(|u| TokenUsage {
                                                prompt_tokens: u.input_tokens,
                                                completion_tokens: u.output_tokens,
                                                total_tokens: u.input_tokens + u.output_tokens,
                                            });
                                            events.push(Ok(LlmStreamChunk::Done { usage, sources: Vec::new() }));
                                        }
                                        _ => {} // Ignore other event types
                                    }
                                }
                                Err(e) => {
                                    consecutive_parse_failures += 1;
                                    if consecutive_parse_failures >= 5 {
                                        events.push(Ok(LlmStreamChunk::Error {
                                            error: format!("Too many consecutive SSE parse failures ({}): {}", consecutive_parse_failures, e),
                                        }));
                                    } else {
                                        tracing::debug!("Failed to parse Anthropic stream event: {}", e);
                                    }
                                }
                            }
                        }
                    }

                    events
                }
                Err(e) => vec![Err(VenoreError::LlmStreamError(format!("Stream error: {}", e)))],
            };

            futures::stream::iter(events)
        });

        Ok(Box::new(stream))
    }

    async fn test(
        &self,
        api_key: &str,
        model: &str,
    ) -> Result<ProviderTestResult> {
        // Validate model
        super::base::validate_model(model, &self.supported_models())?;

        // Create minimal test request
        let test_request = LlmRequest {
            model: model.into(),
            messages: vec![LlmMessage {
                role: MessageRole::User,
                content: "Hi".into(),
                tool_call_id: None,
                tool_calls: None,
                content_parts: None,
            }],
            temperature: Some(0.3),
            max_tokens: Some(5),
            tools: None,
            json_schema: None,
            timeout_secs: Some(30),
            web_search: false,
        };

        // Attempt completion
        match self.complete(api_key, test_request).await {
            Ok(_) => Ok(ProviderTestResult {
                success: true,
                latency_ms: 0, // Will be set by gateway
                model: model.into(),
                error: None,
            }),
            Err(e) => Ok(ProviderTestResult {
                success: false,
                latency_ms: 0,
                model: model.into(),
                error: Some(e.to_string()),
            }),
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::JsonSchema;

    #[test]
    fn test_provider_info() {
        let provider = AnthropicProvider::new();

        assert_eq!(provider.provider_name(), "anthropic");
        assert!(!provider.supported_models().is_empty());
        assert!(!provider.default_model().is_empty());
    }

    #[test]
    fn test_build_request() {
        let provider = AnthropicProvider::new();

        let request = LlmRequest {
            model: "claude-haiku-4-5".into(),
            messages: vec![
                LlmMessage {
                    role: MessageRole::System,
                    content: "You are helpful".into(),
                    tool_call_id: None,
                    tool_calls: None,
                    content_parts: None,
                },
                LlmMessage {
                    role: MessageRole::User,
                    content: "Hello".into(),
                    tool_call_id: None,
                    tool_calls: None,
                    content_parts: None,
                },
            ],
            temperature: Some(0.7),
            max_tokens: Some(100),
            tools: None,
            json_schema: None,
            timeout_secs: None,
            web_search: false,
        };

        let anthropic_req = provider.build_request(&request).unwrap();

        assert_eq!(anthropic_req.model, "claude-haiku-4-5");
        assert_eq!(anthropic_req.messages.len(), 1); // System msg separated
        assert_eq!(anthropic_req.system, Some("You are helpful".into()));
        assert_eq!(anthropic_req.temperature, Some(0.7));
        assert_eq!(anthropic_req.max_tokens, 100);
    }

    #[test]
    fn test_build_request_with_json_schema() {
        let provider = AnthropicProvider::new();

        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            }
        });

        let request = LlmRequest {
            model: "claude-sonnet-4-5".into(),
            messages: vec![LlmMessage {
                role: MessageRole::User,
                content: "Generate JSON".into(),
                tool_call_id: None,
                tool_calls: None,
                content_parts: None,
            }],
            temperature: None,
            max_tokens: None,
            tools: None,
            json_schema: Some(JsonSchema {
                name: "test".into(),
                schema,
                strict: false,
            }),
            timeout_secs: None,
            web_search: false,
        };

        let anthropic_req = provider.build_request(&request).unwrap();

        // System message should include schema
        assert!(anthropic_req.system.is_some());
        assert!(anthropic_req.system.unwrap().contains("schema"));
    }

    #[test]
    fn test_build_request_with_tools() {
        let provider = AnthropicProvider::new();

        let request = LlmRequest {
            model: "claude-sonnet-4-5".into(),
            messages: vec![LlmMessage {
                role: MessageRole::User,
                content: "Run tests".into(),
                tool_call_id: None,
                tool_calls: None,
                content_parts: None,
            }],
            temperature: None,
            max_tokens: None,
            tools: Some(vec![crate::llm::types::LlmTool {
                name: "run_command".into(),
                description: "Execute a command".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": { "type": "string" }
                    },
                    "required": ["command"]
                }),
            }]),
            json_schema: None,
            timeout_secs: None,
            web_search: false,
        };

        let anthropic_req = provider.build_request(&request).unwrap();
        assert!(anthropic_req.tools.is_some());
        assert_eq!(anthropic_req.tools.unwrap().len(), 1);
    }

    #[test]
    fn test_build_request_with_tool_result() {
        let provider = AnthropicProvider::new();

        let request = LlmRequest {
            model: "claude-sonnet-4-5".into(),
            messages: vec![
                LlmMessage {
                    role: MessageRole::User,
                    content: "Run tests".into(),
                    tool_call_id: None,
                    tool_calls: None,
                    content_parts: None,
                },
                LlmMessage {
                    role: MessageRole::Assistant,
                    content: "I'll run the tests.".into(),
                    tool_call_id: None,
                    tool_calls: Some(vec![LlmToolCall {
                        id: "tc_1".into(),
                        name: "run_command".into(),
                        arguments: serde_json::json!({"command": "npm test"}),
                    }]),
                    content_parts: None,
                },
                LlmMessage {
                    role: MessageRole::Tool,
                    content: "All tests passed".into(),
                    tool_call_id: Some("tc_1".into()),
                    tool_calls: None,
                    content_parts: None,
                },
            ],
            temperature: None,
            max_tokens: None,
            tools: None,
            json_schema: None,
            timeout_secs: None,
            web_search: false,
        };

        let anthropic_req = provider.build_request(&request).unwrap();
        assert_eq!(anthropic_req.messages.len(), 3);

        // Assistant message should have content blocks
        let assistant_content = &anthropic_req.messages[1].content;
        assert!(assistant_content.is_array());

        // Tool result should be a user message with tool_result block
        let tool_msg = &anthropic_req.messages[2];
        assert_eq!(tool_msg.role, "user");
        assert!(tool_msg.content.is_array());
    }

    #[tokio::test]
    #[ignore] // Only run with real API key
    async fn test_complete_integration() {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .expect("ANTHROPIC_API_KEY not set");

        let provider = AnthropicProvider::new();

        let request = LlmRequest {
            model: "claude-haiku-4-5".into(),
            messages: vec![LlmMessage {
                role: MessageRole::User,
                content: "Say hello in 5 words or less".into(),
                tool_call_id: None,
                tool_calls: None,
                content_parts: None,
            }],
            temperature: Some(0.7),
            max_tokens: Some(20),
            tools: None,
            json_schema: None,
            timeout_secs: Some(30),
            web_search: false,
        };

        let response = provider.complete(&api_key, request).await.unwrap();

        assert!(!response.content.is_empty());
        assert_eq!(response.provider, LlmProviderType::Anthropic);
        assert!(response.usage.is_some());
    }
}
