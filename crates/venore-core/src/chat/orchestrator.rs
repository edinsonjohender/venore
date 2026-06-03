//! Chat Orchestrator
//!
//! Stateless orchestration for chat streaming. Converts frontend messages
//! to LLM requests and creates streaming connections.

use serde::{Deserialize, Serialize};

use crate::llm::gateway::{GatewayOptions, LlmGateway};
use crate::llm::types::{LlmMessage, LlmRequest, LlmStream, LlmTool, MessageRole, TokenUsage};
use crate::traits::LlmProviderType;
use crate::Result;

/// If the active provider can ground responses against the web natively
/// (Gemini today), flip the request flag so the provider knows the caller
/// wants web grounding. We deliberately do NOT strip the `web_search`
/// function tool here — the provider decides whether to strip it based on
/// whether grounding will actually be active for this specific request.
///
/// Why provider-side: Gemini 2.5 forbids mixing `google_search` with
/// `function_declarations`. When the agent has function tools (the normal
/// chat case), grounding gets dropped. If we'd already stripped
/// `web_search` here, the request ends up with neither grounding nor a
/// Tavily-backed fallback — total dead end. Keeping the function around
/// lets the provider hand back to Tavily when grounding is unavailable.
///
/// Providers without native search keep the function tool in place; the
/// executor falls back to Tavily there (still useful for Ollama).
fn apply_native_web_search(
    provider: LlmProviderType,
    tools: Option<Vec<LlmTool>>,
) -> (Option<Vec<LlmTool>>, bool) {
    match provider {
        LlmProviderType::Gemini => (tools, true),
        _ => (tools, false),
    }
}

// ============================================================================
// TYPES
// ============================================================================

/// Input message from the frontend (role as string for IPC simplicity)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageInput {
    pub role: String,
    pub content: String,
}

// ============================================================================
// SYSTEM PROMPT
// ============================================================================

pub const SYSTEM_PROMPT: &str = r#"You are Venore AI — a coding agent that operates directly on the user's codebase through tools.

You are NOT a chatbot. You are an agent. When the user asks you to do something, you DO it using your tools — you do not describe what you would do or paste code as text.

## Core rules

1. **ACT, don't describe.** When asked to create, modify, or fix code: use `write_file`, `edit_file`, `run_terminal_command`, or `run_app`. NEVER output code as markdown code blocks. NEVER paste file contents as text. Use the tools.
2. **Text budget: 4 lines max** (excluding tool calls). Use tools for actions, use text only for brief status updates (e.g. "Created `auth.py` with login endpoint") or to ask clarifying questions.
3. **Respond in the user's language.** If they write in Spanish, respond in Spanish. English → English.
4. **Be direct and technical.** Do NOT start responses with "Great", "Sure", "Certainly", "Of course". State what you will do, then do it.
5. **Don't guess — read first.** If unsure about file contents or project structure, use `read_file` or `list_files` before making changes.
6. **Act by default, ask only for technical decisions with multiple valid approaches.** For greetings, simple questions, or when the user's intent is clear — respond directly with text. Only use `ask_user` when there are genuinely different technical paths and the choice matters.
7. **One step at a time.** After each tool call, verify the result before proceeding.

## NEVER do these

- NEVER output full file contents as code blocks in chat — use `write_file`
- NEVER describe code changes in text instead of applying them — use `edit_file`
- NEVER recite or summarize documentation unless explicitly asked
- NEVER add unnecessary comments or docstrings unless the user asks
- NEVER create unnecessary files — prefer editing existing ones

## How to use context

You may receive project docs, module descriptions, and code snippets below. These are background knowledge:
- Extract only the specific facts relevant to the user's question
- Reference files naturally (e.g. "the `UserService` in `src/auth`") — don't quote docs
- If you don't have enough context, say so briefly and ask what would help"#;

// ============================================================================
// FUNCTIONS
// ============================================================================

/// Convert frontend ChatMessageInput to LlmMessage, prepending the system prompt.
pub fn build_llm_messages(messages: &[ChatMessageInput], system_prompt: &str) -> Vec<LlmMessage> {
    let mut llm_messages = Vec::with_capacity(messages.len() + 1);

    // System prompt first
    llm_messages.push(LlmMessage {
        role: MessageRole::System,
        content: system_prompt.to_string(),
        tool_call_id: None,
        tool_calls: None,
        content_parts: None,
    });

    // Convert user messages
    for msg in messages {
        let role = match msg.role.as_str() {
            "system" => MessageRole::System,
            "assistant" => MessageRole::Assistant,
            "tool" => MessageRole::Tool,
            _ => MessageRole::User,
        };
        llm_messages.push(LlmMessage {
            role,
            content: msg.content.clone(),
            tool_call_id: None,
            tool_calls: None,
            content_parts: None,
        });
    }

    llm_messages
}

/// Create a streaming LLM connection from chat messages.
///
/// Builds the LlmRequest from messages and options, then calls gateway.stream().
pub async fn create_chat_stream(
    gateway: &LlmGateway,
    messages: Vec<ChatMessageInput>,
    system_prompt: &str,
    options: GatewayOptions,
    tools: Option<Vec<LlmTool>>,
) -> Result<(LlmStream, String)> {
    create_chat_stream_with_attachments(gateway, messages, system_prompt, options, tools, None).await
}

/// Like `create_chat_stream` but accepts optional multimodal attachments
/// that are injected as `content_parts` on the last user message.
pub async fn create_chat_stream_with_attachments(
    gateway: &LlmGateway,
    messages: Vec<ChatMessageInput>,
    system_prompt: &str,
    options: GatewayOptions,
    tools: Option<Vec<LlmTool>>,
    attachments: Option<Vec<crate::llm::types::ContentPart>>,
) -> Result<(LlmStream, String)> {
    // Resolve model using the gateway's full resolution chain (DB → overrides → hardcoded)
    let (provider, model) = gateway.resolve_model(&options).await;

    let mut llm_messages = build_llm_messages(&messages, system_prompt);

    // Inject attachment content_parts into the last user message
    if let Some(parts) = attachments {
        if !parts.is_empty() {
            // Find the last user message and inject content_parts
            for msg in llm_messages.iter_mut().rev() {
                if msg.role == MessageRole::User {
                    msg.content_parts = Some(parts);
                    break;
                }
            }
        }
    }

    let (tools, web_search) = apply_native_web_search(provider, tools);

    let request = LlmRequest {
        model: model.clone(),
        messages: llm_messages,
        temperature: options.temperature,
        max_tokens: options.max_tokens,
        tools,
        json_schema: None,
        timeout_secs: Some(120),
        web_search,
    };

    let stream = gateway.stream(request, options).await?;
    Ok((stream, model))
}

/// Continue a chat stream with pre-built LlmMessages (for agentic tool loop).
///
/// Unlike `create_chat_stream`, this takes already-built LlmMessage vec
/// (including tool results) and doesn't prepend a system prompt.
pub async fn continue_chat_stream(
    gateway: &LlmGateway,
    messages: Vec<LlmMessage>,
    tools: Option<Vec<LlmTool>>,
    model: &str,
    options: GatewayOptions,
) -> Result<LlmStream> {
    // Same provider-aware web-search swap as `create_chat_stream`. We
    // re-resolve the provider so multi-turn tool loops keep the right
    // tool list (no `web_search` function tool when Gemini is active).
    let (provider, _) = gateway.resolve_model(&options).await;
    let (tools, web_search) = apply_native_web_search(provider, tools);

    let request = LlmRequest {
        model: model.to_string(),
        messages,
        temperature: options.temperature,
        max_tokens: options.max_tokens,
        tools,
        json_schema: None,
        timeout_secs: Some(120),
        web_search,
    };

    let stream = gateway.stream(request, options).await?;
    Ok(stream)
}

/// Extract token usage from an LlmStreamChunk::Done, returning defaults if None.
pub fn extract_usage(usage: &Option<TokenUsage>) -> (u32, u32, u32) {
    match usage {
        Some(u) => (u.prompt_tokens, u.completion_tokens, u.total_tokens),
        None => (0, 0, 0),
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_llm_messages() {
        let messages = vec![
            ChatMessageInput {
                role: "user".to_string(),
                content: "Hello".to_string(),
            },
            ChatMessageInput {
                role: "assistant".to_string(),
                content: "Hi there!".to_string(),
            },
        ];

        let result = build_llm_messages(&messages, SYSTEM_PROMPT);

        // System prompt should be first
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].role, MessageRole::System);
        assert!(result[0].content.contains("Venore AI"));

        // User message
        assert_eq!(result[1].role, MessageRole::User);
        assert_eq!(result[1].content, "Hello");

        // Assistant message
        assert_eq!(result[2].role, MessageRole::Assistant);
        assert_eq!(result[2].content, "Hi there!");
    }

    #[test]
    fn test_build_llm_messages_empty() {
        let messages: Vec<ChatMessageInput> = vec![];
        let result = build_llm_messages(&messages, SYSTEM_PROMPT);

        // Only system prompt
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].role, MessageRole::System);
    }

    #[test]
    fn test_build_llm_messages_custom_system_prompt() {
        let messages = vec![ChatMessageInput {
            role: "user".to_string(),
            content: "Test".to_string(),
        }];

        let custom_prompt = "You are a test assistant.";
        let result = build_llm_messages(&messages, custom_prompt);

        assert_eq!(result[0].content, "You are a test assistant.");
    }

    #[test]
    fn test_build_llm_messages_unknown_role_defaults_to_user() {
        let messages = vec![ChatMessageInput {
            role: "unknown_role".to_string(),
            content: "Test".to_string(),
        }];

        let result = build_llm_messages(&messages, SYSTEM_PROMPT);
        assert_eq!(result[1].role, MessageRole::User);
    }

    #[test]
    fn test_extract_usage_with_data() {
        let usage = Some(TokenUsage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
        });
        let (p, c, t) = extract_usage(&usage);
        assert_eq!(p, 10);
        assert_eq!(c, 20);
        assert_eq!(t, 30);
    }

    #[test]
    fn test_extract_usage_none() {
        let (p, c, t) = extract_usage(&None);
        assert_eq!(p, 0);
        assert_eq!(c, 0);
        assert_eq!(t, 0);
    }

}
