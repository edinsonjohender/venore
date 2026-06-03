//! Session module — Branch-per-session workflow
//!
//! Each session creates an isolated git branch for tracking agent work.

pub mod types;
pub mod git_ops;
pub mod repository;

pub use types::{Session, SessionStatus, DiffFile, CommitInfo};
pub use repository::SessionRepository;
