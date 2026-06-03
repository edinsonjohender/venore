//! Git operations for sessions
//!
//! All git CLI calls go through `utils::quiet_command`, which sets
//! `CREATE_NO_WINDOW` on Windows so spawning git doesn't flash a console
//! window (this is a GUI app compiled with `windows_subsystem = "windows"`).

use std::path::Path;

use crate::{Result, VenoreError};
use super::types::{CommitInfo, DiffFile};

/// Check if a path is inside a git repository
pub fn is_git_repo(project_path: &Path) -> bool {
    let result = crate::utils::quiet_command("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(project_path)
        .output();

    match &result {
        Ok(output) => {
            let success = output.status.success();
            if !success {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::warn!(path = %project_path.display(), stderr = %stderr, "is_git_repo: git returned non-zero");
            }
            success
        }
        Err(e) => {
            tracing::error!(path = %project_path.display(), error = %e, "is_git_repo: failed to spawn git");
            false
        }
    }
}

/// Get the current branch name
pub fn get_current_branch(project_path: &Path) -> Result<String> {
    let output = crate::utils::quiet_command("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(project_path)
        .output()
        .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to run git: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(VenoreError::GitCommandFailed(format!("git rev-parse failed: {}", stderr)));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Detect the repository's default branch.
///
/// Prefers the remote's default — `git symbolic-ref --short refs/remotes/origin/HEAD`
/// yields e.g. `origin/master`, set automatically at clone time. Falls back to
/// the currently checked-out branch, then `None` when the path isn't a git repo
/// or the branch can't be determined. Reusable by the clone flow, sessions, and
/// the context updater (so nothing hardcodes "main").
pub fn get_default_branch(project_path: &Path) -> Option<String> {
    let symref = crate::utils::quiet_command("git")
        .args(["symbolic-ref", "--short", "refs/remotes/origin/HEAD"])
        .current_dir(project_path)
        .output()
        .ok()?;

    if symref.status.success() {
        // "origin/master" -> "master" (handles slash-containing branch names).
        let full = String::from_utf8_lossy(&symref.stdout).trim().to_string();
        if let Some((_, branch)) = full.split_once('/') {
            if !branch.is_empty() {
                return Some(branch.to_string());
            }
        }
    }

    // Fallback: the currently checked-out branch (skip detached HEAD).
    get_current_branch(project_path)
        .ok()
        .filter(|b| !b.is_empty() && b != "HEAD")
}

/// List all local branches
pub fn list_local_branches(project_path: &Path) -> Result<Vec<String>> {
    let output = crate::utils::quiet_command("git")
        .args(["branch", "--format=%(refname:short)"])
        .current_dir(project_path)
        .output()
        .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to run git: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(VenoreError::GitCommandFailed(format!("git branch failed: {}", stderr)));
    }

    let branches = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    Ok(branches)
}

/// Compute the worktree directory for a session
pub fn worktree_path(config_dir: &Path, project_id: &str, session_id: &str) -> std::path::PathBuf {
    config_dir.join("worktrees").join(project_id).join(session_id)
}

/// Create a git worktree with a new branch
///
/// Runs: `git -c core.longpaths=true worktree add <worktree_dir> -b <branch> <base>`
/// On failure, cleans up the orphaned branch and partial worktree directory.
pub fn create_worktree(project_path: &Path, worktree_dir: &Path, branch: &str, base: &str) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = worktree_dir.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to create worktree parent dir: {}", e)))?;
    }

    let wt_str = worktree_dir.to_string_lossy();
    let output = crate::utils::quiet_command("git")
        .args(["-c", "core.longpaths=true", "worktree", "add", &wt_str, "-b", branch, base])
        .current_dir(project_path)
        .output()
        .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to run git worktree add: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(branch = %branch, "Worktree add failed, cleaning up orphaned branch/dir");

        // Cleanup: remove partial worktree directory
        if worktree_dir.exists() {
            let _ = std::fs::remove_dir_all(worktree_dir);
        }
        // Cleanup: prune stale worktree metadata
        let _ = prune_worktrees(project_path);
        // Cleanup: delete the orphaned branch (git creates it before checkout)
        let _ = delete_branch(project_path, branch);

        return Err(VenoreError::GitCommandFailed(format!("git worktree add failed: {}", stderr)));
    }

    tracing::info!(worktree = %wt_str, branch = %branch, base = %base, "Created git worktree");
    Ok(())
}

/// Remove a git worktree, with fallback to manual cleanup + prune
pub fn remove_worktree(project_path: &Path, worktree_dir: &Path) -> Result<()> {
    let wt_str = worktree_dir.to_string_lossy();

    let output = crate::utils::quiet_command("git")
        .args(["worktree", "remove", &wt_str, "--force"])
        .current_dir(project_path)
        .output()
        .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to run git worktree remove: {}", e)))?;

    if output.status.success() {
        tracing::info!(worktree = %wt_str, "Removed git worktree");
        return Ok(());
    }

    // Fallback: delete directory manually and prune
    tracing::warn!(worktree = %wt_str, "git worktree remove failed, falling back to manual cleanup");
    if worktree_dir.exists() {
        std::fs::remove_dir_all(worktree_dir)
            .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to remove worktree dir: {}", e)))?;
    }
    prune_worktrees(project_path)?;

    Ok(())
}

/// Delete a local branch (no error if it doesn't exist)
pub fn delete_branch(project_path: &Path, branch: &str) -> Result<()> {
    let output = crate::utils::quiet_command("git")
        .args(["branch", "-D", branch])
        .current_dir(project_path)
        .output()
        .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to run git branch -D: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Not an error if the branch doesn't exist
        tracing::debug!(branch = %branch, stderr = %stderr, "git branch -D returned non-zero (branch may not exist)");
    }

    Ok(())
}

/// Prune stale worktree metadata
pub fn prune_worktrees(project_path: &Path) -> Result<()> {
    let output = crate::utils::quiet_command("git")
        .args(["worktree", "prune"])
        .current_dir(project_path)
        .output()
        .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to run git worktree prune: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(stderr = %stderr, "git worktree prune returned non-zero");
    }

    Ok(())
}

/// Get diff files between base and session branches
pub fn get_diff_files(project_path: &Path, base: &str, session: &str) -> Result<Vec<DiffFile>> {
    // 1. Get numstat (additions/deletions per file)
    let numstat_output = crate::utils::quiet_command("git")
        .args(["diff", "--numstat", &format!("{}..{}", base, session)])
        .current_dir(project_path)
        .output()
        .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to run git diff --numstat: {}", e)))?;

    if !numstat_output.status.success() {
        let stderr = String::from_utf8_lossy(&numstat_output.stderr);
        return Err(VenoreError::GitCommandFailed(format!("git diff --numstat failed: {}", stderr)));
    }

    // 2. Get name-status (A/M/D/R per file)
    let status_output = crate::utils::quiet_command("git")
        .args(["diff", "--name-status", &format!("{}..{}", base, session)])
        .current_dir(project_path)
        .output()
        .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to run git diff --name-status: {}", e)))?;

    if !status_output.status.success() {
        let stderr = String::from_utf8_lossy(&status_output.stderr);
        return Err(VenoreError::GitCommandFailed(format!("git diff --name-status failed: {}", stderr)));
    }

    // Parse name-status into a map
    let status_text = String::from_utf8_lossy(&status_output.stdout);
    let mut status_map = std::collections::HashMap::new();
    for line in status_text.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 2 {
            let status = match parts[0].chars().next() {
                Some('A') => "added",
                Some('M') => "modified",
                Some('D') => "removed",
                Some('R') => "renamed",
                _ => "modified",
            };
            // For renames, the filename is the second tab-separated field
            let filename = if parts.len() >= 3 { parts[2] } else { parts[1] };
            status_map.insert(filename.to_string(), status.to_string());
        }
    }

    // Parse numstat and build DiffFile entries
    let numstat_text = String::from_utf8_lossy(&numstat_output.stdout);
    let mut files: Vec<DiffFile> = Vec::new();

    for line in numstat_text.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 3 {
            let additions = parts[0].parse::<u32>().unwrap_or(0);
            let deletions = parts[1].parse::<u32>().unwrap_or(0);
            let filename = parts[2].to_string();

            let status = status_map
                .get(&filename)
                .cloned()
                .unwrap_or_else(|| "modified".to_string());

            // Get patch for this file
            let patch = get_file_patch(project_path, base, session, &filename).ok();

            files.push(DiffFile {
                filename,
                status,
                additions,
                deletions,
                patch,
            });
        }
    }

    Ok(files)
}

/// Get the patch (unified diff) for a single file
fn get_file_patch(project_path: &Path, base: &str, session: &str, filename: &str) -> Result<String> {
    let output = crate::utils::quiet_command("git")
        .args(["diff", &format!("{}..{}", base, session), "--", filename])
        .current_dir(project_path)
        .output()
        .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to run git diff for file: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(VenoreError::GitCommandFailed(format!("git diff for file failed: {}", stderr)));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Get commits on the session branch that are not on the base branch
pub fn get_commits(project_path: &Path, base: &str, session: &str) -> Result<Vec<CommitInfo>> {
    let output = crate::utils::quiet_command("git")
        .args([
            "log",
            &format!("{}..{}", base, session),
            "--format=%H%n%h%n%s%n%an%n%ai%n---",
        ])
        .current_dir(project_path)
        .output()
        .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to run git log: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(VenoreError::GitCommandFailed(format!("git log failed: {}", stderr)));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut commits = Vec::new();

    for block in text.split("---\n") {
        let lines: Vec<&str> = block.lines().collect();
        if lines.len() >= 5 {
            commits.push(CommitInfo {
                hash: lines[0].trim().to_string(),
                short_hash: lines[1].trim().to_string(),
                message: lines[2].trim().to_string(),
                author: lines[3].trim().to_string(),
                date: lines[4].trim().to_string(),
            });
        }
    }

    Ok(commits)
}

/// Get diff files including uncommitted worktree changes.
///
/// Runs `git diff <base>` from the worktree directory (no `..session`),
/// which compares the base branch tip against the current working tree state.
/// Also picks up untracked files via `git ls-files --others`.
pub fn get_worktree_diff_files(worktree_path: &Path, base: &str) -> Result<Vec<DiffFile>> {
    let mut files: Vec<DiffFile> = Vec::new();
    let mut seen_files = std::collections::HashSet::new();

    // 1. Tracked changes (committed + staged + unstaged modifications) vs base
    let numstat_output = crate::utils::quiet_command("git")
        .args(["diff", "--numstat", base])
        .current_dir(worktree_path)
        .output()
        .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to run git diff: {}", e)))?;

    let status_output = crate::utils::quiet_command("git")
        .args(["diff", "--name-status", base])
        .current_dir(worktree_path)
        .output()
        .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to run git diff: {}", e)))?;

    if numstat_output.status.success() && status_output.status.success() {
        // Parse name-status
        let status_text = String::from_utf8_lossy(&status_output.stdout);
        let mut status_map = std::collections::HashMap::new();
        for line in status_text.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 2 {
                let status = match parts[0].chars().next() {
                    Some('A') => "added",
                    Some('M') => "modified",
                    Some('D') => "removed",
                    Some('R') => "renamed",
                    _ => "modified",
                };
                let filename = if parts.len() >= 3 { parts[2] } else { parts[1] };
                status_map.insert(filename.to_string(), status.to_string());
            }
        }

        // Parse numstat
        let numstat_text = String::from_utf8_lossy(&numstat_output.stdout);
        for line in numstat_text.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 {
                let additions = parts[0].parse::<u32>().unwrap_or(0);
                let deletions = parts[1].parse::<u32>().unwrap_or(0);
                let filename = parts[2].to_string();

                let status = status_map
                    .get(&filename)
                    .cloned()
                    .unwrap_or_else(|| "modified".to_string());

                // Get patch
                let patch = crate::utils::quiet_command("git")
                    .args(["diff", base, "--", &filename])
                    .current_dir(worktree_path)
                    .output()
                    .ok()
                    .filter(|o| o.status.success())
                    .map(|o| String::from_utf8_lossy(&o.stdout).to_string());

                seen_files.insert(filename.clone());
                files.push(DiffFile { filename, status, additions, deletions, patch });
            }
        }
    }

    // 2. Untracked files (new files not yet staged)
    let untracked_output = crate::utils::quiet_command("git")
        .args(["ls-files", "--others", "--exclude-standard"])
        .current_dir(worktree_path)
        .output()
        .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to run git ls-files: {}", e)))?;

    if untracked_output.status.success() {
        let text = String::from_utf8_lossy(&untracked_output.stdout);
        for line in text.lines() {
            let filename = line.trim().to_string();
            if filename.is_empty() || seen_files.contains(&filename) {
                continue;
            }
            // Read file content for additions count + generate pseudo-patch
            let full_path = worktree_path.join(&filename);
            let content = std::fs::read_to_string(&full_path).ok();
            let additions = content.as_ref().map(|c| c.lines().count() as u32).unwrap_or(0);

            let patch = content.map(|c| {
                let lines: Vec<&str> = c.lines().collect();
                let mut p = format!("--- /dev/null\n+++ b/{}\n@@ -0,0 +1,{} @@\n", filename, lines.len());
                for line in &lines {
                    p.push('+');
                    p.push_str(line);
                    p.push('\n');
                }
                p
            });

            files.push(DiffFile {
                filename,
                status: "added".to_string(),
                additions,
                deletions: 0,
                patch,
            });
        }
    }

    Ok(files)
}

/// Get worktree diff summary including uncommitted changes: (files_changed, additions, deletions)
pub fn get_worktree_diff_summary(worktree_path: &Path, base: &str) -> Result<(u32, u32, u32)> {
    // Tracked changes
    let output = crate::utils::quiet_command("git")
        .args(["diff", "--shortstat", base])
        .current_dir(worktree_path)
        .output()
        .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to run git diff --shortstat: {}", e)))?;

    let mut files_changed = 0u32;
    let mut additions = 0u32;
    let mut deletions = 0u32;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        let text = text.trim();
        for part in text.split(", ") {
            let part = part.trim();
            if part.contains("file") {
                files_changed = part.split_whitespace().next().and_then(|n| n.parse().ok()).unwrap_or(0);
            } else if part.contains("insertion") {
                additions = part.split_whitespace().next().and_then(|n| n.parse().ok()).unwrap_or(0);
            } else if part.contains("deletion") {
                deletions = part.split_whitespace().next().and_then(|n| n.parse().ok()).unwrap_or(0);
            }
        }
    }

    // Count untracked files
    let untracked = crate::utils::quiet_command("git")
        .args(["ls-files", "--others", "--exclude-standard"])
        .current_dir(worktree_path)
        .output();

    if let Ok(ut) = untracked {
        if ut.status.success() {
            let text = String::from_utf8_lossy(&ut.stdout);
            for line in text.lines() {
                let filename = line.trim();
                if filename.is_empty() { continue; }
                files_changed += 1;
                let full_path = worktree_path.join(filename);
                additions += std::fs::read_to_string(&full_path)
                    .map(|c| c.lines().count() as u32)
                    .unwrap_or(0);
            }
        }
    }

    Ok((files_changed, additions, deletions))
}

/// Auto-commit a file change in the session worktree.
/// Returns the commit hash on success.
/// If there is nothing to commit, returns the current HEAD hash (not an error).
pub fn auto_commit(worktree_path: &Path, file_path: &str, message: &str) -> Result<String> {
    // git add <file>
    let add_output = crate::utils::quiet_command("git")
        .args(["add", "--", file_path])
        .current_dir(worktree_path)
        .output()
        .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to run git add: {}", e)))?;

    if !add_output.status.success() {
        let stderr = String::from_utf8_lossy(&add_output.stderr);
        tracing::warn!(file = %file_path, stderr = %stderr, "git add failed (file may not exist)");
    }

    // git commit -m <message>
    let commit_output = crate::utils::quiet_command("git")
        .args(["commit", "-m", message, "--no-verify"])
        .current_dir(worktree_path)
        .output()
        .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to run git commit: {}", e)))?;

    if !commit_output.status.success() {
        // Nothing to commit is not an error — return current HEAD
        tracing::debug!(message = %message, "auto_commit: nothing to commit, returning HEAD");
    }

    // git rev-parse HEAD → commit hash
    let head_output = crate::utils::quiet_command("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(worktree_path)
        .output()
        .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to run git rev-parse HEAD: {}", e)))?;

    if !head_output.status.success() {
        let stderr = String::from_utf8_lossy(&head_output.stderr);
        return Err(VenoreError::GitCommandFailed(format!("git rev-parse HEAD failed: {}", stderr)));
    }

    let hash = String::from_utf8_lossy(&head_output.stdout).trim().to_string();
    tracing::debug!(hash = %hash, message = %message, "auto_commit completed");
    Ok(hash)
}

/// Reset worktree to a specific commit (hard reset).
/// Safe because worktrees are isolated from the main project.
pub fn reset_to_commit(worktree_path: &Path, commit_hash: &str) -> Result<()> {
    let output = crate::utils::quiet_command("git")
        .args(["reset", "--hard", commit_hash])
        .current_dir(worktree_path)
        .output()
        .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to run git reset: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(VenoreError::GitCommandFailed(format!("git reset --hard failed: {}", stderr)));
    }

    tracing::info!(commit = %commit_hash, worktree = %worktree_path.display(), "Reset worktree to commit");
    Ok(())
}

/// Get diff summary: (files_changed, additions, deletions)
pub fn get_diff_summary(project_path: &Path, base: &str, session: &str) -> Result<(u32, u32, u32)> {
    let output = crate::utils::quiet_command("git")
        .args(["diff", "--stat", &format!("{}..{}", base, session)])
        .current_dir(project_path)
        .output()
        .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to run git diff --stat: {}", e)))?;

    if !output.status.success() {
        // Branch might not exist yet or no changes
        return Ok((0, 0, 0));
    }

    // Use shortstat for clean parsing
    let output = crate::utils::quiet_command("git")
        .args(["diff", "--shortstat", &format!("{}..{}", base, session)])
        .current_dir(project_path)
        .output()
        .map_err(|e| VenoreError::GitCommandFailed(format!("Failed to run git diff --shortstat: {}", e)))?;

    let text = String::from_utf8_lossy(&output.stdout);
    let text = text.trim();

    if text.is_empty() {
        return Ok((0, 0, 0));
    }

    // Parse: "3 files changed, 10 insertions(+), 5 deletions(-)"
    let mut files_changed = 0u32;
    let mut additions = 0u32;
    let mut deletions = 0u32;

    for part in text.split(", ") {
        let part = part.trim();
        if part.contains("file") {
            files_changed = part.split_whitespace().next().and_then(|n| n.parse().ok()).unwrap_or(0);
        } else if part.contains("insertion") {
            additions = part.split_whitespace().next().and_then(|n| n.parse().ok()).unwrap_or(0);
        } else if part.contains("deletion") {
            deletions = part.split_whitespace().next().and_then(|n| n.parse().ok()).unwrap_or(0);
        }
    }

    Ok((files_changed, additions, deletions))
}

/// Compute diff stats + patch for a single file in a worktree vs its base branch.
/// Returns `None` if the git commands fail (e.g. not a git repo).
pub fn compute_file_diff(
    worktree_path: &Path,
    base_branch: &str,
    relative_path: &str,
) -> Option<DiffFile> {
    // Check if the file existed in the base branch
    let cat_file = crate::utils::quiet_command("git")
        .args(["cat-file", "-e", &format!("{}:{}", base_branch, relative_path)])
        .current_dir(worktree_path)
        .output()
        .ok()?;

    if !cat_file.status.success() {
        // File did NOT exist in base branch — it's a new file
        let file_full = worktree_path.join(relative_path);
        let content = std::fs::read_to_string(&file_full).ok()?;
        let line_count = content.lines().count() as u32;
        let patch_lines: Vec<String> = std::iter::once("--- /dev/null".to_string())
            .chain(std::iter::once(format!("+++ b/{}", relative_path)))
            .chain(std::iter::once(format!("@@ -0,0 +1,{} @@", line_count)))
            .chain(content.lines().map(|l| format!("+{}", l)))
            .collect();
        return Some(DiffFile {
            filename: relative_path.to_string(),
            status: "added".to_string(),
            additions: line_count,
            deletions: 0,
            patch: Some(patch_lines.join("\n")),
        });
    }

    // File exists in base branch — compute numstat
    let numstat = crate::utils::quiet_command("git")
        .args(["diff", "--numstat", base_branch, "--", relative_path])
        .current_dir(worktree_path)
        .output()
        .ok()?;

    let numstat_str = String::from_utf8_lossy(&numstat.stdout);
    let (additions, deletions) = numstat_str
        .lines()
        .next()
        .and_then(|line| {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 2 {
                Some((
                    parts[0].parse::<u32>().unwrap_or(0),
                    parts[1].parse::<u32>().unwrap_or(0),
                ))
            } else {
                None
            }
        })
        .unwrap_or((0, 0));

    // Get the full diff patch
    let diff_output = crate::utils::quiet_command("git")
        .args(["diff", base_branch, "--", relative_path])
        .current_dir(worktree_path)
        .output()
        .ok()?;

    let patch = String::from_utf8_lossy(&diff_output.stdout).to_string();
    let patch = if patch.is_empty() { None } else { Some(patch) };

    Some(DiffFile {
        filename: relative_path.to_string(),
        status: "modified".to_string(),
        additions,
        deletions,
        patch,
    })
}
