//! Atomic JSON read/write primitives.
//!
//! Shared by every `.venore/*.json` writer in the codebase
//! (project identity, ocean layout, analysis output, checkpoint, project memory).
//! Centralizes:
//!   - `to_string_pretty` as the single canonical encoding,
//!   - temp-file + rename for crash-safe writes,
//!   - corrupt-on-parse → backup-and-treat-as-missing recovery.
//!
//! Callers keep their own `tracing` calls so log context (which file,
//! which subsystem) is preserved.

use std::fs;
use std::path::Path;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::{Result, VenoreError};

/// Atomic write: serialize as pretty JSON, write to `<path>.tmp`, then rename.
///
/// Creates the parent directory if it doesn't exist. On NTFS and POSIX
/// `fs::rename` is atomic, so a reader never sees a half-written file.
pub fn write_atomic<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| {
                VenoreError::FileWriteError(format!(
                    "Failed to create directory {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }
    }

    let temp_path = path.with_extension("tmp");
    let content = serde_json::to_string_pretty(value)?;
    fs::write(&temp_path, &content).map_err(|e| {
        VenoreError::FileWriteError(format!(
            "Failed to write {}: {}",
            temp_path.display(),
            e
        ))
    })?;
    fs::rename(&temp_path, path).map_err(|e| {
        VenoreError::FileWriteError(format!(
            "Failed to rename {} to {}: {}",
            temp_path.display(),
            path.display(),
            e
        ))
    })?;

    Ok(())
}

/// Read + parse JSON, backing up the file as `<name>.json.corrupt` on parse error.
///
/// Returns:
///   - `Ok(Some(value))` on successful parse.
///   - `Ok(None)` if the file is missing OR if it existed but failed to parse
///     (in which case the corrupted file is moved aside so the next write
///     can recreate a clean version).
///   - `Err(...)` only on filesystem read errors that are not "not found".
pub fn read_or_backup_corrupt<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(path).map_err(|e| {
        VenoreError::FileReadError(format!("Failed to read {}: {}", path.display(), e))
    })?;

    match serde_json::from_str::<T>(&content) {
        Ok(value) => Ok(Some(value)),
        Err(_) => {
            let backup = path.with_extension("json.corrupt");
            let _ = fs::rename(path, &backup);
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use tempfile::TempDir;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Sample {
        name: String,
        count: u32,
    }

    #[test]
    fn write_atomic_temp_then_rename() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("sample.json");
        let value = Sample { name: "ok".into(), count: 3 };

        write_atomic(&path, &value).unwrap();

        assert!(path.exists());
        assert!(!path.with_extension("tmp").exists(), "no orphan .tmp left");

        let loaded: Sample = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(loaded, value);
    }

    #[test]
    fn write_atomic_creates_parent_directory() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nested").join("dir").join("sample.json");
        let value = Sample { name: "deep".into(), count: 7 };

        write_atomic(&path, &value).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn read_or_backup_corrupt_renames_on_parse_error() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("broken.json");
        fs::write(&path, "{garbage").unwrap();

        let result: Option<Sample> = read_or_backup_corrupt(&path).unwrap();
        assert!(result.is_none());
        assert!(!path.exists(), "original file moved aside");
        assert!(path.with_extension("json.corrupt").exists(), "backup created");
    }

    #[test]
    fn read_or_backup_corrupt_returns_none_on_missing() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.json");

        let result: Option<Sample> = read_or_backup_corrupt(&path).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn roundtrip_through_helpers() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("round.json");
        let value = Sample { name: "roundtrip".into(), count: 42 };

        write_atomic(&path, &value).unwrap();
        let loaded: Option<Sample> = read_or_backup_corrupt(&path).unwrap();

        assert_eq!(loaded.as_ref(), Some(&value));
    }
}
