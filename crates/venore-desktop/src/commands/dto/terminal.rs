//! Terminal DTOs — Request/Response types for terminal commands

use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct SpawnTerminalRequest {
    pub cwd: Option<String>,
    pub cols: Option<u16>,
    pub rows: Option<u16>,
    pub label: Option<String>,
}

#[derive(Serialize)]
pub struct SpawnTerminalResponse {
    pub terminal_id: String,
}

#[derive(Deserialize)]
pub struct WriteTerminalRequest {
    pub terminal_id: String,
    pub data: String,
}

#[derive(Deserialize)]
pub struct ResizeTerminalRequest {
    pub terminal_id: String,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Deserialize)]
pub struct KillTerminalRequest {
    pub terminal_id: String,
}

/// Event payload emitted via `app.emit("terminal:output", ...)`
#[derive(Clone, Serialize)]
pub struct TerminalOutputPayload {
    pub terminal_id: String,
    pub data: String,
}

/// Event payload emitted when the AI auto-spawns a terminal.
#[derive(Clone, Serialize)]
pub struct TerminalAiSpawnedPayload {
    pub terminal_id: String,
}

/// Event payload emitted when a terminal process dies (EOF/error in read-loop).
#[derive(Clone, Serialize)]
pub struct TerminalDeadPayload {
    pub terminal_id: String,
}

/// Event payload emitted when a session-bound terminal is auto-spawned.
#[derive(Clone, Serialize)]
pub struct TerminalSessionSpawnedPayload {
    pub terminal_id: String,
    pub dev_session_id: String,
    pub label: String,
}

/// Response for listing active terminal sessions.
#[derive(Serialize)]
pub struct ListTerminalsResponse {
    pub terminal_ids: Vec<String>,
}
