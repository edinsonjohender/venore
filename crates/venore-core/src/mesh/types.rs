//! Mesh types — peer registration and discovery data structures

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Lightweight project profile for mesh awareness.
/// Built from AnalysisOutput on disk — no extra computation needed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectProfile {
    /// Main programming language (e.g. "TypeScript", "Rust")
    pub language: Option<String>,
    /// Technologies detected (e.g. ["TypeScript", "React", "Node.js"])
    pub technologies: Vec<String>,
    /// Module names in the project (e.g. ["auth", "users", "payments"])
    pub module_names: Vec<String>,
    /// Total files in the project
    pub total_files: usize,
    /// Total modules detected
    pub total_modules: usize,
    /// Short description (first paragraph from root .context.md)
    pub description: Option<String>,
}

/// What gets written to ~/.venore/mesh/{project_id}.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerRegistration {
    pub project_id: String,
    pub project_name: String,
    pub project_path: String,
    pub pid: u32,
    pub port: u16,
    pub registered_at: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    /// Rich project profile (None if analysis hasn't run yet)
    #[serde(default)]
    pub profile: Option<ProjectProfile>,
}

/// What the frontend receives (omits internal fields)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub project_id: String,
    pub project_name: String,
    pub project_path: String,
    pub port: u16,
    pub is_alive: bool,
    /// Rich project profile (None if not available)
    #[serde(default)]
    pub profile: Option<ProjectProfile>,
}
