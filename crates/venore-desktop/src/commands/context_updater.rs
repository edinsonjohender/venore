//! Context Auto-Updater Tauri commands
//!
//! Check for branch updates, regenerate stale modules, and manage updater state.
//!
//! # DEPRECATED
//!
//! These commands wrap [`venore_core::context_updater`], which is superseded by
//! Project Memory (`.venore/project-memory.json`). They are still registered in
//! `main.rs` but **not invoked by any UI component** — the feature is orphaned.
//! Kept temporarily; slated for removal alongside the core module.
#![allow(deprecated)] // calls the deprecated context_updater orchestrator on purpose

use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use venore_core::context::DepthLevel;
use venore_core::context_updater::{self, AffectedModule, CommitSummary, UpdaterState};
use venore_core::error::VenoreError;
use venore_core::traits::LlmProviderType;
use venore_core::wizard::{CallbackEventEmitter, CompleteEvent, ProgressEvent};

use crate::commands::llm::get_services;
use crate::state::LazyAppState;
use crate::utils::{CommandResult, IntoStateCommandResult, StateCommandResult};

const DEFAULT_CONTEXT_SYSTEM_PROMPT: &str = "You are a technical documentation expert specializing in code analysis. Generate clear, comprehensive documentation following the structure provided.";

// =============================================================================
// DTOs
// =============================================================================

#[derive(Serialize)]
pub struct UpdateReportResponse {
    pub commits: Vec<CommitSummaryDto>,
    pub affected_modules: Vec<AffectedModuleDto>,
    pub latest_commit: String,
}

#[derive(Serialize)]
pub struct CommitSummaryDto {
    pub hash: String,
    pub short_hash: String,
    pub message: String,
}

#[derive(Serialize)]
pub struct AffectedModuleDto {
    pub name: String,
    pub path: String,
    pub changed_files: Vec<String>,
}

#[derive(Deserialize)]
pub struct RunUpdateRequest {
    pub project_path: String,
    pub module_names: Vec<String>,
    pub provider: String,
    pub model: String,
    pub depth_level: String,
    pub latest_commit: String,
}

#[derive(Serialize)]
pub struct UpdaterStateResponse {
    pub selected_branch: String,
    pub last_sync_commit: Option<String>,
    pub last_sync_at: Option<String>,
    pub auto_update_enabled: bool,
    pub check_interval_minutes: u32,
}

#[derive(Deserialize)]
pub struct UpdateUpdaterStateRequest {
    pub project_path: String,
    pub selected_branch: String,
    pub auto_update_enabled: bool,
    pub check_interval_minutes: u32,
}

// =============================================================================
// Converters
// =============================================================================

fn commit_to_dto(c: &CommitSummary) -> CommitSummaryDto {
    CommitSummaryDto {
        hash: c.hash.clone(),
        short_hash: c.short_hash.clone(),
        message: c.message.clone(),
    }
}

fn module_to_dto(m: &AffectedModule) -> AffectedModuleDto {
    AffectedModuleDto {
        name: m.name.clone(),
        path: m.path.clone(),
        changed_files: m.changed_files.clone(),
    }
}

fn parse_depth_level(s: &str) -> DepthLevel {
    match s.to_lowercase().as_str() {
        "minimal" => DepthLevel::Minimal,
        "detailed" => DepthLevel::Detailed,
        "expert" => DepthLevel::Expert,
        _ => DepthLevel::Normal,
    }
}

// =============================================================================
// Commands
// =============================================================================

/// Check for new commits on the monitored branch that affect modules.
#[tauri::command]
pub async fn check_for_updates(
    project_path: String,
) -> CommandResult<Option<UpdateReportResponse>> {
    tracing::info!(project = %project_path, "Checking for context updates");

    let result: Result<Option<UpdateReportResponse>, VenoreError> =
        tokio::task::spawn_blocking({
            let project_path = project_path.clone();
            move || {
                let path = Path::new(&project_path);
                match context_updater::orchestrator::check_for_updates(path)? {
                    Some(report) => Ok(Some(UpdateReportResponse {
                        commits: report.commits.iter().map(commit_to_dto).collect(),
                        affected_modules: report
                            .affected_modules
                            .iter()
                            .map(module_to_dto)
                            .collect(),
                        latest_commit: report.latest_commit,
                    })),
                    None => Ok(None),
                }
            }
        })
        .await
        .unwrap_or_else(|e| {
            Err(VenoreError::Unknown(format!("Task join error: {}", e)))
        });

    result.into()
}

/// Regenerate `.context.md` for the specified modules.
/// Progress is emitted via Tauri events: `context-update-progress` and `context-update-complete`.
#[tauri::command]
pub async fn run_context_update(
    app: AppHandle,
    state: tauri::State<'_, LazyAppState>,
    request: RunUpdateRequest,
) -> StateCommandResult<()> {
    tracing::info!(
        project = %request.project_path,
        modules = ?request.module_names,
        "Running context update"
    );

    let services = get_services(&state);
    let prompt_repo = {
        let guard = state.get();
        guard.as_ref().map(|s| Arc::clone(&s.prompt_repository))
    };

    let result: Result<(), VenoreError> = async {
        let (_config_store, llm_gateway) = services?;

        let provider: LlmProviderType = request.provider.parse()?;
        let depth_level = parse_depth_level(&request.depth_level);

        // Resolve system prompt
        let system_prompt = if let Some(repo) = prompt_repo {
            repo.resolve_prompt("context", provider.as_str())
                .await
                .map(|p| p.content)
                .unwrap_or_else(|_| DEFAULT_CONTEXT_SYSTEM_PROMPT.to_string())
        } else {
            DEFAULT_CONTEXT_SYSTEM_PROMPT.to_string()
        };

        let project_path = Path::new(&request.project_path).to_path_buf();
        let module_names = request.module_names.clone();
        let model = request.model.clone();
        let latest_commit = request.latest_commit.clone();

        // Build event emitter that bridges to Tauri events
        let app_progress = app.clone();
        let app_complete = app.clone();

        let emitter = Arc::new(CallbackEventEmitter::new(
            move |event: ProgressEvent| {
                let _ = app_progress.emit(
                    "context-update-progress",
                    serde_json::json!({
                        "current": event.current,
                        "total": event.total,
                        "module_id": event.module_id,
                        "status": event.status,
                        "tokens_used": event.tokens_used,
                        "error": event.error,
                    }),
                );
            },
            move |event: CompleteEvent| {
                let _ = app_complete.emit(
                    "context-update-complete",
                    serde_json::json!({
                        "total_completed": event.total_completed,
                        "total_failed": event.total_failed,
                        "duration_ms": event.duration_ms,
                    }),
                );
            },
        ));

        // Spawn regeneration in background
        tokio::spawn(async move {
            match context_updater::orchestrator::regenerate_modules(
                &project_path,
                &module_names,
                provider,
                &model,
                depth_level,
                system_prompt,
                llm_gateway,
                emitter,
            )
            .await
            {
                Ok(result) => {
                    tracing::info!(
                        completed = result.completed,
                        failed = result.failed,
                        duration_ms = result.duration_ms,
                        "Context update finished"
                    );

                    // Auto-complete the update if all succeeded
                    if result.failed == 0 {
                        if let Err(e) = context_updater::orchestrator::complete_update(
                            &project_path,
                            &latest_commit,
                        ) {
                            tracing::error!("Failed to mark update as complete: {}", e);
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Context update failed: {}", e);
                }
            }
        });

        Ok(())
    }
    .await;

    result.into_state()
}

/// Mark the sync as completed with the given commit SHA.
#[tauri::command]
pub async fn complete_context_update(
    project_path: String,
    latest_commit: String,
) -> CommandResult<()> {
    tracing::info!(commit = %latest_commit, "Completing context update");

    let result: Result<(), VenoreError> = {
        let path = Path::new(&project_path);
        context_updater::orchestrator::complete_update(path, &latest_commit)
    };

    result.into()
}

/// Get the current updater state (branch, interval, etc.).
#[tauri::command]
pub async fn get_updater_state(project_path: String) -> CommandResult<UpdaterStateResponse> {
    let result: Result<UpdaterStateResponse, VenoreError> = (|| {
        let path = Path::new(&project_path);
        let state = UpdaterState::load(path)?;
        Ok(UpdaterStateResponse {
            selected_branch: state.selected_branch,
            last_sync_commit: state.last_sync_commit,
            last_sync_at: state.last_sync_at.map(|dt| dt.to_rfc3339()),
            auto_update_enabled: state.auto_update_enabled,
            check_interval_minutes: state.check_interval_minutes,
        })
    })();

    result.into()
}

/// Update the updater configuration.
#[tauri::command]
pub async fn update_updater_state(request: UpdateUpdaterStateRequest) -> CommandResult<()> {
    tracing::info!(
        branch = %request.selected_branch,
        interval = request.check_interval_minutes,
        "Updating updater state"
    );

    let result: Result<(), VenoreError> = (|| {
        let path = Path::new(&request.project_path);
        let mut state = UpdaterState::load(path)?;
        state.selected_branch = request.selected_branch;
        state.auto_update_enabled = request.auto_update_enabled;
        state.check_interval_minutes = request.check_interval_minutes;
        UpdaterState::save(path, &state)
    })();

    result.into()
}
