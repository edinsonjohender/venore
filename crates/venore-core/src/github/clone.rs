//! GitHub Clone — clone a repository with progress reporting.

use std::path::{Path, PathBuf};

use regex::Regex;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::info;

use crate::error::{Result, VenoreError};

/// Progress update during a clone operation.
#[derive(Debug, Clone)]
pub struct CloneProgress {
    pub phase: String,
    pub percent: Option<u32>,
    pub raw_line: String,
}

/// Clone a repository with progress reporting.
///
/// - `clone_url`: HTTPS clone URL (e.g. `https://github.com/owner/repo.git`)
/// - `dest_dir`: Parent directory where the repo will be cloned into
/// - `repo_name`: Name of the directory to create (e.g. `repo`)
/// - `token`: Optional GitHub token for private repos (passed as an ephemeral
///   auth header via `git_auth`, never persisted to the cloned `.git/config`)
/// - `on_progress`: Callback invoked with progress updates parsed from git stderr
///
/// Returns the full path to the cloned repository.
pub async fn clone_repository<F>(
    clone_url: &str,
    dest_dir: &Path,
    repo_name: &str,
    token: Option<&str>,
    on_progress: F,
) -> Result<PathBuf>
where
    F: Fn(CloneProgress) + Send + 'static,
{
    let target_path = dest_dir.join(repo_name);

    // Validate destination doesn't already exist
    if target_path.exists() {
        return Err(VenoreError::InvalidParams(format!(
            "Destination already exists: {}",
            target_path.display()
        )));
    }

    // Create parent directory if needed
    if !dest_dir.exists() {
        tokio::fs::create_dir_all(dest_dir).await.map_err(|e| {
            VenoreError::FileWriteError(format!(
                "Failed to create directory {}: {}",
                dest_dir.display(),
                e
            ))
        })?;
    }

    info!(repo_name, dest = %dest_dir.display(), "Starting git clone");

    // Authenticate the clone with an ephemeral header (never persisted to
    // `.git/config`, never on argv) so the cloned repo's `origin` stays clean:
    // `https://github.com/owner/repo.git` with no embedded token. See
    // `super::git_auth`. The clean `clone_url` is what git records as origin.
    let mut command = crate::utils::quiet_tokio_command("git");
    for (key, value) in super::git_auth::github_auth_env(token) {
        command.env(key, value);
    }

    let mut child = command
        .args(["clone", "--progress", clone_url, repo_name])
        .current_dir(dest_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to spawn git clone: {}", e)))?;

    // Parse progress from stderr (git clone writes progress to stderr)
    let stderr = child.stderr.take();
    let progress_handle = stderr.map(|stderr| tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            let percent_re = Regex::new(r"(\d+)%").unwrap();

            while let Ok(Some(line)) = lines.next_line().await {
                let percent = percent_re
                    .captures(&line)
                    .and_then(|c| c.get(1))
                    .and_then(|m| m.as_str().parse::<u32>().ok());

                // Extract phase name from lines like "Receiving objects:  45% (123/456)"
                let phase = if line.contains("Enumerating objects") {
                    "Enumerating objects"
                } else if line.contains("Counting objects") {
                    "Counting objects"
                } else if line.contains("Compressing objects") {
                    "Compressing objects"
                } else if line.contains("Receiving objects") {
                    "Receiving objects"
                } else if line.contains("Resolving deltas") {
                    "Resolving deltas"
                } else if line.contains("Updating files") {
                    "Updating files"
                } else {
                    "Cloning"
                };

                on_progress(CloneProgress {
                    phase: phase.to_string(),
                    percent,
                    raw_line: line,
                });
            }
        }));

    let status = child.wait().await.map_err(|e| {
        VenoreError::GitCommandFailed(format!("git clone failed: {}", e))
    })?;

    // Wait for progress reader to finish
    if let Some(handle) = progress_handle {
        let _ = handle.await;
    }

    if !status.success() {
        // Clean up partial clone
        if target_path.exists() {
            let _ = tokio::fs::remove_dir_all(&target_path).await;
        }
        return Err(VenoreError::GitCommandFailed(format!(
            "git clone exited with code: {}",
            status.code().unwrap_or(-1)
        )));
    }

    info!(path = %target_path.display(), "Clone completed successfully");
    Ok(target_path)
}

/// Check if a project directory looks like an already-Venorized project,
/// i.e. it has a committed `.venore/project.json`. Used by the clone flow
/// to decide whether to open the workspace directly (committed snapshot) or
/// route the user into the onboarding wizard (fresh codebase).
pub fn has_venore_project(project_path: &Path) -> bool {
    project_path.join(".venore").join("project.json").exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn has_venore_project_false_when_dir_empty() {
        let dir = TempDir::new().unwrap();
        assert!(!has_venore_project(dir.path()));
    }

    #[test]
    fn has_venore_project_false_when_only_dotvenore_dir_exists() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join(".venore")).unwrap();
        assert!(!has_venore_project(dir.path()), "the dir alone shouldn't count");
    }

    #[test]
    fn has_venore_project_true_when_project_json_present() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join(".venore")).unwrap();
        fs::write(
            dir.path().join(".venore").join("project.json"),
            "{\"id\":\"00000000-0000-0000-0000-000000000000\",\"name\":\"x\",\"created_at\":\"2026-01-01T00:00:00Z\"}",
        )
        .unwrap();
        assert!(has_venore_project(dir.path()));
    }

    #[test]
    fn has_venore_project_does_not_create_files() {
        let dir = TempDir::new().unwrap();
        let _ = has_venore_project(dir.path());
        assert!(!dir.path().join(".venore").exists(), "must be a pure read");
    }
}
