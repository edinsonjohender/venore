//! Research Worker — agentic loop for investigating assigned hexagons
//!
//! Adapted from `venore-desktop/src/commands/chat/sub_agent.rs`.
//! Runs independently in a `tokio::spawn` task with its own LLM stream,
//! tool execution, and cancellation support.

use std::sync::Arc;
use std::time::Instant;

use futures::StreamExt;
use tokio::sync::watch;
use tokio::time::{timeout, Duration};

use crate::chat::{create_chat_stream, continue_chat_stream, ChatMessageInput};
use crate::error::VenoreError;
use crate::llm::prelude::*;
use crate::llm::types::{LlmMessage, LlmTool, LlmToolCall};
use crate::tools::{self, names as N, ToolExecutionContext};

use super::types::{ResearchEvent, WorkerAssignment, WorkerResult};

/// Max time to wait for a single stream chunk before considering the connection stalled.
const STREAM_CHUNK_TIMEOUT: Duration = Duration::from_secs(120);

/// Knowledge tools that mutate hexagons/evidence — trigger UI refresh callback
const KNOWLEDGE_MUTATION_TOOLS: &[&str] = &[
    N::PLAN_HEXAGONS,
    N::UPDATE_HEXAGON,
    N::ADD_EVIDENCE,
    N::MARK_DEAD_END,
];

/// Read-only tools that can be executed in parallel
const PARALLELIZABLE: &[&str] = N::PARALLELIZABLE_TOOLS;

/// Run a research worker's agentic loop for the given assignment.
///
/// The worker investigates the assigned hexagons using its tool set,
/// writing results directly to SQLite via the knowledge tools.
/// After each knowledge mutation, it calls `on_knowledge_changed` to
/// notify the frontend.
pub async fn run_research_worker(
    assignment: WorkerAssignment,
    llm_gateway: Arc<crate::llm::LlmGateway>,
    tool_ctx: ToolExecutionContext,
    options: GatewayOptions,
    llm_tools: Vec<LlmTool>,
    cancel_rx: watch::Receiver<bool>,
    emit: Arc<dyn Fn(ResearchEvent) + Send + Sync>,
    on_knowledge_changed: Arc<dyn Fn(&str) + Send + Sync>,
    feature_id: String,
    run_id: String,
) -> WorkerResult {
    let start = Instant::now();
    let worker_id = assignment.worker_id.clone();
    let mut total_tool_calls = 0u32;
    let mut total_tokens = 0u32;
    let mut hexagons_updated: Vec<String> = Vec::new();
    let mut evidence_added = 0u32;

    let result = run_loop(
        &assignment,
        &llm_gateway,
        &tool_ctx,
        &options,
        &llm_tools,
        &cancel_rx,
        &on_knowledge_changed,
        &feature_id,
        &mut total_tool_calls,
        &mut total_tokens,
        &mut hexagons_updated,
        &mut evidence_added,
    )
    .await;

    let duration_ms = start.elapsed().as_millis() as u64;
    let error = result.err().map(|e| e.to_string());

    if let Some(ref err) = error {
        tracing::warn!(worker_id = %worker_id, error = %err, "Research worker failed");
        emit(ResearchEvent::WorkerFailed {
            run_id: run_id.clone(), // filled by engine
            worker_id: worker_id.clone(),
            error: err.clone(),
        });
    } else {
        tracing::info!(
            worker_id = %worker_id,
            tool_calls = total_tool_calls,
            evidence = evidence_added,
            duration_ms,
            "Research worker completed"
        );
        emit(ResearchEvent::WorkerCompleted {
            run_id: run_id.clone(),
            worker_id: worker_id.clone(),
            duration_ms,
        });
    }

    WorkerResult {
        worker_id,
        hexagons_updated,
        evidence_added,
        tool_calls: total_tool_calls,
        tokens_used: total_tokens,
        duration_ms,
        error,
    }
}

/// Inner loop — separated for clean error propagation via `?`
async fn run_loop(
    assignment: &WorkerAssignment,
    llm_gateway: &Arc<crate::llm::LlmGateway>,
    tool_ctx: &ToolExecutionContext,
    options: &GatewayOptions,
    llm_tools: &[LlmTool],
    cancel_rx: &watch::Receiver<bool>,
    on_knowledge_changed: &Arc<dyn Fn(&str) + Send + Sync>,
    feature_id: &str,
    total_tool_calls: &mut u32,
    total_tokens: &mut u32,
    hexagons_updated: &mut Vec<String>,
    evidence_added: &mut u32,
) -> Result<(), VenoreError> {
    let max_iterations = assignment.max_iterations;
    let max_tool_calls = assignment.max_tool_calls;

    // Build the worker's system prompt
    let hex_context = assignment
        .hexagon_ids
        .iter()
        .map(|id| format!("- {id}"))
        .collect::<Vec<_>>()
        .join("\n");

    let system_prompt = format!(
        "You are a research worker agent. Your assignment:\n\n{}\n\n\
         Hexagons assigned to you:\n{}\n\n\
         Instructions:\n\
         1. Use web_search and web_fetch to find information about each assigned hexagon\n\
         2. Use update_hexagon to record your findings (update percentage, confidence, notes)\n\
         3. Use add_evidence to store specific findings with source URLs\n\
         4. Use mark_dead_end if a research path leads nowhere\n\
         5. Be thorough but focused — investigate your assigned hexagons only\n\
         6. After each finding, update the hexagon's percentage to reflect progress\n\n\
         Work autonomously. Do not ask the user for input.",
        assignment.instructions, hex_context
    );

    let user_message = "Begin investigating the assigned research points. \
         Start with the most important or highest-priority hexagon.".to_string();

    // Create initial stream
    let (initial_stream, model) = create_chat_stream(
        llm_gateway,
        vec![ChatMessageInput {
            role: "user".to_string(),
            content: user_message.clone(),
        }],
        &system_prompt,
        options.clone(),
        Some(llm_tools.to_vec()),
    )
    .await?;

    let mut current_stream = initial_stream;
    let mut llm_messages: Vec<LlmMessage> = vec![
        LlmMessage {
            role: MessageRole::System,
            content: system_prompt,
            tool_call_id: None,
            tool_calls: None,
            content_parts: None,
        },
        LlmMessage {
            role: MessageRole::User,
            content: user_message,
            tool_call_id: None,
            tool_calls: None,
            content_parts: None,
        },
    ];

    for _iteration in 0..max_iterations {
        // Check cancellation
        if *cancel_rx.borrow() {
            tracing::info!("Research worker cancelled");
            return Ok(());
        }

        let mut iteration_text = String::new();
        let mut iteration_tool_calls: Vec<LlmToolCall> = Vec::new();

        // Consume stream
        loop {
            let chunk_result = match timeout(STREAM_CHUNK_TIMEOUT, current_stream.next()).await {
                Ok(Some(chunk)) => chunk,
                Ok(None) => break,
                Err(_) => {
                    tracing::error!("[research-worker] Stream chunk timeout");
                    return Err(VenoreError::Timeout(STREAM_CHUNK_TIMEOUT.as_millis() as u64));
                }
            };
            match chunk_result {
                Ok(chunk) => match chunk {
                    LlmStreamChunk::Text { content } => {
                        iteration_text.push_str(&content);
                    }
                    LlmStreamChunk::ToolCall { call } => {
                        iteration_tool_calls.push(call);
                    }
                    LlmStreamChunk::Done { usage, .. } => {
                        if let Some(u) = usage {
                            *total_tokens += u.total_tokens;
                        }
                        break;
                    }
                    LlmStreamChunk::Error { error } => {
                        return Err(VenoreError::LlmStreamError(error));
                    }
                },
                Err(e) => return Err(e),
            }
        }

        // No tool calls → LLM is done
        if iteration_tool_calls.is_empty() {
            break;
        }

        *total_tool_calls += iteration_tool_calls.len() as u32;
        if *total_tool_calls > max_tool_calls {
            tracing::info!("Research worker hit tool call limit ({})", max_tool_calls);
            break;
        }

        // Push assistant message
        llm_messages.push(LlmMessage {
            role: MessageRole::Assistant,
            content: iteration_text,
            tool_call_id: None,
            tool_calls: Some(iteration_tool_calls.clone()),
            content_parts: None,
        });

        // Partition: parallel (read-only) vs sequential (mutations)
        let mut parallel_calls: Vec<&LlmToolCall> = Vec::new();
        let mut sequential_calls: Vec<&LlmToolCall> = Vec::new();

        for tc in &iteration_tool_calls {
            if PARALLELIZABLE.contains(&tc.name.as_str()) {
                parallel_calls.push(tc);
            } else {
                sequential_calls.push(tc);
            }
        }

        // Execute parallel batch (read-only tools)
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
                    content: result.output,
                    tool_call_id: Some(tc.id.clone()),
                    tool_calls: None,
                    content_parts: None,
                });
            }
        }

        // Execute sequential calls (knowledge mutations + others)
        for tc in &sequential_calls {
            let result = execute_tool_safe(&tc.name, &tc.arguments, tool_ctx).await;

            // Track knowledge mutations
            if KNOWLEDGE_MUTATION_TOOLS.contains(&tc.name.as_str()) {
                on_knowledge_changed(feature_id);

                // Track stats
                if tc.name == N::ADD_EVIDENCE {
                    *evidence_added += 1;
                }
                if tc.name == N::UPDATE_HEXAGON || tc.name == N::MARK_DEAD_END {
                    if let Some(hex_id) = tc.arguments.get("hexagon_id").and_then(|v| v.as_str()) {
                        if !hexagons_updated.contains(&hex_id.to_string()) {
                            hexagons_updated.push(hex_id.to_string());
                        }
                    }
                }
            }

            llm_messages.push(LlmMessage {
                role: MessageRole::Tool,
                content: result.output,
                tool_call_id: Some(tc.id.clone()),
                tool_calls: None,
                content_parts: None,
            });
        }

        // Check cancellation before continuing
        if *cancel_rx.borrow() {
            tracing::info!("Research worker cancelled between iterations");
            return Ok(());
        }

        // Continue the LLM stream
        current_stream = continue_chat_stream(
            llm_gateway,
            llm_messages.clone(),
            Some(llm_tools.to_vec()),
            &model,
            options.clone(),
        )
        .await?;
    }

    Ok(())
}

/// Execute a tool, converting errors to a result (fail-soft)
async fn execute_tool_safe(
    name: &str,
    arguments: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> tools::ToolExecutionResult {
    match tools::execute_tool(name, arguments, ctx).await {
        Ok(r) => r,
        Err(e) => tools::ToolExecutionResult {
            success: false,
            output: e.to_string(),
            baseline: None,
        },
    }
}
