//! DTOs for Dashboard commands

use serde::{Deserialize, Serialize};

// =============================================================================
// Requests
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetProjectDashboardRequest {
    pub project_path: String,
}

// =============================================================================
// Responses
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDashboardResponse {
    pub stats: ProjectStatsDto,
    pub modules: Vec<ModuleSummaryDto>,
    pub orphan_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectStatsDto {
    pub total_modules: usize,
    pub total_connections: usize,
    pub fresh_count: usize,
    pub stale_count: usize,
    pub missing_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleSummaryDto {
    pub name: String,
    pub path: String,
    pub file_count: usize,
    pub dependency_count: usize,
    pub dependent_count: usize,
    pub context_status: String,
    pub generated_at: Option<String>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub context_path: Option<String>,
    pub files: Vec<String>,
}
