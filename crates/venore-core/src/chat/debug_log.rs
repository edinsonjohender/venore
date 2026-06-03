//! Chat debug log — append-only JSONL trace of every chat event.
//!
//! Purpose: give an external observer (me, the Claude-Code agent helping
//! Edinson, plus future tooling) a machine-readable record of what happened
//! inside the Venore chat, without having to query SQLite or read terminal
//! captures.
//!
//! Path resolution (first match wins):
//!   1. `$VENORE_CHAT_DEBUG_LOG` if set
//!   2. `%TEMP%/venore-dev/chat-debug.jsonl` in debug builds
//!   3. `~/.venore/chat-debug.jsonl` in release builds
//!
//! Format: one JSON object per line, `\n` terminated. Each event has at
//! least `type`, `session_id`, and `ts` (ISO8601). Failures to write are
//! logged via `tracing::warn!` and never propagate to the chat path —
//! losing a debug line must not break the user's session.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use once_cell::sync::Lazy;
use serde::Serialize;

/// One event in the chat debug log. The `type` discriminator is emitted
/// in `snake_case` so the file is greppable by the keyword we care about
/// (`tool_call`, `tool_result`, `user_message`, `assistant_message`).
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatDebugEvent {
    UserMessage {
        session_id: String,
        content: String,
        ts: String,
    },
    AssistantMessage {
        session_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        ts: String,
    },
    ToolCall {
        session_id: String,
        name: String,
        arguments: serde_json::Value,
        ts: String,
    },
    ToolResult {
        session_id: String,
        name: String,
        success: bool,
        output: String,
        duration_ms: u128,
        ts: String,
    },
    /// Emitted once per chat turn with the resolved project_kind, tool
    /// inventory size, and the tag that explains where the tool list came
    /// from ("mode-knowledge", "mode-code", "fallback-hardcoded-code", ...).
    /// Lets an external auditor confirm the AI saw the expected toolset.
    SessionInit {
        session_id: String,
        project_path: Option<String>,
        project_kind: String,
        tool_count: usize,
        tool_source: String,
        tool_names: Vec<String>,
        ts: String,
    },
}

/// Lazy-initialized handle. `None` means "we tried to open the file and
/// failed" — subsequent calls log a warning and skip without retry.
static LOG_FILE: Lazy<Mutex<Option<File>>> = Lazy::new(|| Mutex::new(open_log_file()));

fn resolve_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("VENORE_CHAT_DEBUG_LOG") {
        return Some(PathBuf::from(p));
    }
    let dir = if cfg!(debug_assertions) {
        std::env::temp_dir().join("venore-dev")
    } else {
        dirs::home_dir()?.join(".venore")
    };
    if !dir.exists() {
        let _ = std::fs::create_dir_all(&dir);
    }
    Some(dir.join("chat-debug.jsonl"))
}

fn open_log_file() -> Option<File> {
    let path = resolve_path()?;
    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(f) => {
            tracing::info!(path = %path.display(), "Chat debug log open");
            Some(f)
        }
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "Could not open chat debug log");
            None
        }
    }
}

/// Append one event. Failures are swallowed (with a warning) — debug
/// logging must never break the chat path.
pub fn log_event(event: ChatDebugEvent) {
    let line = match serde_json::to_string(&event) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "Could not serialize chat debug event");
            return;
        }
    };

    let Ok(mut guard) = LOG_FILE.lock() else {
        return;
    };
    let Some(file) = guard.as_mut() else {
        return;
    };

    if let Err(e) = writeln!(file, "{}", line) {
        tracing::warn!(error = %e, "Failed writing chat debug log");
    }
}

/// ISO8601 timestamp for event records. Centralized so every event uses
/// the exact same format and downstream filters (`grep "2026-05-08"`)
/// behave predictably.
pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Resolving the path with the env override returns it verbatim.
    /// The static LOG_FILE has already been initialized by the time tests
    /// run (lazy on first call), so we exercise resolve_path directly to
    /// avoid contaminating other tests' write target.
    #[test]
    fn resolve_path_honours_env_override() {
        std::env::set_var("VENORE_CHAT_DEBUG_LOG", "/tmp/some-path.jsonl");
        let p = resolve_path().unwrap();
        assert_eq!(p.to_string_lossy(), "/tmp/some-path.jsonl");
        std::env::remove_var("VENORE_CHAT_DEBUG_LOG");
    }

    /// Each event variant serializes with its expected discriminator and
    /// includes the type-specific fields. We test the serialized JSON
    /// shape directly so downstream consumers can count on it.
    #[test]
    fn user_message_event_serializes_with_type_field() {
        let ev = ChatDebugEvent::UserMessage {
            session_id: "s1".into(),
            content: "hi".into(),
            ts: "2026-05-08T00:00:00Z".into(),
        };
        let json = serde_json::to_value(&ev).unwrap();
        assert_eq!(json["type"], "user_message");
        assert_eq!(json["session_id"], "s1");
        assert_eq!(json["content"], "hi");
    }

    #[test]
    fn tool_result_event_includes_duration_and_success() {
        let ev = ChatDebugEvent::ToolResult {
            session_id: "s1".into(),
            name: "read_file".into(),
            success: true,
            output: "ok".into(),
            duration_ms: 42,
            ts: "2026-05-08T00:00:00Z".into(),
        };
        let json = serde_json::to_value(&ev).unwrap();
        assert_eq!(json["type"], "tool_result");
        assert_eq!(json["name"], "read_file");
        assert_eq!(json["success"], true);
        assert_eq!(json["duration_ms"], 42);
    }
}
