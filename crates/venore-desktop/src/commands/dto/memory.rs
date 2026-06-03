//! Memory DTOs — Request/Response types for project memory commands

use serde::{Deserialize, Serialize};

// =============================================================================
// Request DTOs
// =============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProjectMemoryRequest {
    pub project_id: String,
    pub name: String,
    pub description: String,
    pub state: String,
    pub team_size: String,
    pub goals: Vec<String>,
    pub architecture: String,
    #[serde(default)]
    pub tech_debt: String,
    pub response_language: String,
    pub conventions: Vec<String>,
    pub project_summary: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegenerateSummaryRequest {
    pub project_id: String,
    pub project_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateMemoryRequest {
    pub project_path: String,
    /// User-supplied description (from wizard Step 1). The LLM respects it instead
    /// of writing a new one when present.
    #[serde(default)]
    pub user_description: Option<String>,
    /// User-supplied architecture note (from wizard Step 1).
    #[serde(default)]
    pub user_architecture: Option<String>,
    /// User-supplied tech debt note (from wizard Step 1).
    #[serde(default)]
    pub user_tech_debt: Option<String>,
    /// Module names detected by the wizard's index pipeline (Step 2). Gives the
    /// LLM real structural context instead of guessing from filenames.
    #[serde(default)]
    pub detected_modules: Vec<String>,
    /// Analysis depth level chosen by the user in Step 2.
    /// Accepts: "minimal" | "normal" | "detailed" | "expert". Defaults to "normal".
    #[serde(default)]
    pub depth_level: Option<String>,
    /// Iterative-refinement feedback. When present together with
    /// `previous_draft`, the LLM is told to refine that draft instead of
    /// generating one from scratch. Lets the user steer the analysis
    /// without typing the result themselves.
    #[serde(default)]
    pub user_feedback: Option<String>,
    /// The draft the LLM produced on the previous run. Sent back so the
    /// model can preserve what was correct and only change what the
    /// `user_feedback` asked to change.
    #[serde(default)]
    pub previous_draft: Option<GenerateMemoryResponse>,
}

/// LLM-generated structured fields for a project memory draft.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateMemoryResponse {
    pub description: String,
    pub state: String,
    pub goals: Vec<String>,
    pub architecture: String,
    #[serde(default)]
    pub tech_debt: String,
    pub project_summary: String,
}

// =============================================================================
// Response DTO
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMemoryDto {
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

impl From<venore_core::memory::ProjectMemory> for ProjectMemoryDto {
    fn from(m: venore_core::memory::ProjectMemory) -> Self {
        Self {
            id: m.id,
            project_id: m.project_id,
            name: m.name,
            description: m.description,
            state: m.state,
            team_size: m.team_size,
            goals: m.goals,
            architecture: m.architecture,
            tech_debt: m.tech_debt,
            response_language: m.response_language,
            conventions: m.conventions,
            project_summary: m.project_summary,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}
