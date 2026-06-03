//! Project Memory domain model

use serde::{Deserialize, Serialize};

/// Compact knowledge block for a project, injected into the LLM system prompt.
///
/// One memory per project (UNIQUE on project_id). Stores identity, goals,
/// conventions, architecture, response language, and a condensed summary
/// of the .context.md so the full file doesn't dilute the prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMemory {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub description: String,
    pub state: String,
    pub team_size: String,
    pub goals: Vec<String>,
    pub architecture: String,
    pub tech_debt: String,
    pub response_language: String,
    pub conventions: Vec<String>,
    pub project_summary: String,
    pub created_at: String,
    pub updated_at: String,
}
