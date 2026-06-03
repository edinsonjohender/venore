//! Project Service
//!
//! Reads and writes `.venore/project.json` — the stable project identity file.

use std::fs;
use std::path::Path;

use crate::{Result, VenoreError};
use super::identity::ProjectIdentity;

const VENORE_DIR: &str = ".venore";
const PROJECT_FILE: &str = "project.json";

/// Service for reading/writing `.venore/project.json`
pub struct ProjectService;

impl ProjectService {
    /// Strict identity read: returns the identity only if `.venore/project.json`
    /// exists on disk. Never creates anything.
    ///
    /// Used by the "open existing project" flow: opening an arbitrary folder
    /// should not silently turn it into a Venore project — that's the wizard's
    /// job. Returns `NotFound` when the file is absent so the caller can
    /// distinguish "not a Venore project" from real read errors.
    pub fn read_identity_strict(project_path: &Path) -> Result<ProjectIdentity> {
        let project_file = project_path.join(VENORE_DIR).join(PROJECT_FILE);
        if !project_file.exists() {
            return Err(VenoreError::NotFound(format!(
                "{} is not a Venore project (.venore/project.json missing)",
                project_path.display()
            )));
        }

        let content = fs::read_to_string(&project_file).map_err(|e| {
            VenoreError::FileReadError(format!(
                "Failed to read {}: {}",
                project_file.display(),
                e
            ))
        })?;

        let identity: ProjectIdentity = serde_json::from_str(&content).map_err(|e| {
            VenoreError::ParseError(format!(
                "Failed to parse {}: {}",
                project_file.display(),
                e
            ))
        })?;

        tracing::debug!(
            "Read project identity (strict): {} ({})",
            identity.name,
            identity.id
        );
        Ok(identity)
    }

    /// Read existing identity or auto-generate one for legacy projects.
    ///
    /// - If `.venore/project.json` exists, reads and returns it.
    /// - If `.venore/` exists but `project.json` doesn't (legacy), auto-generates a UUID
    ///   using the directory name as the project name.
    /// - If `.venore/` doesn't exist, returns an error.
    pub fn read_or_create_identity(project_path: &Path) -> Result<ProjectIdentity> {
        let venore_dir = project_path.join(VENORE_DIR);
        let project_file = venore_dir.join(PROJECT_FILE);

        if project_file.exists() {
            // Read existing identity
            let content = fs::read_to_string(&project_file)
                .map_err(|e| VenoreError::FileReadError(
                    format!("Failed to read {}: {}", project_file.display(), e)
                ))?;

            let identity: ProjectIdentity = serde_json::from_str(&content)
                .map_err(|e| VenoreError::ParseError(
                    format!("Failed to parse {}: {}", project_file.display(), e)
                ))?;

            tracing::debug!("Read project identity: {} ({})", identity.name, identity.id);
            return Ok(identity);
        }

        if venore_dir.exists() {
            // Legacy project: .venore/ exists but no project.json
            let name = project_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown-project")
                .to_string();

            tracing::info!("Legacy project detected, auto-generating identity for '{}'", name);
            let identity = ProjectIdentity::new(&name);
            Self::write_identity(&project_file, &identity)?;
            return Ok(identity);
        }

        Err(VenoreError::DirectoryNotFound(
            format!(".venore directory not found at {}", venore_dir.display())
        ))
    }

    /// Create a new project identity. Creates `.venore/` if needed.
    pub fn create_identity(project_path: &Path, name: &str) -> Result<ProjectIdentity> {
        let venore_dir = project_path.join(VENORE_DIR);
        let project_file = venore_dir.join(PROJECT_FILE);

        // If identity already exists, return it
        if project_file.exists() {
            let content = fs::read_to_string(&project_file)
                .map_err(|e| VenoreError::FileReadError(
                    format!("Failed to read {}: {}", project_file.display(), e)
                ))?;

            let identity: ProjectIdentity = serde_json::from_str(&content)
                .map_err(|e| VenoreError::ParseError(
                    format!("Failed to parse {}: {}", project_file.display(), e)
                ))?;

            tracing::debug!("Project identity already exists: {} ({})", identity.name, identity.id);
            return Ok(identity);
        }

        // Create .venore/ if needed
        if !venore_dir.exists() {
            fs::create_dir_all(&venore_dir)
                .map_err(|e| VenoreError::FileWriteError(
                    format!("Failed to create {}: {}", venore_dir.display(), e)
                ))?;
        }

        let identity = ProjectIdentity::new(name);
        Self::write_identity(&project_file, &identity)?;

        tracing::info!("Created project identity: {} ({})", identity.name, identity.id);
        Ok(identity)
    }

    /// Atomic write: temp file + rename.
    /// Delegates to `utils::atomic_json` so every `.venore/*.json` writer
    /// shares the same crash-safe primitive.
    fn write_identity(project_file: &Path, identity: &ProjectIdentity) -> Result<()> {
        crate::utils::atomic_json::write_atomic(project_file, identity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_identity() {
        let dir = TempDir::new().unwrap();
        let identity = ProjectService::create_identity(dir.path(), "test-project").unwrap();

        assert_eq!(identity.name, "test-project");
        assert!(!identity.id.is_nil());

        // .venore/project.json should exist
        assert!(dir.path().join(".venore/project.json").exists());
    }

    #[test]
    fn test_create_identity_idempotent() {
        let dir = TempDir::new().unwrap();
        let first = ProjectService::create_identity(dir.path(), "test-project").unwrap();
        let second = ProjectService::create_identity(dir.path(), "different-name").unwrap();

        // Should return the same identity (doesn't overwrite)
        assert_eq!(first.id, second.id);
        assert_eq!(first.name, second.name);
    }

    #[test]
    fn test_read_or_create_with_existing_identity() {
        let dir = TempDir::new().unwrap();
        let created = ProjectService::create_identity(dir.path(), "my-project").unwrap();
        let read = ProjectService::read_or_create_identity(dir.path()).unwrap();

        assert_eq!(created.id, read.id);
        assert_eq!(created.name, read.name);
    }

    #[test]
    fn test_read_or_create_legacy_project() {
        let dir = TempDir::new().unwrap();
        // Create .venore/ without project.json (legacy project)
        fs::create_dir_all(dir.path().join(".venore")).unwrap();

        let identity = ProjectService::read_or_create_identity(dir.path()).unwrap();

        // Should auto-generate
        assert!(!identity.id.is_nil());

        // Should now persist
        assert!(dir.path().join(".venore/project.json").exists());

        // Reading again should return the same ID
        let again = ProjectService::read_or_create_identity(dir.path()).unwrap();
        assert_eq!(identity.id, again.id);
    }

    #[test]
    fn test_read_or_create_no_venore_dir() {
        let dir = TempDir::new().unwrap();
        // No .venore/ directory
        let result = ProjectService::read_or_create_identity(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_read_identity_strict_missing_returns_not_found() {
        let dir = TempDir::new().unwrap();
        let err = ProjectService::read_identity_strict(dir.path()).unwrap_err();
        assert!(matches!(err, VenoreError::NotFound(_)));
    }

    #[test]
    fn test_read_identity_strict_does_not_create_files() {
        let dir = TempDir::new().unwrap();
        // .venore/ doesn't exist yet.
        let _ = ProjectService::read_identity_strict(dir.path());
        assert!(!dir.path().join(".venore").exists(), "must not create .venore/");
    }

    #[test]
    fn test_read_identity_strict_reads_existing() {
        let dir = TempDir::new().unwrap();
        let created = ProjectService::create_identity(dir.path(), "real-project").unwrap();
        let read = ProjectService::read_identity_strict(dir.path()).unwrap();
        assert_eq!(created.id, read.id);
        assert_eq!(created.name, read.name);
    }

    #[test]
    fn test_write_read_roundtrip() {
        let dir = TempDir::new().unwrap();
        let created = ProjectService::create_identity(dir.path(), "roundtrip-test").unwrap();

        // Read the file directly
        let content = fs::read_to_string(dir.path().join(".venore/project.json")).unwrap();
        let parsed: ProjectIdentity = serde_json::from_str(&content).unwrap();

        assert_eq!(created.id, parsed.id);
        assert_eq!(created.name, parsed.name);
    }
}
