//! File editor Tauri commands
//!
//! Read and write project files with path traversal protection.

use std::fs;
use std::path::{Path, PathBuf};

use venore_core::error::VenoreError;

use crate::commands::dto::editor::{ReadFileRequest, ReadFileResponse, WriteFileRequest};
use crate::utils::CommandResult;

/// Validate that a resolved file path is within the project directory.
///
/// For existing files, canonicalizes both paths and checks containment.
/// For new files (write), canonicalizes the parent directory instead.
fn validate_path_within_project(
    project_path: &str,
    relative_path: &str,
    must_exist: bool,
) -> Result<PathBuf, VenoreError> {
    let project_dir = Path::new(project_path)
        .canonicalize()
        .map_err(|_| VenoreError::PathNotSafe(format!("Invalid project path: {}", project_path)))?;

    let target = Path::new(project_path).join(relative_path);

    if must_exist {
        let canonical = target
            .canonicalize()
            .map_err(|_| VenoreError::FileNotFound(relative_path.to_string()))?;

        if !canonical.starts_with(&project_dir) {
            return Err(VenoreError::PathNotSafe(format!(
                "Path escapes project directory: {}",
                relative_path
            )));
        }
        Ok(canonical)
    } else {
        // For new files: canonicalize parent, then append filename
        let parent = target.parent().ok_or_else(|| {
            VenoreError::PathNotSafe(format!("No parent directory: {}", relative_path))
        })?;

        let canonical_parent = parent
            .canonicalize()
            .map_err(|_| VenoreError::PathNotSafe(format!("Parent dir not found: {}", relative_path)))?;

        if !canonical_parent.starts_with(&project_dir) {
            return Err(VenoreError::PathNotSafe(format!(
                "Path escapes project directory: {}",
                relative_path
            )));
        }

        let file_name = target.file_name().ok_or_else(|| {
            VenoreError::PathNotSafe(format!("No file name: {}", relative_path))
        })?;

        Ok(canonical_parent.join(file_name))
    }
}

/// Read a file from a project directory
#[tauri::command]
pub async fn read_file(request: ReadFileRequest) -> CommandResult<ReadFileResponse> {
    tracing::info!(
        project = %request.project_path,
        file = %request.relative_path,
        "read_file"
    );

    let result = (|| -> Result<ReadFileResponse, VenoreError> {
        let path = validate_path_within_project(
            &request.project_path,
            &request.relative_path,
            true,
        )?;

        let metadata = fs::metadata(&path)
            .map_err(|e| VenoreError::FileNotFound(format!("{}: {}", request.relative_path, e)))?;

        let content = fs::read_to_string(&path)
            .map_err(|e| VenoreError::FileReadError(format!("{}: {}", request.relative_path, e)))?;

        Ok(ReadFileResponse {
            content,
            size: metadata.len(),
        })
    })();

    result.into()
}

/// Write a file in a project directory (atomic: temp + rename)
#[tauri::command]
pub async fn write_file(request: WriteFileRequest) -> CommandResult<()> {
    tracing::info!(
        project = %request.project_path,
        file = %request.relative_path,
        "write_file"
    );

    let result = (|| -> Result<(), VenoreError> {
        let path = validate_path_within_project(
            &request.project_path,
            &request.relative_path,
            false,
        )?;

        // Atomic write: write to temp file, then rename
        let temp_path = path.with_extension("tmp.venore");

        fs::write(&temp_path, &request.content).map_err(|e| {
            VenoreError::FileWriteError(format!("{}: {}", request.relative_path, e))
        })?;

        if let Err(e) = fs::rename(&temp_path, &path) {
            // Cleanup temp file on rename failure
            let _ = fs::remove_file(&temp_path);
            return Err(VenoreError::FileWriteError(format!(
                "Rename failed for {}: {}",
                request.relative_path, e
            )));
        }

        Ok(())
    })();

    result.into()
}
