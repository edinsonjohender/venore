//! Mesh Agent Loop — simplified agentic loop for mesh sub-agents.
//!
//! Self-contained in venore-core (no Tauri/desktop dependencies).
//! Processes inbound mesh questions using LLM reasoning + read-only tools.
//! Extracted from the desktop sub_agent pattern but stripped of:
//! - Tauri event emission
//! - SQLite persistence
//! - LSP diagnostics
//! - Terminal post-processing
//! - Permission checks

use futures::StreamExt;
use tokio::time::{timeout, Duration};

use crate::chat;
use crate::error::{Result, VenoreError};
use crate::llm::gateway::GatewayOptions;
use crate::llm::types::{LlmMessage, LlmStreamChunk, LlmToolCall, LlmTool, MessageRole};
use crate::llm::LlmGateway;
use crate::tools::{self, ToolExecutionContext};
use crate::tools::names::PARALLELIZABLE_TOOLS;

/// Max time to wait for a single stream chunk (LLM response fragment).
const STREAM_CHUNK_TIMEOUT: Duration = Duration::from_secs(60);

/// Max iterations (LLM turns) before stopping. Sized for parity with the main
/// chat agent's loop (`agentic_loop.rs`, 30 turns): a remote sub-agent needs
/// the same room to search → read → reason on a non-trivial question. The
/// caller's wall-clock budget (`execute_ask_project`) is the practical ceiling.
const MAX_ITERATIONS: u32 = 25;

/// Max total tool calls across all iterations. Headroom proportional to the
/// raised iteration budget; the main agent has no hard cap, this keeps a safety
/// bound on an auto-approved remote agent.
const MAX_TOOL_CALLS: u32 = 50;

/// Max output size returned to the requesting agent.
const MAX_OUTPUT_CHARS: usize = 10_000;

/// Max previous messages to carry across multi-turn conversations.
/// When exceeded, the oldest messages (after system prompt) are dropped.
const MAX_PREVIOUS_MESSAGES: usize = 40;

/// Run a simplified agentic loop for a mesh sub-agent.
///
/// The loop:
/// 1. Sends the question to the LLM with a system prompt and read-only tools
/// 2. Consumes the stream, collecting text and tool calls
/// 3. Executes tool calls (parallel for read-only, sequential otherwise)
/// 4. Feeds results back to the LLM for the next iteration
/// 5. Repeats until the LLM responds with no tool calls, or limits are hit
///
/// If `previous_messages` is non-empty, they are prepended to provide
/// conversation context from prior turns (Phase 4a multi-turn support).
///
/// Returns `(accumulated_text, final_llm_messages)` — the caller can store
/// the messages for future turns in the same conversation.
pub async fn run_mesh_agent_loop(
    gateway: &LlmGateway,
    system_prompt: &str,
    question: &str,
    tools_def: Vec<LlmTool>,
    options: GatewayOptions,
    tool_ctx: ToolExecutionContext,
    previous_messages: Vec<LlmMessage>,
) -> Result<(String, Vec<LlmMessage>)> {
    let mut accumulated_content = String::new();
    let mut total_tool_calls = 0u32;

    let has_history = !previous_messages.is_empty();

    // Build LLM message history for continue_chat_stream calls
    let mut llm_messages = if has_history {
        // Multi-turn: start from previous history, append new user question
        let mut msgs = previous_messages;
        // Truncate if history is too large (keep system prompt + tail)
        if msgs.len() > MAX_PREVIOUS_MESSAGES {
            let system = msgs.remove(0);
            let keep = MAX_PREVIOUS_MESSAGES - 1;
            let start = msgs.len().saturating_sub(keep);
            msgs = std::iter::once(system).chain(msgs.into_iter().skip(start)).collect();
        }
        msgs.push(LlmMessage {
            role: MessageRole::User,
            content: question.to_string(),
            tool_call_id: None,
            tool_calls: None,
            content_parts: None,
        });
        msgs
    } else {
        vec![
            LlmMessage {
                role: MessageRole::System,
                content: system_prompt.to_string(),
                tool_call_id: None,
                tool_calls: None,
                content_parts: None,
            },
            LlmMessage {
                role: MessageRole::User,
                content: question.to_string(),
                tool_call_id: None,
                tool_calls: None,
                content_parts: None,
            },
        ]
    };

    // Initial stream
    let (mut current_stream, model) = if has_history {
        // Continue from existing history — use continue_chat_stream
        let (_, model_id) = gateway.resolve_model(&options).await;
        let stream = chat::continue_chat_stream(
            gateway,
            llm_messages.clone(),
            Some(tools_def.clone()),
            &model_id,
            options.clone(),
        )
        .await?;
        (stream, model_id)
    } else {
        // Fresh conversation — create_chat_stream resolves provider/model from config
        chat::create_chat_stream(
            gateway,
            vec![chat::ChatMessageInput {
                role: "user".to_string(),
                content: question.to_string(),
            }],
            system_prompt,
            options.clone(),
            Some(tools_def.clone()),
        )
        .await?
    };

    for iteration in 0..MAX_ITERATIONS {
        let mut iteration_text = String::new();
        let mut iteration_tool_calls: Vec<LlmToolCall> = Vec::new();

        // Consume the stream
        loop {
            let chunk_result = match timeout(STREAM_CHUNK_TIMEOUT, current_stream.next()).await {
                Ok(Some(chunk)) => chunk,
                Ok(None) => break,
                Err(_) => {
                    tracing::error!(
                        iteration,
                        "[mesh-agent] Stream chunk timeout after {}s",
                        STREAM_CHUNK_TIMEOUT.as_secs()
                    );
                    return Err(VenoreError::Timeout(STREAM_CHUNK_TIMEOUT.as_millis() as u64));
                }
            };

            match chunk_result {
                Ok(LlmStreamChunk::Text { content }) => {
                    iteration_text.push_str(&content);
                }
                Ok(LlmStreamChunk::ToolCall { call }) => {
                    iteration_tool_calls.push(call);
                }
                Ok(LlmStreamChunk::Done { .. }) => break,
                Ok(LlmStreamChunk::Error { error }) => {
                    return Err(VenoreError::LlmStreamError(error));
                }
                Err(e) => return Err(e),
            }
        }

        accumulated_content.push_str(&iteration_text);

        // No tool calls → LLM is done reasoning
        if iteration_tool_calls.is_empty() {
            break;
        }

        total_tool_calls += iteration_tool_calls.len() as u32;
        if total_tool_calls > MAX_TOOL_CALLS {
            tracing::warn!(
                total_tool_calls,
                "[mesh-agent] Tool call limit reached, stopping"
            );
            break;
        }

        // Record assistant message with tool calls
        llm_messages.push(LlmMessage {
            role: MessageRole::Assistant,
            content: iteration_text,
            tool_call_id: None,
            tool_calls: Some(iteration_tool_calls.clone()),
            content_parts: None,
        });

        // Partition: parallel (read-only) vs sequential
        let mut parallel_calls: Vec<&LlmToolCall> = Vec::new();
        let mut sequential_calls: Vec<&LlmToolCall> = Vec::new();

        for tc in &iteration_tool_calls {
            if PARALLELIZABLE_TOOLS.contains(&tc.name.as_str()) {
                parallel_calls.push(tc);
            } else {
                sequential_calls.push(tc);
            }
        }

        // Execute parallel batch
        if !parallel_calls.is_empty() {
            let futs: Vec<_> = parallel_calls
                .iter()
                .map(|tc| {
                    let name = tc.name.clone();
                    let args = tc.arguments.clone();
                    let ctx = tool_ctx.clone();
                    async move { execute_tool_safe(&name, &args, &ctx).await }
                })
                .collect();

            let results = futures::future::join_all(futs).await;

            for (tc, result) in parallel_calls.iter().zip(results) {
                llm_messages.push(LlmMessage {
                    role: MessageRole::Tool,
                    content: truncate_output(&result.output),
                    tool_call_id: Some(tc.id.clone()),
                    tool_calls: None,
                    content_parts: None,
                });
            }
        }

        // Execute sequential calls
        for tc in &sequential_calls {
            let result = execute_tool_safe(&tc.name, &tc.arguments, &tool_ctx).await;
            llm_messages.push(LlmMessage {
                role: MessageRole::Tool,
                content: truncate_output(&result.output),
                tool_call_id: Some(tc.id.clone()),
                tool_calls: None,
                content_parts: None,
            });
        }

        // Continue the loop with updated messages
        current_stream = chat::continue_chat_stream(
            gateway,
            llm_messages.clone(),
            Some(tools_def.clone()),
            &model,
            options.clone(),
        )
        .await?;
    }

    // Record the final assistant text in the message history
    if !accumulated_content.is_empty() {
        llm_messages.push(LlmMessage {
            role: MessageRole::Assistant,
            content: accumulated_content.clone(),
            tool_call_id: None,
            tool_calls: None,
            content_parts: None,
        });
    }

    // Truncate final output
    if accumulated_content.len() > MAX_OUTPUT_CHARS {
        accumulated_content.truncate(
            accumulated_content
                .floor_char_boundary(MAX_OUTPUT_CHARS),
        );
        accumulated_content.push_str("\n\n... (mesh agent output truncated)");
    }

    Ok((accumulated_content, llm_messages))
}

/// Execute a tool, catching errors and returning a result (never panics).
async fn execute_tool_safe(
    name: &str,
    arguments: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> tools::ToolExecutionResult {
    match tools::execute_tool(name, arguments, ctx).await {
        Ok(result) => result,
        Err(e) => tools::ToolExecutionResult {
            success: false,
            output: format!("Tool error: {}", e),
            baseline: None,
        },
    }
}

/// Truncate tool output to prevent bloating the LLM context.
fn truncate_output(s: &str) -> String {
    crate::utils::truncate(s, 10_000, "...\n(output truncated)")
}
