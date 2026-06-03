//! Tool dispatch — execute parallel, spawn-agent, and sequential tool calls.

use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use tauri::Emitter;

use venore_core::llm::prelude::*;
use venore_core::llm::types::{LlmMessage, LlmToolCall};
use venore_core::terminal::TerminalSessionManager;
use venore_core::tools;
use venore_core::tools::names as N;

use super::agentic_loop::AgenticLoopCtx;
use super::dto::*;
use super::helpers::{check_permission_action, extract_tool_resource};
use super::state::*;
use venore_core::chat::{ChatRepository, ToolCallRecord};

use super::sub_agent::{decrement_active_sub_agents, hardcoded_sub_agent_prompt, run_sub_agent};

// ── LLM output cap ──────────────────────────────────────────────────

/// Maximum characters of tool output sent to the LLM (middle-truncated).
const LLM_OUTPUT_CAP: usize = 8_000;

/// Tools whose output should never be truncated for the LLM.
const TRUNCATION_EXCLUDED_TOOLS: &[&str] = &[
    N::ASK_USER,
    N::TASK_CREATE,
    N::TASK_UPDATE,
    N::TASK_LIST,
    N::ENTER_PLAN_MODE,
    N::SUBMIT_PLAN,
];

/// Truncate tool output for the LLM context window using middle truncation.
/// Keeps the first 60% and last 40% of the cap, replacing the middle with a marker.
/// Returns the original string if it fits within the cap or the tool is excluded.
fn truncate_for_llm(output: &str, tool_name: &str) -> String {
    if output.len() <= LLM_OUTPUT_CAP || TRUNCATION_EXCLUDED_TOOLS.contains(&tool_name) {
        return output.to_string();
    }

    let head_size = (LLM_OUTPUT_CAP as f64 * 0.6) as usize;
    let tail_size = LLM_OUTPUT_CAP - head_size;
    let truncated_chars = output.len() - head_size - tail_size;

    let head_end = output.floor_char_boundary(head_size);
    let tail_start = output.len() - output.ceil_char_boundary(output.len() - tail_size).min(output.len());
    let tail_start = output.len() - tail_start;

    format!(
        "{}\n\n[...{} chars truncated...]\n\n{}",
        &output[..head_end],
        truncated_chars,
        &output[tail_start..],
    )
}

// ── Tool call persistence helpers ────────────────────────────────────

/// Truncate output to 500 chars for DB storage.
fn truncate_output(s: &str) -> String {
    if s.len() > 500 {
        format!("{}...", &s[..s.floor_char_boundary(497)])
    } else {
        s.to_string()
    }
}

/// Track a tool_call_id against its stream_id for cleanup on abort.
fn track_tool_call(stream_id: &str, tool_call_id: &str) {
    if let Ok(mut map) = STREAM_TOOL_CALL_IDS.lock() {
        map.entry(stream_id.to_string()).or_default().push(tool_call_id.to_string());
    }
}

/// Wait for a oneshot response, re-emitting the event on each timeout interval.
/// Returns `None` if the sender was dropped or all attempts expired.
async fn wait_with_retry<T>(
    mut rx: tokio::sync::oneshot::Receiver<T>,
    interval: Duration,
    max_attempts: u32,
    re_emit: impl Fn(),
) -> Option<T> {
    for attempt in 0..max_attempts {
        match tokio::time::timeout(interval, &mut rx).await {
            Ok(Ok(value)) => return Some(value),
            Ok(Err(_)) => return None,   // sender dropped
            Err(_) if attempt + 1 < max_attempts => {
                tracing::info!(attempt = attempt + 1, max = max_attempts, "Approval timeout, re-emitting");
                re_emit();
            }
            Err(_) => break,
        }
    }
    tracing::warn!("Approval timed out after all retry attempts");
    None
}

/// Save a tool call to the DB (initial insert, before execution).
async fn persist_tool_call_start(
    repo: &ChatRepository,
    session_id: &str,
    tool_call_id: &str,
    tool_name: &str,
    arguments: &serde_json::Value,
) {
    let record = ToolCallRecord {
        id: tool_call_id.to_string(),
        session_id: session_id.to_string(),
        tool_name: tool_name.to_string(),
        arguments: arguments.to_string(),
        success: None,
        output: None,
        commit_hash: None,
        created_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
    };
    if let Err(e) = repo.save_tool_call(&record).await {
        tracing::warn!("Failed to persist tool call start: {}", e);
    }
}

/// Update a tool call in the DB with its result.
async fn persist_tool_call_result(
    repo: &ChatRepository,
    tool_call_id: &str,
    success: bool,
    output: &str,
    commit_hash: Option<&str>,
) {
    if let Err(e) = repo.update_tool_call_result(
        tool_call_id,
        success,
        &truncate_output(output),
        commit_hash,
    ).await {
        tracing::warn!("Failed to persist tool call result: {}", e);
    }
}

// ── Shared tool execution helpers ────────────────────────────────────

/// Execute a tool call, returning a ToolExecutionResult even on error.
/// Wraps the dispatch with chat-debug log entries (one ToolCall on entry,
/// one ToolResult on exit) so external observers can audit what happened
/// without scraping SQLite.
pub(super) async fn execute_tool_safe(
    name: &str,
    arguments: &serde_json::Value,
    ctx: &tools::ToolExecutionContext,
) -> tools::ToolExecutionResult {
    let session_id = ctx.session_id.clone().unwrap_or_default();
    venore_core::chat::log_chat_event(venore_core::chat::ChatDebugEvent::ToolCall {
        session_id: session_id.clone(),
        name: name.to_string(),
        arguments: arguments.clone(),
        ts: venore_core::chat::chat_event_now(),
    });

    let started = std::time::Instant::now();
    let result = match tools::execute_tool(name, arguments, ctx).await {
        Ok(r) => r,
        Err(e) => tools::ToolExecutionResult {
            success: false,
            output: e.to_string(),
            baseline: None,
        },
    };
    let duration_ms = started.elapsed().as_millis();

    venore_core::chat::log_chat_event(venore_core::chat::ChatDebugEvent::ToolResult {
        session_id,
        name: name.to_string(),
        success: result.success,
        // Cap output to keep individual lines readable in the log; full
        // output still lives in the SQLite tool_calls table for deep dives.
        output: truncate_for_log(&result.output, 4_000),
        duration_ms,
        ts: venore_core::chat::chat_event_now(),
    });

    result
}

fn truncate_for_log(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while !s.is_char_boundary(end) && end > 0 {
        end -= 1;
    }
    format!("{}…[truncated {} bytes]", &s[..end], s.len() - end)
}

/// Knowledge tools that trigger a UI refresh when executed.
const KNOWLEDGE_CHANGE_TOOLS: &[&str] = &[
    N::PLAN_HEXAGONS,
    N::UPDATE_HEXAGON,
    N::ADD_EVIDENCE,
    N::MARK_DEAD_END,
];

/// Ocean-mutating tools — when any of these succeeds, the UI needs to
/// refetch the layout to surface new/edited nodes, sections, or connections.
/// The same `ocean-knowledge-changed` event the manual UI commands use is
/// reused here so existing listeners (OceanNodes, NodeLogbook…) just work.
///
/// `propose_logbook_write` deliberately does NOT belong here — it doesn't
/// mutate the layout, it stashes a pending write. The dedicated
/// `ai-write-proposed` emitter below covers it.
const OCEAN_CHANGE_TOOLS: &[&str] = &[
    N::CREATE_LIGHTHOUSE,
    N::CREATE_KNOWLEDGE_NODE,
    N::CREATE_CONNECTION,
    N::PROMOTE_TO_LIGHTHOUSE,
    N::SET_NODE_LIGHTHOUSE,
    N::RENAME_NODE,
];

/// Marker emitted by `execute_propose_logbook_write` (see
/// `venore-core/src/tools/executor.rs`) so the dispatch layer can wire
/// the `ai-write-proposed` event without changing the
/// `ToolExecutionResult` shape. Format:
/// `[ai-write-pending write_id=<id>;node_id=<nid>;kind=<create|edit>]`.
const AI_WRITE_PENDING_PREFIX: &str = "[ai-write-pending ";

/// Parse the trailing pending-write marker out of a tool result. Returns
/// `None` for any tool whose output lacks the marker (i.e. anything except
/// a successful `propose_logbook_write`).
fn parse_ai_write_pending_marker(output: &str) -> Option<(String, String, String)> {
    let start = output.rfind(AI_WRITE_PENDING_PREFIX)?;
    let after = &output[start + AI_WRITE_PENDING_PREFIX.len()..];
    let end = after.find(']')?;
    let body = &after[..end];

    let mut write_id: Option<&str> = None;
    let mut node_id: Option<&str> = None;
    let mut kind: Option<&str> = None;
    for part in body.split(';') {
        let mut it = part.splitn(2, '=');
        match (it.next(), it.next()) {
            (Some("write_id"), Some(v)) => write_id = Some(v),
            (Some("node_id"), Some(v)) => node_id = Some(v),
            (Some("kind"), Some(v)) => kind = Some(v),
            _ => {}
        }
    }
    Some((write_id?.to_string(), node_id?.to_string(), kind?.to_string()))
}

/// Emit `knowledge-hexagons-changed` event if the tool is a knowledge mutation tool.
fn emit_knowledge_change_if_needed(
    tool_name: &str,
    loop_ctx: &AgenticLoopCtx,
) {
    if KNOWLEDGE_CHANGE_TOOLS.contains(&tool_name) {
        if let Some(ref feature_id) = loop_ctx.knowledge_feature_id {
            let _ = loop_ctx.app.emit(
                "knowledge-hexagons-changed",
                serde_json::json!({
                    "featureId": feature_id,
                    "toolName": tool_name,
                }),
            );
        }
    }
}

/// Emit `ocean-knowledge-changed` for tools that mutate ocean state, so the
/// canvas / logbook UI refetches and surfaces the new content. The payload
/// shape mirrors the one Tauri commands use (`{ project_path, node_id }`)
/// so existing UI listeners don't need to special-case the chat path.
fn emit_ocean_change_if_needed(
    tool_name: &str,
    arguments: &serde_json::Value,
    loop_ctx: &AgenticLoopCtx,
) {
    if !OCEAN_CHANGE_TOOLS.contains(&tool_name) {
        return;
    }
    let project_path = match loop_ctx.tool_project_path.as_deref() {
        Some(p) => p,
        None => {
            tracing::warn!(
                tool = %tool_name,
                "ocean_change emit skipped: no tool_project_path on loop_ctx"
            );
            return;
        }
    };
    // Best-effort node_id extraction — most tools take it as an argument.
    // Empty string when not applicable (e.g., create_connection, where the
    // listener will refetch the whole layout anyway).
    let node_id = arguments
        .get("node_id")
        .and_then(|v| v.as_str())
        .or_else(|| arguments.get("from_node_id").and_then(|v| v.as_str()))
        .unwrap_or("");
    let payload = serde_json::json!({
        "project_path": project_path,
        "node_id": node_id,
    });
    match loop_ctx.app.emit("ocean-knowledge-changed", payload.clone()) {
        Ok(()) => tracing::info!(
            tool = %tool_name,
            project_path = %project_path,
            node_id = %node_id,
            "Emitted ocean-knowledge-changed",
        ),
        Err(e) => tracing::warn!(
            tool = %tool_name,
            error = %e,
            "Failed to emit ocean-knowledge-changed"
        ),
    }
}

/// Emit `ai-write-proposed` when `propose_logbook_write` succeeded. The
/// executor stashed the proposal in `pending_writes::PENDING_WRITES`; the
/// frontend listens to refetch the panel's pending list and to bring the
/// node panel to front.
fn emit_ai_write_proposed_if_needed(
    tool_name: &str,
    output: &str,
    loop_ctx: &AgenticLoopCtx,
) {
    if tool_name != N::PROPOSE_LOGBOOK_WRITE {
        return;
    }
    let project_path = match loop_ctx.tool_project_path.as_deref() {
        Some(p) => p,
        None => {
            tracing::warn!(
                tool = %tool_name,
                "ai_write_proposed emit skipped: no tool_project_path on loop_ctx"
            );
            return;
        }
    };
    let (write_id, node_id, kind) = match parse_ai_write_pending_marker(output) {
        Some(t) => t,
        None => {
            tracing::warn!(
                tool = %tool_name,
                "ai_write_proposed emit skipped: marker absent from tool output"
            );
            return;
        }
    };

    // Enrich with node metadata so the frontend can auto-open the floating
    // panel without an extra round-trip. Falls back to empty / "module"
    // if the lookup fails (panel will still open if already mounted, just
    // with the older name; never blocks the event).
    let (node_name, node_variant, module_path) = venore_core::ocean::service::with_service(
        project_path,
        |service| {
            service
                .get_layout()
                .positions
                .get(&node_id)
                .map(|e| {
                    let variant = match e.node_variant {
                        venore_core::ocean::NodeVariant::Module => "module",
                        venore_core::ocean::NodeVariant::KnowledgeNode => "knowledge_node",
                        venore_core::ocean::NodeVariant::Lighthouse => "lighthouse",
                        venore_core::ocean::NodeVariant::Buoy => "buoy",
                        venore_core::ocean::NodeVariant::Cylinder => "cylinder",
                    };
                    (e.module_name.clone(), variant.to_string(), e.module_path.clone())
                })
        },
    )
    .ok()
    .flatten()
    .unwrap_or_else(|| (String::new(), "knowledge_node".to_string(), String::new()));

    let payload = serde_json::json!({
        "project_path": project_path,
        "node_id": node_id,
        "write_id": write_id,
        "kind": kind,
        "node_name": node_name,
        "node_variant": node_variant,
        "module_path": module_path,
    });
    match loop_ctx.app.emit("ai-write-proposed", payload.clone()) {
        Ok(()) => tracing::info!(
            project_path = %project_path,
            node_id = %node_id,
            write_id = %write_id,
            kind = %kind,
            "Emitted ai-write-proposed",
        ),
        Err(e) => tracing::warn!(
            error = %e,
            "Failed to emit ai-write-proposed"
        ),
    }
}

/// Append LSP diagnostics to tool result for file-editing tools.
/// No-op if the tool is not a file edit or the result was unsuccessful.
pub(super) async fn append_lsp_diagnostics(
    tool_call: &LlmToolCall,
    tool_result: &mut tools::ToolExecutionResult,
    project_path: &str,
) {
    if !N::FILE_EDIT_TOOLS.contains(&tool_call.name.as_str()) || !tool_result.success {
        return;
    }
    if let Some(file_path_str) = tool_call.arguments["file_path"].as_str() {
        if let Some(diag_text) = venore_core::lsp::fetch_post_edit_diagnostics(
            file_path_str,
            project_path,
            2500,
        ).await {
            tool_result.output = format!("{}\n\n{}", tool_result.output, diag_text);
        }
    }
}

// ── Emit + persist + push helper ─────────────────────────────────────

/// Emit a `chat-tool-result` event, persist the result to DB, and push
/// a `Tool` message onto the LLM message history.
/// Frontend receives the full output; LLM receives `truncate_for_llm` version.
async fn emit_persist_push(
    loop_ctx: &AgenticLoopCtx,
    llm_messages: &mut Vec<LlmMessage>,
    tool_call_id: &str,
    tool_name: &str,
    arguments: &serde_json::Value,
    success: bool,
    output: String,
) {
    let _ = loop_ctx.app.emit(
        "chat-tool-result",
        ChatToolResultPayload {
            stream_id: loop_ctx.stream_id.clone(),
            session_id: loop_ctx.session_id.clone(),
            tool_call_id: tool_call_id.to_string(),
            success,
            output: output.clone(),
        },
    );
    if let (Some(ref _sid), Some(ref repo)) = (&loop_ctx.session_id, &loop_ctx.chat_repo) {
        persist_tool_call_result(repo, tool_call_id, success, &output, None).await;
    }
    if success {
        emit_knowledge_change_if_needed(tool_name, loop_ctx);
        emit_ocean_change_if_needed(tool_name, arguments, loop_ctx);
        emit_ai_write_proposed_if_needed(tool_name, &output, loop_ctx);
    }
    let llm_content = truncate_for_llm(&output, tool_name);
    llm_messages.push(LlmMessage {
        role: MessageRole::Tool,
        content: llm_content,
        tool_call_id: Some(tool_call_id.to_string()),
        tool_calls: None,
        content_parts: None,
    });
}

// ── Parallel batch execution ─────────────────────────────────────────

/// Execute read-only tool calls in parallel, emit events, push results to messages.
pub(super) async fn execute_parallel_batch(
    calls: &[&LlmToolCall],
    tool_ctx: &tools::ToolExecutionContext,
    loop_ctx: &AgenticLoopCtx,
    llm_messages: &mut Vec<LlmMessage>,
) {
    if calls.is_empty() {
        return;
    }

    tracing::debug!(
        count = calls.len(),
        "Executing {} tool calls in parallel",
        calls.len()
    );

    // Emit chat-tool-call events + persist start
    for tc in calls {
        let _ = loop_ctx.app.emit(
            "chat-tool-call",
            ChatToolCallPayload {
                stream_id: loop_ctx.stream_id.clone(),
                session_id: loop_ctx.session_id.clone(),
                tool_call_id: tc.id.clone(),
                tool_name: tc.name.clone(),
                arguments: tc.arguments.clone(),
            },
        );
        if let (Some(ref sid), Some(ref repo)) = (&loop_ctx.session_id, &loop_ctx.chat_repo) {
            persist_tool_call_start(repo, sid, &tc.id, &tc.name, &tc.arguments).await;
        }
    }

    // Execute all in parallel
    let parallel_futures: Vec<_> = calls
        .iter()
        .map(|tc| {
            let name = tc.name.clone();
            let args = tc.arguments.clone();
            let ctx = tool_ctx.clone();
            async move { execute_tool_safe(&name, &args, &ctx).await }
        })
        .collect();

    let parallel_results = futures::future::join_all(parallel_futures).await;

    // Emit results, persist, and push to messages
    for (tc, result) in calls.iter().zip(parallel_results) {
        emit_persist_push(
            loop_ctx,
            llm_messages,
            &tc.id,
            &tc.name,
            &tc.arguments,
            result.success,
            result.output,
        )
        .await;
    }
}

// ── Spawn agent execution ────────────────────────────────────────────

/// Execute spawn_agent calls concurrently (Phases A-E).
pub(super) async fn execute_spawn_agents(
    calls: &[&LlmToolCall],
    tool_ctx: &tools::ToolExecutionContext,
    loop_ctx: &AgenticLoopCtx,
    llm_messages: &mut Vec<LlmMessage>,
    active_terminal_id: Option<&str>,
) {
    if calls.is_empty() {
        return;
    }

    tracing::debug!(
        count = calls.len(),
        "Executing {} spawn_agent calls concurrently",
        calls.len()
    );

    // Phase A — Pre-resolve profiles
    struct SpawnConfig {
        idx: usize,
        tool_call_id: String,
        agent_id: String,
        agent_type: String,
        task_desc: String,
        can_spawn: bool,
        sub_tools: Vec<venore_core::llm::types::LlmTool>,
        sub_system_prompt: String,
        sub_options: GatewayOptions,
        sub_model: String,
        iter_override: Option<u32>,
        calls_override: Option<u32>,
    }

    let mut configs: Vec<SpawnConfig> = Vec::with_capacity(calls.len());

    for (idx, tc) in calls.iter().enumerate() {
        let agent_type = tc.arguments["agent_type"]
            .as_str()
            .unwrap_or("research")
            .to_string();
        let task_desc = tc.arguments["task"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let agent_id = uuid::Uuid::new_v4().to_string();

        // Check concurrent limit
        let can_spawn = {
            let mut agents = ACTIVE_SUB_AGENTS.lock().unwrap_or_else(|e| {
                tracing::warn!("ACTIVE_SUB_AGENTS mutex poisoned, recovering");
                e.into_inner()
            });
            let count = agents.entry(loop_ctx.stream_id.clone()).or_insert(0);
            if *count >= 5 {
                false
            } else {
                *count += 1;
                true
            }
        };

        if !can_spawn {
            configs.push(SpawnConfig {
                idx,
                tool_call_id: tc.id.clone(),
                agent_id,
                agent_type,
                task_desc,
                can_spawn: false,
                sub_tools: Vec::new(),
                sub_system_prompt: String::new(),
                sub_options: loop_ctx.options.clone(),
                sub_model: String::new(),
                iter_override: None,
                calls_override: None,
            });
            continue;
        }

        // Load profile from DB
        let sub_profile_id = match agent_type.as_str() {
            "executor" => "sub-agent-executor",
            "research" => "sub-agent-research",
            _ => "sub-agent-general",
        };

        let sub_profile = match &loop_ctx.agent_repo {
            Some(repo) => repo.get_profile(sub_profile_id).await.ok(),
            None => None,
        };

        let (sub_tools, sub_system_prompt, sub_options) = if let Some(ref prof) = sub_profile {
            let tool_ids: Vec<String> = serde_json::from_str(&prof.tools_json)
                .unwrap_or_default();
            let prof_tools = match &loop_ctx.agent_repo {
                Some(repo) => repo.load_llm_tools(&tool_ids).await
                    .unwrap_or_else(|_| tools::sub_agent_type_tools(&agent_type)),
                None => tools::sub_agent_type_tools(&agent_type),
            };

            let prompt = if prof.system_prompt.is_empty() {
                hardcoded_sub_agent_prompt(&agent_type, &task_desc)
            } else {
                prof.system_prompt.replace("{task}", &task_desc)
            };

            let opts = if !prof.provider.is_empty() {
                GatewayOptions {
                    provider: venore_core::traits::LlmProviderType::from_str(&prof.provider).ok(),
                    model: if prof.model.is_empty() { None } else { Some(prof.model.clone()) },
                    temperature: Some(prof.temperature),
                    ..loop_ctx.options.clone()
                }
            } else {
                loop_ctx.options.clone()
            };

            (prof_tools, prompt, opts)
        } else {
            let fallback_tools = match &loop_ctx.agent_repo {
                Some(repo) => match repo.load_llm_tools(&[]).await {
                    Ok(db_tools) if !db_tools.is_empty() => {
                        let allowed: HashSet<String> = tools::sub_agent_type_tools(&agent_type)
                            .iter().map(|t| t.name.clone()).collect();
                        db_tools.into_iter().filter(|t| allowed.contains(&t.name)).collect()
                    }
                    _ => tools::sub_agent_type_tools(&agent_type),
                },
                None => tools::sub_agent_type_tools(&agent_type),
            };
            let prompt = hardcoded_sub_agent_prompt(&agent_type, &task_desc);
            (fallback_tools, prompt, loop_ctx.options.clone())
        };

        let sub_model = sub_profile
            .as_ref()
            .filter(|p| !p.model.is_empty())
            .map(|p| p.model.clone())
            .unwrap_or_else(|| loop_ctx.model.clone());

        let (iter_override, calls_override) = if agent_type == "executor" {
            (Some(5), Some(25))
        } else {
            (None, None)
        };

        configs.push(SpawnConfig {
            idx,
            tool_call_id: tc.id.clone(),
            agent_id,
            agent_type,
            task_desc,
            can_spawn: true,
            sub_tools,
            sub_system_prompt,
            sub_options,
            sub_model,
            iter_override,
            calls_override,
        });
    }

    // Phase B — Emit all "started" events + persist start
    for cfg in &configs {
        if cfg.can_spawn {
            let _ = loop_ctx.app.emit(
                "chat-sub-agent",
                ChatSubAgentPayload {
                    stream_id: loop_ctx.stream_id.clone(),
                    session_id: loop_ctx.session_id.clone(),
                    agent_id: cfg.agent_id.clone(),
                    agent_type: cfg.agent_type.clone(),
                    task: cfg.task_desc.clone(),
                    status: "started".to_string(),
                    result: None,
                },
            );
        }
        if let (Some(ref sid), Some(ref repo)) = (&loop_ctx.session_id, &loop_ctx.chat_repo) {
            persist_tool_call_start(
                repo, sid, &cfg.tool_call_id,
                "spawn_agent",
                &serde_json::json!({"agent_type": cfg.agent_type, "task": cfg.task_desc}),
            ).await;
        }
    }

    // Phase C — Launch tokio::spawn per config
    let mut handles: Vec<(usize, String, String, String, String, Option<tokio::task::JoinHandle<(bool, String)>>)> = Vec::new();

    for cfg in configs {
        if !cfg.can_spawn {
            handles.push((cfg.idx, cfg.tool_call_id, cfg.agent_id, cfg.agent_type, cfg.task_desc, None));
            continue;
        }

        let sub_messages = vec![LlmMessage {
            role: MessageRole::User,
            content: cfg.task_desc.clone(),
            tool_call_id: None,
            tool_calls: None,
            content_parts: None,
        }];

        let gw = Arc::clone(&loop_ctx.llm_gateway);
        let ctx_clone = tool_ctx.clone();
        let app_for_spawn = loop_ctx.app.clone();
        let tid_clone = active_terminal_id.map(|s| s.to_string());
        let sid_for_decrement = loop_ctx.stream_id.clone();
        let agent_id_for_event = cfg.agent_id.clone();
        let agent_type_for_event = cfg.agent_type.clone();
        let task_desc_for_event = cfg.task_desc.clone();
        let sid_for_event = loop_ctx.stream_id.clone();
        let session_id_for_event = loop_ctx.session_id.clone();

        let handle = tokio::spawn(async move {
            let sub_result = run_sub_agent(
                gw,
                sub_messages,
                cfg.sub_system_prompt,
                Some(cfg.sub_tools),
                cfg.sub_model,
                cfg.sub_options,
                ctx_clone,
                tid_clone,
                cfg.iter_override,
                cfg.calls_override,
            )
            .await;

            decrement_active_sub_agents(&sid_for_decrement);

            let (success, result_text) = match sub_result {
                Ok(text) => (true, text),
                Err(e) => (false, format!("Sub-agent failed: {}", e)),
            };

            let _ = app_for_spawn.emit(
                "chat-sub-agent",
                ChatSubAgentPayload {
                    stream_id: sid_for_event,
                    session_id: session_id_for_event,
                    agent_id: agent_id_for_event,
                    agent_type: agent_type_for_event,
                    task: task_desc_for_event,
                    status: if success { "completed" } else { "failed" }.to_string(),
                    result: Some(result_text.clone()),
                },
            );

            (success, result_text)
        });

        handles.push((cfg.idx, cfg.tool_call_id, cfg.agent_id, cfg.agent_type, cfg.task_desc, Some(handle)));
    }

    // Phase D — Await all JoinHandles
    let mut results: Vec<(usize, String, String, bool, String)> = Vec::new();

    for (idx, tool_call_id, _agent_id, agent_type, _task_desc, maybe_handle) in handles {
        match maybe_handle {
            Some(handle) => match handle.await {
                Ok((success, text)) => {
                    results.push((idx, tool_call_id, agent_type, success, text));
                }
                Err(e) => {
                    decrement_active_sub_agents(&loop_ctx.stream_id);
                    results.push((idx, tool_call_id, agent_type, false, format!("Sub-agent panicked: {}", e)));
                }
            },
            None => {
                results.push((idx, tool_call_id, agent_type, false,
                    "Cannot spawn sub-agent: maximum of 5 concurrent sub-agents reached. Wait for existing agents to complete.".to_string()));
            }
        }
    }

    // Sort by original index to maintain LLM request order
    results.sort_by_key(|(idx, _, _, _, _)| *idx);

    // Phase E — Push results to llm_messages + emit chat-tool-result + persist
    for (_idx, tool_call_id, agent_type, success, result_text) in results {
        let result_msg = format!("Sub-agent ({}) result:\n{}", agent_type, result_text);
        // spawn_agent doesn't mutate ocean state — pass an empty args object;
        // emit_persist_push only fires the ocean event when the tool name is
        // in OCEAN_CHANGE_TOOLS, which spawn_agent isn't.
        emit_persist_push(
            loop_ctx,
            llm_messages,
            &tool_call_id,
            "spawn_agent",
            &serde_json::Value::Null,
            success,
            result_msg,
        )
        .await;
    }
}

// ── Sequential tool execution ────────────────────────────────────────

/// Execute sequential tool calls one by one with special-case handling and permissions.
pub(super) async fn execute_sequential_tools(
    calls: &[&LlmToolCall],
    tool_ctx: &tools::ToolExecutionContext,
    loop_ctx: &AgenticLoopCtx,
    llm_messages: &mut Vec<LlmMessage>,
    plan_mode: &mut bool,
    active_terminal_id: Option<&str>,
) {
    for tool_call in calls {
        // Emit tool call event + persist start
        let _ = loop_ctx.app.emit(
            "chat-tool-call",
            ChatToolCallPayload {
                stream_id: loop_ctx.stream_id.clone(),
                session_id: loop_ctx.session_id.clone(),
                tool_call_id: tool_call.id.clone(),
                tool_name: tool_call.name.clone(),
                arguments: tool_call.arguments.clone(),
            },
        );
        if let (Some(ref sid), Some(ref repo)) = (&loop_ctx.session_id, &loop_ctx.chat_repo) {
            persist_tool_call_start(repo, sid, &tool_call.id, &tool_call.name, &tool_call.arguments).await;
        }

        // Try special-case tools first
        if handle_special_tool(tool_call, loop_ctx, llm_messages, plan_mode).await {
            continue;
        }

        // Permission check
        if !enforce_permission(tool_call, loop_ctx, llm_messages).await {
            continue;
        }

        // Normal tool execution
        let mut tool_result = execute_tool_safe(&tool_call.name, &tool_call.arguments, tool_ctx).await;

        // Emit result + persist
        let _ = loop_ctx.app.emit(
            "chat-tool-result",
            ChatToolResultPayload {
                stream_id: loop_ctx.stream_id.clone(),
                session_id: loop_ctx.session_id.clone(),
                tool_call_id: tool_call.id.clone(),
                success: tool_result.success,
                output: tool_result.output.clone(),
            },
        );
        if let (Some(ref _sid), Some(ref repo)) = (&loop_ctx.session_id, &loop_ctx.chat_repo) {
            persist_tool_call_result(repo, &tool_call.id, tool_result.success, &tool_result.output, None).await;
        }

        // Emit knowledge change event for UI refresh
        if tool_result.success {
            emit_knowledge_change_if_needed(&tool_call.name, loop_ctx);
            emit_ocean_change_if_needed(&tool_call.name, &tool_call.arguments, loop_ctx);
            emit_ai_write_proposed_if_needed(&tool_call.name, &tool_result.output, loop_ctx);
        }

        // Post-process file edits
        if N::FILE_EDIT_TOOLS.contains(&tool_call.name.as_str()) && tool_result.success {
            post_process_file_edit(tool_call, loop_ctx, &mut tool_result).await;
        }

        // Post-process terminal output
        if tool_call.name == N::RUN_TERMINAL_COMMAND && tool_result.success
            && post_process_terminal(tool_call, active_terminal_id, &tool_result, llm_messages).await {
                continue; // Skip the normal push
            }

        // Add tool result to message history (truncated for LLM)
        let llm_content = truncate_for_llm(&tool_result.output, &tool_call.name);
        llm_messages.push(LlmMessage {
            role: MessageRole::Tool,
            content: llm_content,
            tool_call_id: Some(tool_call.id.clone()),
            tool_calls: None,
            content_parts: None,
        });
    }
}

/// Handle special-case tools (ask_user, task_*, plan_*). Returns true if handled.
async fn handle_special_tool(
    tool_call: &LlmToolCall,
    loop_ctx: &AgenticLoopCtx,
    llm_messages: &mut Vec<LlmMessage>,
    plan_mode: &mut bool,
) -> bool {
    match tool_call.name.as_str() {
        N::ASK_USER => {
            handle_ask_user(tool_call, loop_ctx, llm_messages).await;
            true
        }
        N::TASK_CREATE => {
            handle_task_create(tool_call, loop_ctx, llm_messages);
            true
        }
        N::TASK_UPDATE => {
            handle_task_update(tool_call, loop_ctx, llm_messages);
            true
        }
        N::TASK_LIST => {
            handle_task_list(tool_call, loop_ctx, llm_messages);
            true
        }
        N::ENTER_PLAN_MODE => {
            handle_enter_plan_mode(tool_call, loop_ctx, llm_messages, plan_mode);
            true
        }
        N::SUBMIT_PLAN => {
            handle_submit_plan(tool_call, loop_ctx, llm_messages, plan_mode).await;
            true
        }
        _ => false,
    }
}

async fn handle_ask_user(
    tool_call: &LlmToolCall,
    loop_ctx: &AgenticLoopCtx,
    llm_messages: &mut Vec<LlmMessage>,
) {
    let question = tool_call.arguments["question"]
        .as_str()
        .unwrap_or("What would you like to do?")
        .to_string();
    let options: Vec<AskUserOption> = tool_call.arguments["options"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    v["label"].as_str().map(|label| AskUserOption {
                        label: label.to_string(),
                        description: v["description"].as_str().map(|s| s.to_string()),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let _ = loop_ctx.app.emit(
        "chat-ask-user",
        ChatAskUserPayload {
            stream_id: loop_ctx.stream_id.clone(),
            session_id: loop_ctx.session_id.clone(),
            tool_call_id: tool_call.id.clone(),
            question: question.clone(),
            options: options.clone(),
        },
    );

    let (tx, rx) = tokio::sync::oneshot::channel::<String>();
    if let Ok(mut pending) = PENDING_USER_RESPONSES.lock() {
        pending.insert(tool_call.id.clone(), tx);
    }
    track_tool_call(&loop_ctx.stream_id, &tool_call.id);

    let emit_payload = ChatAskUserPayload {
        stream_id: loop_ctx.stream_id.clone(),
        session_id: loop_ctx.session_id.clone(),
        tool_call_id: tool_call.id.clone(),
        question: question.clone(),
        options: options.clone(),
    };
    let app = loop_ctx.app.clone();
    let user_response = wait_with_retry(rx, Duration::from_secs(60), 3,
        || { let _ = app.emit("chat-ask-user", emit_payload.clone()); },
    ).await.unwrap_or_else(|| "(no response — timed out after 3 minutes)".to_string());

    let result_msg = format!("User responded: {}", user_response);
    emit_persist_push(
        loop_ctx,
        llm_messages,
        &tool_call.id,
        N::ASK_USER,
        &tool_call.arguments,
        true,
        result_msg,
    )
    .await;
}

fn handle_task_create(
    tool_call: &LlmToolCall,
    loop_ctx: &AgenticLoopCtx,
    llm_messages: &mut Vec<LlmMessage>,
) {
    let subject = tool_call.arguments["subject"]
        .as_str()
        .unwrap_or("Untitled task")
        .to_string();
    let description = tool_call.arguments["description"]
        .as_str()
        .unwrap_or("")
        .to_string();

    let task = TaskItem {
        id: uuid::Uuid::new_v4().to_string(),
        subject: subject.clone(),
        status: "pending".to_string(),
        description,
    };
    let task_id = task.id.clone();

    let tasks_snapshot = {
        let mut stores = TASK_STORES.lock().unwrap_or_else(|e| {
                tracing::warn!("TASK_STORES mutex poisoned, recovering");
                e.into_inner()
            });
        let tasks = stores
            .entry(loop_ctx.stream_id.clone())
            .or_default();
        tasks.push(task);
        tasks.clone()
    };

    let _ = loop_ctx.app.emit(
        "chat-task-update",
        ChatTaskUpdatePayload {
            stream_id: loop_ctx.stream_id.clone(),
            session_id: loop_ctx.session_id.clone(),
            tasks: tasks_snapshot,
        },
    );

    let result_msg = format!("Created task '{}' (id: {})", subject, task_id);
    let _ = loop_ctx.app.emit(
        "chat-tool-result",
        ChatToolResultPayload {
            stream_id: loop_ctx.stream_id.clone(),
            session_id: loop_ctx.session_id.clone(),
            tool_call_id: tool_call.id.clone(),
            success: true,
            output: result_msg.clone(),
        },
    );
    llm_messages.push(LlmMessage {
        role: MessageRole::Tool,
        content: result_msg,
        tool_call_id: Some(tool_call.id.clone()),
        tool_calls: None,
        content_parts: None,
    });
}

fn handle_task_update(
    tool_call: &LlmToolCall,
    loop_ctx: &AgenticLoopCtx,
    llm_messages: &mut Vec<LlmMessage>,
) {
    let task_id = tool_call.arguments["task_id"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let new_status = tool_call.arguments["status"]
        .as_str()
        .unwrap_or("pending")
        .to_string();

    let result_msg = {
        let mut stores = TASK_STORES.lock().unwrap_or_else(|e| {
                tracing::warn!("TASK_STORES mutex poisoned, recovering");
                e.into_inner()
            });
        if let Some(tasks) = stores.get_mut(&loop_ctx.stream_id) {
            if let Some(task) = tasks.iter_mut().find(|t| t.id == task_id) {
                task.status = new_status.clone();
                let msg = format!("Updated task '{}' to {}", task.subject, new_status);

                let _ = loop_ctx.app.emit(
                    "chat-task-update",
                    ChatTaskUpdatePayload {
                        stream_id: loop_ctx.stream_id.clone(),
                        session_id: loop_ctx.session_id.clone(),
                        tasks: tasks.clone(),
                    },
                );
                msg
            } else {
                format!("Task not found: {}", task_id)
            }
        } else {
            "No tasks in this session".to_string()
        }
    };

    let _ = loop_ctx.app.emit(
        "chat-tool-result",
        ChatToolResultPayload {
            stream_id: loop_ctx.stream_id.clone(),
            session_id: loop_ctx.session_id.clone(),
            tool_call_id: tool_call.id.clone(),
            success: true,
            output: result_msg.clone(),
        },
    );
    llm_messages.push(LlmMessage {
        role: MessageRole::Tool,
        content: result_msg,
        tool_call_id: Some(tool_call.id.clone()),
        tool_calls: None,
        content_parts: None,
    });
}

fn handle_task_list(
    tool_call: &LlmToolCall,
    loop_ctx: &AgenticLoopCtx,
    llm_messages: &mut Vec<LlmMessage>,
) {
    let result_msg = {
        let stores = TASK_STORES.lock().unwrap_or_else(|e| {
                tracing::warn!("TASK_STORES mutex poisoned, recovering");
                e.into_inner()
            });
        if let Some(tasks) = stores.get(&loop_ctx.stream_id) {
            if tasks.is_empty() {
                "No tasks created yet.".to_string()
            } else {
                let mut out = format!("{} tasks:\n", tasks.len());
                for t in tasks {
                    let icon = match t.status.as_str() {
                        "completed" => "[x]",
                        "in_progress" => "[~]",
                        _ => "[ ]",
                    };
                    out.push_str(&format!("{} {} ({})\n", icon, t.subject, t.id));
                }
                out
            }
        } else {
            "No tasks created yet.".to_string()
        }
    };

    let _ = loop_ctx.app.emit(
        "chat-tool-result",
        ChatToolResultPayload {
            stream_id: loop_ctx.stream_id.clone(),
            session_id: loop_ctx.session_id.clone(),
            tool_call_id: tool_call.id.clone(),
            success: true,
            output: result_msg.clone(),
        },
    );
    llm_messages.push(LlmMessage {
        role: MessageRole::Tool,
        content: result_msg,
        tool_call_id: Some(tool_call.id.clone()),
        tool_calls: None,
        content_parts: None,
    });
}

fn handle_enter_plan_mode(
    tool_call: &LlmToolCall,
    loop_ctx: &AgenticLoopCtx,
    llm_messages: &mut Vec<LlmMessage>,
    plan_mode: &mut bool,
) {
    *plan_mode = true;
    let result_msg = "Entered plan mode. Only read-only tools are available now. Explore the codebase and use submit_plan when ready.";

    let _ = loop_ctx.app.emit(
        "chat-tool-result",
        ChatToolResultPayload {
            stream_id: loop_ctx.stream_id.clone(),
            session_id: loop_ctx.session_id.clone(),
            tool_call_id: tool_call.id.clone(),
            success: true,
            output: result_msg.to_string(),
        },
    );
    llm_messages.push(LlmMessage {
        role: MessageRole::Tool,
        content: result_msg.to_string(),
        tool_call_id: Some(tool_call.id.clone()),
        tool_calls: None,
        content_parts: None,
    });
}

async fn handle_submit_plan(
    tool_call: &LlmToolCall,
    loop_ctx: &AgenticLoopCtx,
    llm_messages: &mut Vec<LlmMessage>,
    plan_mode: &mut bool,
) {
    let summary = tool_call.arguments["summary"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let steps: Vec<String> = tool_call.arguments["steps"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let _ = loop_ctx.app.emit(
        "chat-plan-ready",
        ChatPlanReadyPayload {
            stream_id: loop_ctx.stream_id.clone(),
            session_id: loop_ctx.session_id.clone(),
            tool_call_id: tool_call.id.clone(),
            summary: summary.clone(),
            steps: steps.clone(),
        },
    );

    let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
    if let Ok(mut pending) = PENDING_PLAN_APPROVALS.lock() {
        pending.insert(tool_call.id.clone(), tx);
    }
    track_tool_call(&loop_ctx.stream_id, &tool_call.id);

    let emit_payload = ChatPlanReadyPayload {
        stream_id: loop_ctx.stream_id.clone(),
        session_id: loop_ctx.session_id.clone(),
        tool_call_id: tool_call.id.clone(),
        summary: summary.clone(),
        steps: steps.clone(),
    };
    let app = loop_ctx.app.clone();
    let approved = wait_with_retry(rx, Duration::from_secs(120), 3,
        || { let _ = app.emit("chat-plan-ready", emit_payload.clone()); },
    ).await.unwrap_or(false);

    let result_msg = if approved {
        *plan_mode = false;
        "Plan approved by user. Exiting plan mode — all tools are now available. Proceed with implementation.".to_string()
    } else {
        "Plan rejected by user. Revise your approach based on their feedback.".to_string()
    };

    emit_persist_push(
        loop_ctx,
        llm_messages,
        &tool_call.id,
        N::SUBMIT_PLAN,
        &tool_call.arguments,
        approved,
        result_msg,
    )
    .await;
}

// ── Permission enforcement ───────────────────────────────────────────

/// Check and enforce permissions. Returns true if tool execution should proceed.
async fn enforce_permission(
    tool_call: &LlmToolCall,
    loop_ctx: &AgenticLoopCtx,
    llm_messages: &mut Vec<LlmMessage>,
) -> bool {
    let resource = extract_tool_resource(&tool_call.name, &tool_call.arguments);
    let action = check_permission_action(
        &tool_call.name,
        &tool_call.arguments,
        &loop_ctx.stream_id,
        loop_ctx.dev_session_id.as_deref(),
        loop_ctx.session_id.as_deref(),
    );

    match action {
        venore_core::permissions::PermissionAction::Deny => {
            let denied_msg = format!(
                "Permission denied for tool '{}'. This tool is not allowed.",
                tool_call.name
            );
            let _ = loop_ctx.app.emit(
                "chat-tool-result",
                ChatToolResultPayload {
                    stream_id: loop_ctx.stream_id.clone(),
                    session_id: loop_ctx.session_id.clone(),
                    tool_call_id: tool_call.id.clone(),
                    success: false,
                    output: denied_msg.clone(),
                },
            );
            llm_messages.push(LlmMessage {
                role: MessageRole::Tool,
                content: denied_msg,
                tool_call_id: Some(tool_call.id.clone()),
                tool_calls: None,
                content_parts: None,
            });
            false
        }
        venore_core::permissions::PermissionAction::Ask => {
            let _ = loop_ctx.app.emit(
                "chat-tool-confirm",
                ChatToolConfirmPayload {
                    stream_id: loop_ctx.stream_id.clone(),
                    session_id: loop_ctx.session_id.clone(),
                    tool_call_id: tool_call.id.clone(),
                    tool_name: tool_call.name.clone(),
                    arguments: tool_call.arguments.clone(),
                    resource: resource.clone(),
                },
            );

            let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
            if let Ok(mut pending) = PENDING_APPROVALS.lock() {
                pending.insert(tool_call.id.clone(), tx);
            }
            track_tool_call(&loop_ctx.stream_id, &tool_call.id);

            let emit_payload = ChatToolConfirmPayload {
                stream_id: loop_ctx.stream_id.clone(),
                session_id: loop_ctx.session_id.clone(),
                tool_call_id: tool_call.id.clone(),
                tool_name: tool_call.name.clone(),
                arguments: tool_call.arguments.clone(),
                resource: resource.clone(),
            };
            let app = loop_ctx.app.clone();
            let approved = wait_with_retry(rx, Duration::from_secs(60), 3,
                || { let _ = app.emit("chat-tool-confirm", emit_payload.clone()); },
            ).await.unwrap_or(false);

            if !approved {
                let denied_msg = format!(
                    "User denied permission for '{}'.",
                    tool_call.name
                );
                let _ = loop_ctx.app.emit(
                    "chat-tool-result",
                    ChatToolResultPayload {
                        stream_id: loop_ctx.stream_id.clone(),
                        session_id: loop_ctx.session_id.clone(),
                        tool_call_id: tool_call.id.clone(),
                        success: false,
                        output: denied_msg.clone(),
                    },
                );
                llm_messages.push(LlmMessage {
                    role: MessageRole::Tool,
                    content: denied_msg,
                    tool_call_id: Some(tool_call.id.clone()),
                    tool_calls: None,
                    content_parts: None,
                });
                return false;
            }

            // Approved — cache for this session
            if let Ok(mut approvals) = SESSION_APPROVALS.lock() {
                let approval_key = loop_ctx.dev_session_id.clone()
                    .unwrap_or_else(|| loop_ctx.stream_id.clone());
                let set = approvals
                    .entry(approval_key)
                    .or_default();
                let key = format!(
                    "{}:{}",
                    tool_call.name,
                    resource.as_deref().unwrap_or("*")
                );
                set.insert(key);
            }
            true
        }
        venore_core::permissions::PermissionAction::Allow => true,
    }
}

// ── Post-processing ──────────────────────────────────────────────────

/// Post-process file edit tools: emit session:file-changed, auto-commit snapshot, fetch LSP diagnostics.
async fn post_process_file_edit(
    tool_call: &LlmToolCall,
    loop_ctx: &AgenticLoopCtx,
    tool_result: &mut tools::ToolExecutionResult,
) {
    // Emit session:file-changed
    if let (Some(ref dev_sid), Some(ref base_br), Some(ref wt_path)) =
        (&loop_ctx.dev_session_id, &loop_ctx.dev_session_base_branch, &loop_ctx.tool_project_path)
    {
        if let Some(file_path_str) = tool_call.arguments["file_path"].as_str() {
            let abs = std::path::Path::new(file_path_str);
            let relative = if abs.is_absolute() {
                abs.strip_prefix(wt_path)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| file_path_str.to_string())
            } else {
                file_path_str.to_string()
            };
            let relative = relative.replace('\\', "/");

            if let Some(diff) = venore_core::session::git_ops::compute_file_diff(
                std::path::Path::new(wt_path), base_br, &relative,
            ) {
                let _ = loop_ctx.app.emit(
                    "session:file-changed",
                    SessionFileChangedPayload {
                        dev_session_id: dev_sid.clone(),
                        filename: relative.clone(),
                        status: diff.status,
                        additions: diff.additions,
                        deletions: diff.deletions,
                        patch: diff.patch,
                    },
                );
            }

            // Auto-commit for snapshot/revert support
            let commit_msg = format!("venore: {} {}", tool_call.name, relative);
            if let Ok(hash) = venore_core::session::git_ops::auto_commit(
                std::path::Path::new(wt_path),
                &relative,
                &commit_msg,
            ) {
                let _ = loop_ctx.app.emit(
                    "chat-snapshot",
                    ChatSnapshotPayload {
                        stream_id: loop_ctx.stream_id.clone(),
                        session_id: loop_ctx.session_id.clone(),
                        tool_call_id: tool_call.id.clone(),
                        commit_hash: hash.clone(),
                    },
                );
                if let (Some(ref sid), Some(ref repo)) = (&loop_ctx.session_id, &loop_ctx.chat_repo) {
                    if let Err(e) = repo.save_snapshot(sid, &tool_call.id, &hash).await {
                        tracing::warn!("Failed to persist snapshot: {}", e);
                    }
                    // Update the tool call record with the commit hash
                    persist_tool_call_result(repo, &tool_call.id, true, "File edited (snapshot)", Some(&hash)).await;
                }
            }
        }
    }

    // Fetch LSP diagnostics
    if let Some(ref proj_path) = loop_ctx.tool_project_path {
        append_lsp_diagnostics(tool_call, tool_result, proj_path).await;
    }
}

/// Post-process terminal command: wait for output and auto-read. Returns true if handled (skip normal push).
pub(super) async fn post_process_terminal(
    tool_call: &LlmToolCall,
    active_terminal_id: Option<&str>,
    tool_result: &tools::ToolExecutionResult,
    llm_messages: &mut Vec<LlmMessage>,
) -> bool {
    if let (Some(tid), Some(baseline)) = (active_terminal_id, tool_result.baseline) {
        venore_core::tools::wait_for_output(tid, baseline, 15).await;

        let terminal_output = {
            let mgr = TerminalSessionManager::global();
            let guard = mgr.lock();
            guard.ok().and_then(|m| m.get_output_after(tid, baseline, 50).ok())
        };
        if let Some(output) = terminal_output {
            let combined = format!("{}\n\nTerminal output:\n{}", tool_result.output, output);
            let llm_content = truncate_for_llm(&combined, &tool_call.name);
            llm_messages.push(LlmMessage {
                role: MessageRole::Tool,
                content: llm_content,
                tool_call_id: Some(tool_call.id.clone()),
                tool_calls: None,
                content_parts: None,
            });
            return true;
        }
    }
    false
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_for_llm_short_output_unchanged() {
        let output = "Hello, world!";
        assert_eq!(truncate_for_llm(output, "read_file"), output);
    }

    #[test]
    fn truncate_for_llm_long_output_truncated() {
        let output = "x".repeat(20_000);
        let result = truncate_for_llm(&output, "read_file");
        assert!(result.len() < output.len());
        assert!(result.contains("[..."));
        assert!(result.contains("chars truncated...]"));
    }

    #[test]
    fn truncate_for_llm_excluded_tool_untouched() {
        let output = "x".repeat(20_000);
        let result = truncate_for_llm(&output, "ask_user");
        assert_eq!(result.len(), output.len());
    }
}
