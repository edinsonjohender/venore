//! Pipeline entities — run and step models for agent pipeline execution

use serde::{Deserialize, Serialize};

// =============================================================================
// Pipeline Run
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineRun {
    pub id: String,
    pub team_id: String,
    pub team_name: String,
    pub task_type: String,
    pub title: String,
    pub status: PipelineRunStatus,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PipelineRunStatus {
    Running,
    Completed,
    Failed,
}

impl PipelineRunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "running" => Some(Self::Running),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

// =============================================================================
// Pipeline Step
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStep {
    pub id: String,
    pub run_id: String,
    pub profile_id: String,
    pub profile_name: String,
    pub stage: String,
    pub status: PipelineStepStatus,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PipelineStepStatus {
    Running,
    Completed,
    Failed,
}

impl PipelineStepStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "running" => Some(Self::Running),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}
