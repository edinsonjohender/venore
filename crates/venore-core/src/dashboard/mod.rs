//! Dashboard module — project overview with per-module freshness.
//!
//! Stateless. Given an `AnalysisOutput` and the project path, computes a
//! `Fresh / Stale / Missing` status per module by comparing the live source
//! tree against the per-module fingerprints stored in
//! `.venore/code-hashes.json`.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::analysis::AnalysisOutput;
use crate::context::hash::calculate_module_hash;
use crate::context::hash_storage;

// =============================================================================
// Types
// =============================================================================

/// Freshness of a module relative to the committed snapshot.
///
/// - `Fresh` — current code hashes to the same value stored in `.venore/code-hashes.json`.
/// - `Stale` — entry exists in the snapshot but the current hash differs.
/// - `Missing` — no entry in the snapshot (module added after the snapshot was taken,
///   or no snapshot exists at all).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContextStatus {
    Fresh,
    Stale,
    Missing,
}

/// Snapshot metadata about how the module was last recorded. Kept on the DTO
/// for UI compatibility; populated when an entry exists in `code-hashes.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextFileMeta {
    /// `computed_at` from the hash entry — when this module's fingerprint
    /// was last written.
    pub generated_at: Option<String>,
    /// Reserved for compatibility with the previous `.context.md`-based shape.
    pub model: Option<String>,
    pub provider: Option<String>,
    pub context_path: String,
}

/// Summary of a single module for the dashboard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleSummary {
    pub name: String,
    pub path: String,
    pub file_count: usize,
    pub dependency_count: usize,
    pub dependent_count: usize,
    pub context_status: ContextStatus,
    pub context_meta: Option<ContextFileMeta>,
    pub files: Vec<String>,
}

/// Aggregate stats for the project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectStats {
    pub total_modules: usize,
    pub total_connections: usize,
    pub fresh_count: usize,
    pub stale_count: usize,
    pub missing_count: usize,
}

/// Complete dashboard payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    pub stats: ProjectStats,
    pub modules: Vec<ModuleSummary>,
    pub orphan_files: Vec<String>,
}

// =============================================================================
// Public API
// =============================================================================

/// Build dashboard data from an analysis output.
///
/// Per-module status is driven by `.venore/code-hashes.json`. If the file is
/// absent (a project that hasn't snapshotted yet) every module reports
/// `Missing` — same outward shape as before, just sourced from the portable
/// snapshot instead of from `.context.md` mtimes.
pub fn build_dashboard(analysis: &AnalysisOutput, project_path: &Path) -> DashboardData {
    // Load the snapshot once and index it by module_name for O(1) lookup
    // while iterating modules. A missing or corrupt file yields an empty
    // map, which makes every module fall through to `Missing` — the same
    // UX as a brand-new project.
    let stored_by_name: HashMap<String, hash_storage::ModuleHashEntry> = hash_storage::load(
        project_path,
    )
    .ok()
    .flatten()
    .map(|v| v.into_iter().map(|e| (e.module_name.clone(), e)).collect())
    .unwrap_or_default();

    let modules: Vec<ModuleSummary> = analysis
        .modules
        .iter()
        .map(|m| {
            let (status, meta) = classify_module(project_path, m, &stored_by_name);
            ModuleSummary {
                name: m.name.clone(),
                path: m.path.clone(),
                file_count: m.file_count,
                dependency_count: m.architecture.dependencies.len(),
                dependent_count: m.architecture.dependents.len(),
                context_status: status,
                context_meta: meta,
                files: m.files.clone(),
            }
        })
        .collect();

    let fresh_count = modules.iter().filter(|m| m.context_status == ContextStatus::Fresh).count();
    let stale_count = modules.iter().filter(|m| m.context_status == ContextStatus::Stale).count();
    let missing_count = modules
        .iter()
        .filter(|m| m.context_status == ContextStatus::Missing)
        .count();

    let total_connections: usize = analysis
        .modules
        .iter()
        .map(|m| m.architecture.dependencies.len())
        .sum();

    DashboardData {
        stats: ProjectStats {
            total_modules: modules.len(),
            total_connections,
            fresh_count,
            stale_count,
            missing_count,
        },
        modules,
        orphan_files: analysis.orphan_files.clone(),
    }
}

// =============================================================================
// Private helpers
// =============================================================================

/// Decide a module's status by comparing the stored hash to a freshly
/// computed one. Hash computation errors degrade to `Stale` rather than
/// erroring out the dashboard — a partially readable tree shouldn't crash
/// the UI.
fn classify_module(
    project_path: &Path,
    module: &crate::analysis::analysis_output::ModuleAnalysis,
    stored_by_name: &HashMap<String, hash_storage::ModuleHashEntry>,
) -> (ContextStatus, Option<ContextFileMeta>) {
    let Some(stored) = stored_by_name.get(&module.name) else {
        return (ContextStatus::Missing, None);
    };

    let meta = Some(ContextFileMeta {
        generated_at: Some(stored.computed_at.clone()),
        model: None,
        provider: None,
        context_path: format!(".venore/code-hashes.json#{}", stored.module_name),
    });

    match calculate_module_hash(project_path, &module.path) {
        Ok((current, _)) if current == stored.code_hash => (ContextStatus::Fresh, meta),
        Ok(_) => (ContextStatus::Stale, meta),
        Err(_) => (ContextStatus::Stale, meta),
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    use crate::analysis::{
        AnalysisOutput, ModuleAnalysis, ModuleArchitecture, ModuleSymbols, RepositoryInfo,
    };
    use crate::context::hash_storage::ModuleHashEntry;

    fn make_analysis(modules: Vec<ModuleAnalysis>) -> AnalysisOutput {
        AnalysisOutput {
            repository: RepositoryInfo {
                name: "test".to_string(),
                language: None,
                technologies: vec![],
                total_files: 0,
                total_modules: modules.len(),
            },
            modules,
            orphan_files: vec![],
        }
    }

    fn make_module(name: &str, path: &str) -> ModuleAnalysis {
        ModuleAnalysis {
            name: name.to_string(),
            path: path.to_string(),
            file_count: 1,
            entry_point: None,
            architecture: ModuleArchitecture {
                dependencies: vec![],
                dependents: vec![],
                external_deps: vec![],
            },
            symbols: ModuleSymbols { exports: vec![], all: vec![] },
            imports: vec![],
            code_snippets: String::new(),
            files: vec![],
        }
    }

    fn write_source(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    #[test]
    fn missing_when_no_snapshot_file() {
        let tmp = TempDir::new().unwrap();
        write_source(tmp.path(), "src/auth/index.ts", "export {}");
        let analysis = make_analysis(vec![make_module("auth", "src/auth")]);

        let dashboard = build_dashboard(&analysis, tmp.path());
        assert_eq!(dashboard.stats.missing_count, 1);
        assert_eq!(dashboard.modules[0].context_status, ContextStatus::Missing);
        assert!(dashboard.modules[0].context_meta.is_none());
    }

    #[test]
    fn fresh_when_hash_matches_snapshot() {
        let tmp = TempDir::new().unwrap();
        write_source(tmp.path(), "src/auth/index.ts", "export {}");
        let (hash, _) = calculate_module_hash(tmp.path(), "src/auth").unwrap();
        hash_storage::save(
            tmp.path(),
            &[ModuleHashEntry {
                module_name: "auth".into(),
                module_path: "src/auth".into(),
                code_hash: hash,
                file_count: 1,
                total_size: 0,
                max_mtime: 0,
                computed_at: "2026-05-12T00:00:00Z".into(),
            }],
        )
        .unwrap();

        let analysis = make_analysis(vec![make_module("auth", "src/auth")]);
        let dashboard = build_dashboard(&analysis, tmp.path());
        assert_eq!(dashboard.stats.fresh_count, 1);
        assert_eq!(dashboard.modules[0].context_status, ContextStatus::Fresh);
        assert!(dashboard.modules[0].context_meta.is_some());
    }

    #[test]
    fn stale_when_source_changed_after_snapshot() {
        let tmp = TempDir::new().unwrap();
        write_source(tmp.path(), "src/auth/index.ts", "export {}");
        let (hash, _) = calculate_module_hash(tmp.path(), "src/auth").unwrap();
        hash_storage::save(
            tmp.path(),
            &[ModuleHashEntry {
                module_name: "auth".into(),
                module_path: "src/auth".into(),
                code_hash: hash,
                file_count: 1,
                total_size: 0,
                max_mtime: 0,
                computed_at: "2026-05-12T00:00:00Z".into(),
            }],
        )
        .unwrap();
        // Bump content → hash differs.
        write_source(tmp.path(), "src/auth/index.ts", "export const x = 1");

        let analysis = make_analysis(vec![make_module("auth", "src/auth")]);
        let dashboard = build_dashboard(&analysis, tmp.path());
        assert_eq!(dashboard.stats.stale_count, 1);
        assert_eq!(dashboard.modules[0].context_status, ContextStatus::Stale);
    }

    #[test]
    fn missing_when_module_not_in_snapshot_but_others_are() {
        let tmp = TempDir::new().unwrap();
        write_source(tmp.path(), "src/auth/index.ts", "x");
        write_source(tmp.path(), "src/new_module/index.ts", "y");

        let (hash, _) = calculate_module_hash(tmp.path(), "src/auth").unwrap();
        hash_storage::save(
            tmp.path(),
            &[ModuleHashEntry {
                module_name: "auth".into(),
                module_path: "src/auth".into(),
                code_hash: hash,
                file_count: 1,
                total_size: 0,
                max_mtime: 0,
                computed_at: "2026-05-12T00:00:00Z".into(),
            }],
        )
        .unwrap();

        let analysis = make_analysis(vec![
            make_module("auth", "src/auth"),
            make_module("new_module", "src/new_module"),
        ]);
        let dashboard = build_dashboard(&analysis, tmp.path());
        assert_eq!(dashboard.stats.fresh_count, 1);
        assert_eq!(dashboard.stats.missing_count, 1);
    }

    #[test]
    fn aggregates_connections_from_dependencies() {
        let tmp = TempDir::new().unwrap();
        let mut m1 = make_module("mod1", "mod1");
        m1.architecture.dependencies = vec!["mod2".to_string()];
        let m2 = make_module("mod2", "mod2");

        let analysis = make_analysis(vec![m1, m2]);
        let dashboard = build_dashboard(&analysis, tmp.path());
        assert_eq!(dashboard.stats.total_modules, 2);
        assert_eq!(dashboard.stats.total_connections, 1);
        assert_eq!(dashboard.stats.missing_count, 2);
    }
}
