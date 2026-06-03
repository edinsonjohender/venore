//! GitHub repo detection — detect the GitHub repository from `.git/config`.
//!
//! `.git/config` (remote origin) is the ONLY source of truth for owner/repo.

use std::fs;
use std::path::Path;

use tracing::{debug, info};

use crate::error::Result;

// =============================================================================
// Auto-detection
// =============================================================================

/// Detect GitHub owner/repo from the project's git remote.
///
/// Tries `git remote get-url origin` first, falls back to parsing
/// `.git/config` if the git command fails.
pub fn detect_github_repo(project_path: &Path) -> Result<Option<(String, String)>> {
    info!(path = ?project_path, "Detecting GitHub repo from git remote");

    // Try git command first
    let output = crate::utils::quiet_command("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(project_path)
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
            debug!(url = %url, "Git remote URL detected");
            if let Some(parsed) = parse_github_remote(&url) {
                return Ok(Some(parsed));
            }
        }
    }

    // Fallback: read .git/config
    let git_config_path = project_path.join(".git/config");
    if git_config_path.exists() {
        debug!("Falling back to .git/config parsing");
        if let Ok(content) = fs::read_to_string(&git_config_path) {
            return Ok(extract_remote_from_git_config(&content));
        }
    }

    debug!("No GitHub remote detected");
    Ok(None)
}

/// Parse a GitHub remote URL into (owner, repo).
///
/// Supports:
/// - `https://github.com/owner/repo.git`
/// - `https://github.com/owner/repo`
/// - `git@github.com:owner/repo.git`
/// - `git@github.com:owner/repo`
/// - `ssh://git@github.com/owner/repo.git`
/// - `ssh://git@github.com/owner/repo`
///
/// HTTPS URLs that carry embedded credentials
/// (`https://x-access-token:TOKEN@github.com/owner/repo.git`) are also
/// accepted — the `user:pass@` userinfo is stripped before matching. Such URLs
/// shouldn't be produced anymore (see `git_auth`), but repos cloned by older
/// builds or other tools may still have them.
pub fn parse_github_remote(url: &str) -> Option<(String, String)> {
    let url = url.trim();

    // SSH format: git@github.com:owner/repo.git
    if let Some(rest) = url.strip_prefix("git@github.com:") {
        return parse_owner_repo(rest);
    }

    // SSH URL format: ssh://git@github.com/owner/repo.git
    if let Some(rest) = url.strip_prefix("ssh://git@github.com/") {
        return parse_owner_repo(rest);
    }

    // HTTPS format, possibly with embedded credentials:
    // https://[x-access-token:TOKEN@]github.com/owner/repo.git
    if let Some(after_scheme) = url.strip_prefix("https://") {
        if let Some(rest) = strip_userinfo(after_scheme).strip_prefix("github.com/") {
            return parse_owner_repo(rest);
        }
    }

    None
}

/// Remove `user:pass@` userinfo from a URL authority (the part after the
/// scheme). `x-access-token:TOKEN@github.com/owner/repo` becomes
/// `github.com/owner/repo`. Returns the input unchanged when there is no
/// userinfo (no `@`, or a `/` precedes the first `@`, meaning the `@` is in the
/// path, not the authority).
fn strip_userinfo(after_scheme: &str) -> &str {
    match after_scheme.find('@') {
        Some(at) if !after_scheme[..at].contains('/') => &after_scheme[at + 1..],
        _ => after_scheme,
    }
}

/// Extract owner/repo from "owner/repo.git" or "owner/repo".
fn parse_owner_repo(path: &str) -> Option<(String, String)> {
    let path = path.strip_suffix(".git").unwrap_or(path);
    let parts: Vec<&str> = path.splitn(3, '/').collect();
    if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
        Some((parts[0].to_string(), parts[1].to_string()))
    } else {
        None
    }
}

/// Extract remote origin URL from .git/config content.
fn extract_remote_from_git_config(content: &str) -> Option<(String, String)> {
    let mut in_remote_origin = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed == "[remote \"origin\"]" {
            in_remote_origin = true;
            continue;
        }

        if trimmed.starts_with('[') {
            in_remote_origin = false;
            continue;
        }

        if in_remote_origin {
            if let Some(url) = trimmed.strip_prefix("url = ") {
                return parse_github_remote(url.trim());
            }
        }
    }

    None
}


// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- parse_github_remote ----

    #[test]
    fn test_parse_https_with_git_suffix() {
        let result = parse_github_remote("https://github.com/octocat/hello-world.git");
        assert_eq!(result, Some(("octocat".to_string(), "hello-world".to_string())));
    }

    #[test]
    fn test_parse_https_without_git_suffix() {
        let result = parse_github_remote("https://github.com/octocat/hello-world");
        assert_eq!(result, Some(("octocat".to_string(), "hello-world".to_string())));
    }

    #[test]
    fn test_parse_ssh_colon_format() {
        let result = parse_github_remote("git@github.com:octocat/hello-world.git");
        assert_eq!(result, Some(("octocat".to_string(), "hello-world".to_string())));
    }

    #[test]
    fn test_parse_ssh_colon_without_git_suffix() {
        let result = parse_github_remote("git@github.com:octocat/hello-world");
        assert_eq!(result, Some(("octocat".to_string(), "hello-world".to_string())));
    }

    #[test]
    fn test_parse_ssh_url_format() {
        let result = parse_github_remote("ssh://git@github.com/octocat/hello-world.git");
        assert_eq!(result, Some(("octocat".to_string(), "hello-world".to_string())));
    }

    #[test]
    fn test_parse_ssh_url_without_git_suffix() {
        let result = parse_github_remote("ssh://git@github.com/octocat/hello-world");
        assert_eq!(result, Some(("octocat".to_string(), "hello-world".to_string())));
    }

    #[test]
    fn test_parse_https_with_embedded_token() {
        // URL as written by older builds that injected the token into origin.
        let result = parse_github_remote(
            "https://x-access-token:ghp_secret123@github.com/octocat/hello-world.git",
        );
        assert_eq!(result, Some(("octocat".to_string(), "hello-world".to_string())));
    }

    #[test]
    fn test_parse_https_with_userinfo_no_token() {
        let result = parse_github_remote("https://user@github.com/octocat/hello-world");
        assert_eq!(result, Some(("octocat".to_string(), "hello-world".to_string())));
    }

    #[test]
    fn test_parse_non_github_url() {
        let result = parse_github_remote("https://gitlab.com/octocat/hello-world.git");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_empty_string() {
        let result = parse_github_remote("");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_invalid_format() {
        let result = parse_github_remote("not-a-url");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_with_whitespace() {
        let result = parse_github_remote("  https://github.com/octocat/hello-world.git  \n");
        assert_eq!(result, Some(("octocat".to_string(), "hello-world".to_string())));
    }

    // ---- extract_remote_from_git_config ----

    #[test]
    fn test_extract_remote_from_git_config() {
        let config = r#"
[core]
    repositoryformatversion = 0
[remote "origin"]
    url = https://github.com/octocat/hello-world.git
    fetch = +refs/heads/*:refs/remotes/origin/*
[branch "main"]
    remote = origin
"#;
        let result = extract_remote_from_git_config(config);
        assert_eq!(result, Some(("octocat".to_string(), "hello-world".to_string())));
    }

    #[test]
    fn test_extract_remote_no_origin() {
        let config = r#"
[core]
    repositoryformatversion = 0
[remote "upstream"]
    url = https://github.com/other/repo.git
"#;
        let result = extract_remote_from_git_config(config);
        assert!(result.is_none());
    }

}
