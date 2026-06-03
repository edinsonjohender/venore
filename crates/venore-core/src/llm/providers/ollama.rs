//! Ollama Provider Implementation
//!
//! Implements the LlmProvider trait for Ollama local models.
//! Supports text generation, streaming, and tool calling.
//!
//! ## API Documentation
//! - API: http://localhost:11434/api/*
//! - Docs: https://github.com/ollama/ollama/blob/main/docs/api.md

use crate::traits::{LlmProvider, LlmProviderType};
use crate::{Result, VenoreError};

use super::super::types::{
    LlmRequest, LlmResponse, LlmToolCall, MessageRole,
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

const OLLAMA_HOST: &str = "http://localhost:11434";

// ============================================================================
// OLLAMA REQUEST/RESPONSE TYPES
// ============================================================================

/// Ollama API message format
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OllamaMessage {
    role: String,
    content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OllamaToolCall>>,
}

/// Tool call in Ollama messages (no id, no type — Ollama format)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OllamaToolCall {
    function: OllamaFunctionCall,
}

/// Function call details — arguments is a JSON object (not a string like OpenAI)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OllamaFunctionCall {
    name: String,
    arguments: serde_json::Value,
}

/// Tool definition in request
#[derive(Debug, Clone, Serialize)]
struct OllamaToolDefinition {
    #[serde(rename = "type")]
    tool_type: String,
    function: OllamaFunctionDefinition,
}

/// Function definition in tool
#[derive(Debug, Clone, Serialize)]
struct OllamaFunctionDefinition {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

/// Ollama API options
#[derive(Debug, Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<u32>, // Ollama uses num_predict instead of max_tokens
}

/// Ollama API request
#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OllamaToolDefinition>>,
}

/// Ollama API response
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OllamaResponse {
    model: String,
    message: OllamaMessage,
    done: bool,
    #[serde(default)]
    prompt_eval_count: Option<u32>,
    #[serde(default)]
    eval_count: Option<u32>,
}

/// Ollama tags (models list) response
#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OllamaModel {
    name: String,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    digest: Option<String>,
}

/// Ollama streaming chunk — uses OllamaMessage so it inherits tool_calls
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OllamaStreamChunk {
    model: String,
    message: OllamaMessage,
    done: bool,
    #[serde(default)]
    prompt_eval_count: Option<u32>,
    #[serde(default)]
    eval_count: Option<u32>,
}

// ============================================================================
// PROVIDER IMPLEMENTATION
// ============================================================================

/// Ollama provider
pub struct OllamaProvider {
    client: Client,
}

impl OllamaProvider {
    /// Create a new Ollama provider
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .connect_timeout(std::time::Duration::from_secs(30))
                .timeout(std::time::Duration::from_secs(300)) // 5 min timeout for local models
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    /// Check if Ollama is running
    async fn check_ollama_running(&self) -> Result<()> {
        let response = self.client
            .get(format!("{}/api/version", OLLAMA_HOST))
            .send()
            .await
            .map_err(|e| {
                if e.is_connect() {
                    VenoreError::LlmProviderError(
                        "Ollama is not running. Start it with: ollama serve".into()
                    )
                } else {
                    VenoreError::LlmProviderError(
                        format!("Failed to connect to Ollama: {}", e)
                    )
                }
            })?;

        if !response.status().is_success() {
            return Err(VenoreError::LlmProviderError(
                "Ollama is not responding correctly".into()
            ));
        }

        Ok(())
    }

    /// Get list of installed models
    pub async fn list_models(&self) -> Result<Vec<String>> {
        let response = self.client
            .get(format!("{}/api/tags", OLLAMA_HOST))
            .send()
            .await
            .map_err(|e| VenoreError::LlmProviderError(
                format!("Failed to list Ollama models: {}", e)
            ))?;

        let tags: OllamaTagsResponse = response.json().await
            .map_err(|e| VenoreError::LlmProviderError(
                format!("Failed to parse Ollama models: {}", e)
            ))?;

        Ok(tags.models.into_iter().map(|m| m.name).collect())
    }

    /// Build Ollama request from generic request
    fn build_request(&self, req: &LlmRequest, stream: bool) -> Result<OllamaRequest> {
        let mut messages: Vec<OllamaMessage> = Vec::new();

        for msg in &req.messages {
            match msg.role {
                MessageRole::System => {
                    messages.push(OllamaMessage {
                        role: "system".into(),
                        content: msg.content.clone(),
                        tool_calls: None,
                    });
                }
                MessageRole::User => {
                    messages.push(OllamaMessage {
                        role: "user".into(),
                        content: msg.content.clone(),
                        tool_calls: None,
                    });
                }
                MessageRole::Assistant => {
                    if let Some(ref tool_calls) = msg.tool_calls {
                        // Assistant message with tool calls
                        let ollama_tool_calls: Vec<OllamaToolCall> = tool_calls
                            .iter()
                            .map(|tc| OllamaToolCall {
                                function: OllamaFunctionCall {
                                    name: tc.name.clone(),
                                    arguments: tc.arguments.clone(), // JSON object directly
                                },
                            })
                            .collect();

                        messages.push(OllamaMessage {
                            role: "assistant".into(),
                            content: msg.content.clone(),
                            tool_calls: Some(ollama_tool_calls),
                        });
                    } else {
                        messages.push(OllamaMessage {
                            role: "assistant".into(),
                            content: msg.content.clone(),
                            tool_calls: None,
                        });
                    }
                }
                MessageRole::Tool => {
                    // Tool result — Ollama does NOT use tool_call_id
                    messages.push(OllamaMessage {
                        role: "tool".into(),
                        content: msg.content.clone(),
                        tool_calls: None,
                    });
                }
            }
        }

        if messages.is_empty() {
            return Err(VenoreError::LlmInvalidRequest(
                "At least one message is required".into()
            ));
        }

        let options = if req.temperature.is_some() || req.max_tokens.is_some() {
            Some(OllamaOptions {
                temperature: req.temperature,
                num_predict: req.max_tokens,
            })
        } else {
            None
        };

        // Convert tools
        let tools = req.tools.as_ref().map(|tools| {
            tools.iter().map(|t| OllamaToolDefinition {
                tool_type: "function".into(),
                function: OllamaFunctionDefinition {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: t.parameters.clone(),
                },
            }).collect()
        });

        Ok(OllamaRequest {
            model: req.model.clone(),
            messages,
            stream,
            options,
            tools,
        })
    }

    /// Convert Ollama response to generic response
    fn convert_response(&self, res: OllamaResponse) -> Result<LlmResponse> {
        // Ollama uses prompt_eval_count and eval_count instead of standard token counts
        let usage = if res.prompt_eval_count.is_some() || res.eval_count.is_some() {
            Some(TokenUsage {
                prompt_tokens: res.prompt_eval_count.unwrap_or(0),
                completion_tokens: res.eval_count.unwrap_or(0),
                total_tokens: res.prompt_eval_count.unwrap_or(0) + res.eval_count.unwrap_or(0),
            })
        } else {
            None
        };

        // Extract tool calls with synthetic IDs (Ollama doesn't provide IDs)
        let tool_calls = res.message.tool_calls.as_ref().map(|tcs| {
            tcs.iter().enumerate().map(|(i, tc)| LlmToolCall {
                id: format!("call_{}", i),
                name: tc.function.name.clone(),
                arguments: tc.function.arguments.clone(),
            }).collect::<Vec<_>>()
        });

        Ok(LlmResponse {
            content: res.message.content,
            tool_calls: tool_calls.filter(|v| !v.is_empty()),
            usage,
            provider: LlmProviderType::Ollama,
            model: res.model,
            sources: Vec::new(),
        })
    }

    /// Parse error from Ollama response
    fn parse_error(&self, status: u16, body: &str) -> VenoreError {
        // Ollama errors are usually plain text
        match status {
            404 => {
                if body.contains("model") && body.contains("not found") {
                    VenoreError::LlmProviderError(
                        "Model not found. Pull it with: ollama pull <model>".to_string()
                    )
                } else {
                    VenoreError::LlmProviderError(
                        format!("Ollama API error (404): {}", body)
                    )
                }
            }
            500 => VenoreError::LlmProviderError(
                format!("Ollama internal error: {}", body)
            ),
            _ => VenoreError::LlmProviderError(
                format!("Ollama API error ({}): {}", status, body)
            ),
        }
    }
}

// ============================================================================
// LlmProvider TRAIT IMPLEMENTATION
// ============================================================================

#[async_trait::async_trait]
impl LlmProvider for OllamaProvider {
    fn provider_name(&self) -> &str {
        "ollama"
    }

    fn supported_models(&self) -> Vec<String> {
        registry::get_provider_models(LlmProviderType::Ollama)
    }

    fn default_model(&self) -> String {
        registry::get_default_model(LlmProviderType::Ollama)
    }

    async fn complete(&self, _api_key: &str, request: LlmRequest) -> Result<LlmResponse> {
        // Check if Ollama is running
        self.check_ollama_running().await?;

        let ollama_req = self.build_request(&request, false)?;

        tracing::debug!(
            "Sending request to Ollama: model={}, messages={}",
            ollama_req.model,
            ollama_req.messages.len()
        );

        let response = self.client
            .post(format!("{}/api/chat", OLLAMA_HOST))
            .header("Content-Type", "application/json")
            .json(&ollama_req)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    VenoreError::LlmProviderError("Request timed out".into())
                } else {
                    VenoreError::LlmProviderError(
                        format!("Failed to send request to Ollama: {}", e)
                    )
                }
            })?;

        let status = response.status();

        let body = response.text().await
            .map_err(|e| VenoreError::LlmProviderError(
                format!("Failed to read Ollama response: {}", e)
            ))?;

        if !status.is_success() {
            return Err(self.parse_error(status.as_u16(), &body));
        }

        let ollama_response: OllamaResponse = serde_json::from_str(&body)
            .map_err(|e| VenoreError::LlmInvalidResponse(
                format!("Failed to parse Ollama response: {}", e)
            ))?;

        tracing::debug!(
            "Received response from Ollama: prompt_tokens={:?}, completion_tokens={:?}",
            ollama_response.prompt_eval_count,
            ollama_response.eval_count
        );

        self.convert_response(ollama_response)
    }

    async fn stream(&self, _api_key: &str, request: LlmRequest) -> Result<LlmStream> {
        // Check if Ollama is running
        self.check_ollama_running().await?;

        let ollama_req = self.build_request(&request, true)?;

        tracing::debug!(
            "Starting stream from Ollama: model={}, messages={}",
            ollama_req.model,
            ollama_req.messages.len()
        );

        let response = self.client
            .post(format!("{}/api/chat", OLLAMA_HOST))
            .header("Content-Type", "application/json")
            .json(&ollama_req)
            .send()
            .await
            .map_err(|e| VenoreError::LlmProviderError(
                format!("Failed to start Ollama stream: {}", e)
            ))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await
                .unwrap_or_else(|_| "Failed to read error body".to_string());
            return Err(self.parse_error(status.as_u16(), &body));
        }

        // Convert response stream to LlmStream
        // Ollama sends tool calls as complete objects (not incremental like OpenAI)
        // Buffer persists across HTTP chunks to handle JSON split across TCP packet boundaries.
        let byte_stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut tool_call_counter: usize = 0;

        let chunk_stream = byte_stream.flat_map(move |result| {
            let events: Vec<crate::Result<LlmStreamChunk>> = match result {
                Ok(bytes) => {
                    // Ollama sends line-delimited JSON (one JSON object per line)
                    let text = String::from_utf8_lossy(&bytes);
                    buffer.push_str(&text);
                    let mut events = Vec::new();

                    // Only process complete newline-delimited lines
                    while let Some(newline_pos) = buffer.find('\n') {
                        let line: String = buffer.drain(..=newline_pos).collect();
                        let line = line.trim().to_string();
                        if line.is_empty() {
                            continue;
                        }

                        // Parse JSON chunk
                        if let Ok(chunk) = serde_json::from_str::<OllamaStreamChunk>(&line) {
                            // Emit tool calls if present (complete objects, not incremental)
                            if let Some(ref tool_calls) = chunk.message.tool_calls {
                                for tc in tool_calls {
                                    events.push(Ok(LlmStreamChunk::ToolCall {
                                        call: LlmToolCall {
                                            id: format!("call_{}", tool_call_counter),
                                            name: tc.function.name.clone(),
                                            arguments: tc.function.arguments.clone(),
                                        },
                                    }));
                                    tool_call_counter += 1;
                                }
                            }

                            if chunk.done {
                                // Stream finished
                                let usage = if chunk.prompt_eval_count.is_some() || chunk.eval_count.is_some() {
                                    Some(TokenUsage {
                                        prompt_tokens: chunk.prompt_eval_count.unwrap_or(0),
                                        completion_tokens: chunk.eval_count.unwrap_or(0),
                                        total_tokens: chunk.prompt_eval_count.unwrap_or(0) + chunk.eval_count.unwrap_or(0),
                                    })
                                } else {
                                    None
                                };
                                events.push(Ok(LlmStreamChunk::Done { usage, sources: Vec::new() }));
                            } else if !chunk.message.content.is_empty() {
                                events.push(Ok(LlmStreamChunk::Text { content: chunk.message.content }));
                            }
                        }
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

    async fn test(&self, _api_key: &str, model: &str) -> Result<ProviderTestResult> {
        let start = Instant::now();

        // Just check if Ollama is running - don't require a model
        match self.check_ollama_running().await {
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
                Ok(ProviderTestResult {
                    success: false,
                    model: model.to_string(),
                    latency_ms: latency,
                    error: Some(format!("{}", e)),
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
    use crate::llm::types::{LlmMessage, LlmTool};

    #[test]
    fn test_build_request() {
        let provider = OllamaProvider::new();

        let request = LlmRequest {
            model: "qwen3:8b".into(),
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

        let result = provider.build_request(&request, false);
        assert!(result.is_ok());

        let ollama_req = result.unwrap();
        assert_eq!(ollama_req.model, "qwen3:8b");
        assert_eq!(ollama_req.messages.len(), 1);
        assert_eq!(ollama_req.messages[0].role, "user");
        assert_eq!(ollama_req.messages[0].content, "Hello");
        assert!(!ollama_req.stream);
    }

    #[test]
    fn test_build_request_with_options() {
        let provider = OllamaProvider::new();

        let request = LlmRequest {
            model: "qwen3:8b".into(),
            messages: vec![
                LlmMessage {
                    role: MessageRole::User,
                    content: "Hello".into(),
                    tool_call_id: None,
                    tool_calls: None,
                    content_parts: None,
                }
            ],
            temperature: Some(0.5),
            max_tokens: Some(200),
            tools: None,
            json_schema: None,
            timeout_secs: Some(30),
            web_search: false,
        };

        let result = provider.build_request(&request, false);
        assert!(result.is_ok());

        let ollama_req = result.unwrap();
        assert!(ollama_req.options.is_some());
        let options = ollama_req.options.unwrap();
        assert_eq!(options.temperature, Some(0.5));
        assert_eq!(options.num_predict, Some(200));
    }

    #[test]
    fn test_build_request_with_tools() {
        let provider = OllamaProvider::new();

        let request = LlmRequest {
            model: "qwen3:8b".into(),
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

        let ollama_req = provider.build_request(&request, false).unwrap();
        assert!(ollama_req.tools.is_some());

        let tools = ollama_req.tools.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].tool_type, "function");
        assert_eq!(tools[0].function.name, "write_file");
    }

    #[test]
    fn test_build_request_with_tool_result_messages() {
        let provider = OllamaProvider::new();

        let request = LlmRequest {
            model: "qwen3:8b".into(),
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
                        id: "call_0".into(),
                        name: "write_file".into(),
                        arguments: serde_json::json!({"path": "test.txt", "content": "hello"}),
                    }]),
                    content_parts: None,
                },
                LlmMessage {
                    role: MessageRole::Tool,
                    content: "File written successfully".into(),
                    tool_call_id: Some("call_0".into()),
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

        let ollama_req = provider.build_request(&request, false).unwrap();
        assert_eq!(ollama_req.messages.len(), 3);

        // Assistant message with tool_calls
        let assistant = &ollama_req.messages[1];
        assert_eq!(assistant.role, "assistant");
        assert!(assistant.tool_calls.is_some());
        let tcs = assistant.tool_calls.as_ref().unwrap();
        assert_eq!(tcs.len(), 1);
        assert_eq!(tcs[0].function.name, "write_file");
        // arguments should be a JSON object (not a string)
        assert_eq!(tcs[0].function.arguments["path"], "test.txt");

        // Tool result — NO tool_call_id (Ollama doesn't use it)
        let tool_msg = &ollama_req.messages[2];
        assert_eq!(tool_msg.role, "tool");
        assert_eq!(tool_msg.content, "File written successfully");
        // Verify tool_call_id is NOT serialized — OllamaMessage doesn't have it
    }

    #[test]
    fn test_convert_response_with_tool_calls() {
        let provider = OllamaProvider::new();

        let response = OllamaResponse {
            model: "qwen3:8b".into(),
            message: OllamaMessage {
                role: "assistant".into(),
                content: "".into(),
                tool_calls: Some(vec![
                    OllamaToolCall {
                        function: OllamaFunctionCall {
                            name: "write_file".into(),
                            arguments: serde_json::json!({"path": "test.txt", "content": "hello"}),
                        },
                    },
                    OllamaToolCall {
                        function: OllamaFunctionCall {
                            name: "read_file".into(),
                            arguments: serde_json::json!({"path": "other.txt"}),
                        },
                    },
                ]),
            },
            done: true,
            prompt_eval_count: Some(50),
            eval_count: Some(30),
        };

        let result = provider.convert_response(response).unwrap();
        assert!(result.tool_calls.is_some());

        let tool_calls = result.tool_calls.unwrap();
        assert_eq!(tool_calls.len(), 2);

        // Verify synthetic IDs
        assert_eq!(tool_calls[0].id, "call_0");
        assert_eq!(tool_calls[0].name, "write_file");
        assert_eq!(tool_calls[0].arguments["path"], "test.txt");

        assert_eq!(tool_calls[1].id, "call_1");
        assert_eq!(tool_calls[1].name, "read_file");
        assert_eq!(tool_calls[1].arguments["path"], "other.txt");
    }
}
