//! Gemini Provider Implementation
//!
//! Implements the LlmProvider trait for Google's Gemini models.
//!
//! ## API Documentation
//! - API: https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent
//! - Docs: https://ai.google.dev/docs

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::traits::{LlmProvider, LlmProviderType};
use crate::{Result, VenoreError};

use super::super::types::{
    LlmRequest, LlmResponse, LlmMessage, LlmToolCall, MessageRole,
    LlmStream, LlmStreamChunk, TokenUsage, ProviderTestResult,
};

/// Global counter for Gemini tool call IDs.
/// Must be global (not per-stream) because the agentic loop makes multiple
/// stream() calls — per-stream counters produce colliding IDs (call_1, call_2…)
/// across iterations, causing the frontend to match results to wrong tool cards.
static GEMINI_TOOL_CALL_COUNTER: AtomicUsize = AtomicUsize::new(1);
use super::super::registry;

use serde::{Deserialize, Serialize};
use reqwest::Client;
use futures::StreamExt;
use std::time::Instant;

// ============================================================================
// CONSTANTS
// ============================================================================

const API_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

// ============================================================================
// GEMINI REQUEST/RESPONSE TYPES
// ============================================================================

/// Gemini API content part — can be text, functionCall, or functionResponse
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiPart {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(default, rename = "inlineData", skip_serializing_if = "Option::is_none")]
    inline_data: Option<GeminiInlineData>,
    #[serde(default, rename = "functionCall", skip_serializing_if = "Option::is_none")]
    function_call: Option<GeminiFunctionCall>,
    #[serde(default, rename = "functionResponse", skip_serializing_if = "Option::is_none")]
    function_response: Option<GeminiFunctionResponse>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    thought: Option<bool>,
    /// Opaque signature returned by Gemini when thinking + tools are used.
    /// Must be preserved and sent back in subsequent turns for coherent reasoning.
    #[serde(default, rename = "thoughtSignature", skip_serializing_if = "Option::is_none")]
    thought_signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiInlineData {
    #[serde(rename = "mimeType")]
    mime_type: String,
    data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiFunctionCall {
    name: String,
    args: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiFunctionResponse {
    name: String,
    response: serde_json::Value,
}

/// One entry inside the Gemini `tools` array.
///
/// The Gemini API treats `function_declarations` (custom function tools)
/// and built-in tools like `google_search` as siblings inside the same
/// `tools` array — never as one nested under the other. We model each
/// entry as a tagless enum so both shapes serialize correctly.
///
/// Spec: https://ai.google.dev/gemini-api/docs/google-search
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
enum GeminiToolEntry {
    /// `{ "function_declarations": [...] }` — the agent's custom tools.
    Functions {
        function_declarations: Vec<GeminiFunctionDeclItem>,
    },
    /// `{ "google_search": {} }` — Grounding with Google Search. The empty
    /// object is required by the API; an absent key disables grounding.
    GoogleSearch {
        google_search: serde_json::Map<String, serde_json::Value>,
    },
}

#[derive(Debug, Clone, Serialize)]
struct GeminiFunctionDeclItem {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

/// Gemini API content
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiContent {
    #[serde(default)]
    role: String,
    #[serde(default)]
    parts: Vec<GeminiPart>,
}

/// Gemini thinking mode config — enables internal reasoning (thought parts).
/// The model reasons internally; only the final concise text reaches the user.
#[derive(Debug, Serialize)]
struct GeminiThinkingConfig {
    /// Must be true for the API to return thought parts separately.
    #[serde(rename = "includeThoughts")]
    include_thoughts: bool,
    /// Token budget for thinking. 16384 = high reasoning budget.
    #[serde(rename = "thinkingBudget")]
    thinking_budget: i32,
}

/// Gemini API generation config
#[derive(Debug, Serialize)]
struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "thinkingConfig")]
    thinking_config: Option<GeminiThinkingConfig>,
}

/// Gemini API system instruction (dedicated field, not mixed into user messages)
#[derive(Debug, Serialize)]
struct GeminiSystemInstruction {
    parts: Vec<GeminiPart>,
}

/// Gemini function calling config — controls whether the model must/may/cannot call tools.
#[derive(Debug, Clone, Serialize)]
struct GeminiFunctionCallingConfig {
    /// "AUTO" = model decides, "ANY" = must call a tool, "NONE" = no tools.
    mode: String,
}

/// Gemini tool config — wraps function calling behavior settings.
#[derive(Debug, Clone, Serialize)]
struct GeminiToolConfig {
    #[serde(rename = "functionCallingConfig")]
    function_calling_config: GeminiFunctionCallingConfig,
}

/// Gemini API request
#[derive(Debug, Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "systemInstruction")]
    system_instruction: Option<GeminiSystemInstruction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "generationConfig")]
    generation_config: Option<GeminiGenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<GeminiToolEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "toolConfig")]
    tool_config: Option<GeminiToolConfig>,
}

/// Gemini API response
///
/// All fields use `default` or `Option` because Gemini streaming events have
/// varying structures: early chunks may omit `candidatesTokenCount` in
/// `usageMetadata`, some events lack `candidates` entirely (thinking phase,
/// content filtering), and the `content` field inside candidates is absent
/// on the final event. Strict required fields cause serde to reject valid
/// events, silently losing their text content.
#[derive(Debug, Deserialize)]
struct GeminiResponse {
    #[serde(default)]
    candidates: Vec<GeminiCandidate>,
    #[serde(rename = "usageMetadata")]
    usage_metadata: Option<GeminiUsageMetadata>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiContent>,
    #[serde(rename = "finishReason")]
    finish_reason: Option<String>,
    /// Set when `google_search` ran during this turn. Carries the queries
    /// the model used and the list of cited web pages.
    #[serde(rename = "groundingMetadata", default)]
    grounding_metadata: Option<GroundingMetadata>,
}

/// Returned by Gemini when grounding ran. We only consume `groundingChunks`
/// today; the other fields are kept on the deserialized side via
/// `#[serde(default)]` so future use doesn't require a schema change.
#[derive(Debug, Default, Deserialize)]
struct GroundingMetadata {
    #[serde(rename = "groundingChunks", default)]
    grounding_chunks: Vec<GroundingChunk>,
}

#[derive(Debug, Default, Deserialize)]
struct GroundingChunk {
    /// Web sources have a `web` object — other variants (e.g. internal
    /// retrievers) may appear in the future; we ignore them.
    #[serde(default)]
    web: Option<GroundingChunkWeb>,
}

#[derive(Debug, Default, Deserialize)]
struct GroundingChunkWeb {
    #[serde(default)]
    uri: String,
    #[serde(default)]
    title: String,
}

/// Token usage metadata — ALL fields use `default` because Gemini sends
/// partial metadata in early streaming chunks (e.g. `candidatesTokenCount`
/// is absent until the model starts generating tokens).
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GeminiUsageMetadata {
    #[serde(rename = "promptTokenCount", default)]
    prompt_token_count: u32,
    #[serde(rename = "candidatesTokenCount", default)]
    candidates_token_count: u32,
    #[serde(rename = "totalTokenCount", default)]
    total_token_count: u32,
    #[serde(rename = "thoughtsTokenCount", default)]
    thoughts_token_count: u32,
}

/// Gemini error response
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GeminiError {
    code: u16,
    message: String,
    status: String,
}

#[derive(Debug, Deserialize)]
struct GeminiErrorResponse {
    error: GeminiError,
}

// ============================================================================
// GROUNDING HELPERS
// ============================================================================
//
// Knobs for which Gemini variants accept the built-in `google_search` tool
// and which can combine it with custom `function_declarations`. Source:
// https://ai.google.dev/gemini-api/docs/google-search (May 2026 docs).
//
// Kept as simple string-prefix checks so they don't rot when Google adds a
// new minor revision — anything under `gemini-3.*` or the listed 2.x lines
// just works.

/// Whether this Gemini model id supports the `google_search` grounding
/// tool. Older "google_search_retrieval"-only models aren't matched on
/// purpose — we never want to fall back to the deprecated tool name.
fn supports_grounding(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    m.starts_with("gemini-3.")
        || m.starts_with("gemini-2.5-pro")
        || m.starts_with("gemini-2.5-flash")
        || m.starts_with("gemini-2.0-flash")
}

/// Gemini 2.5 forbids mixing `google_search` with `function_declarations` in
/// the same request — only 3.x lifted that restriction. Callers use this to
/// decide whether to drop grounding when both kinds of tools are requested.
fn grounding_combines_with_functions(model: &str) -> bool {
    model.to_ascii_lowercase().starts_with("gemini-3.")
}

/// Flatten a `groundingMetadata` payload into the cross-provider
/// `LlmSource` list. Drops chunks without a usable URI.
fn sources_from_grounding(meta: Option<&GroundingMetadata>) -> Vec<crate::llm::types::LlmSource> {
    let Some(meta) = meta else {
        return Vec::new();
    };
    meta.grounding_chunks
        .iter()
        .filter_map(|chunk| chunk.web.as_ref())
        .filter(|w| !w.uri.is_empty())
        .map(|w| crate::llm::types::LlmSource {
            uri: w.uri.clone(),
            title: w.title.clone(),
        })
        .collect()
}

// ============================================================================
// PROVIDER IMPLEMENTATION
// ============================================================================

/// Gemini provider
pub struct GeminiProvider {
    client: Client,
}

impl GeminiProvider {
    /// Create a new Gemini provider
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .connect_timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    /// Build Gemini request from generic request
    fn build_request(&self, req: &LlmRequest) -> Result<GeminiRequest> {
        // Gemini uses "contents" with role and parts
        // System messages go into the dedicated systemInstruction field
        let mut system_instruction: Option<GeminiSystemInstruction> = None;
        let mut contents = Vec::new();

        for msg in &req.messages {
            match msg.role {
                MessageRole::System => {
                    system_instruction = Some(GeminiSystemInstruction {
                        parts: vec![GeminiPart {
                            text: Some(msg.content.clone()),
                            inline_data: None,
                            function_call: None,
                            function_response: None,
                            thought: None,
                            thought_signature: None,
                        }],
                    });
                }
                MessageRole::User => {
                    // Check for multimodal content_parts (images + text)
                    let parts = if let Some(ref content_parts) = msg.content_parts {
                        let mut parts = Vec::new();
                        for part in content_parts {
                            match part {
                                super::super::types::ContentPart::ImageBase64 { media_type, data } => {
                                    parts.push(GeminiPart {
                                        text: None,
                                        inline_data: Some(GeminiInlineData {
                                            mime_type: media_type.clone(),
                                            data: data.clone(),
                                        }),
                                        function_call: None,
                                        function_response: None,
                                        thought: None,
                                        thought_signature: None,
                                    });
                                }
                                super::super::types::ContentPart::Text { text } => {
                                    parts.push(GeminiPart {
                                        text: Some(text.clone()),
                                        inline_data: None,
                                        function_call: None,
                                        function_response: None,
                                        thought: None,
                                        thought_signature: None,
                                    });
                                }
                            }
                        }
                        if !msg.content.is_empty() {
                            parts.push(GeminiPart {
                                text: Some(msg.content.clone()),
                                inline_data: None,
                                function_call: None,
                                function_response: None,
                                thought: None,
                                thought_signature: None,
                            });
                        }
                        parts
                    } else {
                        vec![GeminiPart {
                            text: Some(msg.content.clone()),
                            inline_data: None,
                            function_call: None,
                            function_response: None,
                            thought: None,
                            thought_signature: None,
                        }]
                    };
                    contents.push(GeminiContent {
                        role: "user".into(),
                        parts,
                    });
                }
                MessageRole::Assistant => {
                    // If assistant has tool_calls, create functionCall parts
                    if let Some(ref tool_calls) = msg.tool_calls {
                        let mut parts = Vec::new();

                        if !msg.content.is_empty() {
                            parts.push(GeminiPart {
                                text: Some(msg.content.clone()),
                                inline_data: None,
                                function_call: None,
                                function_response: None,
                                thought: None,
                                thought_signature: None,
                            });
                        }

                        for tc in tool_calls {
                            parts.push(GeminiPart {
                                text: None,
                                inline_data: None,
                                function_call: Some(GeminiFunctionCall {
                                    name: tc.name.clone(),
                                    args: tc.arguments.clone(),
                                }),
                                function_response: None,
                                thought: None,
                                thought_signature: None,
                            });
                        }

                        contents.push(GeminiContent {
                            role: "model".into(),
                            parts,
                        });
                    } else {
                        contents.push(GeminiContent {
                            role: "model".into(),
                            parts: vec![GeminiPart {
                                text: Some(msg.content.clone()),
                                inline_data: None,
                                function_call: None,
                                function_response: None,
                                thought: None,
                                thought_signature: None,
                            }],
                        });
                    }
                }
                MessageRole::Tool => {
                    // Gemini: all consecutive tool results must be merged into ONE
                    // user turn with multiple functionResponse parts.
                    // Gemini requires strict user→model alternation — consecutive
                    // "user" turns cause silent failures or confused responses.
                    let tool_name = Self::find_tool_name_for_id(
                        &req.messages,
                        msg.tool_call_id.as_deref().unwrap_or(""),
                    );

                    let part = GeminiPart {
                        text: None,
                        inline_data: None,
                        function_call: None,
                        function_response: Some(GeminiFunctionResponse {
                            name: tool_name,
                            response: serde_json::json!({ "content": msg.content }),
                        }),
                        thought: None,
                        thought_signature: None,
                    };

                    // Merge into previous user turn if it contains functionResponse parts
                    let should_merge = contents.last()
                        .map(|c: &GeminiContent| {
                            c.role == "user" && c.parts.iter().any(|p| p.function_response.is_some())
                        })
                        .unwrap_or(false);

                    if should_merge {
                        contents.last_mut().unwrap().parts.push(part);
                    } else {
                        contents.push(GeminiContent {
                            role: "user".into(),
                            parts: vec![part],
                        });
                    }
                }
            }
        }

        if contents.is_empty() {
            return Err(VenoreError::LlmInvalidRequest(
                "At least one message is required".into()
            ));
        }

        // Disable thinking when a JSON schema is requested. Structured outputs
        // have rigid format requirements; on Gemini 2.5 the `thinkingBudget`
        // competes with output tokens under `maxOutputTokens`, so a large
        // thinking allowance can starve the actual JSON and produce a truncated
        // response that fails to parse. Free-form generation still benefits
        // from thinking and keeps it enabled.
        let thinking_config = if req.json_schema.is_some() {
            None
        } else {
            Some(GeminiThinkingConfig {
                include_thoughts: true,
                thinking_budget: 16384,
            })
        };

        let generation_config = Some(GeminiGenerationConfig {
            temperature: req.temperature,
            max_output_tokens: req.max_tokens,
            thinking_config,
        });

        // Convert tools to Gemini format + set toolConfig.
        //
        // The tools array can carry two distinct kinds of entries:
        //   - `function_declarations` — the agent's custom tools (read_file,
        //     search_code, etc.).
        //   - `google_search` — built-in grounding tool, present when the
        //     caller opted into web search via `LlmRequest::web_search`.
        //
        // Gemini 2.5 forbids mixing `google_search` with
        // `function_declarations`; only Gemini 3.x allows it. So we resolve
        // a small state machine first:
        //
        //   - If grounding is wanted AND it can coexist with functions (or
        //     there are no functions) → emit grounding, drop the redundant
        //     `web_search` function declaration so the agent uses native
        //     grounding instead of the Tavily fallback.
        //   - If grounding is wanted but cannot coexist with functions on
        //     this model → keep `web_search` in the function declarations
        //     (Tavily fallback), no grounding entry. The agent still has a
        //     way to search the web; the request still succeeds.
        let mut entries: Vec<GeminiToolEntry> = Vec::new();
        let request_tools = req.tools.as_deref().unwrap_or(&[]);
        let has_functions = !request_tools.is_empty();

        let wants_grounding = req.web_search && supports_grounding(&req.model);
        let grounding_active = wants_grounding
            && (!has_functions || grounding_combines_with_functions(&req.model));

        if wants_grounding && !grounding_active {
            tracing::warn!(
                model = %req.model,
                "Gemini 2.5 cannot combine google_search with function_declarations — \
                 falling back to web_search function tool (Tavily) for this request"
            );
        }

        let function_declarations: Vec<GeminiFunctionDeclItem> = request_tools
            .iter()
            .filter(|t| !(grounding_active && t.name == "web_search"))
            .map(|t| GeminiFunctionDeclItem {
                name: t.name.clone(),
                description: t.description.clone(),
                parameters: t.parameters.clone(),
            })
            .collect();

        let emits_functions = !function_declarations.is_empty();
        if emits_functions {
            entries.push(GeminiToolEntry::Functions {
                function_declarations,
            });
        }

        if grounding_active {
            entries.push(GeminiToolEntry::GoogleSearch {
                google_search: serde_json::Map::new(),
            });
        }

        let tools = if entries.is_empty() { None } else { Some(entries) };
        let tool_config = if emits_functions {
            Some(GeminiToolConfig {
                function_calling_config: GeminiFunctionCallingConfig {
                    mode: "AUTO".to_string(),
                },
            })
        } else {
            None
        };

        Ok(GeminiRequest {
            contents,
            system_instruction,
            generation_config,
            tools,
            tool_config,
        })
    }

    /// Find the tool name that matches a tool_call_id from previous messages
    fn find_tool_name_for_id(messages: &[LlmMessage], tool_call_id: &str) -> String {
        for msg in messages.iter().rev() {
            if let Some(ref calls) = msg.tool_calls {
                for tc in calls {
                    if tc.id == tool_call_id {
                        return tc.name.clone();
                    }
                }
            }
        }
        // Fallback: use the tool_call_id itself
        tool_call_id.to_string()
    }

    /// Convert Gemini response to generic response
    fn convert_response(&self, res: GeminiResponse, model: String) -> Result<LlmResponse> {
        let candidate = res.candidates.first()
            .ok_or_else(|| VenoreError::LlmInvalidResponse("No candidates in response".into()))?;

        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();

        if let Some(ref content) = candidate.content {
            for part in &content.parts {
                if part.thought == Some(true) {
                    continue;
                }
                if let Some(ref text) = part.text {
                    text_parts.push(text.as_str());
                }
                if let Some(ref fc) = part.function_call {
                    tool_calls.push(LlmToolCall {
                        id: format!("call_{}", tool_calls.len()),
                        name: fc.name.clone(),
                        arguments: fc.args.clone(),
                    });
                }
            }
        }

        let usage = res.usage_metadata.map(|meta| TokenUsage {
            prompt_tokens: meta.prompt_token_count,
            completion_tokens: meta.candidates_token_count,
            total_tokens: meta.total_token_count,
        });

        // When grounding ran during this turn the candidate carries a
        // `groundingMetadata` block; flatten the chunks into the
        // cross-provider `sources` array so the UI can render citations
        // without knowing anything about Gemini's payload shape.
        let sources = sources_from_grounding(candidate.grounding_metadata.as_ref());

        Ok(LlmResponse {
            content: text_parts.join(""),
            tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
            usage,
            provider: LlmProviderType::Gemini,
            model,
            sources,
        })
    }

    /// Parse error from Gemini response
    fn parse_error(&self, status: u16, body: &str, retry_after: Option<&str>) -> VenoreError {
        // Try to parse as Gemini error
        if let Ok(error_response) = serde_json::from_str::<GeminiErrorResponse>(body) {
            let message = error_response.error.message;

            return match status {
                400 => {
                    if message.contains("API key not valid") {
                        VenoreError::LlmNoApiKey("Invalid Gemini API key".into())
                    } else {
                        VenoreError::LlmInvalidRequest(message)
                    }
                }
                403 => VenoreError::LlmNoApiKey("Gemini API key forbidden".into()),
                429 => {
                    let retry_after_secs = retry_after.and_then(|h| h.parse::<u64>().ok());
                    VenoreError::LlmRateLimit { retry_after_secs }
                }
                _ => VenoreError::LlmProviderError(
                    format!("Gemini API error ({}): {}", status, message)
                ),
            };
        }

        // Fallback: generic error
        VenoreError::LlmProviderError(
            format!("Gemini API error ({}): {}", status, body)
        )
    }

    /// Build API URL for a model
    fn build_url(&self, model: &str, streaming: bool) -> String {
        let endpoint = if streaming {
            "streamGenerateContent"
        } else {
            "generateContent"
        };

        format!("{}/models/{}:{}", API_BASE_URL, model, endpoint)
    }
}

// ============================================================================
// LlmProvider TRAIT IMPLEMENTATION
// ============================================================================

#[async_trait::async_trait]
impl LlmProvider for GeminiProvider {
    fn provider_name(&self) -> &str {
        "gemini"
    }

    fn supported_models(&self) -> Vec<String> {
        registry::get_provider_models(LlmProviderType::Gemini)
    }

    fn default_model(&self) -> String {
        registry::get_default_model(LlmProviderType::Gemini)
    }

    async fn complete(&self, api_key: &str, request: LlmRequest) -> Result<LlmResponse> {
        let gemini_req = self.build_request(&request)?;
        let model = request.model.clone();

        tracing::debug!(
            "Sending request to Gemini: model={}, messages={}",
            model,
            gemini_req.contents.len()
        );

        let url = self.build_url(&model, false);

        let response = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .query(&[("key", api_key)])
            .json(&gemini_req)
            .send()
            .await
            .map_err(|e| VenoreError::LlmProviderError(
                format!("Failed to send request to Gemini: {}", e)
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
                format!("Failed to read Gemini response: {}", e)
            ))?;

        if !status.is_success() {
            return Err(self.parse_error(status.as_u16(), &body, retry_after.as_deref()));
        }

        let gemini_response: GeminiResponse = serde_json::from_str(&body)
            .map_err(|e| VenoreError::LlmInvalidResponse(
                format!("Failed to parse Gemini response: {}", e)
            ))?;

        tracing::debug!(
            "Received response from Gemini: tokens={:?}",
            gemini_response.usage_metadata.as_ref().map(|u| u.total_token_count)
        );

        self.convert_response(gemini_response, model)
    }

    async fn stream(&self, api_key: &str, request: LlmRequest) -> Result<LlmStream> {
        let gemini_req = self.build_request(&request)?;
        let model = request.model.clone();

        tracing::debug!(
            "Starting stream from Gemini: model={}, messages={}",
            model,
            gemini_req.contents.len()
        );

        let url = self.build_url(&model, true);

        let response = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .query(&[("key", api_key), ("alt", "sse")])
            .json(&gemini_req)
            .send()
            .await
            .map_err(|e| VenoreError::LlmProviderError(
                format!("Failed to start Gemini stream: {}", e)
            ))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await
                .unwrap_or_else(|_| "Failed to read error body".to_string());
            return Err(self.parse_error(status.as_u16(), &body, None));
        }

        // Buffer persists across HTTP chunks to handle SSE events split across
        // TCP packet boundaries. Without this, partial JSON is silently lost.
        let byte_stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut consecutive_parse_failures: u32 = 0;

        let chunk_stream = byte_stream.flat_map(move |result| {
            let events: Vec<crate::Result<LlmStreamChunk>> = match result {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    buffer.push_str(&text);

                    let mut events = Vec::new();
                    let mut accumulated_text = String::new();

                    // Process only complete lines; incomplete lines stay in buffer
                    while let Some(newline_pos) = buffer.find('\n') {
                        let line: String = buffer.drain(..=newline_pos).collect();
                        let trimmed = line.trim().to_string();

                        if trimmed.is_empty() {
                            continue;
                        }

                        if let Some(json_str) = trimmed.strip_prefix("data: ") {
                            match serde_json::from_str::<GeminiResponse>(json_str) {
                                Ok(chunk) => {
                                    consecutive_parse_failures = 0;
                                    if let Some(candidate) = chunk.candidates.first() {
                                        if let Some(ref content_obj) = candidate.content {
                                            for part in &content_obj.parts {
                                                if part.thought == Some(true) {
                                                    continue;
                                                }
                                                // Text part
                                                if let Some(ref t) = part.text {
                                                    accumulated_text.push_str(t);
                                                }
                                                // Function call part
                                                if let Some(ref fc) = part.function_call {
                                                    // Flush accumulated text first
                                                    if !accumulated_text.is_empty() {
                                                        events.push(Ok(LlmStreamChunk::Text {
                                                            content: std::mem::take(&mut accumulated_text),
                                                        }));
                                                    }
                                                    let call_id = GEMINI_TOOL_CALL_COUNTER.fetch_add(1, Ordering::Relaxed);
                                                    events.push(Ok(LlmStreamChunk::ToolCall {
                                                        call: LlmToolCall {
                                                            id: format!("call_{}", call_id),
                                                            name: fc.name.clone(),
                                                            arguments: fc.args.clone(),
                                                        },
                                                    }));
                                                }
                                            }
                                        }

                                        if candidate.finish_reason.is_some() {
                                            // Flush text before done
                                            if !accumulated_text.is_empty() {
                                                events.push(Ok(LlmStreamChunk::Text {
                                                    content: std::mem::take(&mut accumulated_text),
                                                }));
                                            }
                                            let usage = chunk.usage_metadata.map(|meta| TokenUsage {
                                                prompt_tokens: meta.prompt_token_count,
                                                completion_tokens: meta.candidates_token_count,
                                                total_tokens: meta.total_token_count,
                                            });
                                            // groundingMetadata travels in
                                            // the final candidate chunk
                                            // (when finish_reason is set).
                                            let sources = sources_from_grounding(
                                                candidate.grounding_metadata.as_ref(),
                                            );
                                            events.push(Ok(LlmStreamChunk::Done { usage, sources }));
                                        }
                                    }
                                }
                                Err(e) => {
                                    consecutive_parse_failures += 1;
                                    if consecutive_parse_failures >= 5 {
                                        events.push(Ok(LlmStreamChunk::Error {
                                            error: format!("Too many consecutive SSE parse failures ({}): {}", consecutive_parse_failures, e),
                                        }));
                                    } else {
                                        tracing::debug!("[Gemini] SSE parse skip: {}", e);
                                    }
                                }
                            }
                        }
                    }

                    // Flush remaining accumulated text
                    if !accumulated_text.is_empty() {
                        events.push(Ok(LlmStreamChunk::Text { content: accumulated_text }));
                    }

                    events
                }
                Err(e) => vec![Ok(LlmStreamChunk::Error {
                    error: format!("Stream error: {}", e),
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

    #[test]
    fn test_build_request() {
        let provider = GeminiProvider::new();

        let request = LlmRequest {
            model: "gemini-2.5-flash".into(),
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

        let gemini_req = result.unwrap();
        assert_eq!(gemini_req.contents.len(), 1);
        assert_eq!(gemini_req.contents[0].role, "user");
        assert_eq!(gemini_req.contents[0].parts[0].text.as_deref().unwrap(), "Hello");
    }

    #[test]
    fn test_build_request_with_system_message() {
        let provider = GeminiProvider::new();

        let request = LlmRequest {
            model: "gemini-2.5-flash".into(),
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

        let gemini_req = result.unwrap();
        // System message goes to dedicated systemInstruction field
        let si = gemini_req.system_instruction.expect("system_instruction should be set");
        assert!(si.parts[0].text.as_deref().unwrap().contains("You are helpful"));
        // User message stays clean
        assert_eq!(gemini_req.contents.len(), 1);
        assert_eq!(gemini_req.contents[0].role, "user");
        let text = gemini_req.contents[0].parts[0].text.as_deref().unwrap();
        assert_eq!(text, "Hello");
    }

    #[test]
    fn test_build_url() {
        let provider = GeminiProvider::new();

        let url = provider.build_url("gemini-2.5-flash", false);
        assert!(url.contains("gemini-2.5-flash"));
        assert!(url.contains("generateContent"));

        let stream_url = provider.build_url("gemini-2.5-flash", true);
        assert!(stream_url.contains("streamGenerateContent"));
    }

    #[test]
    fn test_deserialize_partial_usage_metadata() {
        // Gemini sends partial usageMetadata in early streaming chunks
        // (candidatesTokenCount is absent until model starts generating)
        let json = r#"{"candidates":[{"content":{"parts":[{"text":"Hello"}],"role":"model"}}],"usageMetadata":{"promptTokenCount":100,"totalTokenCount":100},"modelVersion":"gemini-2.5-flash"}"#;
        let response: GeminiResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.candidates.len(), 1);
        let content = response.candidates[0].content.as_ref().unwrap();
        assert_eq!(content.parts[0].text.as_deref().unwrap(), "Hello");
        let usage = response.usage_metadata.unwrap();
        assert_eq!(usage.candidates_token_count, 0);
        assert_eq!(usage.thoughts_token_count, 0);
    }

    #[test]
    fn test_deserialize_usage_metadata_with_thoughts() {
        let json = r#"{"candidates":[{"content":{"parts":[{"text":"Done"}],"role":"model"}}],"usageMetadata":{"promptTokenCount":100,"candidatesTokenCount":50,"totalTokenCount":250,"thoughtsTokenCount":100}}"#;
        let response: GeminiResponse = serde_json::from_str(json).unwrap();
        let usage = response.usage_metadata.unwrap();
        assert_eq!(usage.thoughts_token_count, 100);
        assert_eq!(usage.total_token_count, 250);
    }

    #[test]
    fn test_thought_parts_filtered_in_convert_response() {
        // Thinking mode: model returns thought parts (internal reasoning) + final text.
        // convert_response must skip thought parts and only return the final text.
        let json = r#"{
            "candidates": [{
                "content": {
                    "parts": [
                        {"text": "Let me think about this carefully...", "thought": true},
                        {"text": "The answer is 42."}
                    ],
                    "role": "model"
                }
            }],
            "usageMetadata": {"promptTokenCount": 10, "candidatesTokenCount": 20, "totalTokenCount": 30, "thoughtsTokenCount": 15}
        }"#;
        let provider = GeminiProvider::new();
        let gemini_response: GeminiResponse = serde_json::from_str(json).unwrap();
        let response = provider.convert_response(gemini_response, "gemini-2.5-flash".into()).unwrap();
        assert_eq!(response.content, "The answer is 42.");
    }

    #[test]
    fn test_deserialize_no_candidates() {
        // Some events (thinking phase, content filtering) have no candidates
        let json = r#"{"usageMetadata":{"promptTokenCount":50,"totalTokenCount":50}}"#;
        let response: GeminiResponse = serde_json::from_str(json).unwrap();
        assert!(response.candidates.is_empty());
    }

    #[test]
    fn test_deserialize_candidate_without_content() {
        // Final event may have finishReason but no content
        let json = r#"{"candidates":[{"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":50,"candidatesTokenCount":100,"totalTokenCount":150}}"#;
        let response: GeminiResponse = serde_json::from_str(json).unwrap();
        assert!(response.candidates[0].content.is_none());
        assert_eq!(response.candidates[0].finish_reason.as_deref(), Some("STOP"));
    }
}
