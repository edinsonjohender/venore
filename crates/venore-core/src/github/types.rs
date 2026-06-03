//! GitHub integration data types.
//!
//! Pure data structures for GitHub API responses, Device Flow,
//! and repo linking. No I/O or business logic here.

use serde::{Deserialize, Serialize};

// =============================================================================
// GitHub API responses
// =============================================================================

/// Authenticated GitHub user info (from GET /user).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubUser {
    pub id: u64,
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: String,
    pub html_url: String,
}

/// Rate limit info extracted from GitHub response headers.
#[derive(Debug, Clone)]
pub struct RateLimitInfo {
    pub remaining: u32,
    pub limit: u32,
    /// Unix timestamp when rate limit resets.
    pub reset: u64,
}

// =============================================================================
// Device Flow (RFC 8628)
// =============================================================================

/// Response from POST /login/device/code.
#[derive(Debug, Clone, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

/// Response from POST /login/oauth/access_token during polling.
#[derive(Debug, Clone, Deserialize)]
pub struct AccessTokenResponse {
    pub access_token: Option<String>,
    pub token_type: Option<String>,
    pub scope: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

/// Status updates emitted during Device Flow authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum DeviceFlowStatus {
    #[serde(rename = "pending")]
    PendingUserAction {
        user_code: String,
        verification_uri: String,
        expires_in: u64,
    },
    #[serde(rename = "authenticated")]
    Authenticated {
        login: String,
        name: Option<String>,
        avatar_url: String,
    },
    #[serde(rename = "failed")]
    Failed {
        reason: String,
    },
}

// =============================================================================
// User Repos (for listing user's repositories)
// =============================================================================

/// A GitHub repository from GET /user/repos.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubUserRepo {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub owner: GitHubUserSummary,
    pub description: Option<String>,
    pub html_url: String,
    pub clone_url: String,
    pub ssh_url: String,
    #[serde(rename = "private")]
    pub is_private: bool,
    pub language: Option<String>,
    pub stargazers_count: u64,
    pub updated_at: String,
    pub default_branch: String,
}

// =============================================================================
// Pull Requests & Issues (shared types)
// =============================================================================

/// A GitHub label (used on PRs and Issues).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubLabel {
    pub name: String,
    pub color: String,
}

/// Lightweight user summary (used in PR/Issue listings).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubUserSummary {
    pub login: String,
    pub avatar_url: String,
}

// =============================================================================
// Pull Requests
// =============================================================================

/// A git ref (branch) in a pull request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubRef {
    #[serde(rename = "ref")]
    pub ref_name: String,
}

/// A GitHub pull request from GET /repos/{owner}/{repo}/pulls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubPullRequest {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub user: GitHubUserSummary,
    pub created_at: String,
    pub updated_at: String,
    pub html_url: String,
    pub body: Option<String>,
    pub head: GitHubRef,
    pub base: GitHubRef,
    #[serde(default)]
    pub labels: Vec<GitHubLabel>,
    #[serde(default)]
    pub draft: bool,
    pub comments: Option<u64>,
    pub review_comments: Option<u64>,
    pub additions: Option<u64>,
    pub deletions: Option<u64>,
    pub changed_files: Option<u64>,
}

// =============================================================================
// Issues
// =============================================================================

/// A GitHub issue from GET /repos/{owner}/{repo}/issues.
///
/// Note: GitHub's issues endpoint also returns PRs. Items with
/// `pull_request` set to `Some(...)` are PRs and should be filtered.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubIssue {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub user: GitHubUserSummary,
    pub created_at: String,
    pub updated_at: String,
    pub html_url: String,
    pub body: Option<String>,
    #[serde(default)]
    pub labels: Vec<GitHubLabel>,
    #[serde(default)]
    pub assignees: Vec<GitHubUserSummary>,
    pub comments: Option<u64>,
    /// Present when this "issue" is actually a PR.
    pub pull_request: Option<serde_json::Value>,
}

// =============================================================================
// Pull Request Files (diff)
// =============================================================================

/// A file changed in a PR (from GET /repos/{o}/{r}/pulls/{n}/files).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubPullRequestFile {
    pub sha: String,
    pub filename: String,
    pub status: String,
    pub additions: u64,
    pub deletions: u64,
    pub changes: u64,
    pub patch: Option<String>,
}

// =============================================================================
// Comments
// =============================================================================

/// A comment on an issue or PR (from GET /repos/{o}/{r}/issues/{n}/comments).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubComment {
    pub id: u64,
    pub user: GitHubUserSummary,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
    pub html_url: String,
}

/// An inline review comment on a PR (from GET /repos/{o}/{r}/pulls/{n}/comments).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubReviewComment {
    pub id: u64,
    pub user: GitHubUserSummary,
    pub body: String,
    pub path: String,
    pub line: Option<u64>,
    pub created_at: String,
    pub updated_at: String,
    pub html_url: String,
    pub diff_hunk: Option<String>,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_flow_status_serialization() {
        let pending = DeviceFlowStatus::PendingUserAction {
            user_code: "ABCD-1234".to_string(),
            verification_uri: "https://github.com/login/device".to_string(),
            expires_in: 900,
        };
        let json = serde_json::to_string(&pending).unwrap();
        assert!(json.contains("\"status\":\"pending\""));
        assert!(json.contains("ABCD-1234"));

        let authenticated = DeviceFlowStatus::Authenticated {
            login: "octocat".to_string(),
            name: Some("Mona".to_string()),
            avatar_url: "https://avatars.githubusercontent.com/u/1".to_string(),
        };
        let json = serde_json::to_string(&authenticated).unwrap();
        assert!(json.contains("\"status\":\"authenticated\""));
        assert!(json.contains("octocat"));

        let failed = DeviceFlowStatus::Failed {
            reason: "expired".to_string(),
        };
        let json = serde_json::to_string(&failed).unwrap();
        assert!(json.contains("\"status\":\"failed\""));
    }

    #[test]
    fn test_pull_request_deserialization() {
        let json = r#"{
            "number": 42,
            "title": "Add feature X",
            "state": "open",
            "user": { "login": "octocat", "avatar_url": "https://example.com/avatar.png" },
            "created_at": "2026-02-14T10:00:00Z",
            "updated_at": "2026-02-14T12:00:00Z",
            "html_url": "https://github.com/owner/repo/pull/42",
            "body": "This PR adds feature X",
            "head": { "ref": "feature-x" },
            "base": { "ref": "main" },
            "labels": [{ "name": "enhancement", "color": "84b6eb" }],
            "draft": false,
            "comments": 3,
            "review_comments": 1
        }"#;

        let pr: GitHubPullRequest = serde_json::from_str(json).unwrap();
        assert_eq!(pr.number, 42);
        assert_eq!(pr.title, "Add feature X");
        assert_eq!(pr.user.login, "octocat");
        assert_eq!(pr.head.ref_name, "feature-x");
        assert_eq!(pr.base.ref_name, "main");
        assert_eq!(pr.labels.len(), 1);
        assert_eq!(pr.labels[0].name, "enhancement");
        assert!(!pr.draft);
        assert_eq!(pr.comments, Some(3));
        // Optional fields not present
        assert!(pr.additions.is_none());
    }

    #[test]
    fn test_issue_deserialization() {
        let json = r#"{
            "number": 100,
            "title": "Bug report",
            "state": "open",
            "user": { "login": "dev123", "avatar_url": "https://example.com/a.png" },
            "created_at": "2026-02-13T08:00:00Z",
            "updated_at": "2026-02-14T09:00:00Z",
            "html_url": "https://github.com/owner/repo/issues/100",
            "body": "Something is broken",
            "labels": [{ "name": "bug", "color": "d73a4a" }],
            "assignees": [{ "login": "dev456", "avatar_url": "https://example.com/b.png" }],
            "comments": 5
        }"#;

        let issue: GitHubIssue = serde_json::from_str(json).unwrap();
        assert_eq!(issue.number, 100);
        assert_eq!(issue.title, "Bug report");
        assert_eq!(issue.body, Some("Something is broken".to_string()));
        assert_eq!(issue.labels.len(), 1);
        assert_eq!(issue.assignees.len(), 1);
        assert_eq!(issue.assignees[0].login, "dev456");
        assert!(issue.pull_request.is_none());
    }

    #[test]
    fn test_issue_that_is_pr_has_pull_request_field() {
        let json = r#"{
            "number": 50,
            "title": "PR masquerading as issue",
            "state": "open",
            "user": { "login": "octocat", "avatar_url": "https://example.com/a.png" },
            "created_at": "2026-02-14T00:00:00Z",
            "updated_at": "2026-02-14T00:00:00Z",
            "html_url": "https://github.com/owner/repo/issues/50",
            "body": null,
            "labels": [],
            "assignees": [],
            "comments": 0,
            "pull_request": { "url": "https://api.github.com/repos/owner/repo/pulls/50" }
        }"#;

        let issue: GitHubIssue = serde_json::from_str(json).unwrap();
        assert_eq!(issue.number, 50);
        assert!(issue.pull_request.is_some());
    }
}
