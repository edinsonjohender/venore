//! Terminal module — PTY session management
//!
//! Provides embedded terminal sessions using portable-pty.

pub mod debug;
pub mod manager;
pub mod session;

pub use manager::TerminalSessionManager;
pub use session::TerminalSession;
