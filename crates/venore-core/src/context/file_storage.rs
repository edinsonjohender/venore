//! Portable module-layers persistence to `<project_path>/.venore/module-layers.json`.
//!
//! The file is the source of truth when present; the SQLite `module_layers`
//! rows are dual-written as backup and used as fallback when the file is
//! missing (silent migration on first read).
//!
//! Owner of `<project_path>/.venore/module-layers.json` — no other module
//! should read or write that path directly.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::utils::atomic_json;
use crate::{Result, VenoreError};

use super::repository::{ContextRepository, ModuleLayerRecord};

const VENORE_DIR: &str = ".venore";
const LAYERS_FILE: &str = "module-layers.json";

const CURRENT_SCHEMA_VERSION: u32 = 1;

fn default_schema_version() -> u32 {
    CURRENT_SCHEMA_VERSION
}

/// On-disk envelope. `schema_version` at the top lets us evolve the shape
/// later without breaking existing files. Layers are sorted by
/// `(module_name, layer_type)` so the JSON has a deterministic diff in git.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct ModuleLayersFile {
    #[serde(default = "default_schema_version")]
    schema_version: u32,
    /// RFC3339 timestamp of when this snapshot was written (project-wide).
    /// Different from per-layer `analyzed_at` so we can tell when the whole
    /// file was last regenerated vs when individual modules were re-analyzed.
    generated_at: String,
    layers: Vec<ModuleLayerEntry>,
}

/// One row of `module_layers` in portable, git-friendly form.
///
/// Differences from `ModuleLayerRecord`:
///   - `id` is dropped (DB primary key, redundant on disk).
///   - `project_id` is dropped (the file lives inside the project, so the
///     owner is implicit from its location).
///   - `details_json: String` becomes `details: serde_json::Value` so the
///     file stays human-readable instead of containing escaped JSON strings.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct ModuleLayerEntry {
    module_name: String,
    module_path: String,
    /// context|tests|documentation|connections|status
    layer_type: String,
    /// complete|partial|missing
    status: String,
    #[serde(default)]
    details: serde_json::Value,
    analyzed_at: String,
}

/// Resolve the path to the portable module-layers file inside a project.
pub fn path_for(project_path: &Path) -> PathBuf {
    project_path.join(VENORE_DIR).join(LAYERS_FILE)
}

/// Whether the portable module-layers file exists on disk.
pub fn exists(project_path: &Path) -> bool {
    path_for(project_path).exists()
}

/// Load layers from `<project_path>/.venore/module-layers.json`.
///
/// Returns hydrated `ModuleLayerRecord`s with `project_id` injected from the
/// argument (the file doesn't store it — its owner is implicit from location).
/// `id` is left empty since no caller of `get_all_layers` depends on it.
///
/// Corrupt files are renamed to `module-layers.json.corrupt` and treated as
/// missing, matching the recovery behavior of the other `.venore/*.json` files.
pub fn load(project_path: &Path, project_id: &str) -> Result<Option<Vec<ModuleLayerRecord>>> {
    let path = path_for(project_path);
    let envelope: Option<ModuleLayersFile> = atomic_json::read_or_backup_corrupt(&path)?;

    Ok(envelope.map(|env| {
        env.layers
            .into_iter()
            .map(|e| ModuleLayerRecord {
                id: String::new(),
                project_id: project_id.to_string(),
                module_name: e.module_name,
                module_path: e.module_path,
                layer_type: e.layer_type,
                status: e.status,
                // Repository expects a stringified blob; convert back so
                // downstream consumers see the exact same shape they get
                // from the DB read path.
                details_json: serde_json::to_string(&e.details).unwrap_or_else(|_| "{}".into()),
                analyzed_at: e.analyzed_at,
            })
            .collect()
    }))
}

/// Save layers to `<project_path>/.venore/module-layers.json` atomically.
///
/// Sorts by `(module_name, layer_type)` before serialization to keep git
/// diffs minimal when only one module changes.
pub fn save(project_path: &Path, records: &[ModuleLayerRecord]) -> Result<()> {
    let mut sorted: Vec<&ModuleLayerRecord> = records.iter().collect();
    sorted.sort_by(|a, b| {
        a.module_name
            .cmp(&b.module_name)
            .then_with(|| a.layer_type.cmp(&b.layer_type))
    });

    let layers: Vec<ModuleLayerEntry> = sorted
        .into_iter()
        .map(|r| ModuleLayerEntry {
            module_name: r.module_name.clone(),
            module_path: r.module_path.clone(),
            layer_type: r.layer_type.clone(),
            status: r.status.clone(),
            details: serde_json::from_str(&r.details_json).unwrap_or(serde_json::Value::Null),
            analyzed_at: r.analyzed_at.clone(),
        })
        .collect();

    let envelope = ModuleLayersFile {
        schema_version: CURRENT_SCHEMA_VERSION,
        generated_at: chrono::Utc::now().to_rfc3339(),
        layers,
    };

    let path = path_for(project_path);
    atomic_json::write_atomic(&path, &envelope)
}

/// Delete the portable file. No-op if it doesn't exist.
pub fn delete(project_path: &Path) -> Result<()> {
    let path = path_for(project_path);
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(&path).map_err(|e| {
        VenoreError::FileWriteError(format!("Failed to delete {}: {}", path.display(), e))
    })
}

/// File-first read with DB fallback and silent migration.
///
/// Used by the ocean canvas and the chat context builder so neither has to
/// inline the file/DB selection logic.
///
///   1. If the portable file exists and parses, return its layers.
///   2. Otherwise read from the DB.
///   3. If the DB had layers and the file was missing, write the file so the
///      next read is fast and the snapshot starts traveling with the repo
///      (silent migration). Migration errors are swallowed with a warning —
///      reads must never fail just because the FS is read-only.
pub async fn load_layers_file_first(
    project_path: &Path,
    project_id: &str,
    repo: &ContextRepository,
) -> Result<Vec<ModuleLayerRecord>> {
    if let Some(records) = load(project_path, project_id)? {
        tracing::debug!(
            project_id,
            count = records.len(),
            "Loaded module layers from .venore/module-layers.json"
        );
        return Ok(records);
    }

    let from_db = repo.get_all_layers(project_id).await?;
    if !from_db.is_empty() {
        if let Err(e) = save(project_path, &from_db) {
            tracing::warn!(
                project_id,
                error = %e,
                "Could not migrate module layers to .venore/module-layers.json (DB-only mode)"
            );
        } else {
            tracing::info!(
                project_id,
                count = from_db.len(),
                "Migrated module layers to .venore/module-layers.json"
            );
        }
    }
    Ok(from_db)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn record(module: &str, layer_type: &str, status: &str, details_json: &str) -> ModuleLayerRecord {
        ModuleLayerRecord {
            id: format!("id-{}-{}", module, layer_type),
            project_id: "proj-1".into(),
            module_name: module.into(),
            module_path: format!("src/{}", module),
            layer_type: layer_type.into(),
            status: status.into(),
            details_json: details_json.into(),
            analyzed_at: "2026-05-12T08:00:00Z".into(),
        }
    }

    #[test]
    fn roundtrip_preserves_records_and_injects_project_id() {
        let dir = TempDir::new().unwrap();
        let records = vec![
            record("Button", "status", "complete", r#"{"hint":"ok"}"#),
            record("Button", "tests", "missing", "{}"),
            record("Card", "status", "partial", r#"{"reason":"no docs"}"#),
        ];
        save(dir.path(), &records).unwrap();

        // load() injects project_id from the argument since the file doesn't
        // store it; assert all entries pick it up.
        let loaded = load(dir.path(), "different-project").unwrap().unwrap();
        assert_eq!(loaded.len(), 3);
        for r in &loaded {
            assert_eq!(r.project_id, "different-project");
            assert!(r.id.is_empty(), "id is intentionally dropped on the file");
        }
        // Spot-check one entry.
        let button_status = loaded.iter().find(|r| r.module_name == "Button" && r.layer_type == "status").unwrap();
        assert_eq!(button_status.status, "complete");
        let parsed: serde_json::Value = serde_json::from_str(&button_status.details_json).unwrap();
        assert_eq!(parsed["hint"], "ok");
    }

    #[test]
    fn save_sorts_entries_deterministically() {
        let dir = TempDir::new().unwrap();
        let records = vec![
            record("Card", "tests", "missing", "{}"),
            record("Button", "status", "complete", "{}"),
            record("Card", "status", "complete", "{}"),
            record("Button", "tests", "partial", "{}"),
        ];
        save(dir.path(), &records).unwrap();

        let raw = fs::read_to_string(path_for(dir.path())).unwrap();
        // Expected serialization order: Button/status, Button/tests, Card/status, Card/tests
        let button_status = raw.find("\"Button\"").unwrap();
        let card_status = raw.find("\"Card\"").unwrap();
        assert!(button_status < card_status, "Modules sorted by name");

        // Within Button, status comes before tests alphabetically.
        let btn_block: &str = &raw[button_status..card_status];
        assert!(btn_block.find("status").unwrap() < btn_block.find("tests").unwrap());
    }

    #[test]
    fn details_round_trip_as_object_not_escaped_string() {
        let dir = TempDir::new().unwrap();
        let records = vec![record("Modal", "documentation", "partial", r#"{"missing":["api","examples"]}"#)];
        save(dir.path(), &records).unwrap();

        // The file must store `details` as a real object, not a string-of-json.
        let raw = fs::read_to_string(path_for(dir.path())).unwrap();
        assert!(raw.contains("\"missing\""), "details unescaped on disk");
        assert!(raw.contains("[\n"), "pretty-printed");
        // And on load it must restringify back to the DB shape.
        let loaded = load(dir.path(), "p").unwrap().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&loaded[0].details_json).unwrap();
        assert_eq!(parsed["missing"][1], "examples");
    }

    #[test]
    fn load_missing_file_returns_none() {
        let dir = TempDir::new().unwrap();
        assert!(load(dir.path(), "p").unwrap().is_none());
        assert!(!exists(dir.path()));
    }

    #[test]
    fn load_corrupt_file_backs_up_and_returns_none() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join(VENORE_DIR)).unwrap();
        let path = path_for(dir.path());
        fs::write(&path, "{not valid").unwrap();

        let result = load(dir.path(), "p").unwrap();
        assert!(result.is_none());
        assert!(!path.exists());
        assert!(path.with_extension("json.corrupt").exists());
    }

    #[test]
    fn save_creates_venore_dir_if_missing() {
        let dir = TempDir::new().unwrap();
        save(dir.path(), &[record("A", "status", "complete", "{}")]).unwrap();
        assert!(exists(dir.path()));
    }

    #[test]
    fn schema_version_at_top_and_generated_at_present() {
        let dir = TempDir::new().unwrap();
        save(dir.path(), &[record("A", "status", "complete", "{}")]).unwrap();
        let raw = fs::read_to_string(path_for(dir.path())).unwrap();
        assert!(raw.contains("\"schema_version\""));
        assert!(raw.contains("\"generated_at\""));
        assert!(raw.contains("\"layers\""));
    }

    #[test]
    fn delete_is_idempotent() {
        let dir = TempDir::new().unwrap();
        save(dir.path(), &[record("A", "status", "complete", "{}")]).unwrap();
        delete(dir.path()).unwrap();
        assert!(!exists(dir.path()));
        delete(dir.path()).unwrap(); // no-op
    }
}
