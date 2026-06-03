//! GitHub DTOs — Request/Response types for GitHub Tauri commands.

use serde::{Deserialize, Serialize};

// =============================================================================
// Auth
// =============================================================================

/// Response for github_auth_status, github_store_pat, and github_accept_gcm.
#[derive(Serialize, Deserialize)]
pub struct GitHubAuthStatusResponse {
    pub authenticated: bool,
    pub login: Option<String>,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    // GCM-detected account (not yet stored in keyring)
    pub gcm_detected: bool,
    pub gcm_login: Option<String>,
    pub gcm_name: Option<String>,
    pub gcm_avatar_url: Option<String>,
}

/// Response for github_start_device_flow.
#[derive(Serialize, Deserialize)]
pub struct GitHubDeviceFlowStartResponse {
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
}

/// Request for github_store_pat.
#[derive(Serialize, Deserialize)]
pub struct GitHubStorePATRequest {
    pub token: String,
}

// =============================================================================
// Repo
// =============================================================================

/// Request for github_detect_repo.
#[derive(Serialize, Deserialize)]
pub struct GitHubDetectRepoRequest {
    pub project_path: String,
}

/// Response for github_detect_repo.
#[derive(Serialize, Deserialize)]
pub struct GitHubDetectRepoResponse {
    pub detected: bool,
    pub owner: Option<String>,
    pub repo: Option<String>,
}

// =============================================================================
// Pull Requests
// =============================================================================

/// Request for github_list_pulls.
#[derive(Serialize, Deserialize)]
pub struct GitHubListPullsRequest {
    pub project_path: String,
    pub state: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

/// A flattened pull request for the frontend.
#[derive(Serialize, Deserialize)]
pub struct GitHubPullRequestDto {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub author: String,
    pub author_avatar: String,
    pub created_at: String,
    pub updated_at: String,
    pub html_url: String,
    pub body: Option<String>,
    pub head_ref: String,
    pub base_ref: String,
    pub labels: Vec<GitHubLabelDto>,
    pub draft: bool,
    pub comments: u64,
    pub review_comments: u64,
}

/// Request for github_get_pr_detail.
#[derive(Serialize, Deserialize)]
pub struct GitHubGetPrDetailRequest {
    pub project_path: String,
    pub pr_number: u64,
}

/// Response for github_get_pr_detail (single PR with full body + stats).
#[derive(Serialize, Deserialize)]
pub struct GitHubPrDetailResponse {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub author: String,
    pub author_avatar: String,
    pub created_at: String,
    pub updated_at: String,
    pub html_url: String,
    pub body: Option<String>,
    pub head_ref: String,
    pub base_ref: String,
    pub labels: Vec<GitHubLabelDto>,
    pub draft: bool,
    pub comments: u64,
    pub review_comments: u64,
    pub additions: u64,
    pub deletions: u64,
    pub changed_files: u64,
}

/// A label DTO for the frontend.
#[derive(Serialize, Deserialize)]
pub struct GitHubLabelDto {
    pub name: String,
    pub color: String,
}

/// Response for github_list_pulls.
#[derive(Serialize, Deserialize)]
pub struct GitHubListPullsResponse {
    pub pulls: Vec<GitHubPullRequestDto>,
    pub has_more: bool,
    pub page: u32,
    pub per_page: u32,
}

// =============================================================================
// Issues
// =============================================================================

/// Request for github_list_issues.
#[derive(Serialize, Deserialize)]
pub struct GitHubListIssuesRequest {
    pub project_path: String,
    pub state: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

/// A flattened issue for the frontend.
#[derive(Serialize, Deserialize)]
pub struct GitHubIssueDto {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub author: String,
    pub author_avatar: String,
    pub created_at: String,
    pub updated_at: String,
    pub html_url: String,
    pub body: Option<String>,
    pub labels: Vec<GitHubLabelDto>,
    pub assignees: Vec<String>,
    pub comments: u64,
}

/// Response for github_list_issues.
#[derive(Serialize, Deserialize)]
pub struct GitHubListIssuesResponse {
    pub issues: Vec<GitHubIssueDto>,
    pub has_more: bool,
    pub page: u32,
    pub per_page: u32,
}

// =============================================================================
// PR Files (diff)
// =============================================================================

/// Request for github_get_pr_files.
#[derive(Serialize, Deserialize)]
pub struct GitHubGetPrFilesRequest {
    pub project_path: String,
    pub pr_number: u64,
}

/// A file changed in a PR (for the frontend).
#[derive(Serialize, Deserialize)]
pub struct GitHubPrFileDto {
    pub filename: String,
    pub status: String,
    pub additions: u64,
    pub deletions: u64,
    pub patch: Option<String>,
}

/// Response for github_get_pr_files.
#[derive(Serialize, Deserialize)]
pub struct GitHubPrFilesResponse {
    pub files: Vec<GitHubPrFileDto>,
}

// =============================================================================
// Comments
// =============================================================================

/// Request for github_get_comments.
#[derive(Serialize, Deserialize)]
pub struct GitHubGetCommentsRequest {
    pub project_path: String,
    pub number: u64,
    pub is_pull_request: bool,
}

/// A general comment on an issue or PR (for the frontend).
#[derive(Serialize, Deserialize)]
pub struct GitHubCommentDto {
    pub id: u64,
    pub author: String,
    pub author_avatar: String,
    pub body: String,
    pub created_at: String,
    pub html_url: String,
}

/// An inline review comment on a PR (for the frontend).
#[derive(Serialize, Deserialize)]
pub struct GitHubReviewCommentDto {
    pub id: u64,
    pub author: String,
    pub author_avatar: String,
    pub body: String,
    pub path: String,
    pub line: Option<u64>,
    pub diff_hunk: Option<String>,
    pub created_at: String,
}

/// Response for github_get_comments.
#[derive(Serialize, Deserialize)]
pub struct GitHubCommentsResponse {
    pub comments: Vec<GitHubCommentDto>,
    pub review_comments: Vec<GitHubReviewCommentDto>,
}

// =============================================================================
// User Repos (Clone from GitHub)
// =============================================================================

/// Request for github_list_user_repos.
#[derive(Serialize, Deserialize)]
pub struct GitHubListUserReposRequest {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

/// A user repository DTO for the frontend.
#[derive(Serialize, Deserialize)]
pub struct GitHubUserRepoDto {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub owner: String,
    pub description: Option<String>,
    pub html_url: String,
    pub clone_url: String,
    pub is_private: bool,
    pub language: Option<String>,
    pub stargazers_count: u64,
    pub updated_at: String,
    pub default_branch: String,
}

/// Response for github_list_user_repos.
#[derive(Serialize, Deserialize)]
pub struct GitHubListUserReposResponse {
    pub repos: Vec<GitHubUserRepoDto>,
    pub has_more: bool,
    pub page: u32,
    pub per_page: u32,
}

/// Request for github_clone_repo.
#[derive(Serialize, Deserialize)]
pub struct GitHubCloneRepoRequest {
    /// Caller-generated id, echoed in every `github:clone:*` event. The
    /// frontend records it before invoking, so even an error emitted
    /// immediately (e.g. destination already exists) is matched instead of
    /// dropped — which previously left the clone modal stuck and unclosable.
    pub clone_id: String,
    pub clone_url: String,
    pub owner: String,
    pub repo: String,
    pub dest_dir: String,
}

/// Response for github_clone_repo (returns immediately, progress via events).
#[derive(Serialize, Deserialize)]
pub struct GitHubCloneRepoResponse {
    pub clone_id: String,
}

/// Request for github_inspect_clone_destination.
#[derive(Serialize, Deserialize)]
pub struct GitHubInspectDestinationRequest {
    pub dest_dir: String,
    pub repo: String,
}

/// Whether `{dest_dir}/{repo}` already exists, and how to proceed.
#[derive(Serialize, Deserialize)]
pub struct GitHubInspectDestinationResponse {
    /// True if `{dest_dir}/{repo}` already exists on disk.
    pub exists: bool,
    /// Absolute path of `{dest_dir}/{repo}` (whether or not it exists).
    pub path: String,
    /// True if the existing folder already holds a Venore project
    /// (`.venore/project.json`) — i.e. "open existing" can go straight to the
    /// workspace instead of the onboarding wizard.
    pub is_venore: bool,
    /// First non-colliding folder name for a fresh clone (`repo`, then
    /// `repo-1`, `repo-2`, …). Equals `repo` when nothing collides.
    pub suggested_name: String,
}

// =============================================================================
// PR Analysis
// =============================================================================

/// Request for github_analyze_pr.
#[derive(Serialize, Deserialize)]
pub struct GitHubAnalyzePrRequest {
    pub project_path: String,
    pub pr_number: u64,
    pub stream_id: String,
}

/// Response for github_analyze_pr (returns immediately, streaming via events).
#[derive(Serialize, Deserialize)]
pub struct GitHubAnalyzePrResponse {
    pub stream_id: String,
}

/// Request for github_stop_pr_analysis.
#[derive(Serialize, Deserialize)]
pub struct GitHubStopPrAnalysisRequest {
    pub stream_id: String,
}
