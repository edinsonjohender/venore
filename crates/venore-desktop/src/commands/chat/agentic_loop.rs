//! Agentic loop — iterative stream consumption with tool dispatch, compaction, and persistence.

use std::sync::Arc;

use tauri::AppHandle;
use tauri::Emitter;
use tokio::time::{timeout, Duration};

use venore_core::agents::AgentRepository;
use venore_core::chat::{ChatMessageInput, ChatMessageRecord, ChatRepository};
use venore_core::error::VenoreError;
use venore_core::llm::prelude::*;
use venore_core::llm::types::{LlmMessage, LlmTool, LlmToolCall};
use venore_core::rag::RagRepository;
use venore_core::tools;
use venore_core::tools::names as N;

use super::dto::*;
use super::helpers::{check_permission_action, is_parallelizable, resolve_or_spawn_terminal};
use super::state::{
    ACTIVE_STREAMS, ACTIVE_SUB_AGENTS, PENDING_APPROVALS, PENDING_PLAN_APPROVALS,
    PENDING_USER_RESPONSES, SESSION_STREAMS, STREAM_TOOL_CALL_IDS, TASK_STORES,
};
use super::tool_dispatch;

/// Max time to wait for a single stream chunk before considering the connection stalled.
const STREAM_CHUNK_TIMEOUT: Duration = Duration::from_secs(120);

/// RAII guard that ensures `chat-stream-done` is always emitted when the agentic loop exits.
/// If `run_main_loop` completes normally, it sets `emitted = true` before the explicit emit.
/// If the function returns early (bug) or the task is unwinding, the Drop impl fires.
struct StreamDoneGuard {
    app: AppHandle,
    stream_id: String,
    session_id: Option<String>,
    emitted: bool,
}

impl Drop for StreamDoneGuard {
    fn drop(&mut self) {
        if !self.emitted {
            tracing::warn!(
                "[stream:{}] Drop guard emitting chat-stream-done (abnormal exit)",
                self.stream_id
            );
            let _ = self.app.emit(
                "chat-stream-done",
                ChatStreamDonePayload {
                    stream_id: self.stream_id.clone(),
                    session_id: self.session_id.clone(),
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    total_tokens: 0,
                    provider: String::new(),
                    model: String::new(),
                },
            );
        }
    }
}

/// Groups the cloned variables needed throughout the agentic loop.
pub(super) struct AgenticLoopCtx {
    pub app: AppHandle,
    pub stream_id: String,
    pub dev_session_id: Option<String>,
    pub dev_session_base_branch: Option<String>,
    pub session_id: Option<String>,
    pub tool_project_path: Option<String>,
    pub dev_session_name: Option<String>,
    pub llm_gateway: Arc<venore_core::llm::LlmGateway>,
    pub options: GatewayOptions,
    pub llm_tools: Option<Vec<LlmTool>>,
    pub agent_repo: Option<Arc<AgentRepository>>,
    pub rag_repo: Option<Arc<RagRepository>>,
    pub logbook_repo: Option<Arc<venore_core::rag::LogbookRepository>>,
    pub project_id: Option<String>,
    pub tavily_api_key: Option<String>,
    /// Embedding provider for hybrid logbook/code search (None = FTS5 only).
    pub embedding_provider: Option<Arc<dyn venore_core::traits::EmbeddingProvider>>,
    /// API key for the embedding provider.
    pub embedding_api_key: Option<String>,
    pub chat_repo: Option<Arc<ChatRepository>>,
    pub provider_name: String,
    pub model: String,
    pub provider_type: venore_core::traits::LlmProviderType,
    pub messages: Vec<ChatMessageInput>,
    /// Serialized attachment metadata JSON for the user message (name + mimeType only)
    pub attachments_json: Option<String>,
    /// Knowledge feature ID for research sessions
    pub knowledge_feature_id: Option<String>,
    /// Knowledge repository for research tools
    pub knowledge_repo: Option<Arc<venore_core::knowledge::KnowledgeRepository>>,
}

/// Run the main agentic loop: consume stream, dispatch tools, compact, continue.
pub(super) async fn run_main_loop(
    ctx: AgenticLoopCtx,
    initial_stream: venore_core::llm::LlmStream,
    system_prompt: &str,
) {
    use futures::StreamExt;

    let mut llm_messages = venore_core::chat::build_llm_messages(&ctx.messages, system_prompt);

    let mut done_guard = StreamDoneGuard {
        app: ctx.app.clone(),
        stream_id: ctx.stream_id.clone(),
        session_id: ctx.session_id.clone(),
        emitted: false,
    };

    // Persist user message immediately so pop-out windows can load it from DB
    persist_user_message(&ctx).await;

    let mut accumulated_content = String::new();
    let final_provider = ctx.provider_name.clone();
    let final_model = ctx.model.clone();
    let mut final_usage = (0u32, 0u32, 0u32);
    let max_tool_iterations = 30;
    let max_total_tool_calls = 80;
    let mut plan_mode = false;
    let mut total_tool_calls = 0u32;
    let mut current_stream = initial_stream;
    let mut had_error = false;
    let mut repetition_tracker = venore_core::chat::guardrails::RepetitionTracker::new();
    let mut checkpoint_injected = false;

    // Extract original user message for focus chain reminders
    let original_user_message = ctx.messages.iter().rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.clone())
        .unwrap_or_default();

    for _iteration in 0..max_tool_iterations {
        let mut iteration_text = String::new();
        let mut iteration_tool_calls: Vec<LlmToolCall> = Vec::new();

        // Warn 2 iterations before the limit so the model can wrap up
        if _iteration == max_tool_iterations - 2 {
            llm_messages.push(LlmMessage {
                role: MessageRole::User,
                content: venore_core::chat::guardrails::STEP_LIMIT_WARNING.to_string(),
                tool_call_id: None,
                tool_calls: None,
                content_parts: None,
            });
        }

        // Focus chain: remind the model of the original task every N iterations
        if _iteration > 0
            && _iteration % venore_core::chat::guardrails::FOCUS_CHAIN_INTERVAL == 0
            && !original_user_message.is_empty()
        {
            tracing::debug!(iteration = _iteration, "Injecting focus chain reminder");
            llm_messages.push(LlmMessage {
                role: MessageRole::User,
                content: venore_core::chat::guardrails::build_focus_reminder(&original_user_message),
                tool_call_id: None,
                tool_calls: None,
                content_parts: None,
            });
        }

        // Consume this iteration's stream with per-chunk timeout to detect stalled connections
        loop {
            let chunk_result = match timeout(STREAM_CHUNK_TIMEOUT, current_stream.next()).await {
                Ok(Some(chunk)) => chunk,
                Ok(None) => break, // Stream ended normally
                Err(_) => {
                    // No chunk received in 120s — connection is stalled
                    tracing::error!(
                        "[stream:{}] Stream chunk timeout after {}s — connection stalled",
                        ctx.stream_id, STREAM_CHUNK_TIMEOUT.as_secs()
                    );
                    let _ = ctx.app.emit(
                        "chat-stream-error",
                        ChatStreamErrorPayload {
                            stream_id: ctx.stream_id.clone(),
                            session_id: ctx.session_id.clone(),
                            message: "Connection timed out — no response received from AI provider. Please try again.".into(),
                            code: "STREAM_CHUNK_TIMEOUT".into(),
                        },
                    );
                    had_error = true;
                    break;
                }
            };
            match chunk_result {
                Ok(chunk) => match chunk {
                    LlmStreamChunk::Text { content } => {
                        if content.is_empty() {
                            continue;
                        }
                        iteration_text.push_str(&content);
                        let _ = ctx.app.emit(
                            "chat-stream-delta",
                            ChatStreamDeltaPayload {
                                stream_id: ctx.stream_id.clone(),
                                session_id: ctx.session_id.clone(),
                                content,
                            },
                        );
                    }
                    LlmStreamChunk::ToolCall { call } => {
                        iteration_tool_calls.push(call);
                    }
                    LlmStreamChunk::Done { usage, .. } => {
                        let (p, c, t) =
                            venore_core::chat::orchestrator::extract_usage(&usage);
                        final_usage = (final_usage.0 + p, final_usage.1 + c, final_usage.2 + t);
                    }
                    LlmStreamChunk::Error { error } => {
                        let _ = ctx.app.emit(
                            "chat-stream-error",
                            ChatStreamErrorPayload {
                                stream_id: ctx.stream_id.clone(),
                                session_id: ctx.session_id.clone(),
                                message: error,
                                code: "LLM_STREAM_ERROR".to_string(),
                            },
                        );
                        had_error = true;
                        break;
                    }
                },
                Err(ref e) => {
                    let ve: &VenoreError = e;
                    let _ = ctx.app.emit(
                        "chat-stream-error",
                        ChatStreamErrorPayload {
                            stream_id: ctx.stream_id.clone(),
                            session_id: ctx.session_id.clone(),
                            message: ve.to_string(),
                            code: ve.code().to_string(),
                        },
                    );
                    // Emit toast notification for config-related errors
                    if matches!(ve, VenoreError::LlmModelNotAvailable { .. } | VenoreError::LlmNoApiKey(_)) {
                        crate::notifications::emit_error(
                            &ctx.app,
                            "AI Configuration Error",
                            ve.to_string(),
                            Some(ve.code()),
                        );
                    }
                    had_error = true;
                    break;
                }
            }
        }

        // Strip any pasted tool-call syntax (Gemini quirk) before persist /
        // re-injection. The model already invoked the tools through the
        // structured channel; the literal echo as text is just noise.
        let iteration_text =
            venore_core::chat::guardrails::strip_tool_call_syntax(&iteration_text);

        accumulated_content.push_str(&iteration_text);

        if had_error {
            break;
        }

        // No tool calls? Check guardrails, then done.
        if iteration_tool_calls.is_empty() {
            // Guard: if model narrated actions without calling tools, inject correction and retry.
            if _iteration <= 1
                && venore_core::chat::guardrails::detect_narrated_actions(&iteration_text)
            {
                tracing::warn!(
                    "[stream:{}] Narrated-action guard triggered — described actions without tool calls, retrying",
                    ctx.stream_id
                );

                llm_messages.push(LlmMessage {
                    role: MessageRole::Assistant,
                    content: iteration_text.clone(),
                    tool_call_id: None,
                    tool_calls: None,
                    content_parts: None,
                });
                llm_messages.push(LlmMessage {
                    role: MessageRole::User,
                    content: venore_core::chat::guardrails::CORRECTION_MESSAGE.to_string(),
                    tool_call_id: None,
                    tool_calls: None,
                    content_parts: None,
                });

                match venore_core::chat::continue_chat_stream(
                    &ctx.llm_gateway,
                    llm_messages.clone(),
                    ctx.llm_tools.clone(),
                    &ctx.model,
                    ctx.options.clone(),
                ).await {
                    Ok(retry_stream) => {
                        current_stream = retry_stream;
                        continue;
                    }
                    Err(e) => {
                        tracing::error!("[stream:{}] Narrated-action retry failed: {}", ctx.stream_id, e);
                    }
                }
            }

            // Guard: if model tries to surrender/give up, inject correction and force retry.
            if venore_core::chat::guardrails::detect_surrender(&iteration_text) {
                tracing::warn!(
                    "[stream:{}] Surrender guard triggered — model tried to give up, forcing retry",
                    ctx.stream_id
                );

                llm_messages.push(LlmMessage {
                    role: MessageRole::Assistant,
                    content: iteration_text,
                    tool_call_id: None,
                    tool_calls: None,
                    content_parts: None,
                });
                llm_messages.push(LlmMessage {
                    role: MessageRole::User,
                    content: venore_core::chat::guardrails::SURRENDER_CORRECTION.to_string(),
                    tool_call_id: None,
                    tool_calls: None,
                    content_parts: None,
                });

                match venore_core::chat::continue_chat_stream(
                    &ctx.llm_gateway,
                    llm_messages.clone(),
                    ctx.llm_tools.clone(),
                    &ctx.model,
                    ctx.options.clone(),
                ).await {
                    Ok(retry_stream) => {
                        current_stream = retry_stream;
                        continue;
                    }
                    Err(e) => {
                        tracing::error!("[stream:{}] Surrender retry failed: {}", ctx.stream_id, e);
                    }
                }
            }

            break;
        }

        // Cap total tool calls to prevent runaway loops
        total_tool_calls += iteration_tool_calls.len() as u32;
        if total_tool_calls > max_total_tool_calls {
            tracing::warn!(
                total = total_tool_calls,
                max = max_total_tool_calls,
                "Tool call limit reached, stopping agentic loop"
            );
            break;
        }

        // Checkpoint: flag for injection after tool dispatch
        let should_inject_checkpoint = !checkpoint_injected
            && total_tool_calls >= venore_core::chat::guardrails::CHECKPOINT_TOOL_CALLS;

        // Resolve terminal before executing any tool calls
        let active_terminal_id = resolve_or_spawn_terminal(
            &ctx.app,
            ctx.tool_project_path.as_deref(),
            ctx.dev_session_id.as_deref(),
            ctx.dev_session_name.as_deref(),
        ).ok();

        let tool_ctx = tools::ToolExecutionContext {
            terminal_id: active_terminal_id.clone(),
            project_path: ctx.tool_project_path.clone(),
            rag_repository: ctx.rag_repo.clone(),
            logbook_repository: ctx.logbook_repo.clone(),
            project_id: ctx.project_id.clone(),
            embedding_provider: ctx.embedding_provider.clone(),
            embedding_api_key: ctx.embedding_api_key.clone(),
            web_search_api_key: ctx.tavily_api_key.clone(),
            llm_gateway: Some(Arc::clone(&ctx.llm_gateway)),
            mesh_follow_up: None,
            knowledge_repo: ctx.knowledge_repo.clone(),
            knowledge_feature_id: ctx.knowledge_feature_id.clone(),
            model: Some(ctx.model.clone()),
            session_id: ctx.session_id.clone(),
            allowed_tools: ctx
                .llm_tools
                .as_ref()
                .map(|tools| tools.iter().map(|t| t.name.clone()).collect()),
        };

        llm_messages.push(LlmMessage {
            role: MessageRole::Assistant,
            content: iteration_text.clone(),
            tool_call_id: None,
            tool_calls: Some(iteration_tool_calls.clone()),
            content_parts: None,
        });

        // ─── Partition: parallel vs spawn vs sequential ─────────
        let mut parallel_calls: Vec<&LlmToolCall> = Vec::new();
        let mut spawn_calls: Vec<&LlmToolCall> = Vec::new();
        let mut sequential_calls: Vec<&LlmToolCall> = Vec::new();

        for tc in &iteration_tool_calls {
            if tc.name == N::SPAWN_AGENT {
                spawn_calls.push(tc);
            } else if is_parallelizable(&tc.name)
                && check_permission_action(
                    &tc.name,
                    &tc.arguments,
                    &ctx.stream_id,
                    ctx.dev_session_id.as_deref(),
                    ctx.session_id.as_deref(),
                ) == venore_core::permissions::PermissionAction::Allow
            {
                parallel_calls.push(tc);
            } else {
                sequential_calls.push(tc);
            }
        }

        // ─── Execute parallel batch ─────────────────────────────
        tool_dispatch::execute_parallel_batch(
            &parallel_calls,
            &tool_ctx,
            &ctx,
            &mut llm_messages,
        ).await;

        // ─── Execute spawn_agent calls concurrently ─────────────
        tool_dispatch::execute_spawn_agents(
            &spawn_calls,
            &tool_ctx,
            &ctx,
            &mut llm_messages,
            active_terminal_id.as_deref(),
        ).await;

        // ─── Execute sequential calls ───────────────────────────
        tool_dispatch::execute_sequential_tools(
            &sequential_calls,
            &tool_ctx,
            &ctx,
            &mut llm_messages,
            &mut plan_mode,
            active_terminal_id.as_deref(),
        ).await;

        // ─── Repetition detection ───────────────────────────────
        for tc in &iteration_tool_calls {
            if repetition_tracker.record_and_check(&tc.name, &tc.arguments) {
                tracing::warn!(
                    tool = %tc.name,
                    "[stream:{}] Repetition guard triggered — same tool+args repeated 3+ times",
                    ctx.stream_id
                );
                llm_messages.push(LlmMessage {
                    role: MessageRole::User,
                    content: venore_core::chat::guardrails::REPETITION_CORRECTION.to_string(),
                    tool_call_id: None,
                    tool_calls: None,
                    content_parts: None,
                });
                break; // One correction per iteration is enough
            }
        }

        // ─── Checkpoint injection ───────────────────────────────
        if should_inject_checkpoint {
            checkpoint_injected = true;
            tracing::info!(total_tool_calls, "Injecting checkpoint message at 50 tool calls");
            llm_messages.push(LlmMessage {
                role: MessageRole::User,
                content: venore_core::chat::guardrails::CHECKPOINT_MESSAGE.to_string(),
                tool_call_id: None,
                tool_calls: None,
                content_parts: None,
            });
        }

        // Check for context overflow and compact if needed
        match venore_core::chat::compaction::maybe_compact(
            &ctx.llm_gateway,
            &mut llm_messages,
            ctx.provider_type,
            &final_model,
            ctx.options.clone(),
        ).await {
            Ok(ref result) => match result {
                venore_core::chat::compaction::CompactionResult::Pruned { tokens_saved } => {
                    tracing::info!(tokens_saved, "Pruned old tool outputs");
                    let _ = ctx.app.emit("chat-compacted", ChatCompactedPayload {
                        stream_id: ctx.stream_id.clone(),
                        session_id: ctx.session_id.clone(),
                        action: "pruned".to_string(),
                        tokens_saved: *tokens_saved,
                    });
                }
                venore_core::chat::compaction::CompactionResult::Compacted { original_tokens, summary_tokens } => {
                    tracing::info!(original_tokens, summary_tokens, "Compacted conversation");
                    let _ = ctx.app.emit("chat-compacted", ChatCompactedPayload {
                        stream_id: ctx.stream_id.clone(),
                        session_id: ctx.session_id.clone(),
                        action: "compacted".to_string(),
                        tokens_saved: original_tokens - summary_tokens,
                    });
                }
                venore_core::chat::compaction::CompactionResult::NoAction => {}
            },
            Err(e) => {
                tracing::warn!("Compaction failed (continuing without): {}", e);
            }
        }

        // Continue the loop — call LLM again with tool results
        let next_tools = if plan_mode {
            match &ctx.agent_repo {
                Some(repo) => match repo.load_read_only_llm_tools(&[]).await {
                    Ok(t) if !t.is_empty() => Some(t),
                    _ => Some(tools::read_only_tools()),
                },
                None => Some(tools::read_only_tools()),
            }
        } else {
            ctx.llm_tools.clone()
        };
        match venore_core::chat::continue_chat_stream(
            &ctx.llm_gateway,
            llm_messages.clone(),
            next_tools,
            &final_model,
            ctx.options.clone(),
        ).await {
            Ok(next_stream) => {
                current_stream = next_stream;
            }
            Err(e) => {
                let _ = ctx.app.emit(
                    "chat-stream-error",
                    ChatStreamErrorPayload {
                        stream_id: ctx.stream_id.clone(),
                        session_id: ctx.session_id.clone(),
                        message: e.to_string(),
                        code: e.code().to_string(),
                    },
                );
                break;
            }
        }
    }

    // Mark guard so it won't double-emit on drop
    done_guard.emitted = true;

    // Always emit done so frontend unblocks (even after errors)
    let _ = ctx.app.emit(
        "chat-stream-done",
        ChatStreamDonePayload {
            stream_id: ctx.stream_id.clone(),
            session_id: ctx.session_id.clone(),
            prompt_tokens: final_usage.0,
            completion_tokens: final_usage.1,
            total_tokens: final_usage.2,
            provider: final_provider.clone(),
            model: final_model.clone(),
        },
    );

    // Append the assistant turn to the chat-debug log. Empty content means
    // the model only emitted tool calls — still worth a record so timelines
    // align (every user turn pairs with at least one assistant entry).
    venore_core::chat::log_chat_event(venore_core::chat::ChatDebugEvent::AssistantMessage {
        session_id: ctx.session_id.clone().unwrap_or_default(),
        content: accumulated_content.clone(),
        model: Some(final_model.clone()),
        ts: venore_core::chat::chat_event_now(),
    });

    persist_messages(&ctx, &accumulated_content, &final_provider, &final_model, final_usage).await;
    cleanup_stream(&ctx.stream_id, ctx.session_id.as_deref());
}

/// Clean orphaned pending approval/response/plan channels for a stream.
/// Called on abort and on normal stream cleanup to prevent stuck overlays.
pub(super) fn cleanup_pending_channels(stream_id: &str) {
    let ids: Vec<String> = STREAM_TOOL_CALL_IDS
        .lock().ok()
        .and_then(|mut map| map.remove(stream_id))
        .unwrap_or_default();
    if ids.is_empty() { return; }
    tracing::debug!(stream_id, count = ids.len(), "Cleaning pending channels");
    if let Ok(mut m) = PENDING_APPROVALS.lock() { for id in &ids { m.remove(id); } }
    if let Ok(mut m) = PENDING_USER_RESPONSES.lock() { for id in &ids { m.remove(id); } }
    if let Ok(mut m) = PENDING_PLAN_APPROVALS.lock() { for id in &ids { m.remove(id); } }
}

/// Remove stale entries from global state maps after a stream ends.
fn cleanup_stream(stream_id: &str, session_id: Option<&str>) {
    if let Ok(mut streams) = ACTIVE_STREAMS.lock() {
        streams.remove(stream_id);
    }
    if let Ok(mut stores) = TASK_STORES.lock() {
        stores.remove(stream_id);
    }
    if let Ok(mut agents) = ACTIVE_SUB_AGENTS.lock() {
        agents.remove(stream_id);
    }
    // Remove session → stream mapping (only if it still points to THIS stream)
    if let Some(sid) = session_id {
        if let Ok(mut m) = SESSION_STREAMS.lock() {
            if m.get(sid).map(|s| s.as_str()) == Some(stream_id) {
                m.remove(sid);
            }
        }
    }
    cleanup_pending_channels(stream_id);
}

/// Persist the user message to DB immediately so pop-out windows can load it.
async fn persist_user_message(ctx: &AgenticLoopCtx) {
    if let (Some(ref session_id), Some(ref repo)) = (&ctx.session_id, &ctx.chat_repo) {
        if let Some(last_user) = ctx.messages.iter().rev().find(|m| m.role == "user") {
            let user_record = ChatMessageRecord::new_user(
                session_id,
                &last_user.content,
                ctx.attachments_json.clone(),
            );
            if let Err(e) = repo.save_message(&user_record).await {
                tracing::error!("Failed to save user message early: {}", e);
            }
        }
    }
}

/// Persist assistant message and touch session timestamp in the DB.
async fn persist_messages(
    ctx: &AgenticLoopCtx,
    accumulated_content: &str,
    final_provider: &str,
    final_model: &str,
    final_usage: (u32, u32, u32),
) {
    if let (Some(ref session_id), Some(ref repo)) = (&ctx.session_id, &ctx.chat_repo) {
        if !accumulated_content.is_empty() {
            let assistant_record = ChatMessageRecord::new_assistant(
                session_id,
                accumulated_content,
                final_provider,
                final_model,
                final_usage.0,
                final_usage.1,
            );
            if let Err(e) = repo.save_message(&assistant_record).await {
                tracing::error!("Failed to save assistant message: {}", e);
            }
        }

        if let Err(e) = repo.touch_session(session_id).await {
            tracing::error!("Failed to touch session: {}", e);
        }
    }
}
