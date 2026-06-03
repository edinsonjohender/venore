//! # Venore Desktop (Tauri)
//!
//! Thin wrapper around venore-core for desktop application.
//! This crate must NOT contain business logic.
//! It should only:
//! 1. Expose Tauri commands
//! 2. Delegate to venore-core
//! 3. Manage Tauri state

pub mod ai_connections;
pub mod commands;
pub mod notifications;
pub mod state;
pub mod utils;

// Re-export for use from main.rs
pub use commands::*;
pub use state::{AppState, LazyAppState};
