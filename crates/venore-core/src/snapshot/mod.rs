//! Snapshot pipeline — produces every `.venore/*.json` portable file from
//! the current code, modulo project memory and ocean layout.
//!
//! Used by:
//!   - the wizard, after collecting user input, to populate the snapshot
//!     during onboarding;
//!   - the `resnapshot_project` Tauri command, to refresh the snapshot when
//!     the user has drifted from it (stale badges appearing on the canvas).
//!
//! Pure backend — no Tauri events. Callers pass optional progress callbacks
//! to forward into their own UI eventing.

use std::collections::HashMap;
use std::path::Path;

use tokio_util::sync::CancellationToken;

use crate::analysis::AnalysisOutput;
use crate::context::file_storage as layers_file;
use crate::context::hash::calculate_module_hash_and_fingerprint;
use crate::context::hash_storage::{self, ModuleHashEntry};
use crate::context::repository::ModuleLayerRecord;
use crate::layers::{analyze_module_layers, ModuleConnectionInfo};
use crate::rag::types::IndexProgressEvent;
use crate::rag::{self, IndexConfig, RagRepository};
use crate::{Result, VenoreError};

/// Heuristic layer types that don't require an LLM. Used when the caller
/// doesn't supply a custom set (e.g. re-snapshot, which has no wizard
/// session). `context` is intentionally excluded — it was tied to the now-
/// deprecated `.context.md` flow.
const DEFAULT_LAYER_TYPES: &[&str] = &["tests", "documentation", "connections", "status"];

const VALID_LAYER_TYPES: &[&str] = &["context", "tests", "documentation", "connections", "status"];

/// Aggregate counts of what the snapshot pipeline produced. Used by callers
/// to show a "restored / refreshed N modules" banner.
#[derive(Debug, Clone, Default)]
pub struct SnapshotReport {
    pub indexed: u32,
    pub skipped: u32,
    pub removed: u32,
    pub modules_mapped: u32,
    pub deps_created: u32,
    pub refs_created: u32,
    /// Number of modules whose layer analysis was successfully persisted.
    pub layers_written: u32,
    /// Number of modules whose code-hash was successfully written to disk.
    pub hashes_written: u32,
}

pub struct SnapshotInputs<'a> {
    pub project_path: &'a Path,
    pub project_id: &'a str,
    pub analysis: &'a AnalysisOutput,
    pub rag_config: &'a IndexConfig,
    /// Layer types to compute. Empty → defaults (everything except `context`).
    pub layer_types: Vec<String>,
    pub cancel: Option<&'a CancellationToken>,
}

/// Run the full snapshot pipeline: RAG index → layer analysis → portable
/// file writes. No LLM. No `.context.md`. Idempotent (safe to call twice on
/// the same project).
///
/// `on_rag` is forwarded to the indexer for per-file progress; `on_layer`
/// fires once per module before its layers are computed. Both are optional —
/// re-snapshot passes None for both.
pub async fn run_snapshot(
    inputs: SnapshotInputs<'_>,
    rag_repo: &RagRepository,
    on_rag: Option<&(dyn Fn(IndexProgressEvent) + Send + Sync)>,
    on_layer: Option<&(dyn Fn(u32, u32, &str) + Send + Sync)>,
) -> Result<SnapshotReport> {
    let mut report = SnapshotReport::default();

    // 1. RAG index (files, chunks, graph). Cancellation is checked inside.
    let graph = rag::index_project_with_graph(
        rag_repo,
        inputs.project_id,
        inputs.project_path,
        inputs.rag_config,
        on_rag,
        inputs.analysis,
        inputs.cancel,
    )
    .await?;
    report.indexed = graph.indexed;
    report.skipped = graph.skipped;
    report.removed = graph.removed;
    report.modules_mapped = graph.modules_mapped;
    report.deps_created = graph.deps_created;
    report.refs_created = graph.refs_created;

    if cancelled(inputs.cancel) {
        return Err(VenoreError::Cancelled(
            "snapshot cancelled before layers analysis".into(),
        ));
    }

    // 2. Resolve the layer types to compute.
    let layer_types: Vec<String> = if inputs.layer_types.is_empty() {
        DEFAULT_LAYER_TYPES.iter().map(|s| s.to_string()).collect()
    } else {
        inputs
            .layer_types
            .iter()
            .filter(|name| VALID_LAYER_TYPES.contains(&name.as_str()))
            .cloned()
            .collect()
    };

    // 3. Build module_name → ModuleConnectionInfo map from the graph the
    //    indexer just populated. Feeds the `connections` analyzer so it can
    //    report dep/dependent counts instead of always Missing.
    let connection_map: HashMap<String, ModuleConnectionInfo> = {
        let mut map: HashMap<String, ModuleConnectionInfo> = HashMap::new();
        match rag_repo.get_all_module_deps(inputs.project_id).await {
            Ok(deps) => {
                for dep in deps {
                    map.entry(dep.from_module.clone())
                        .or_default()
                        .dependencies
                        .push(dep.to_module.clone());
                    map.entry(dep.to_module.clone())
                        .or_default()
                        .dependents
                        .push(dep.from_module.clone());
                }
            }
            Err(e) => {
                tracing::warn!("Could not load module deps for connections layer: {}", e);
            }
        }
        map
    };

    // 4. Per-module layer analysis. Records collected in-memory so the
    //    single file write at the end is the only persistence step — no DB
    //    round-trip. `analyzed_at` is captured once at the top of the loop
    //    so the whole snapshot shares a consistent timestamp.
    let analyzed_at = chrono::Utc::now().to_rfc3339();
    let total = inputs.analysis.modules.len() as u32;
    let mut all_records: Vec<ModuleLayerRecord> = Vec::with_capacity(
        inputs.analysis.modules.len() * layer_types.len(),
    );
    for (idx, module) in inputs.analysis.modules.iter().enumerate() {
        if cancelled(inputs.cancel) {
            return Err(VenoreError::Cancelled(format!(
                "snapshot cancelled after analyzing {} modules",
                report.layers_written
            )));
        }

        if let Some(cb) = on_layer {
            cb(idx as u32, total, &module.name);
        }

        let conn_info = connection_map.get(&module.name);
        let analysis_layer = analyze_module_layers(
            inputs.project_path,
            &module.path,
            conn_info,
            &layer_types,
        );

        for layer in &analysis_layer.layers {
            all_records.push(ModuleLayerRecord {
                // The file format ignores id/project_id (location implies
                // ownership, and the file shape drops them) — leaving these
                // empty signals "synthesized for the snapshot" if anything
                // ever inspects them through the legacy shape.
                id: String::new(),
                project_id: inputs.project_id.to_string(),
                module_name: analysis_layer.module_name.clone(),
                module_path: analysis_layer.module_path.clone(),
                layer_type: layer.layer_type.as_str().to_string(),
                status: layer.status.as_str().to_string(),
                details_json: serde_json::to_string(&layer.details)
                    .unwrap_or_else(|_| "{}".into()),
                analyzed_at: analyzed_at.clone(),
            });
        }
        report.layers_written += 1;
    }

    // 5. Portable module-layers snapshot — the only persistence step now
    //    that the DB dual-write was retired. A failure here means the
    //    layers are lost (no DB safety net), so let the error propagate
    //    instead of swallowing it with a warn.
    if !all_records.is_empty() {
        layers_file::save(inputs.project_path, &all_records)?;
        tracing::info!(
            count = all_records.len(),
            "Wrote portable module layers snapshot"
        );
    }

    // 6. Per-module SHA-256 fingerprints. Failure is non-fatal — missing
    //    hashes mean nothing is reported as stale, never a false alarm.
    let now = chrono::Utc::now().to_rfc3339();
    let mut hash_entries: Vec<ModuleHashEntry> =
        Vec::with_capacity(inputs.analysis.modules.len());
    for module in &inputs.analysis.modules {
        let module_path_str = module.path.replace('\\', "/");
        match calculate_module_hash_and_fingerprint(inputs.project_path, &module_path_str) {
            Ok((code_hash, fp)) => hash_entries.push(ModuleHashEntry {
                module_name: module.name.clone(),
                module_path: module_path_str,
                code_hash,
                file_count: fp.file_count,
                total_size: fp.total_size,
                max_mtime: fp.max_mtime,
                computed_at: now.clone(),
            }),
            Err(e) => tracing::warn!(
                module = %module.name,
                error = %e,
                "Skipping module from code-hashes snapshot"
            ),
        }
    }
    if !hash_entries.is_empty() {
        match hash_storage::save(inputs.project_path, &hash_entries) {
            Ok(()) => {
                report.hashes_written = hash_entries.len() as u32;
                tracing::info!(
                    count = hash_entries.len(),
                    "Wrote portable code-hashes snapshot"
                );
            }
            Err(e) => tracing::warn!("Failed to write .venore/code-hashes.json: {}", e),
        }
    }

    Ok(report)
}

fn cancelled(token: Option<&CancellationToken>) -> bool {
    token.map(|t| t.is_cancelled()).unwrap_or(false)
}
