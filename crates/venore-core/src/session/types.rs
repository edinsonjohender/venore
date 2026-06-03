//! Session types
//!
//! Domain types for branch-per-session workflow.

use serde::{Deserialize, Serialize};

/// Status of a work session
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SessionStatus {
    Active,
    Completed,
    Abandoned,
}

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionStatus::Active => "active",
            SessionStatus::Completed => "completed",
            SessionStatus::Abandoned => "abandoned",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "active" => Some(SessionStatus::Active),
            "completed" => Some(SessionStatus::Completed),
            "abandoned" => Some(SessionStatus::Abandoned),
            _ => None,
        }
    }
}

/// A work session tied to a git branch (isolated via git worktree)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub objective: String,
    pub project_id: String,
    pub base_branch: String,
    pub session_branch: String,
    pub worktree_path: String,
    pub status: SessionStatus,
    pub created_at: String,
    pub updated_at: String,
}

/// A file changed between base and session branches.
/// Matches GitHubPrFileDto shape for DiffViewer/FileTree reuse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffFile {
    pub filename: String,
    /// "added" | "modified" | "removed" | "renamed"
    pub status: String,
    pub additions: u32,
    pub deletions: u32,
    pub patch: Option<String>,
}

/// A commit on the session branch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub hash: String,
    pub short_hash: String,
    pub message: String,
    pub author: String,
    pub date: String,
}
