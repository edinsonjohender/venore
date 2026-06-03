//! Snapshot Tauri commands
//!
//! Exposes the snapshot pipeline to the frontend as `resnapshot_project`.
//! Used by the workspace UI's "refresh snapshot" button when stale modules
//! appear on the canvas — regenerates `.venore/analysis-output.json`,
//! `.venore/module-layers.json`, and `.venore/code-hashes.json` from the
//! current source tree without re-running the wizard or calling the LLM.

use std::path::Path;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use venore_core::analysis::pipeline::{run_analysis, RunAnalysisConfig};
use venore_core::error::VenoreError;
use venore_core::project::ProjectService;
use venore_core::rag::{IndexConfig, RagRepository};
use venore_core::snapshot::{run_snapshot, SnapshotInputs};

use crate::state::LazyAppState;
use crate::utils::{IntoStateCommandResult, StateCommandResult};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResnapshotReport {
    pub modules: u32,
    pub indexed: u32,
    pub skipped: u32,
    pub removed: u32,
    pub modules_mapped: u32,
    pub deps_created: u32,
    pub refs_created: u32,
    pub layers_written: u32,
    pub hashes_written: u32,
}

/// Refresh the portable snapshot for an already-Venorized project.
///
/// Pipeline: re-scan + re-detect modules → write `analysis-output.json` →
/// RAG re-index → per-module layer analysis → write `module-layers.json` and
/// `code-hashes.json`. Memory and ocean-layout files are not touched.
///
/// Refuses to run on folders that don't have `.venore/project.json` — the
/// wizard is the only path that should establish a Venore project.
#[tauri::command]
pub async fn resnapshot_project(
    app: AppHandle,
    lazy_state: tauri::State<'_, LazyAppState>,
    project_path: String,
) -> StateCommandResult<ResnapshotReport> {
    tracing::info!("resnapshot_project: {}", project_path);

    let rag_repo = {
        let guard = lazy_state.get();
        match guard.as_ref() {
            Some(state) => Arc::clone(&state.rag_repository),
            None => {
                return Err(VenoreError::NotFound("Backend not initialized".into()))
                    .into_state();
            }
        }
    };

    let result: Result<ResnapshotReport, VenoreError> = async {
        let path = Path::new(&project_path).to_path_buf();

        // Strict identity: refuse to re-snapshot folders that aren't
        // Venore projects yet. The wizard is the only path that creates
        // `.venore/project.json`.
        let identity = ProjectService::read_identity_strict(&path)?;
        let project_id = identity.id.to_string();

        // 1. Fresh analysis — re-scans source tree, re-detects modules,
        //    persists `.venore/analysis-output.json`. Picks up renames,
        //    new files, and deleted modules so the rest of the pipeline
        //    runs against the current code.
        let analysis_config = RunAnalysisConfig {
            project_path: path.clone(),
            ..RunAnalysisConfig::default()
        };
        let analysis = run_analysis(analysis_config).await?;

        // 2. Snapshot pipeline — RAG index, layer analysis, portable files.
        //    Uses default ignore patterns; if the user previously set
        //    custom exclusions in the wizard, those don't roundtrip yet
        //    (would need a `.venore/snapshot-config.json` in a later phase).
        let rag_config = IndexConfig::default();
        let report = run_snapshot(
            SnapshotInputs {
                project_path: &path,
                project_id: &project_id,
                analysis: &analysis,
                rag_config: &rag_config,
                layer_types: Vec::new(),
                cancel: None,
            },
            &rag_repo as &RagRepository,
            None,
            None,
        )
        .await?;

        // Reuse the existing `context-update-complete` event so the canvas
        // (OceanNodes) re-fetches layers + stale modules automatically.
        // Tauri events are fire-and-forget — failure to emit is non-fatal.
        let _ = app.emit("context-update-complete", ());

        Ok(ResnapshotReport {
            modules: analysis.modules.len() as u32,
            indexed: report.indexed,
            skipped: report.skipped,
            removed: report.removed,
            modules_mapped: report.modules_mapped,
            deps_created: report.deps_created,
            refs_created: report.refs_created,
            layers_written: report.layers_written,
            hashes_written: report.hashes_written,
        })
    }
    .await;

    result.into_state()
}
