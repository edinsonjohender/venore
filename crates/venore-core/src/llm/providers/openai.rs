// ! OpenAI Provider Implementation
//!
//! Implements the LlmProvider trait for OpenAI's GPT models.
//! Supports text generation, streaming, and tool calling.
//!
//! ## API Documentation
//! - API: https://api.openai.com/v1/chat/completions
//! - Docs: https://platform.openai.com/docs/api-reference

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
use std::time::Instant;

// ============================================================================
// CONSTANTS
// ============================================================================

const API_BASE_URL: &str = "https://api.openai.com/v1";

/// Returns true for o-series reasoning models (o3, o4-mini, etc.)
fn is_reasoning_model(model: &str) -> bool {
    model.starts_with("o")
}

// ============================================================================
// OPENAI REQUEST/RESPONSE TYPES
// ============================================================================

/// OpenAI API message format
/// `content` is `serde_json::Value` to support both plain strings and
/// multimodal arrays (e.g. image_url + text blocks for vision).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIMessage {
    role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    content: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAIToolCall>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

/// Tool call in messages (request + response)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: OpenAIFunctionCall,
}

/// Function call details
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIFunctionCall {
    name: String,
    arguments: String, // JSON string, NOT an object
}

/// Tool definition in request
#[derive(Debug, Clone, Serialize)]
struct OpenAIToolDefinition {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAIFunctionDefinition,
}

/// Function definition in tool
#[derive(Debug, Clone, Serialize)]
struct OpenAIFunctionDefinition {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

/// Options for streaming usage reporting
#[derive(Debug, Serialize)]
struct StreamOptions {
    include_usage: bool,
}

/// Structured output response format
#[derive(Debug, Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    format_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    json_schema: Option<ResponseJsonSchema>,
}

#[derive(Debug, Serialize)]
struct ResponseJsonSchema {
    name: String,
    schema: serde_json::Value,
    strict: bool,
}

/// OpenAI API request
#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream_options: Option<StreamOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAIToolDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
}

/// OpenAI API response
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAIResponse {
    id: String,
    object: String,
    created: u64,
    model: String,
    choices: Vec<OpenAIChoice>,
    usage: OpenAIUsage,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAIChoice {
    index: u32,
    message: OpenAIMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

/// OpenAI error response
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAIError {
    message: String,
    #[serde(rename = "type")]
    error_type: String,
    #[serde(default)]
    code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIErrorResponse {
    error: OpenAIError,
}

/// OpenAI streaming chunk
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAIStreamChunk {
    id: String,
    object: String,
    created: u64,
    model: String,
    choices: Vec<OpenAIStreamChoice>,
    #[serde(default)]
    usage: Option<OpenAIUsage>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAIStreamChoice {
    index: u32,
    delta: OpenAIDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAIDelta {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAIDeltaToolCall>>,
}

/// Streaming delta for tool calls — arguments arrive incrementally by index
#[derive(Debug, Deserialize)]
struct OpenAIDeltaToolCall {
    index: usize,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<OpenAIDeltaFunction>,
}

#[derive(Debug, Deserialize)]
struct OpenAIDeltaFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

// ============================================================================
// PROVIDER IMPLEMENTATION
// ============================================================================

/// OpenAI provider
pub struct OpenAIProvider {
    client: Client,
}

impl OpenAIProvider {
    /// Create a new OpenAI provider
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .connect_timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    /// Build OpenAI request from generic request
    fn build_request(&self, req: &LlmRequest) -> Result<OpenAIRequest> {
        let reasoning = is_reasoning_model(&req.model);

        // OpenAI uses a flat messages array with role field
        let mut messages: Vec<OpenAIMessage> = Vec::new();

        for msg in &req.messages {
            match msg.role {
                MessageRole::System => {
                    // Reasoning models (o3, o4-mini) require "developer" role instead of "system"
                    messages.push(OpenAIMessage {
                        role: if reasoning { "developer" } else { "system" }.into(),
                        content: Some(serde_json::Value::String(msg.content.clone())),
                        tool_calls: None,
                        tool_call_id: None,
                    });
                }
                MessageRole::User => {
                    // Check for multimodal content_parts (images + text)
                    let content = if let Some(ref parts) = msg.content_parts {
                        let mut blocks = Vec::new();
                        for part in parts {
                            match part {
                                super::super::types::ContentPart::ImageBase64 { media_type, data } => {
                                    blocks.push(serde_json::json!({
                                        "type": "image_url",
                                        "image_url": {
                                            "url": format!("data:{};base64,{}", media_type, data),
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
                        if !msg.content.is_empty() {
                            blocks.push(serde_json::json!({
                                "type": "text",
                                "text": msg.content,
                            }));
                        }
                        Some(serde_json::Value::Array(blocks))
                    } else {
                        Some(serde_json::Value::String(msg.content.clone()))
                    };
                    messages.push(OpenAIMessage {
                        role: "user".into(),
                        content,
                        tool_calls: None,
                        tool_call_id: None,
                    });
                }
                MessageRole::Assistant => {
                    if let Some(ref tool_calls) = msg.tool_calls {
                        // Assistant message with tool calls
                        let openai_tool_calls: Vec<OpenAIToolCall> = tool_calls
                            .iter()
                            .map(|tc| OpenAIToolCall {
                                id: tc.id.clone(),
                                call_type: "function".into(),
                                function: OpenAIFunctionCall {
                                    name: tc.name.clone(),
                                    // Critical: arguments must be a JSON string
                                    arguments: serde_json::to_string(&tc.arguments)
                                        .unwrap_or_else(|_| "{}".into()),
                                },
                            })
                            .collect();

                        messages.push(OpenAIMessage {
                            role: "assistant".into(),
                            content: if msg.content.is_empty() { None } else { Some(serde_json::Value::String(msg.content.clone())) },
                            tool_calls: Some(openai_tool_calls),
                            tool_call_id: None,
                        });
                    } else {
                        messages.push(OpenAIMessage {
                            role: "assistant".into(),
                            content: Some(serde_json::Value::String(msg.content.clone())),
                            tool_calls: None,
                            tool_call_id: None,
                        });
                    }
                }
                MessageRole::Tool => {
                    // Tool result message
                    messages.push(OpenAIMessage {
                        role: "tool".into(),
                        content: Some(serde_json::Value::String(msg.content.clone())),
                        tool_calls: None,
                        tool_call_id: msg.tool_call_id.clone(),
                    });
                }
            }
        }

        if messages.is_empty() {
            return Err(VenoreError::LlmInvalidRequest(
                "At least one message is required".into()
            ));
        }

        // Convert tools
        let tools = req.tools.as_ref().map(|tools| {
            tools.iter().map(|t| OpenAIToolDefinition {
                tool_type: "function".into(),
                function: OpenAIFunctionDefinition {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: t.parameters.clone(),
                },
            }).collect()
        });

        // Reasoning models: max_completion_tokens instead of max_tokens, no temperature
        let (max_tokens, max_completion_tokens) = if reasoning {
            (None, req.max_tokens)
        } else {
            (req.max_tokens, None)
        };
        let temperature = if reasoning { None } else { req.temperature };
        let reasoning_effort = if reasoning { Some("medium".into()) } else { None };

        // Structured output via json_schema
        let response_format = req.json_schema.as_ref().map(|js| ResponseFormat {
            format_type: "json_schema".into(),
            json_schema: Some(ResponseJsonSchema {
                name: js.name.clone(),
                schema: js.schema.clone(),
                strict: js.strict,
            }),
        });

        Ok(OpenAIRequest {
            model: req.model.clone(),
            messages,
            temperature,
            max_tokens,
            max_completion_tokens,
            reasoning_effort,
            stream: Some(false),
            stream_options: None,
            tools,
            response_format,
        })
    }

    /// Convert OpenAI response to generic response
    fn convert_response(&self, res: OpenAIResponse) -> Result<LlmResponse> {
        let choice = res.choices.first()
            .ok_or_else(|| VenoreError::LlmInvalidResponse("No choices in response".into()))?;

        // Extract tool calls if present
        let tool_calls = choice.message.tool_calls.as_ref().map(|tcs| {
            tcs.iter().map(|tc| LlmToolCall {
                id: tc.id.clone(),
                name: tc.function.name.clone(),
                // Parse arguments from JSON string back to Value
                arguments: serde_json::from_str(&tc.function.arguments)
                    .unwrap_or(serde_json::Value::Object(serde_json::Map::new())),
            }).collect::<Vec<_>>()
        });

        // Extract text content from Value (string or first text block)
        let content = match &choice.message.content {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(serde_json::Value::Array(arr)) => {
                arr.iter()
                    .filter_map(|v| v.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("")
            }
            _ => String::new(),
        };

        Ok(LlmResponse {
            content,
            tool_calls: tool_calls.filter(|v| !v.is_empty()),
            usage: Some(TokenUsage {
                prompt_tokens: res.usage.prompt_tokens,
                completion_tokens: res.usage.completion_tokens,
                total_tokens: res.usage.total_tokens,
            }),
            provider: LlmProviderType::OpenAI,
            model: res.model,
            sources: Vec::new(),
        })
    }

    /// Parse error from OpenAI response
    fn parse_error(&self, status: u16, body: &str, retry_after: Option<&str>) -> VenoreError {
        // Try to parse as OpenAI error
        if let Ok(error_response) = serde_json::from_str::<OpenAIErrorResponse>(body) {
            let message = error_response.error.message;

            return match status {
                401 => VenoreError::LlmNoApiKey(
                    "Invalid OpenAI API key".into()
                ),
                403 => VenoreError::LlmNoApiKey(
                    "OpenAI API key forbidden".into()
                ),
                429 => {
                    let retry_after_secs = retry_after
                        .and_then(|h| h.parse::<u64>().ok());
                    VenoreError::LlmRateLimit { retry_after_secs }
                }
                400 => {
                    if message.contains("context_length_exceeded") || message.contains("maximum context length") {
                        VenoreError::LlmContextTooLong {
                            current: 0, // OpenAI doesn't provide exact count in error
                            max: 0,
                        }
                    } else {
                        VenoreError::LlmInvalidRequest(message)
                    }
                }
                _ => VenoreError::LlmProviderError(
                    format!("OpenAI API error ({}): {}", status, message)
                ),
            };
        }

        // Fallback: generic error
        VenoreError::LlmProviderError(
            format!("OpenAI API error ({}): {}", status, body)
        )
    }
}

// ============================================================================
// LlmProvider TRAIT IMPLEMENTATION
// ============================================================================

#[async_trait::async_trait]
impl LlmProvider for OpenAIProvider {
    fn provider_name(&self) -> &str {
        "openai"
    }

    fn supported_models(&self) -> Vec<String> {
        registry::get_provider_models(LlmProviderType::OpenAI)
    }

    fn default_model(&self) -> String {
        registry::get_default_model(LlmProviderType::OpenAI)
    }

    async fn complete(&self, api_key: &str, request: LlmRequest) -> Result<LlmResponse> {
        let openai_req = self.build_request(&request)?;

        tracing::debug!(
            "Sending request to OpenAI: model={}, messages={}",
            openai_req.model,
            openai_req.messages.len()
        );

        let mut http_req = self.client
            .post(format!("{}/chat/completions", API_BASE_URL))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&openai_req);

        if let Some(secs) = request.timeout_secs {
            http_req = http_req.timeout(std::time::Duration::from_secs(secs));
        }

        let response = http_req
            .send()
            .await
            .map_err(|e| VenoreError::LlmProviderError(
                format!("Failed to send request to OpenAI: {}", e)
            ))?;

        let status = response.status();

        // Extract Retry-After header before consuming body
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string());

        let body = response.text().await
            .map_err(|e| VenoreError::LlmProviderError(
                format!("Failed to read OpenAI response: {}", e)
            ))?;

        if !status.is_success() {
            return Err(self.parse_error(status.as_u16(), &body, retry_after.as_deref()));
        }

        let openai_response: OpenAIResponse = serde_json::from_str(&body)
            .map_err(|e| VenoreError::LlmInvalidResponse(
                format!("Failed to parse OpenAI response: {}", e)
            ))?;

        tracing::debug!(
            "Received response from OpenAI: tokens={:?}",
            openai_response.usage.total_tokens
        );

        self.convert_response(openai_response)
    }

    async fn stream(&self, api_key: &str, request: LlmRequest) -> Result<LlmStream> {
        let mut openai_req = self.build_request(&request)?;
        openai_req.stream = Some(true);
        openai_req.stream_options = Some(StreamOptions { include_usage: true });

        tracing::debug!(
            "Starting stream from OpenAI: model={}, messages={}",
            openai_req.model,
            openai_req.messages.len()
        );

        let mut http_req = self.client
            .post(format!("{}/chat/completions", API_BASE_URL))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&openai_req);

        // Streaming can take longer, use 2x timeout
        if let Some(secs) = request.timeout_secs {
            http_req = http_req.timeout(std::time::Duration::from_secs(secs * 2));
        }

        let response = http_req
            .send()
            .await
            .map_err(|e| VenoreError::LlmProviderError(
                format!("Failed to start OpenAI stream: {}", e)
            ))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await
                .unwrap_or_else(|_| "Failed to read error body".to_string());
            return Err(self.parse_error(status.as_u16(), &body, None));
        }

        // Convert response stream to LlmStream
        // Buffer persists across HTTP chunks to handle SSE events split across packet boundaries
        let byte_stream = response.bytes_stream();
        let mut buffer = String::new();

        // Tool call accumulators: Vec<(id, name, arguments_string)> indexed by `index`
        // OpenAI sends tool call arguments as incremental string deltas keyed by index.
        let mut tool_call_accumulators: Vec<(String, String, String)> = Vec::new();

        // Track usage from the final streaming chunk (with stream_options.include_usage)
        let mut stream_usage: Option<TokenUsage> = None;

        let chunk_stream = byte_stream.flat_map(move |result| {
            let events: Vec<crate::Result<LlmStreamChunk>> = match result {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    buffer.push_str(&text);

                    let mut events = Vec::new();
                    let mut accumulated_text = String::new();

                    // Process only complete lines from the buffer
                    while let Some(newline_pos) = buffer.find('\n') {
                        let line: String = buffer.drain(..=newline_pos).collect();
                        let line = line.trim();

                        if line.is_empty() {
                            continue; // Skip empty lines (SSE separators)
                        }

                        if let Some(json_str) = line.strip_prefix("data: ") {
                            // Check for stream end
                            if json_str.trim() == "[DONE]" {
                                // Flush any remaining tool calls
                                for (id, name, args) in tool_call_accumulators.drain(..) {
                                    let arguments = serde_json::from_str(&args)
                                        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                                    events.push(Ok(LlmStreamChunk::ToolCall {
                                        call: LlmToolCall { id, name, arguments },
                                    }));
                                }
                                // Flush accumulated text
                                if !accumulated_text.is_empty() {
                                    events.push(Ok(LlmStreamChunk::Text {
                                        content: std::mem::take(&mut accumulated_text),
                                    }));
                                }
                                events.push(Ok(LlmStreamChunk::Done { usage: stream_usage.take(), sources: Vec::new() }));
                                break;
                            }

                            // Parse JSON chunk
                            if let Ok(chunk) = serde_json::from_str::<OpenAIStreamChunk>(json_str) {
                                // Capture usage from the final chunk (sent when stream_options.include_usage is true)
                                if let Some(ref u) = chunk.usage {
                                    stream_usage = Some(TokenUsage {
                                        prompt_tokens: u.prompt_tokens,
                                        completion_tokens: u.completion_tokens,
                                        total_tokens: u.total_tokens,
                                    });
                                }

                                if let Some(choice) = chunk.choices.first() {
                                    // Accumulate text content
                                    if let Some(content) = &choice.delta.content {
                                        if !content.is_empty() {
                                            accumulated_text.push_str(content);
                                        }
                                    }

                                    // Accumulate tool call deltas
                                    if let Some(ref delta_tool_calls) = choice.delta.tool_calls {
                                        for dtc in delta_tool_calls {
                                            let idx = dtc.index;

                                            // Grow accumulators if needed
                                            while tool_call_accumulators.len() <= idx {
                                                tool_call_accumulators.push((String::new(), String::new(), String::new()));
                                            }

                                            // Set id if present (first delta for this index)
                                            if let Some(ref id) = dtc.id {
                                                tool_call_accumulators[idx].0 = id.clone();
                                            }

                                            // Set/accumulate function data
                                            if let Some(ref func) = dtc.function {
                                                if let Some(ref name) = func.name {
                                                    tool_call_accumulators[idx].1 = name.clone();
                                                }
                                                if let Some(ref args) = func.arguments {
                                                    tool_call_accumulators[idx].2.push_str(args);
                                                }
                                            }
                                        }
                                    }

                                    // On finish_reason == "tool_calls", emit all accumulated tool calls
                                    if choice.finish_reason.as_deref() == Some("tool_calls") {
                                        // Flush text first
                                        if !accumulated_text.is_empty() {
                                            events.push(Ok(LlmStreamChunk::Text {
                                                content: std::mem::take(&mut accumulated_text),
                                            }));
                                        }

                                        for (id, name, args) in tool_call_accumulators.drain(..) {
                                            let arguments = serde_json::from_str(&args)
                                                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                                            events.push(Ok(LlmStreamChunk::ToolCall {
                                                call: LlmToolCall { id, name, arguments },
                                            }));
                                        }
                                        events.push(Ok(LlmStreamChunk::Done { usage: stream_usage.take(), sources: Vec::new() }));
                                    } else if choice.finish_reason.as_deref() == Some("stop") {
                                        // Normal text completion
                                        if !accumulated_text.is_empty() {
                                            events.push(Ok(LlmStreamChunk::Text {
                                                content: std::mem::take(&mut accumulated_text),
                                            }));
                                        }
                                        events.push(Ok(LlmStreamChunk::Done { usage: stream_usage.take(), sources: Vec::new() }));
                                    }
                                }
                            }
                        }
                    }
                    // Incomplete lines remain in buffer for the next chunk

                    // Flush accumulated text (if no finish_reason yet)
                    if !accumulated_text.is_empty() {
                        events.push(Ok(LlmStreamChunk::Text { content: accumulated_text }));
                    }

                    events
                }
                Err(e) => vec![Ok(LlmStreamChunk::Error {
                    error: format!("Stream error: {}", e)
                })],
            };

            futures::stream::iter(events)
        });

        Ok(Box::new(chunk_stream))
    }

    async fn test(&self, api_key: &str, model: &str) -> Result<ProviderTestResult> {
        use crate::traits::LlmProvider as LlmProviderTrait;

        let start = Instant::now();

        // Simple test request
        let request = LlmRequest {
            model: model.to_string(),
            messages: vec![
                LlmMessage {
                    role: MessageRole::User,
                    content: "Say 'test'".into(),
                    tool_call_id: None,
                    tool_calls: None,
                    content_parts: None,
                }
            ],
            temperature: Some(0.7),
            max_tokens: Some(10),
            tools: None,
            json_schema: None,
            timeout_secs: Some(30),
            web_search: false,
        };

        match LlmProviderTrait::complete(self, api_key, request).await {
            Ok(_) => {
                let latency = start.elapsed().as_millis() as u64;
                Ok(ProviderTestResult {
                    success: true,
                    model: model.to_string(),
                    latency_ms: latency,
                    error: None,
                })
            }
            Err(e) => {
                let latency = start.elapsed().as_millis() as u64;
                let error_msg = format!("{}", e);
                Ok(ProviderTestResult {
                    success: false,
                    model: model.to_string(),
                    latency_ms: latency,
                    error: Some(error_msg),
                })
            }
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::types::LlmTool;

    #[test]
    fn test_build_request() {
        let provider = OpenAIProvider::new();

        let request = LlmRequest {
            model: "gpt-4.1".into(),
            messages: vec![
                LlmMessage {
                    role: MessageRole::User,
                    content: "Hello".into(),
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

        let result = provider.build_request(&request);
        assert!(result.is_ok());

        let openai_req = result.unwrap();
        assert_eq!(openai_req.model, "gpt-4.1");
        assert_eq!(openai_req.messages.len(), 1);
        assert_eq!(openai_req.messages[0].role, "user");
        assert_eq!(openai_req.messages[0].content, Some(serde_json::Value::String("Hello".into())));
    }

    #[test]
    fn test_build_request_with_system_message() {
        let provider = OpenAIProvider::new();

        let request = LlmRequest {
            model: "gpt-4.1".into(),
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
                }
            ],
            temperature: Some(0.7),
            max_tokens: Some(100),
            tools: None,
            json_schema: None,
            timeout_secs: Some(30),
            web_search: false,
        };

        let result = provider.build_request(&request);
        assert!(result.is_ok());

        let openai_req = result.unwrap();
        assert_eq!(openai_req.messages.len(), 2);
        assert_eq!(openai_req.messages[0].role, "system");
        assert_eq!(openai_req.messages[1].role, "user");
    }

    #[test]
    fn test_build_request_with_tools() {
        let provider = OpenAIProvider::new();

        let request = LlmRequest {
            model: "gpt-4.1".into(),
            messages: vec![LlmMessage {
                role: MessageRole::User,
                content: "Create a file".into(),
                tool_call_id: None,
                tool_calls: None,
                content_parts: None,
            }],
            temperature: None,
            max_tokens: None,
            tools: Some(vec![LlmTool {
                name: "write_file".into(),
                description: "Write content to a file".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "content": { "type": "string" }
                    },
                    "required": ["path", "content"]
                }),
            }]),
            json_schema: None,
            timeout_secs: None,
            web_search: false,
        };

        let openai_req = provider.build_request(&request).unwrap();
        assert!(openai_req.tools.is_some());

        let tools = openai_req.tools.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].tool_type, "function");
        assert_eq!(tools[0].function.name, "write_file");
    }

    #[test]
    fn test_build_request_with_tool_result_messages() {
        let provider = OpenAIProvider::new();

        let request = LlmRequest {
            model: "gpt-4.1".into(),
            messages: vec![
                LlmMessage {
                    role: MessageRole::User,
                    content: "Create a file".into(),
                    tool_call_id: None,
                    tool_calls: None,
                    content_parts: None,
                },
                LlmMessage {
                    role: MessageRole::Assistant,
                    content: "".into(),
                    tool_call_id: None,
                    tool_calls: Some(vec![LlmToolCall {
                        id: "call_abc123".into(),
                        name: "write_file".into(),
                        arguments: serde_json::json!({"path": "test.txt", "content": "hello"}),
                    }]),
                    content_parts: None,
                },
                LlmMessage {
                    role: MessageRole::Tool,
                    content: "File written successfully".into(),
                    tool_call_id: Some("call_abc123".into()),
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

        let openai_req = provider.build_request(&request).unwrap();
        assert_eq!(openai_req.messages.len(), 3);

        // Assistant message with tool_calls
        let assistant = &openai_req.messages[1];
        assert_eq!(assistant.role, "assistant");
        assert!(assistant.content.is_none()); // empty content becomes None
        assert!(assistant.tool_calls.is_some());
        let tcs = assistant.tool_calls.as_ref().unwrap();
        assert_eq!(tcs.len(), 1);
        assert_eq!(tcs[0].id, "call_abc123");
        assert_eq!(tcs[0].call_type, "function");
        assert_eq!(tcs[0].function.name, "write_file");
        // arguments should be a JSON string
        let parsed: serde_json::Value = serde_json::from_str(&tcs[0].function.arguments).unwrap();
        assert_eq!(parsed["path"], "test.txt");

        // Tool result message
        let tool_msg = &openai_req.messages[2];
        assert_eq!(tool_msg.role, "tool");
        assert_eq!(tool_msg.content, Some(serde_json::Value::String("File written successfully".into())));
        assert_eq!(tool_msg.tool_call_id.as_deref(), Some("call_abc123"));
    }

    #[test]
    fn test_convert_response_with_tool_calls() {
        let provider = OpenAIProvider::new();

        let response = OpenAIResponse {
            id: "chatcmpl-123".into(),
            object: "chat.completion".into(),
            created: 1234567890,
            model: "gpt-4.1".into(),
            choices: vec![OpenAIChoice {
                index: 0,
                message: OpenAIMessage {
                    role: "assistant".into(),
                    content: None,
                    tool_calls: Some(vec![
                        OpenAIToolCall {
                            id: "call_abc".into(),
                            call_type: "function".into(),
                            function: OpenAIFunctionCall {
                                name: "write_file".into(),
                                arguments: r#"{"path":"test.txt","content":"hello"}"#.into(),
                            },
                        },
                        OpenAIToolCall {
                            id: "call_def".into(),
                            call_type: "function".into(),
                            function: OpenAIFunctionCall {
                                name: "read_file".into(),
                                arguments: r#"{"path":"other.txt"}"#.into(),
                            },
                        },
                    ]),
                    tool_call_id: None,
                },
                finish_reason: Some("tool_calls".into()),
            }],
            usage: OpenAIUsage {
                prompt_tokens: 50,
                completion_tokens: 30,
                total_tokens: 80,
            },
        };

        let result = provider.convert_response(response).unwrap();
        assert_eq!(result.content, ""); // No text content
        assert!(result.tool_calls.is_some());

        let tool_calls = result.tool_calls.unwrap();
        assert_eq!(tool_calls.len(), 2);
        assert_eq!(tool_calls[0].id, "call_abc");
        assert_eq!(tool_calls[0].name, "write_file");
        assert_eq!(tool_calls[0].arguments["path"], "test.txt");
        assert_eq!(tool_calls[1].id, "call_def");
        assert_eq!(tool_calls[1].name, "read_file");
    }

    #[test]
    fn test_build_request_reasoning_model() {
        let provider = OpenAIProvider::new();

        let request = LlmRequest {
            model: "o3".into(),
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
                    content: "Solve this".into(),
                    tool_call_id: None,
                    tool_calls: None,
                    content_parts: None,
                },
            ],
            temperature: Some(0.7),
            max_tokens: Some(1000),
            tools: None,
            json_schema: None,
            timeout_secs: None,
            web_search: false,
        };

        let openai_req = provider.build_request(&request).unwrap();

        // Reasoning model: system → developer role
        assert_eq!(openai_req.messages[0].role, "developer");
        assert_eq!(openai_req.messages[1].role, "user");

        // Reasoning model: max_completion_tokens instead of max_tokens
        assert!(openai_req.max_tokens.is_none());
        assert_eq!(openai_req.max_completion_tokens, Some(1000));

        // Reasoning model: no temperature
        assert!(openai_req.temperature.is_none());

        // Reasoning model: reasoning_effort set
        assert_eq!(openai_req.reasoning_effort.as_deref(), Some("medium"));
    }

    #[test]
    fn test_build_request_standard_model_no_reasoning_fields() {
        let provider = OpenAIProvider::new();

        let request = LlmRequest {
            model: "gpt-4.1".into(),
            messages: vec![LlmMessage {
                role: MessageRole::System,
                content: "You are helpful".into(),
                tool_call_id: None,
                tool_calls: None,
                content_parts: None,
            }],
            temperature: Some(0.5),
            max_tokens: Some(500),
            tools: None,
            json_schema: None,
            timeout_secs: None,
            web_search: false,
        };

        let openai_req = provider.build_request(&request).unwrap();

        // Standard model: system role stays "system"
        assert_eq!(openai_req.messages[0].role, "system");

        // Standard model: max_tokens, not max_completion_tokens
        assert_eq!(openai_req.max_tokens, Some(500));
        assert!(openai_req.max_completion_tokens.is_none());

        // Standard model: temperature preserved
        assert_eq!(openai_req.temperature, Some(0.5));

        // Standard model: no reasoning_effort
        assert!(openai_req.reasoning_effort.is_none());
    }

    #[test]
    fn test_build_request_with_json_schema() {
        use crate::llm::types::JsonSchema;

        let provider = OpenAIProvider::new();

        let request = LlmRequest {
            model: "gpt-4.1".into(),
            messages: vec![LlmMessage {
                role: MessageRole::User,
                content: "Extract data".into(),
                tool_call_id: None,
                tool_calls: None,
                content_parts: None,
            }],
            temperature: None,
            max_tokens: None,
            tools: None,
            json_schema: Some(JsonSchema {
                name: "extraction".into(),
                schema: serde_json::json!({
                    "type": "object",
                    "properties": { "name": { "type": "string" } },
                    "required": ["name"]
                }),
                strict: true,
            }),
            timeout_secs: None,
            web_search: false,
        };

        let openai_req = provider.build_request(&request).unwrap();
        let rf = openai_req.response_format.expect("response_format should be set");
        assert_eq!(rf.format_type, "json_schema");
        let js = rf.json_schema.expect("json_schema should be set");
        assert_eq!(js.name, "extraction");
        assert!(js.strict);
        assert_eq!(js.schema["type"], "object");
    }

    #[test]
    fn test_is_reasoning_model() {
        assert!(is_reasoning_model("o3"));
        assert!(is_reasoning_model("o4-mini"));
        assert!(!is_reasoning_model("gpt-4.1"));
        assert!(!is_reasoning_model("gpt-4.1-mini"));
        assert!(!is_reasoning_model("gpt-4.1-nano"));
    }

    #[test]
    fn test_deserialize_stream_chunk_with_usage() {
        let json = r#"{
            "id": "chatcmpl-123",
            "object": "chat.completion.chunk",
            "created": 1234567890,
            "model": "gpt-4.1",
            "choices": [],
            "usage": {
                "prompt_tokens": 100,
                "completion_tokens": 50,
                "total_tokens": 150
            }
        }"#;

        let chunk: OpenAIStreamChunk = serde_json::from_str(json).unwrap();
        assert!(chunk.usage.is_some());
        let usage = chunk.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
    }

    #[test]
    fn test_deserialize_stream_chunk_without_usage() {
        let json = r#"{
            "id": "chatcmpl-123",
            "object": "chat.completion.chunk",
            "created": 1234567890,
            "model": "gpt-4.1",
            "choices": [{
                "index": 0,
                "delta": { "content": "Hello" },
                "finish_reason": null
            }]
        }"#;

        let chunk: OpenAIStreamChunk = serde_json::from_str(json).unwrap();
        assert!(chunk.usage.is_none());
        assert_eq!(chunk.choices[0].delta.content.as_deref(), Some("Hello"));
    }
}
