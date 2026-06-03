//! Tauri commands for the AI connection registry.
//!
//! Every mutation broadcasts the full snapshot via `ai-connection:update`
//! so all windows stay in sync. The frontend store mirrors this — there is
//! no per-window cached state.

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::ai_connections::{AiConnectionRegistry, AiConnectionTarget};
use crate::state::LazyAppState;
use crate::utils::{CommandResult, StateCommandResult};

const UPDATE_EVENT: &str = "ai-connection:update";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConnectionDto {
    pub id: String,
    pub active: bool,
    pub window_label: String,
    /// What kind of entity is attached + its resolution payload. Carried
    /// to the frontend so chat-input chips can render typed badges and
    /// ChatInput knows what's about to travel as context.
    pub target: AiConnectionTarget,
}

fn snapshot_dto(reg: &AiConnectionRegistry) -> Vec<AiConnectionDto> {
    reg.snapshot()
        .into_iter()
        .map(|r| AiConnectionDto {
            id: r.id,
            active: r.active,
            window_label: r.window_label,
            target: r.target,
        })
        .collect()
}

fn broadcast(app: &AppHandle, reg: &AiConnectionRegistry) {
    let snap = snapshot_dto(reg);
    if let Err(e) = app.emit(UPDATE_EVENT, &snap) {
        tracing::warn!(error = %e, "failed to emit ai-connection:update");
    }
}

#[tauri::command]
pub async fn list_ai_connections(
    state: State<'_, LazyAppState>,
) -> StateCommandResult<Vec<AiConnectionDto>> {
    Ok(CommandResult::ok(snapshot_dto(&state.ai_connections)))
}

#[tauri::command]
pub async fn register_ai_connection(
    id: String,
    target: AiConnectionTarget,
    window_label: Option<String>,
    state: State<'_, LazyAppState>,
    app: AppHandle,
) -> StateCommandResult<Vec<AiConnectionDto>> {
    let label = window_label.unwrap_or_default();
    state.ai_connections.register(&id, target, &label);
    broadcast(&app, &state.ai_connections);
    Ok(CommandResult::ok(snapshot_dto(&state.ai_connections)))
}

#[tauri::command]
pub async fn unregister_ai_connection(
    id: String,
    state: State<'_, LazyAppState>,
    app: AppHandle,
) -> StateCommandResult<Vec<AiConnectionDto>> {
    state.ai_connections.unregister(&id);
    broadcast(&app, &state.ai_connections);
    Ok(CommandResult::ok(snapshot_dto(&state.ai_connections)))
}

#[tauri::command]
pub async fn toggle_ai_connection(
    id: String,
    state: State<'_, LazyAppState>,
    app: AppHandle,
) -> StateCommandResult<Vec<AiConnectionDto>> {
    state.ai_connections.toggle(&id);
    broadcast(&app, &state.ai_connections);
    Ok(CommandResult::ok(snapshot_dto(&state.ai_connections)))
}

#[tauri::command]
pub async fn disconnect_all_ai_connections(
    state: State<'_, LazyAppState>,
    app: AppHandle,
) -> StateCommandResult<Vec<AiConnectionDto>> {
    state.ai_connections.disconnect_all();
    broadcast(&app, &state.ai_connections);
    Ok(CommandResult::ok(snapshot_dto(&state.ai_connections)))
}
