//! Tauri commands for GitHub integration.
//!
//! Thin wrappers that delegate to venore-core's github module.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use once_cell::sync::Lazy;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::task::AbortHandle;
use tracing::{debug, info, warn};

use crate::state::LazyAppState;
use crate::utils::{CommandResult, IntoStateCommandResult, StateCommandResult};
use super::dto::github::{
    GitHubAnalyzePrRequest, GitHubAnalyzePrResponse, GitHubAuthStatusResponse,
    GitHubCloneRepoRequest, GitHubCloneRepoResponse,
    GitHubInspectDestinationRequest, GitHubInspectDestinationResponse,
    GitHubCommentDto, GitHubCommentsResponse, GitHubDetectRepoRequest,
    GitHubDetectRepoResponse, GitHubDeviceFlowStartResponse, GitHubGetCommentsRequest,
    GitHubGetPrDetailRequest, GitHubGetPrFilesRequest,
    GitHubIssueDto, GitHubLabelDto, GitHubListIssuesRequest,
    GitHubListIssuesResponse, GitHubListPullsRequest, GitHubListPullsResponse,
    GitHubListUserReposRequest, GitHubListUserReposResponse, GitHubUserRepoDto,
    GitHubPrDetailResponse, GitHubPrFileDto, GitHubPrFilesResponse,
    GitHubPullRequestDto, GitHubReviewCommentDto,
    GitHubStopPrAnalysisRequest, GitHubStorePATRequest,
};
use venore_core::error::VenoreError;
use venore_core::github::{auth, client::GitHubClient, clone, comments, issues, pr_analyzer, pr_detail, pulls, repo, repos};

/// Cancellation flag for Device Flow polling.
static DEVICE_FLOW_CANCEL: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));

/// Active PR analysis streams (for cancellation).
static ACTIVE_PR_ANALYSES: Lazy<Mutex<HashMap<String, AbortHandle>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

// =============================================================================
// Auth Commands
// =============================================================================

/// Check if the user is authenticated with GitHub.
///
/// 1. Keyring token valid → authenticated: true (gcm fields false/None)
/// 2. Keyring token invalid → remove, fall through to step 3
/// 3. GCM token valid → authenticated: false, gcm_detected: true with user info
/// 4. Nothing → all false/None
///
/// GCM tokens are NEVER auto-stored. Only reported for UI to offer a choice.
#[tauri::command]
pub async fn github_auth_status() -> CommandResult<GitHubAuthStatusResponse> {
    info!("Checking GitHub auth status (full check, may probe GCM)");

    let result: Result<GitHubAuthStatusResponse, VenoreError> = async {
        // Step 1: Check keyring
        if let Some(keyring_token) = auth::get_stored_token()? {
            let client = GitHubClient::new(keyring_token);
            match client.validate_token().await {
                Ok(user) => {
                    return Ok(GitHubAuthStatusResponse {
                        authenticated: true,
                        login: Some(user.login),
                        name: user.name,
                        avatar_url: Some(user.avatar_url),
                        gcm_detected: false,
                        gcm_login: None,
                        gcm_name: None,
                        gcm_avatar_url: None,
                    });
                }
                Err(_) => {
                    // Step 2: Keyring token invalid — remove it
                    warn!("Stored GitHub token is invalid, removing");
                    let _ = auth::remove_token();
                }
            }
        }

        // Step 3: Try GCM (report only, never store)
        if let Ok(Some(gcm_token)) = auth::try_git_credential_token().await {
            let client = GitHubClient::new(gcm_token);
            if let Ok(user) = client.validate_token().await {
                return Ok(GitHubAuthStatusResponse {
                    authenticated: false,
                    login: None,
                    name: None,
                    avatar_url: None,
                    gcm_detected: true,
                    gcm_login: Some(user.login),
                    gcm_name: user.name,
                    gcm_avatar_url: Some(user.avatar_url),
                });
            }
        }

        // Step 4: Nothing found
        Ok(GitHubAuthStatusResponse {
            authenticated: false,
            login: None,
            name: None,
            avatar_url: None,
            gcm_detected: false,
            gcm_login: None,
            gcm_name: None,
            gcm_avatar_url: None,
        })
    }
    .await;

    result.into()
}

/// Validate the stored GitHub session — keyring token only, never prompts.
///
/// Used at boot to populate the GitHub auth cache without touching GCM (no
/// interactive picker). Unlike `github_auth_status`, it removes the keyring
/// token ONLY on a real 401 (token rejected); a network error keeps the token
/// so going offline never wipes a valid login.
#[tauri::command]
pub async fn github_validate_session() -> CommandResult<GitHubAuthStatusResponse> {
    info!("Validating stored GitHub session (keyring only)");

    let disconnected = || GitHubAuthStatusResponse {
        authenticated: false,
        login: None,
        name: None,
        avatar_url: None,
        gcm_detected: false,
        gcm_login: None,
        gcm_name: None,
        gcm_avatar_url: None,
    };

    let result: Result<GitHubAuthStatusResponse, VenoreError> = async {
        let token = match auth::get_stored_token()? {
            Some(t) => t,
            None => return Ok(disconnected()),
        };

        let client = GitHubClient::new(token);
        match client.validate_token().await {
            Ok(user) => {
                info!(login = %user.login, "GitHub session validated (authenticated)");
                Ok(GitHubAuthStatusResponse {
                    authenticated: true,
                    login: Some(user.login),
                    name: user.name,
                    avatar_url: Some(user.avatar_url),
                    gcm_detected: false,
                    gcm_login: None,
                    gcm_name: None,
                    gcm_avatar_url: None,
                })
            }
            // Real 401 — the token is dead, safe to remove.
            Err(VenoreError::GitHubAuthRequired) => {
                warn!("Stored GitHub token rejected (401), removing");
                let _ = auth::remove_token();
                Ok(disconnected())
            }
            // Network / other error — keep the token, just report disconnected
            // for this session so the user can retry once back online.
            Err(e) => {
                warn!(error = %e, "Could not verify GitHub token (offline?), keeping it");
                Ok(disconnected())
            }
        }
    }
    .await;

    result.into()
}

/// Start the GitHub Device Flow authentication.
///
/// Returns the user_code and verification_uri immediately.
/// Spawns a background task that polls for token and emits events:
/// - `github:auth:pending` — still waiting
/// - `github:auth:success` — user authorized, includes login/name/avatar
/// - `github:auth:error` — flow failed
#[tauri::command]
pub async fn github_start_device_flow(
    app: AppHandle,
) -> CommandResult<GitHubDeviceFlowStartResponse> {
    info!("Starting GitHub Device Flow");

    let result: Result<GitHubDeviceFlowStartResponse, VenoreError> = async {
        let device_code_response = auth::request_device_code().await?;

        let response = GitHubDeviceFlowStartResponse {
            user_code: device_code_response.user_code.clone(),
            verification_uri: device_code_response.verification_uri.clone(),
            expires_in: device_code_response.expires_in,
        };

        // Reset cancellation flag
        DEVICE_FLOW_CANCEL.store(false, Ordering::SeqCst);

        let device_code = device_code_response.device_code.clone();
        let interval = device_code_response.interval;
        let expires_in = device_code_response.expires_in;

        // Spawn background polling task
        tokio::spawn(async move {
            let start = std::time::Instant::now();
            let timeout = std::time::Duration::from_secs(expires_in);
            let mut poll_interval = interval;

            loop {
                // Check cancellation
                if DEVICE_FLOW_CANCEL.load(Ordering::SeqCst) {
                    info!("Device Flow cancelled by user");
                    let _ = app.emit("github:auth:error", serde_json::json!({
                        "reason": "Cancelled by user"
                    }));
                    return;
                }

                // Check timeout
                if start.elapsed() > timeout {
                    warn!("Device Flow expired");
                    let _ = app.emit("github:auth:error", serde_json::json!({
                        "reason": "Device code expired. Please try again."
                    }));
                    return;
                }

                match auth::poll_for_token(&device_code, poll_interval).await {
                    Ok(Some(token)) => {
                        // Token received — store and validate
                        if let Err(e) = auth::store_token(&token) {
                            let _ = app.emit("github:auth:error", serde_json::json!({
                                "reason": format!("Failed to store token: {}", e)
                            }));
                            return;
                        }

                        let client = GitHubClient::new(token);
                        match client.get_authenticated_user().await {
                            Ok(user) => {
                                info!(login = %user.login, "Device Flow: authenticated");
                                let _ = app.emit("github:auth:success", serde_json::json!({
                                    "login": user.login,
                                    "name": user.name,
                                    "avatar_url": user.avatar_url,
                                }));
                            }
                            Err(e) => {
                                let _ = app.emit("github:auth:error", serde_json::json!({
                                    "reason": format!("Token validation failed: {}", e)
                                }));
                            }
                        }
                        return;
                    }
                    Ok(None) => {
                        // Still pending — continue polling
                        debug!("Device Flow: still pending");
                    }
                    Err(e) => {
                        warn!("Device Flow error: {}", e);
                        let _ = app.emit("github:auth:error", serde_json::json!({
                            "reason": e.to_string()
                        }));
                        return;
                    }
                }

                // The poll_for_token function already sleeps for `interval`,
                // but if GitHub sent slow_down, increase the interval
                poll_interval = interval;
            }
        });

        Ok(response)
    }
    .await;

    result.into()
}

/// Cancel an in-progress Device Flow.
#[tauri::command]
pub async fn github_cancel_device_flow() -> CommandResult<()> {
    info!("Cancelling GitHub Device Flow");
    DEVICE_FLOW_CANCEL.store(true, Ordering::SeqCst);
    CommandResult::ok(())
}

/// Store a Personal Access Token (PAT).
///
/// Validates the token first, then stores in keyring.
#[tauri::command]
pub async fn github_store_pat(
    request: GitHubStorePATRequest,
) -> CommandResult<GitHubAuthStatusResponse> {
    info!("Storing GitHub PAT");

    let result: Result<GitHubAuthStatusResponse, VenoreError> = async {
        let user = auth::store_pat(&request.token).await?;

        Ok(GitHubAuthStatusResponse {
            authenticated: true,
            login: Some(user.login),
            name: user.name,
            avatar_url: Some(user.avatar_url),
            gcm_detected: false,
            gcm_login: None,
            gcm_name: None,
            gcm_avatar_url: None,
        })
    }
    .await;

    result.into()
}

/// Disconnect from GitHub (remove token from keyring).
#[tauri::command]
pub async fn github_disconnect() -> CommandResult<()> {
    info!("Disconnecting from GitHub");

    let result: Result<(), VenoreError> = (|| {
        auth::remove_token()?;
        Ok(())
    })();

    result.into()
}

/// Accept the GCM-detected account: fetch token from GCM, validate, store in keyring.
///
/// Called when the user clicks "Use this account" in the GCM choice UI.
#[tauri::command]
pub async fn github_accept_gcm() -> CommandResult<GitHubAuthStatusResponse> {
    info!("Accepting GCM-detected GitHub account");

    let result: Result<GitHubAuthStatusResponse, VenoreError> = async {
        let token = auth::try_git_credential_token()
            .await?
            .ok_or(VenoreError::GitHubAuthRequired)?;

        let client = GitHubClient::new(token.clone());
        let user = client.validate_token().await?;

        auth::store_token(&token)?;

        Ok(GitHubAuthStatusResponse {
            authenticated: true,
            login: Some(user.login),
            name: user.name,
            avatar_url: Some(user.avatar_url),
            gcm_detected: false,
            gcm_login: None,
            gcm_name: None,
            gcm_avatar_url: None,
        })
    }
    .await;

    result.into()
}

// =============================================================================
// Repo Commands
// =============================================================================

/// Auto-detect GitHub owner/repo from git remote.
#[tauri::command]
pub async fn github_detect_repo(
    request: GitHubDetectRepoRequest,
) -> CommandResult<GitHubDetectRepoResponse> {
    debug!(project_path = %request.project_path, "Detecting GitHub repo");

    let result: Result<GitHubDetectRepoResponse, VenoreError> = (|| {
        let project_path = Path::new(&request.project_path);
        match repo::detect_github_repo(project_path)? {
            Some((owner, repo_name)) => Ok(GitHubDetectRepoResponse {
                detected: true,
                owner: Some(owner),
                repo: Some(repo_name),
            }),
            None => Ok(GitHubDetectRepoResponse {
                detected: false,
                owner: None,
                repo: None,
            }),
        }
    })();

    result.into()
}

// =============================================================================
// Helpers
// =============================================================================

/// Get a GitHubClient from the stored token or GCM, or error with GitHubAuthRequired.
async fn require_github_client() -> Result<GitHubClient, VenoreError> {
    let token = auth::resolve_token().await?;
    match token {
        Some(t) => Ok(GitHubClient::new(t)),
        None => Err(VenoreError::GitHubAuthRequired),
    }
}

/// Detect owner/repo from .git/config, or error with GitHubRepoNotDetected.
fn require_github_repo(project_path: &str) -> Result<(String, String), VenoreError> {
    let path = Path::new(project_path);
    match repo::detect_github_repo(path)? {
        Some((owner, repo)) => Ok((owner, repo)),
        None => Err(VenoreError::GitHubRepoNotDetected(project_path.to_string())),
    }
}

// =============================================================================
// Pull Request Commands
// =============================================================================

/// List pull requests for the linked repository.
#[tauri::command]
pub async fn github_list_pulls(
    request: GitHubListPullsRequest,
) -> CommandResult<GitHubListPullsResponse> {
    debug!(project_path = %request.project_path, "Listing GitHub pull requests");

    let result: Result<GitHubListPullsResponse, VenoreError> = async {
        let client = require_github_client().await?;
        let (owner, repo_name) = require_github_repo(&request.project_path)?;

        let state = request.state.as_deref().unwrap_or("open");
        let page = request.page.unwrap_or(1);
        let per_page = request.per_page.unwrap_or(20);

        let (prs, has_more) =
            pulls::list_pull_requests(&client, &owner, &repo_name, state, page, per_page).await?;

        let pull_dtos: Vec<GitHubPullRequestDto> = prs
            .into_iter()
            .map(|pr| GitHubPullRequestDto {
                number: pr.number,
                title: pr.title,
                state: pr.state,
                author: pr.user.login,
                author_avatar: pr.user.avatar_url,
                created_at: pr.created_at,
                updated_at: pr.updated_at,
                html_url: pr.html_url,
                body: pr.body,
                head_ref: pr.head.ref_name,
                base_ref: pr.base.ref_name,
                labels: pr
                    .labels
                    .into_iter()
                    .map(|l| GitHubLabelDto {
                        name: l.name,
                        color: l.color,
                    })
                    .collect(),
                draft: pr.draft,
                comments: pr.comments.unwrap_or(0),
                review_comments: pr.review_comments.unwrap_or(0),
            })
            .collect();

        Ok(GitHubListPullsResponse {
            pulls: pull_dtos,
            has_more,
            page,
            per_page,
        })
    }
    .await;

    result.into()
}

// =============================================================================
// Issue Commands
// =============================================================================

/// List issues for the linked repository (excludes PRs).
#[tauri::command]
pub async fn github_list_issues(
    request: GitHubListIssuesRequest,
) -> CommandResult<GitHubListIssuesResponse> {
    debug!(project_path = %request.project_path, "Listing GitHub issues");

    let result: Result<GitHubListIssuesResponse, VenoreError> = async {
        let client = require_github_client().await?;
        let (owner, repo_name) = require_github_repo(&request.project_path)?;

        let state = request.state.as_deref().unwrap_or("open");
        let page = request.page.unwrap_or(1);
        let per_page = request.per_page.unwrap_or(20);

        let (issue_list, has_more) =
            issues::list_issues(&client, &owner, &repo_name, state, page, per_page).await?;

        let issue_dtos: Vec<GitHubIssueDto> = issue_list
            .into_iter()
            .map(|issue| GitHubIssueDto {
                number: issue.number,
                title: issue.title,
                state: issue.state,
                author: issue.user.login,
                author_avatar: issue.user.avatar_url,
                created_at: issue.created_at,
                updated_at: issue.updated_at,
                html_url: issue.html_url,
                body: issue.body,
                labels: issue
                    .labels
                    .into_iter()
                    .map(|l| GitHubLabelDto {
                        name: l.name,
                        color: l.color,
                    })
                    .collect(),
                assignees: issue.assignees.into_iter().map(|a| a.login).collect(),
                comments: issue.comments.unwrap_or(0),
            })
            .collect();

        Ok(GitHubListIssuesResponse {
            issues: issue_dtos,
            has_more,
            page,
            per_page,
        })
    }
    .await;

    result.into()
}

// =============================================================================
// PR Detail Commands
// =============================================================================

/// Get a single pull request with full body and stats.
///
/// The list endpoint may truncate the body; this fetches the full PR.
#[tauri::command]
pub async fn github_get_pr_detail(
    request: GitHubGetPrDetailRequest,
) -> CommandResult<GitHubPrDetailResponse> {
    debug!(project_path = %request.project_path, pr_number = request.pr_number, "Getting PR detail");

    let result: Result<GitHubPrDetailResponse, VenoreError> = async {
        let client = require_github_client().await?;
        let (owner, repo_name) = require_github_repo(&request.project_path)?;

        let pr = pulls::get_pull_request(&client, &owner, &repo_name, request.pr_number).await?;

        Ok(GitHubPrDetailResponse {
            number: pr.number,
            title: pr.title,
            state: pr.state,
            author: pr.user.login,
            author_avatar: pr.user.avatar_url,
            created_at: pr.created_at,
            updated_at: pr.updated_at,
            html_url: pr.html_url,
            body: pr.body,
            head_ref: pr.head.ref_name,
            base_ref: pr.base.ref_name,
            labels: pr
                .labels
                .into_iter()
                .map(|l| GitHubLabelDto {
                    name: l.name,
                    color: l.color,
                })
                .collect(),
            draft: pr.draft,
            comments: pr.comments.unwrap_or(0),
            review_comments: pr.review_comments.unwrap_or(0),
            additions: pr.additions.unwrap_or(0),
            deletions: pr.deletions.unwrap_or(0),
            changed_files: pr.changed_files.unwrap_or(0),
        })
    }
    .await;

    result.into()
}

/// Get files changed in a pull request.
#[tauri::command]
pub async fn github_get_pr_files(
    request: GitHubGetPrFilesRequest,
) -> CommandResult<GitHubPrFilesResponse> {
    debug!(project_path = %request.project_path, pr_number = request.pr_number, "Getting PR files");

    let result: Result<GitHubPrFilesResponse, VenoreError> = async {
        let client = require_github_client().await?;
        let (owner, repo_name) = require_github_repo(&request.project_path)?;

        let files = pr_detail::list_pr_files(&client, &owner, &repo_name, request.pr_number, 1, 100).await?;

        let file_dtos: Vec<GitHubPrFileDto> = files
            .into_iter()
            .map(|f| GitHubPrFileDto {
                filename: f.filename,
                status: f.status,
                additions: f.additions,
                deletions: f.deletions,
                patch: f.patch,
            })
            .collect();

        Ok(GitHubPrFilesResponse { files: file_dtos })
    }
    .await;

    result.into()
}

// =============================================================================
// Comment Commands
// =============================================================================

/// Get comments for an issue or PR.
///
/// For PRs, fetches both general comments and inline review comments.
/// For issues, only fetches general comments (review_comments will be empty).
#[tauri::command]
pub async fn github_get_comments(
    request: GitHubGetCommentsRequest,
) -> CommandResult<GitHubCommentsResponse> {
    debug!(project_path = %request.project_path, number = request.number, is_pr = request.is_pull_request, "Getting comments");

    let result: Result<GitHubCommentsResponse, VenoreError> = async {
        let client = require_github_client().await?;
        let (owner, repo_name) = require_github_repo(&request.project_path)?;

        // General comments (works for both issues and PRs)
        let issue_comments = comments::list_issue_comments(&client, &owner, &repo_name, request.number).await?;

        let comment_dtos: Vec<GitHubCommentDto> = issue_comments
            .into_iter()
            .map(|c| GitHubCommentDto {
                id: c.id,
                author: c.user.login,
                author_avatar: c.user.avatar_url,
                body: c.body,
                created_at: c.created_at,
                html_url: c.html_url,
            })
            .collect();

        // Inline review comments (only for PRs)
        let review_comment_dtos = if request.is_pull_request {
            let review_comments = comments::list_pr_review_comments(&client, &owner, &repo_name, request.number).await?;

            review_comments
                .into_iter()
                .map(|c| GitHubReviewCommentDto {
                    id: c.id,
                    author: c.user.login,
                    author_avatar: c.user.avatar_url,
                    body: c.body,
                    path: c.path,
                    line: c.line,
                    diff_hunk: c.diff_hunk,
                    created_at: c.created_at,
                })
                .collect()
        } else {
            Vec::new()
        };

        Ok(GitHubCommentsResponse {
            comments: comment_dtos,
            review_comments: review_comment_dtos,
        })
    }
    .await;

    result.into()
}

// =============================================================================
// User Repos & Clone Commands
// =============================================================================

/// List repositories for the authenticated user.
#[tauri::command]
pub async fn github_list_user_repos(
    request: GitHubListUserReposRequest,
) -> CommandResult<GitHubListUserReposResponse> {
    debug!("Listing user repos");

    let result: Result<GitHubListUserReposResponse, VenoreError> = async {
        let client = require_github_client().await?;

        let page = request.page.unwrap_or(1);
        let per_page = request.per_page.unwrap_or(30);

        let (repo_list, has_more) = repos::list_user_repos(&client, page, per_page).await?;

        let repo_dtos: Vec<GitHubUserRepoDto> = repo_list
            .into_iter()
            .map(|r| GitHubUserRepoDto {
                id: r.id,
                name: r.name,
                full_name: r.full_name,
                owner: r.owner.login,
                description: r.description,
                html_url: r.html_url,
                clone_url: r.clone_url,
                is_private: r.is_private,
                language: r.language,
                stargazers_count: r.stargazers_count,
                updated_at: r.updated_at,
                default_branch: r.default_branch,
            })
            .collect();

        Ok(GitHubListUserReposResponse {
            repos: repo_dtos,
            has_more,
            page,
            per_page,
        })
    }
    .await;

    result.into()
}

/// Event payloads for clone progress streaming.
#[derive(Clone, Serialize)]
struct CloneProgressPayload {
    clone_id: String,
    percent: Option<u32>,
    phase: String,
}

#[derive(Clone, Serialize)]
struct CloneDonePayload {
    clone_id: String,
    path: String,
    owner: String,
    repo: String,
    /// True when the cloned repo carries a committed `.venore/project.json`
    /// — the launcher uses this to skip the wizard and open the workspace
    /// directly from the portable snapshot.
    has_venore: bool,
}

#[derive(Clone, Serialize)]
struct CloneErrorPayload {
    clone_id: String,
    message: String,
}

/// Clone a GitHub repository. Returns immediately with a clone_id.
///
/// Progress is streamed via Tauri events:
/// - `github:clone:progress` — progress updates with percent and phase
/// - `github:clone:done` — clone completed successfully
/// - `github:clone:error` — clone failed
#[tauri::command]
pub async fn github_clone_repo(
    app: AppHandle,
    request: GitHubCloneRepoRequest,
) -> CommandResult<GitHubCloneRepoResponse> {
    // Clone id is generated by the caller so the frontend can record it before
    // invoking — see `GitHubCloneRepoRequest::clone_id`.
    let clone_id = request.clone_id.clone();
    info!(clone_id = %clone_id, repo = %request.repo, "Starting repo clone");

    let result: Result<GitHubCloneRepoResponse, VenoreError> = async {
        if request.clone_id.trim().is_empty() {
            return Err(VenoreError::InvalidParams("Clone id is required".into()));
        }
        // Reject empty inputs before spawning the background task. Without
        // this guard `git clone` got invoked with an empty dest and surfaced
        // as opaque Windows error 123 ("invalid filename") in the modal.
        if request.dest_dir.trim().is_empty() {
            return Err(VenoreError::InvalidParams(
                "Destination directory is required".into(),
            ));
        }
        if request.repo.trim().is_empty() {
            return Err(VenoreError::InvalidParams("Repository name is required".into()));
        }

        let response_clone_id = clone_id.clone();
        let token = auth::resolve_token().await?;

        let clone_id_for_task = clone_id.clone();
        let clone_url = request.clone_url.clone();
        let dest_dir = request.dest_dir.clone();
        let repo_name = request.repo.clone();
        let owner = request.owner.clone();

        // Spawn background task
        tokio::spawn(async move {
            let dest_path = Path::new(&dest_dir);
            let app_ref = app.clone();
            let cid = clone_id_for_task.clone();

            let result = clone::clone_repository(
                &clone_url,
                dest_path,
                &repo_name,
                token.as_deref(),
                move |progress| {
                    let _ = app_ref.emit(
                        "github:clone:progress",
                        CloneProgressPayload {
                            clone_id: cid.clone(),
                            percent: progress.percent,
                            phase: progress.phase,
                        },
                    );
                },
            )
            .await;

            match result {
                Ok(path) => {
                    let has_venore = clone::has_venore_project(&path);
                    let _ = app.emit(
                        "github:clone:done",
                        CloneDonePayload {
                            clone_id: clone_id_for_task,
                            path: path.to_string_lossy().to_string(),
                            owner,
                            repo: repo_name,
                            has_venore,
                        },
                    );
                }
                Err(e) => {
                    warn!(error = %e, "Clone failed");
                    let _ = app.emit(
                        "github:clone:error",
                        CloneErrorPayload {
                            clone_id: clone_id_for_task,
                            message: e.to_string(),
                        },
                    );
                }
            }
        });

        Ok(GitHubCloneRepoResponse {
            clone_id: response_clone_id,
        })
    }
    .await;

    result.into()
}

/// Inspect the clone destination before cloning.
///
/// Tells the frontend whether `{dest_dir}/{repo}` already exists so it can
/// offer "open existing" vs "clone a fresh copy" instead of failing the clone.
/// Also computes the first free folder name for the fresh-copy path.
#[tauri::command]
pub async fn github_inspect_clone_destination(
    request: GitHubInspectDestinationRequest,
) -> CommandResult<GitHubInspectDestinationResponse> {
    let result: Result<GitHubInspectDestinationResponse, VenoreError> = (|| {
        if request.dest_dir.trim().is_empty() || request.repo.trim().is_empty() {
            return Err(VenoreError::InvalidParams(
                "Destination directory and repo are required".into(),
            ));
        }

        let dest_dir = Path::new(&request.dest_dir);
        let target = dest_dir.join(&request.repo);
        let exists = target.exists();
        let is_venore = exists && venore_core::github::clone::has_venore_project(&target);

        // First free name: repo, repo-1, repo-2, … (only searched when the
        // base name collides; bounded to avoid an unbounded loop).
        let suggested_name = if !exists {
            request.repo.clone()
        } else {
            (1..1000)
                .map(|n| format!("{}-{}", request.repo, n))
                .find(|name| !dest_dir.join(name).exists())
                .unwrap_or_else(|| format!("{}-{}", request.repo, uuid::Uuid::new_v4()))
        };

        Ok(GitHubInspectDestinationResponse {
            exists,
            path: target.to_string_lossy().to_string(),
            is_venore,
            suggested_name,
        })
    })();

    result.into()
}

// =============================================================================
// PR Analysis Commands
// =============================================================================

/// Event payloads for PR analysis streaming.
#[derive(Clone, Serialize)]
struct PrAnalysisDeltaPayload {
    stream_id: String,
    content: String,
}

#[derive(Clone, Serialize)]
struct PrAnalysisDonePayload {
    stream_id: String,
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
    provider: String,
    model: String,
}

#[derive(Clone, Serialize)]
struct PrAnalysisErrorPayload {
    stream_id: String,
    message: String,
}

/// Analyze a PR with LLM. Returns immediately with stream_id.
/// Streaming happens via Tauri events:
/// - `github:pr-analysis:delta` — text chunks
/// - `github:pr-analysis:done` — completion with token usage
/// - `github:pr-analysis:error` — errors
#[tauri::command]
pub async fn github_analyze_pr(
    app: AppHandle,
    lazy_state: tauri::State<'_, LazyAppState>,
    request: GitHubAnalyzePrRequest,
) -> StateCommandResult<GitHubAnalyzePrResponse> {
    use super::llm::get_services;
    use venore_core::chat::{ChatMessageInput, create_chat_stream};
    use venore_core::llm::prelude::*;
    use venore_core::traits::TaskConfigStore;

    let services = get_services(&lazy_state);

    let result: Result<GitHubAnalyzePrResponse, VenoreError> = async {
        let (config_store, llm_gateway) = services?;

        let stream_id = request.stream_id.clone();
        let project_path_str = request.project_path.clone();
        let pr_number = request.pr_number;

        // Read task settings for Analysis task
        let task_settings = config_store.get_task_settings(venore_core::traits::LlmTask::Analysis).await?;
        let configured_provider = task_settings.provider;
        let configured_model = task_settings.model.clone();

        info!(
            provider = configured_provider.as_str(),
            model = %configured_model,
            pr_number,
            "Starting PR analysis"
        );

        // Fetch PR detail and files
        let client = require_github_client().await?;
        let (owner, repo_name) = require_github_repo(&project_path_str)?;

        let pr = pulls::get_pull_request(&client, &owner, &repo_name, pr_number).await?;
        let files = pr_detail::list_pr_files(&client, &owner, &repo_name, pr_number, 1, 100).await?;

        // Assemble context and build prompt
        let project_path = Path::new(&project_path_str);
        let analysis_ctx = pr_analyzer::assemble_pr_context(
            project_path,
            &pr.title,
            pr.body.as_deref(),
            &pr.user.login,
            &pr.head.ref_name,
            &pr.base.ref_name,
            &files,
        );

        let analysis_prompt = pr_analyzer::build_pr_analysis_prompt(&analysis_ctx, pr_analyzer::AnalysisDepthLevel::Normal);

        // Create the LLM stream
        let messages = vec![ChatMessageInput {
            role: "user".to_string(),
            content: analysis_prompt,
        }];

        let system_prompt = {
            let prompt_repo = {
                let guard = lazy_state.get();
                guard.as_ref().map(|s| std::sync::Arc::clone(&s.prompt_repository))
            };
            if let Some(repo) = prompt_repo {
                let provider_str = configured_provider.as_str();
                repo.resolve_prompt("github", provider_str).await
                    .map(|p| p.content)
                    .unwrap_or_else(|_| "You are a senior code reviewer analyzing a pull request. Evaluate the changes against the project's established patterns and conventions.".to_string())
            } else {
                "You are a senior code reviewer analyzing a pull request. Evaluate the changes against the project's established patterns and conventions.".to_string()
            }
        };

        // Gateway resolves provider/model from DB internally
        let options = GatewayOptions::for_task(venore_core::traits::LlmTask::Analysis);

        let (initial_stream, model_name) = create_chat_stream(
            &llm_gateway,
            messages,
            &system_prompt,
            options,
            None,
        )
        .await?;

        // Spawn the stream consumer
        let app_clone = app.clone();
        let stream_id_clone = stream_id.clone();
        let provider_name = configured_provider.as_str().to_string();

        let join_handle = tokio::spawn(async move {
            use futures::StreamExt;

            let mut current_stream = initial_stream;

            while let Some(chunk_result) = current_stream.next().await {
                match chunk_result {
                    Ok(chunk) => match chunk {
                        LlmStreamChunk::Text { content } => {
                            if content.is_empty() {
                                continue;
                            }
                            let _ = app_clone.emit(
                                "github:pr-analysis:delta",
                                PrAnalysisDeltaPayload {
                                    stream_id: stream_id_clone.clone(),
                                    content,
                                },
                            );
                        }
                        LlmStreamChunk::Done { usage, .. } => {
                            let (p, c, t) =
                                venore_core::chat::orchestrator::extract_usage(&usage);
                            let _ = app_clone.emit(
                                "github:pr-analysis:done",
                                PrAnalysisDonePayload {
                                    stream_id: stream_id_clone.clone(),
                                    prompt_tokens: p,
                                    completion_tokens: c,
                                    total_tokens: t,
                                    provider: provider_name.clone(),
                                    model: model_name.clone(),
                                },
                            );
                        }
                        LlmStreamChunk::Error { error } => {
                            let _ = app_clone.emit(
                                "github:pr-analysis:error",
                                PrAnalysisErrorPayload {
                                    stream_id: stream_id_clone.clone(),
                                    message: error,
                                },
                            );
                            return;
                        }
                        _ => {} // Ignore tool calls — no tools in analysis
                    },
                    Err(e) => {
                        let _ = app_clone.emit(
                            "github:pr-analysis:error",
                            PrAnalysisErrorPayload {
                                stream_id: stream_id_clone.clone(),
                                message: e.to_string(),
                            },
                        );
                        return;
                    }
                }
            }

            // Cleanup
            if let Ok(mut guard) = ACTIVE_PR_ANALYSES.lock() {
                guard.remove(&stream_id_clone);
            }
        });

        // Store abort handle for cancellation
        if let Ok(mut guard) = ACTIVE_PR_ANALYSES.lock() {
            guard.insert(stream_id.clone(), join_handle.abort_handle());
        }

        Ok(GitHubAnalyzePrResponse { stream_id })
    }
    .await;

    result.into_state()
}

/// Stop an in-progress PR analysis.
#[tauri::command]
pub async fn github_stop_pr_analysis(
    request: GitHubStopPrAnalysisRequest,
) -> CommandResult<()> {
    info!(stream_id = %request.stream_id, "Stopping PR analysis");

    if let Ok(mut guard) = ACTIVE_PR_ANALYSES.lock() {
        if let Some(handle) = guard.remove(&request.stream_id) {
            handle.abort();
        }
    }

    CommandResult::ok(())
}
