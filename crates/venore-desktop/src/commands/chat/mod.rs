//! Chat Tauri commands
//!
//! Exposes chat streaming and session management to the frontend.
//! Supports an agentic loop: the AI can call tools (e.g. terminal commands)
//! and the loop continues until no more tool calls are made.
//! Terminal management is fully autonomous — the AI never sees terminal IDs.

pub mod dto;
mod state;
pub(crate) mod helpers;
mod stream;
mod agentic_loop;
mod tool_dispatch;
mod sub_agent;
mod session;
mod actions;

// Re-export all #[tauri::command] functions + their Tauri-generated __cmd__ companions.
// Wildcard re-exports ensure the hidden items are accessible from commands::chat::*.
pub use stream::*;
pub use actions::*;
pub use session::*;

// Re-export DTOs used by other modules
pub use dto::*;
