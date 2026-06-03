//! Terminal Tauri commands
//!
//! Exposes PTY terminal sessions to the frontend.
//! Uses portable-pty (blocking I/O) with a tokio::task::spawn_blocking read loop.

use std::io::Read;
use std::sync::{Arc, Mutex};

use tauri::AppHandle;
use tauri::Emitter;
use tracing::{debug, error, info};

use venore_core::terminal::TerminalSessionManager;

use crate::utils::CommandResult;
use super::dto::terminal::{
    KillTerminalRequest, ListTerminalsResponse, ResizeTerminalRequest, SpawnTerminalRequest,
    SpawnTerminalResponse, TerminalDeadPayload, TerminalOutputPayload, WriteTerminalRequest,
};

// =============================================================================
// Public helper — reusable read-loop for terminal output
// =============================================================================

/// Start the blocking read-loop that pipes PTY output to both the xterm frontend
/// (via `terminal:output` event) and the AI tool output buffer.
///
/// Used by `spawn_terminal` (user-initiated) and by `chat.rs` (AI auto-spawn).
pub fn start_terminal_read_loop(
    app: AppHandle,
    terminal_id: String,
    reader: Arc<Mutex<Box<dyn Read + Send>>>,
) {
    let tid = terminal_id;
    tokio::task::spawn_blocking(move || {
        let mut buf = [0u8; 4096];
        loop {
            let n = {
                let mut r = match reader.lock() {
                    Ok(r) => r,
                    Err(e) => {
                        error!(terminal_id = %tid, error = %e, "reader mutex poisoned");
                        break;
                    }
                };
                match r.read(&mut buf) {
                    Ok(0) => {
                        info!(terminal_id = %tid, "terminal read loop: EOF");
                        break;
                    }
                    Ok(n) => n,
                    Err(e) => {
                        debug!(terminal_id = %tid, error = %e, "terminal read loop ended");
                        break;
                    }
                }
            };

            venore_core::terminal::debug::log(&tid, "read", &buf[..n]);

            let data = String::from_utf8_lossy(&buf[..n]).to_string();

            // Feed output buffer for AI tools
            {
                let mgr = TerminalSessionManager::global();
                let _ = mgr.lock().map(|mut m| m.append_output(&tid, &data));
            }

            if let Err(e) = app.emit(
                "terminal:output",
                TerminalOutputPayload {
                    terminal_id: tid.clone(),
                    data,
                },
            ) {
                error!(terminal_id = %tid, error = %e, "failed to emit terminal output");
                break;
            }
        }

        // Cleanup: remove the dead session from the manager
        {
            let mgr = TerminalSessionManager::global();
            let _ = mgr.lock().map(|mut m| m.remove_dead_session(&tid));
        }

        // Notify frontend that this terminal is dead
        let _ = app.emit(
            "terminal:dead",
            TerminalDeadPayload {
                terminal_id: tid.clone(),
            },
        );
        info!(terminal_id = %tid, "terminal dead — cleaned up");
    });
}

// =============================================================================
// Commands
// =============================================================================

/// Spawn a new terminal PTY session and start the output read-loop.
#[tauri::command]
pub async fn spawn_terminal(
    app: AppHandle,
    request: SpawnTerminalRequest,
) -> CommandResult<SpawnTerminalResponse> {
    let cwd = request.cwd.unwrap_or_else(|| {
        dirs::home_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string())
    });
    let cols = request.cols.unwrap_or(80);
    let rows = request.rows.unwrap_or(24);
    let label = request.label;

    // Spawn session and get the reader (without holding the manager lock)
    let (terminal_id, reader) = {
        let manager = TerminalSessionManager::global();
        let mut guard = match manager.lock() {
            Ok(g) => g,
            Err(_) => return CommandResult::err(
                venore_core::error::VenoreError::TerminalError("terminal manager mutex poisoned".into())
            ),
        };
        match guard.spawn(&cwd, cols, rows, label.as_deref()) {
            Ok(result) => result,
            Err(e) => return CommandResult::err(e),
        }
    };

    // Start blocking read-loop in a background thread
    start_terminal_read_loop(app, terminal_id.clone(), reader);

    CommandResult::ok(SpawnTerminalResponse { terminal_id })
}

/// Write user keystrokes to a terminal session.
#[tauri::command]
pub async fn write_terminal(request: WriteTerminalRequest) -> CommandResult<()> {
    let result: Result<(), venore_core::error::VenoreError> = (|| {
        let manager = TerminalSessionManager::global();
        let guard = manager.lock()
            .map_err(|_| venore_core::error::VenoreError::TerminalError("terminal manager mutex poisoned".into()))?;
        guard.write(&request.terminal_id, request.data.as_bytes())
    })();
    result.into()
}

/// Resize a terminal session.
#[tauri::command]
pub async fn resize_terminal(request: ResizeTerminalRequest) -> CommandResult<()> {
    let result: Result<(), venore_core::error::VenoreError> = (|| {
        let manager = TerminalSessionManager::global();
        let mut guard = manager.lock()
            .map_err(|_| venore_core::error::VenoreError::TerminalError("terminal manager mutex poisoned".into()))?;
        guard.resize(&request.terminal_id, request.cols, request.rows)
    })();
    result.into()
}

/// Kill and remove a terminal session.
#[tauri::command]
pub async fn kill_terminal(request: KillTerminalRequest) -> CommandResult<()> {
    let result: Result<(), venore_core::error::VenoreError> = (|| {
        let manager = TerminalSessionManager::global();
        let mut guard = manager.lock()
            .map_err(|_| venore_core::error::VenoreError::TerminalError("terminal manager mutex poisoned".into()))?;
        guard.kill(&request.terminal_id)
    })();
    result.into()
}

/// List all active terminal session IDs.
#[tauri::command]
pub async fn list_terminals() -> CommandResult<ListTerminalsResponse> {
    let result: Result<ListTerminalsResponse, venore_core::error::VenoreError> = (|| {
        let manager = TerminalSessionManager::global();
        let guard = manager.lock()
            .map_err(|_| venore_core::error::VenoreError::TerminalError("terminal manager mutex poisoned".into()))?;
        Ok(ListTerminalsResponse {
            terminal_ids: guard.list(),
        })
    })();
    result.into()
}
