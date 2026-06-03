//! Pipeline DTOs — Request/Response types for pipeline execution commands

use serde::{Deserialize, Serialize};

// =============================================================================
// Requests
// =============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartPipelineRequest {
    pub project_path: String,
    pub pr_number: u64,
    pub pr_title: String,
    pub team_id: Option<String>,
}

// =============================================================================
// Responses
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartPipelineResponse {
    pub run_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineRunDto {
    pub id: String,
    pub team_id: String,
    pub team_name: String,
    pub task_type: String,
    pub title: String,
    pub status: String,
    pub pr_number: Option<u64>,
    pub project_path: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub duration_ms: u64,
    pub total_tokens: u32,
    pub created_at: String,
    pub pr_author: Option<String>,
    pub pr_author_avatar: Option<String>,
    pub pr_additions: Option<u64>,
    pub pr_deletions: Option<u64>,
    pub pr_changed_files: Option<u64>,
    pub depth_level: Option<String>,
}

impl From<venore_core::agents::PipelineRun> for PipelineRunDto {
    fn from(r: venore_core::agents::PipelineRun) -> Self {
        Self {
            id: r.id,
            team_id: r.team_id,
            team_name: r.team_name,
            task_type: r.task_type,
            title: r.title,
            status: r.status.as_str().to_string(),
            pr_number: r.pr_number,
            project_path: r.project_path,
            started_at: r.started_at,
            finished_at: r.finished_at,
            duration_ms: r.duration_ms,
            total_tokens: r.total_tokens,
            created_at: r.created_at,
            pr_author: r.pr_author,
            pr_author_avatar: r.pr_author_avatar,
            pr_additions: r.pr_additions,
            pr_deletions: r.pr_deletions,
            pr_changed_files: r.pr_changed_files,
            depth_level: r.depth_level,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineStepDto {
    pub id: String,
    pub run_id: String,
    pub profile_id: String,
    pub profile_name: String,
    pub stage: String,
    pub status: String,
    pub input_context: String,
    pub output: String,
    pub provider: String,
    pub model: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub duration_ms: u64,
    pub error: Option<String>,
    pub step_order: u32,
    pub started_at: String,
    pub finished_at: Option<String>,
}

impl From<venore_core::agents::PipelineStep> for PipelineStepDto {
    fn from(s: venore_core::agents::PipelineStep) -> Self {
        Self {
            id: s.id,
            run_id: s.run_id,
            profile_id: s.profile_id,
            profile_name: s.profile_name,
            stage: s.stage,
            status: s.status.as_str().to_string(),
            input_context: s.input_context,
            output: s.output,
            provider: s.provider,
            model: s.model,
            prompt_tokens: s.prompt_tokens,
            completion_tokens: s.completion_tokens,
            total_tokens: s.total_tokens,
            duration_ms: s.duration_ms,
            error: s.error,
            step_order: s.step_order,
            started_at: s.started_at,
            finished_at: s.finished_at,
        }
    }
}

// =============================================================================
// Analysis Context DTOs
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunAnalysisContextDto {
    pub run: PipelineRunDto,
    pub author_stats: Option<AuthorStatsDto>,
    pub author_category_averages: Vec<CategoryAverageDto>,
    pub project_category_averages: Vec<CategoryAverageDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorStatsDto {
    pub login: String,
    pub avatar_url: String,
    pub total_runs: u32,
    pub avg_overall_score: f64,
    pub last_overall_score: u32,
    pub last_run_at: String,
}

impl From<venore_core::agents::AuthorStats> for AuthorStatsDto {
    fn from(s: venore_core::agents::AuthorStats) -> Self {
        Self {
            login: s.login,
            avatar_url: s.avatar_url,
            total_runs: s.total_runs,
            avg_overall_score: s.avg_overall_score,
            last_overall_score: s.last_overall_score,
            last_run_at: s.last_run_at,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryAverageDto {
    pub category_name: String,
    pub avg_score: f64,
    pub run_count: u32,
}

impl From<venore_core::agents::CategoryAverage> for CategoryAverageDto {
    fn from(a: venore_core::agents::CategoryAverage) -> Self {
        Self {
            category_name: a.category_name,
            avg_score: a.avg_score,
            run_count: a.run_count,
        }
    }
}
