//! Chat action commands — approve/deny tool calls, respond to agent, plan approval, stop stream.

use crate::utils::CommandResult;

use super::state::{
    ACTIVE_STREAMS, ACTIVE_SUB_AGENTS, PENDING_APPROVALS, PENDING_PLAN_APPROVALS,
    PENDING_USER_RESPONSES, SESSION_APPROVALS, SESSION_STREAMS, TASK_STORES,
};

/// Approve or deny a tool call from the AI agent.
/// When `allow_session` is true and approved, caches `tool_name:*` so the same
/// tool is auto-approved for the rest of the dev session.
#[tauri::command]
pub async fn approve_tool_call(
    tool_call_id: String,
    approved: bool,
    allow_session: Option<bool>,
    session_id: Option<String>,
    tool_name: Option<String>,
) -> CommandResult<()> {
    if approved && allow_session.unwrap_or(false) {
        if let (Some(sid), Some(tname)) = (&session_id, &tool_name) {
            if let Ok(mut approvals) = SESSION_APPROVALS.lock() {
                let set = approvals.entry(sid.clone()).or_default();
                set.insert(format!("{}:*", tname));
                tracing::info!(tool = %tname, session = %sid, "session-wide approval cached");
            }
        }
    }

    let sender = match PENDING_APPROVALS.lock() {
        Ok(mut pending) => pending.remove(&tool_call_id),
        Err(e) => {
            tracing::error!("PENDING_APPROVALS mutex poisoned: {}", e);
            None
        }
    };
    if let Some(sender) = sender {
        let _ = sender.send(approved);
    }
    CommandResult::ok(())
}

/// Clear session approvals for a dev session (call when session changes or is deleted).
#[tauri::command]
pub async fn clear_session_approvals(session_id: String) -> CommandResult<()> {
    if let Ok(mut approvals) = SESSION_APPROVALS.lock() {
        approvals.remove(&session_id);
        tracing::info!(session = %session_id, "session approvals cleared");
    }
    CommandResult::ok(())
}

/// Respond to an ask_user tool call from the AI agent.
#[tauri::command]
pub async fn respond_to_agent(tool_call_id: String, response: String) -> CommandResult<()> {
    let sender = match PENDING_USER_RESPONSES.lock() {
        Ok(mut pending) => pending.remove(&tool_call_id),
        Err(e) => {
            tracing::error!("PENDING_USER_RESPONSES mutex poisoned: {}", e);
            None
        }
    };
    if let Some(sender) = sender {
        let _ = sender.send(response);
    }
    CommandResult::ok(())
}

/// Approve or reject a plan from the AI agent.
#[tauri::command]
pub async fn approve_plan(tool_call_id: String, approved: bool) -> CommandResult<()> {
    let sender = match PENDING_PLAN_APPROVALS.lock() {
        Ok(mut pending) => pending.remove(&tool_call_id),
        Err(e) => {
            tracing::error!("PENDING_PLAN_APPROVALS mutex poisoned: {}", e);
            None
        }
    };
    if let Some(sender) = sender {
        let _ = sender.send(approved);
    }
    CommandResult::ok(())
}

/// Stop an active chat stream.
#[tauri::command]
pub async fn stop_chat_stream(stream_id: String) -> CommandResult<()> {
    let mut streams = match ACTIVE_STREAMS.lock() {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("ACTIVE_STREAMS mutex poisoned: {}", e);
            return CommandResult::ok(());
        }
    };
    if let Some(handle) = streams.remove(&stream_id) {
        handle.abort();
        tracing::info!("Aborted chat stream: {}", stream_id);
        drop(streams); // release lock before cleanup
        // Clean orphaned pending channels + per-stream state
        super::agentic_loop::cleanup_pending_channels(&stream_id);
        if let Ok(mut s) = TASK_STORES.lock() { s.remove(&stream_id); }
        if let Ok(mut s) = ACTIVE_SUB_AGENTS.lock() { s.remove(&stream_id); }
        // Clean session → stream mapping (find by stream_id value)
        if let Ok(mut m) = SESSION_STREAMS.lock() {
            m.retain(|_, v| v != &stream_id);
        }
        CommandResult::ok(())
    } else {
        tracing::warn!("Stream not found for abort: {}", stream_id);
        CommandResult::ok(())
    }
}

/// Check if a session has an active stream (for window reconnection).
#[tauri::command]
pub async fn get_session_stream_status(
    session_id: String,
) -> CommandResult<Option<String>> {
    let stream_id = SESSION_STREAMS
        .lock()
        .ok()
        .and_then(|m| m.get(&session_id).cloned());

    // Verify the stream is actually still active
    if let Some(ref sid) = stream_id {
        let is_active = ACTIVE_STREAMS
            .lock()
            .ok()
            .map(|m| m.contains_key(sid))
            .unwrap_or(false);
        if !is_active {
            // Stale entry — clean it up
            if let Ok(mut m) = SESSION_STREAMS.lock() {
                m.remove(&session_id);
            }
            return CommandResult::ok(None);
        }
    }

    CommandResult::ok(stream_id)
}
