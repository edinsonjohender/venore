//! Branch monitor — detect new commits via local git operations.
//!
//! Uses `git fetch`, `git rev-parse`, `git diff --name-only`, and `git log`
//! to detect changes without switching the user's current branch.

use std::path::Path;

use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::error::{Result, VenoreError};

/// Summary of a single commit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitSummary {
    pub hash: String,
    pub short_hash: String,
    pub message: String,
}

/// Fetch the latest refs for a branch from origin.
///
/// Runs: `git fetch origin {branch}`
pub fn fetch_branch(project_path: &Path, branch: &str) -> Result<()> {
    info!(branch, "Fetching origin/{}", branch);

    let output = crate::utils::quiet_command("git")
        .args(["fetch", "origin", branch])
        .current_dir(project_path)
        .output()
        .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to run git fetch: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(VenoreError::GitCommandFailed(
            format!("git fetch origin {} failed: {}", branch, stderr),
        ));
    }

    debug!(branch, "Fetch completed");
    Ok(())
}

/// Get the SHA of the HEAD of origin/{branch}.
///
/// Runs: `git rev-parse origin/{branch}`
pub fn get_latest_remote_commit(project_path: &Path, branch: &str) -> Result<String> {
    let ref_name = format!("origin/{}", branch);

    let output = crate::utils::quiet_command("git")
        .args(["rev-parse", &ref_name])
        .current_dir(project_path)
        .output()
        .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to run git rev-parse: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(VenoreError::GitCommandFailed(
            format!("git rev-parse {} failed: {}", ref_name, stderr),
        ));
    }

    let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
    debug!(sha = %sha, "Latest remote commit for {}", ref_name);
    Ok(sha)
}

/// Get the list of files changed between two commits.
///
/// Runs: `git diff --name-only {base_commit}..origin/{branch}`
/// Returns relative paths (e.g. `src/auth/login.ts`).
pub fn get_changed_files(
    project_path: &Path,
    base_commit: &str,
    branch: &str,
) -> Result<Vec<String>> {
    let range = format!("{}..origin/{}", base_commit, branch);

    let output = crate::utils::quiet_command("git")
        .args(["diff", "--name-only", &range])
        .current_dir(project_path)
        .output()
        .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to run git diff: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(VenoreError::GitCommandFailed(
            format!("git diff --name-only {} failed: {}", range, stderr),
        ));
    }

    let files: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    info!(count = files.len(), "Changed files in {}", range);
    Ok(files)
}

/// Get commit summaries between two points.
///
/// Runs: `git log --oneline {base_commit}..origin/{branch}`
pub fn get_new_commits(
    project_path: &Path,
    base_commit: &str,
    branch: &str,
) -> Result<Vec<CommitSummary>> {
    let range = format!("{}..origin/{}", base_commit, branch);

    let output = crate::utils::quiet_command("git")
        .args(["log", "--oneline", &range])
        .current_dir(project_path)
        .output()
        .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to run git log: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(VenoreError::GitCommandFailed(
            format!("git log {} failed: {}", range, stderr),
        ));
    }

    let commits: Vec<CommitSummary> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let (short_hash, message) = line.split_once(' ')?;
            Some(CommitSummary {
                hash: short_hash.to_string(), // oneline only gives short hash
                short_hash: short_hash.to_string(),
                message: message.to_string(),
            })
        })
        .collect();

    info!(count = commits.len(), "New commits in {}", range);
    Ok(commits)
}

/// Get a base commit SHA for initial sync (e.g. ~20 commits back).
///
/// Runs: `git rev-parse origin/{branch}~{count}`
/// Falls back to the oldest reachable commit if the branch has fewer commits.
pub fn get_initial_base_commit(
    project_path: &Path,
    branch: &str,
    lookback: u32,
) -> Result<String> {
    let ref_name = format!("origin/{}~{}", branch, lookback);

    let output = crate::utils::quiet_command("git")
        .args(["rev-parse", &ref_name])
        .current_dir(project_path)
        .output()
        .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to run git rev-parse: {}", e)))?;

    if output.status.success() {
        let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
        debug!(sha = %sha, "Initial base commit ({})", ref_name);
        return Ok(sha);
    }

    // Branch has fewer commits than lookback — use first commit
    let output = crate::utils::quiet_command("git")
        .args(["rev-list", "--max-parents=0", &format!("origin/{}", branch)])
        .current_dir(project_path)
        .output()
        .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to run git rev-list: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(VenoreError::GitCommandFailed(
            format!("git rev-list failed: {}", stderr),
        ));
    }

    let sha = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();

    debug!(sha = %sha, "Initial base commit (root of origin/{})", branch);
    Ok(sha)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commit_summary_serialization() {
        let commit = CommitSummary {
            hash: "abc1234".to_string(),
            short_hash: "abc1234".to_string(),
            message: "feat: add login".to_string(),
        };
        let json = serde_json::to_string(&commit).unwrap();
        assert!(json.contains("abc1234"));
        assert!(json.contains("feat: add login"));

        let parsed: CommitSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.short_hash, "abc1234");
    }
}
