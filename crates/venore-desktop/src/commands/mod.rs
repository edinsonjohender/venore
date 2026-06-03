//! Tauri commands
//!
//! Thin commands that delegate to venore-core.
//! No business logic should live here.

pub mod agents;
pub mod ai_connections;
pub mod chat;
pub mod memory;
pub mod cloud;
pub mod skills;
pub mod prompts;
pub mod dashboard;
pub mod dto;
pub mod knowledge;
pub mod research;
pub mod mesh;
pub mod editor;
pub mod github;
pub mod ocean;
pub mod currents;
pub mod pending_writes;
pub mod projects;
pub mod context;
pub mod context_updater;
pub mod llm;
pub mod rag;
pub mod session;
pub mod snapshot;
pub mod system;
pub mod terminal;
pub mod wizard;

use crate::utils::CommandResult;

/// Health check command
#[tauri::command]
pub async fn health() -> CommandResult<String> {
    tracing::info!("health check");
    CommandResult::ok("Venore Desktop is running".to_string())
}
