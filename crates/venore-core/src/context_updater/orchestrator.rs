//! Orchestrator — coordinates branch monitoring, change mapping, and regeneration.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::analysis::AnalysisOutput;
use crate::checkpoint::CheckpointManager;
use crate::context::DepthLevel;
use crate::error::{Result, VenoreError};
use crate::llm::prelude::*;
use crate::wizard::{
    BatchGenerationConfig, BatchManager,
    WizardEventEmitter, generate_batch_contexts_with_emitter,
};

use super::branch_monitor::{self, CommitSummary};
use super::change_mapper::{self, AffectedModule};
use super::updater_state::UpdaterState;

/// Report of detected updates (before regeneration).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateReport {
    pub commits: Vec<CommitSummary>,
    pub affected_modules: Vec<AffectedModule>,
    pub latest_commit: String,
}

/// Result of module regeneration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegenerationResult {
    pub completed: usize,
    pub failed: usize,
    pub duration_ms: u64,
}

/// Check if there are new commits on the monitored branch that affect any modules.
///
/// Returns `None` if up-to-date or no modules are affected.
#[deprecated(
    note = "Superseded by Project Memory (.venore/project-memory.json, see crate::memory); \
            the .context.md auto-updater is no longer wired into the UI. Slated for removal."
)]
pub fn check_for_updates(project_path: &Path) -> Result<Option<UpdateReport>> {
    let state = UpdaterState::load(project_path)?;
    let branch = &state.selected_branch;

    // Fetch latest from origin
    branch_monitor::fetch_branch(project_path, branch)?;

    // Get latest remote HEAD
    let latest_commit = branch_monitor::get_latest_remote_commit(project_path, branch)?;

    // Determine base commit
    let base_commit = match &state.last_sync_commit {
        Some(commit) => {
            // Already up-to-date?
            if commit == &latest_commit {
                info!(branch, "Up to date — no new commits");
                return Ok(None);
            }
            commit.clone()
        }
        None => {
            // First sync — look back 20 commits
            branch_monitor::get_initial_base_commit(project_path, branch, 20)?
        }
    };

    // Get changed files
    let changed_files = branch_monitor::get_changed_files(project_path, &base_commit, branch)?;
    if changed_files.is_empty() {
        info!("No changed files detected");
        return Ok(None);
    }

    // Load analysis output
    let analysis = AnalysisOutput::load_from_disk(project_path)?
        .ok_or_else(|| VenoreError::NotFound(
            "No analysis output found. Run the wizard first.".into(),
        ))?;

    // Map to modules
    let affected_modules = change_mapper::map_files_to_modules(&changed_files, &analysis);
    if affected_modules.is_empty() {
        info!("Changed files don't affect any modules");
        return Ok(None);
    }

    // Get commit summaries for the report
    let commits = branch_monitor::get_new_commits(project_path, &base_commit, branch)?;

    info!(
        modules = affected_modules.len(),
        commits = commits.len(),
        "Update detected: {} modules affected by {} commits",
        affected_modules.len(),
        commits.len(),
    );

    Ok(Some(UpdateReport {
        commits,
        affected_modules,
        latest_commit,
    }))
}

/// Regenerate `.context.md` for the specified modules.
///
/// Reuses the wizard's `generate_batch_contexts_with_emitter` with a
/// `BatchGenerationConfig` built from the checkpoint's `WizardConfig`.
#[deprecated(
    note = "Superseded by Project Memory (.venore/project-memory.json, see crate::memory); \
            the .context.md auto-updater is no longer wired into the UI. Slated for removal."
)]
pub async fn regenerate_modules(
    project_path: &Path,
    module_names: &[String],
    provider: LlmProviderType,
    model: &str,
    depth_level: DepthLevel,
    system_prompt: String,
    llm_gateway: Arc<LlmGateway>,
    event_emitter: Arc<dyn WizardEventEmitter>,
) -> Result<RegenerationResult> {
    let start = Instant::now();

    // Load analysis
    let analysis = AnalysisOutput::load_from_disk(project_path)?
        .ok_or_else(|| VenoreError::NotFound(
            "No analysis output found. Run the wizard first.".into(),
        ))?;

    // Load wizard config from checkpoint
    let checkpoint_manager = CheckpointManager::new(project_path);
    let checkpoint = checkpoint_manager.load()
        .map_err(|e| VenoreError::Unknown(format!("Failed to load checkpoint: {}", e)))?
        .ok_or_else(|| VenoreError::NotFound(
            "No checkpoint found. Run the wizard first.".into(),
        ))?;
    let wizard_config = checkpoint.wizard_config;

    // Build module_ids and module_paths for selected modules
    let mut module_ids: Vec<String> = Vec::new();
    let mut module_paths: Vec<String> = Vec::new();

    for module_name in module_names {
        if let Some(module) = analysis.modules.iter().find(|m| &m.name == module_name) {
            module_ids.push(module.name.clone());
            module_paths.push(module.path.clone());
        } else {
            warn!(module = %module_name, "Module not found in analysis, skipping");
        }
    }

    if module_ids.is_empty() {
        return Err(VenoreError::InvalidParams(
            "None of the specified modules were found in the analysis".into(),
        ));
    }

    let total = module_ids.len();
    info!(count = total, "Regenerating {} modules", total);

    // Register a batch in BatchManager
    let batch_id = {
        let batch_manager = BatchManager::global();
        let mut guard = batch_manager.lock().map_err(|e| {
            VenoreError::Unknown(format!("Failed to lock BatchManager: {}", e))
        })?;
        let (id, _pause_flag) = guard.create_batch(project_path.to_string_lossy().to_string())
            .map_err(|e| VenoreError::Unknown(format!("Failed to create batch: {}", e)))?;
        id
    };

    let config = BatchGenerationConfig {
        batch_id: batch_id.clone(),
        project_path: PathBuf::from(project_path),
        module_ids,
        module_paths,
        model: model.to_string(),
        provider,
        depth_level,
        system_prompt,
        analysis,
        wizard_config,
        total_selected: total,
        previously_completed: 0,
    };

    let result = generate_batch_contexts_with_emitter(config, llm_gateway, event_emitter).await;

    // Cleanup batch registration
    {
        let batch_manager = BatchManager::global();
        if let Ok(mut guard) = batch_manager.lock() {
            guard.remove_batch(&batch_id);
        };
    }

    let result = result?;

    Ok(RegenerationResult {
        completed: result.completed,
        failed: result.failed,
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

/// Mark the update as complete — persists the latest commit SHA.
#[deprecated(
    note = "Superseded by Project Memory (.venore/project-memory.json, see crate::memory); \
            the .context.md auto-updater is no longer wired into the UI. Slated for removal."
)]
pub fn complete_update(project_path: &Path, latest_commit: &str) -> Result<()> {
    let mut state = UpdaterState::load(project_path)?;
    state.last_sync_commit = Some(latest_commit.to_string());
    state.last_sync_at = Some(chrono::Utc::now());
    UpdaterState::save(project_path, &state)?;
    info!(commit = %latest_commit, "Update marked as complete");
    Ok(())
}
