//! Portable project-memory persistence to `<project_path>/.venore/project-memory.json`.
//!
//! The file is the source of truth when present; the SQLite `project_memory`
//! row is dual-written as backup and used as fallback when the file is
//! missing (silent migration on first read).
//!
//! Owner of `<project_path>/.venore/project-memory.json` — no other module
//! should read or write that path directly.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::utils::atomic_json;
use crate::{Result, VenoreError};

use super::models::ProjectMemory;

const VENORE_DIR: &str = ".venore";
const MEMORY_FILE: &str = "project-memory.json";

const CURRENT_SCHEMA_VERSION: u32 = 1;

fn default_schema_version() -> u32 {
    CURRENT_SCHEMA_VERSION
}

/// On-disk envelope for `ProjectMemory`.
///
/// Adds a `schema_version` tag at the top while keeping the underlying
/// `ProjectMemory` shape unchanged thanks to `#[serde(flatten)]`. This lets
/// us evolve the schema later without breaking existing files.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct ProjectMemoryFile {
    #[serde(default = "default_schema_version")]
    schema_version: u32,
    #[serde(flatten)]
    memory: ProjectMemory,
}

/// Resolve the path to the project-memory file inside a project.
pub fn path_for(project_path: &Path) -> PathBuf {
    project_path.join(VENORE_DIR).join(MEMORY_FILE)
}

/// Whether the project-memory file exists on disk.
pub fn exists(project_path: &Path) -> bool {
    path_for(project_path).exists()
}

/// Load the project memory from `<project_path>/.venore/project-memory.json`.
///
/// Returns `Ok(None)` if the file is missing or corrupted. Corrupt files are
/// renamed to `project-memory.json.corrupt` so the next save can recreate
/// a clean version without losing forensic state.
pub fn load(project_path: &Path) -> Result<Option<ProjectMemory>> {
    let path = path_for(project_path);
    let envelope: Option<ProjectMemoryFile> = atomic_json::read_or_backup_corrupt(&path)?;
    Ok(envelope.map(|env| env.memory))
}

/// Save the project memory to `<project_path>/.venore/project-memory.json`.
///
/// Atomic via temp + rename. Race window between two writers is "last writer
/// wins" — same effective behavior as the SQLite `UPSERT`, so no file
/// locking is added.
pub fn save(project_path: &Path, memory: &ProjectMemory) -> Result<()> {
    let path = path_for(project_path);
    let envelope = ProjectMemoryFile {
        schema_version: CURRENT_SCHEMA_VERSION,
        memory: memory.clone(),
    };
    atomic_json::write_atomic(&path, &envelope)
}

/// Delete the project-memory file. No-op if the file doesn't exist.
pub fn delete(project_path: &Path) -> Result<()> {
    let path = path_for(project_path);
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(&path).map_err(|e| {
        VenoreError::FileWriteError(format!("Failed to delete {}: {}", path.display(), e))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_memory() -> ProjectMemory {
        ProjectMemory {
            id: "mem-1".into(),
            project_id: "proj-1".into(),
            name: "demo".into(),
            description: "A demo project for tests.".into(),
            state: "active".into(),
            team_size: "1-3".into(),
            goals: vec!["understand".into(), "document".into()],
            architecture: "Rust workspace + Tauri".into(),
            tech_debt: "Some duplicated atomic-write code.".into(),
            response_language: "es".into(),
            conventions: vec!["no unwrap in libs".into()],
            project_summary: "# Demo\nLine two.".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-05-12T00:00:00Z".into(),
        }
    }

    #[test]
    fn roundtrip_memory_preserves_all_fields() {
        let dir = TempDir::new().unwrap();
        let memory = sample_memory();

        save(dir.path(), &memory).unwrap();
        let loaded = load(dir.path()).unwrap().expect("should load");

        assert_eq!(loaded.id, memory.id);
        assert_eq!(loaded.project_id, memory.project_id);
        assert_eq!(loaded.name, memory.name);
        assert_eq!(loaded.description, memory.description);
        assert_eq!(loaded.state, memory.state);
        assert_eq!(loaded.team_size, memory.team_size);
        assert_eq!(loaded.goals, memory.goals);
        assert_eq!(loaded.architecture, memory.architecture);
        assert_eq!(loaded.tech_debt, memory.tech_debt);
        assert_eq!(loaded.response_language, memory.response_language);
        assert_eq!(loaded.conventions, memory.conventions);
        assert_eq!(loaded.project_summary, memory.project_summary);
        assert_eq!(loaded.created_at, memory.created_at);
        assert_eq!(loaded.updated_at, memory.updated_at);
    }

    #[test]
    fn load_missing_file_returns_none() {
        let dir = TempDir::new().unwrap();
        assert!(load(dir.path()).unwrap().is_none());
        assert!(!exists(dir.path()));
    }

    #[test]
    fn load_corrupt_file_backs_up_and_returns_none() {
        let dir = TempDir::new().unwrap();
        let venore_dir = dir.path().join(VENORE_DIR);
        fs::create_dir_all(&venore_dir).unwrap();
        let path = venore_dir.join(MEMORY_FILE);
        fs::write(&path, "{garbage").unwrap();

        let result = load(dir.path()).unwrap();
        assert!(result.is_none());
        assert!(!path.exists(), "corrupt file moved aside");
        assert!(path.with_extension("json.corrupt").exists());
    }

    #[test]
    fn save_creates_venore_dir_if_missing() {
        let dir = TempDir::new().unwrap();
        // No .venore/ directory yet.
        save(dir.path(), &sample_memory()).unwrap();
        assert!(dir.path().join(VENORE_DIR).is_dir());
        assert!(exists(dir.path()));
    }

    #[test]
    fn schema_version_defaults_to_1_when_absent() {
        // A legacy file without the `schema_version` field must still parse
        // (the `#[serde(default)]` on the envelope guarantees this).
        let dir = TempDir::new().unwrap();
        let venore_dir = dir.path().join(VENORE_DIR);
        fs::create_dir_all(&venore_dir).unwrap();
        let memory = sample_memory();
        // Write WITHOUT envelope wrapper to simulate legacy file.
        let raw = serde_json::to_string_pretty(&memory).unwrap();
        fs::write(venore_dir.join(MEMORY_FILE), raw).unwrap();

        let loaded = load(dir.path()).unwrap().expect("legacy file should parse");
        assert_eq!(loaded.id, memory.id);
        assert_eq!(loaded.description, memory.description);
    }

    #[test]
    fn save_writes_schema_version_at_top() {
        // Lock the format: the on-disk JSON must include `schema_version`
        // and the flattened `ProjectMemory` fields side-by-side.
        let dir = TempDir::new().unwrap();
        save(dir.path(), &sample_memory()).unwrap();

        let content = fs::read_to_string(path_for(dir.path())).unwrap();
        assert!(content.contains("\"schema_version\""));
        assert!(content.contains("\"project_id\""));
        assert!(content.contains("\"project_summary\""));
        // Pretty format means newlines + 2-space indentation.
        assert!(content.contains("\n  "));
    }

    #[test]
    fn delete_removes_file_and_is_idempotent() {
        let dir = TempDir::new().unwrap();
        save(dir.path(), &sample_memory()).unwrap();
        assert!(exists(dir.path()));

        delete(dir.path()).unwrap();
        assert!(!exists(dir.path()));

        // Second delete is a no-op.
        delete(dir.path()).unwrap();
    }
}
