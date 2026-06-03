//! LSP language server integration — post-edit diagnostics
//!
//! Spawns and manages LSP servers (typescript-language-server, rust-analyzer)
//! to provide real-time compiler diagnostics after file edits.
//! The diagnostics are appended to tool outputs so the AI agent can self-correct.

pub mod client;
pub mod config;
pub mod diagnostics;
pub mod manager;
pub mod server;

pub use diagnostics::{fetch_post_edit_diagnostics, DiagnosticEntry, DiagnosticSeverity};
pub use manager::LspManager;
