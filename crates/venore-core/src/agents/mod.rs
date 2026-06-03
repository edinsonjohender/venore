//! Agents module — AI agent profiles and team composition
//!
//! Provides domain models, SQLite persistence, default seeds,
//! and pipeline execution for agent-based analysis.

pub mod executor;
pub mod models;
pub mod pipeline;
pub mod repository;
pub mod seed;
pub mod snapshot;

pub use models::{
    AgentProfile, AgentTeam, AgentRule,
    AgentStage, Severity, ToolCategory, ToolDefinition,
    ChatMode,
};
pub use pipeline::{
    PipelineRun, PipelineRunStatus, PipelineStep, PipelineStepStatus,
};
pub use executor::{PipelineExecutor, PipelineEvent, PipelineDeps, PipelineRequest, prepare_pipeline};
pub use repository::AgentRepository;
pub use snapshot::{AuthorStats, CategoryAverage};
