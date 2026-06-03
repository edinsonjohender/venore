//! Session Tauri commands
//!
//! Branch-per-session workflow: create, list, complete, abandon sessions.

use std::path::Path;
use std::sync::Arc;

use venore_core::error::VenoreError;
use venore_core::session::{
    git_ops, Session, SessionRepository, SessionStatus,
};
use venore_core::terminal::TerminalSessionManager;
use venore_core::github::{auth, client::GitHubClient, repo as gh_repo, branches as gh_branches};

use crate::state::LazyAppState;
use crate::utils::{CommandResult, IntoStateCommandResult, StateCommandResult};

use super::dto::session::*;

// =============================================================================
// Helpers
// =============================================================================

fn get_state_parts(lazy: &LazyAppState) -> Result<(Arc<SessionRepository>, std::path::PathBuf), VenoreError> {
    let guard = lazy.get();
    match guard.as_ref() {
        Some(state) => Ok((Arc::clone(&state.session_repository), state.config_dir.clone())),
        None => Err(VenoreError::NotFound("Backend not initialized".into())),
    }
}

fn get_session_repo(lazy: &LazyAppState) -> Result<Arc<SessionRepository>, VenoreError> {
    let (repo, _) = get_state_parts(lazy)?;
    Ok(repo)
}

fn session_to_dto(session: &Session, stats: (u32, u32, u32)) -> SessionDto {
    SessionDto {
        id: session.id.clone(),
        name: session.name.clone(),
        objective: session.objective.clone(),
        project_id: session.project_id.clone(),
        base_branch: session.base_branch.clone(),
        session_branch: session.session_branch.clone(),
        worktree_path: session.worktree_path.clone(),
        status: session.status.as_str().to_string(),
        files_changed: stats.0,
        additions: stats.1,
        deletions: stats.2,
        created_at: session.created_at.clone(),
        updated_at: session.updated_at.clone(),
    }
}

// =============================================================================
// Commands
// =============================================================================

#[tauri::command]
pub async fn create_session(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: CreateSessionRequest,
) -> StateCommandResult<SessionDto> {
    let parts = get_state_parts(&lazy_state);
    let result: Result<SessionDto, VenoreError> = async {
        let (repo, config_dir) = parts?;
        let project_path = Path::new(&request.project_path);

        // Validate git repo
        if !git_ops::is_git_repo(project_path) {
            return Err(VenoreError::NotGitRepository(request.project_path.clone()));
        }

        let now = chrono::Utc::now().to_rfc3339();
        let id = uuid::Uuid::new_v4().to_string();

        // Compute worktree path and create isolated checkout
        let wt_path = git_ops::worktree_path(&config_dir, &request.project_id, &id);
        git_ops::create_worktree(project_path, &wt_path, &request.branch_name, &request.base_branch)?;

        let wt_path_str = wt_path.to_string_lossy().to_string();

        let session = Session {
            id,
            name: request.name,
            objective: request.objective,
            project_id: request.project_id,
            base_branch: request.base_branch,
            session_branch: request.branch_name,
            worktree_path: wt_path_str,
            status: SessionStatus::Active,
            created_at: now.clone(),
            updated_at: now,
        };

        repo.create(&session).await?;

        tracing::info!(session_id = %session.id, branch = %session.session_branch, worktree = %session.worktree_path, "Session created with worktree");

        Ok(session_to_dto(&session, (0, 0, 0)))
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn list_sessions(
    lazy_state: tauri::State<'_, LazyAppState>,
    project_id: String,
    project_path: String,
) -> StateCommandResult<Vec<SessionDto>> {
    let repo = get_session_repo(&lazy_state);
    let result: Result<Vec<SessionDto>, VenoreError> = async {
        let repo = repo?;
        let sessions = repo.list_by_project(&project_id).await?;
        let path = Path::new(&project_path);

        let mut dtos = Vec::new();
        for session in &sessions {
            let stats = if !session.worktree_path.is_empty() && Path::new(&session.worktree_path).exists() {
                git_ops::get_worktree_diff_summary(Path::new(&session.worktree_path), &session.base_branch)
                    .unwrap_or((0, 0, 0))
            } else {
                git_ops::get_diff_summary(path, &session.base_branch, &session.session_branch)
                    .unwrap_or((0, 0, 0))
            };
            dtos.push(session_to_dto(session, stats));
        }

        Ok(dtos)
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn get_session(
    lazy_state: tauri::State<'_, LazyAppState>,
    session_id: String,
    project_path: String,
) -> StateCommandResult<SessionDto> {
    let repo = get_session_repo(&lazy_state);
    let result: Result<SessionDto, VenoreError> = async {
        let repo = repo?;
        let session = repo.get(&session_id).await?
            .ok_or_else(|| VenoreError::NotFound(format!("Session not found: {}", session_id)))?;

        let stats = if !session.worktree_path.is_empty() && Path::new(&session.worktree_path).exists() {
            git_ops::get_worktree_diff_summary(Path::new(&session.worktree_path), &session.base_branch)
                .unwrap_or((0, 0, 0))
        } else {
            let path = Path::new(&project_path);
            git_ops::get_diff_summary(path, &session.base_branch, &session.session_branch)
                .unwrap_or((0, 0, 0))
        };

        Ok(session_to_dto(&session, stats))
    }
    .await;
    result.into_state()
}

/// Kill and unbind the terminal dedicated to a dev session (if any).
/// Called when a session is completed or abandoned so processes don't linger.
fn kill_session_terminal(session_id: &str) {
    let mgr = TerminalSessionManager::global();
    let mut guard = match mgr.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    if let Some(tid) = guard.get_session_terminal(session_id).map(|s| s.to_string()) {
        if let Err(e) = guard.kill(&tid) {
            tracing::warn!(session_id = %session_id, terminal_id = %tid, "Failed to kill session terminal: {}", e);
        }
    }
}

#[tauri::command]
pub async fn complete_session(
    lazy_state: tauri::State<'_, LazyAppState>,
    session_id: String,
    project_path: String,
) -> StateCommandResult<SessionDto> {
    let repo = get_session_repo(&lazy_state);
    let result: Result<SessionDto, VenoreError> = async {
        let repo = repo?;
        let session = repo.get(&session_id).await?
            .ok_or_else(|| VenoreError::NotFound(format!("Session not found: {}", session_id)))?;

        if session.status != SessionStatus::Active {
            return Err(VenoreError::InvalidParams("Only active sessions can be completed".into()));
        }

        let path = Path::new(&project_path);

        // Kill the session's dedicated terminal (if any)
        kill_session_terminal(&session_id);

        // Remove worktree (branch stays for history)
        if !session.worktree_path.is_empty() {
            let wt = Path::new(&session.worktree_path);
            git_ops::remove_worktree(path, wt)?;
        }

        // Update status
        repo.update_status(&session_id, SessionStatus::Completed).await?;

        let stats = git_ops::get_diff_summary(path, &session.base_branch, &session.session_branch)
            .unwrap_or((0, 0, 0));

        let mut updated_session = session;
        updated_session.status = SessionStatus::Completed;

        tracing::info!(session_id = %session_id, "Session completed");

        Ok(session_to_dto(&updated_session, stats))
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn abandon_session(
    lazy_state: tauri::State<'_, LazyAppState>,
    session_id: String,
    project_path: String,
) -> StateCommandResult<()> {
    let repo = get_session_repo(&lazy_state);
    let result: Result<(), VenoreError> = async {
        let repo = repo?;
        let session = repo.get(&session_id).await?
            .ok_or_else(|| VenoreError::NotFound(format!("Session not found: {}", session_id)))?;

        if session.status != SessionStatus::Active {
            return Err(VenoreError::InvalidParams("Only active sessions can be abandoned".into()));
        }

        let path = Path::new(&project_path);

        // Kill the session's dedicated terminal (if any)
        kill_session_terminal(&session_id);

        // Remove worktree
        if !session.worktree_path.is_empty() {
            let wt = Path::new(&session.worktree_path);
            git_ops::remove_worktree(path, wt)?;
        }

        // Delete branch (abandoned branch has no value)
        git_ops::delete_branch(path, &session.session_branch)?;

        repo.update_status(&session_id, SessionStatus::Abandoned).await?;

        tracing::info!(session_id = %session_id, "Session abandoned (worktree + branch removed)");

        Ok(())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn session_diff_files(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: SessionDiffRequest,
) -> StateCommandResult<Vec<SessionDiffFileDto>> {
    let repo = get_session_repo(&lazy_state);
    let result: Result<Vec<SessionDiffFileDto>, VenoreError> = async {
        let repo = repo?;
        let session = repo.get(&request.session_id).await?
            .ok_or_else(|| VenoreError::NotFound(format!("Session not found: {}", request.session_id)))?;

        // Use worktree diff (includes uncommitted changes) when worktree exists
        let diff_files = if !session.worktree_path.is_empty() && Path::new(&session.worktree_path).exists() {
            git_ops::get_worktree_diff_files(Path::new(&session.worktree_path), &session.base_branch)?
        } else {
            let path = Path::new(&request.project_path);
            git_ops::get_diff_files(path, &session.base_branch, &session.session_branch)?
        };

        Ok(diff_files
            .into_iter()
            .map(|f| SessionDiffFileDto {
                filename: f.filename,
                status: f.status,
                additions: f.additions,
                deletions: f.deletions,
                patch: f.patch,
            })
            .collect())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn session_commits(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: SessionDiffRequest,
) -> StateCommandResult<Vec<SessionCommitDto>> {
    let repo = get_session_repo(&lazy_state);
    let result: Result<Vec<SessionCommitDto>, VenoreError> = async {
        let repo = repo?;
        let session = repo.get(&request.session_id).await?
            .ok_or_else(|| VenoreError::NotFound(format!("Session not found: {}", request.session_id)))?;

        let path = Path::new(&request.project_path);
        let commits = git_ops::get_commits(path, &session.base_branch, &session.session_branch)?;

        Ok(commits
            .into_iter()
            .map(|c| SessionCommitDto {
                hash: c.hash,
                short_hash: c.short_hash,
                message: c.message,
                author: c.author,
                date: c.date,
            })
            .collect())
    }
    .await;
    result.into_state()
}

/// Revert a dev session worktree to a previous snapshot commit.
/// Also deletes chat messages after the revert point.
/// `message_id` is optional: when called from the chat panel it's provided (prune by message),
/// when called from the Activity Tab it's absent (prune by snapshot timestamp).
#[tauri::command]
pub async fn revert_to_snapshot(
    app: tauri::AppHandle,
    lazy_state: tauri::State<'_, LazyAppState>,
    dev_session_id: String,
    commit_hash: String,
    message_id: Option<String>,
) -> StateCommandResult<()> {
    // Extract Arc'd repos before any await (MutexGuard is not Send)
    let (session_repo, chat_repo) = {
        let guard = lazy_state.get();
        match guard.as_ref() {
            Some(s) => (Arc::clone(&s.session_repository), Arc::clone(&s.chat_repository)),
            None => return CommandResult::err(
                VenoreError::NotFound("Backend not initialized".into())
            ).into_state(),
        }
    };

    let result: Result<(), VenoreError> = async {
        // 1. Get session → worktree_path
        let session = session_repo.get(&dev_session_id).await?
            .ok_or_else(|| VenoreError::NotFound(format!("Session not found: {}", dev_session_id)))?;

        if session.worktree_path.is_empty() || !Path::new(&session.worktree_path).exists() {
            return Err(VenoreError::InvalidParams("Session worktree does not exist".into()));
        }

        // 2. git reset --hard <commit>
        git_ops::reset_to_commit(Path::new(&session.worktree_path), &commit_hash)?;

        // 3. Find the chat session linked to this dev session, then delete messages + snapshots after revert point
        if let Some(chat_session) = chat_repo.find_by_dev_session_id(&dev_session_id).await? {
            // Delete messages: by message_id (chat panel) or by snapshot timestamp (activity tab)
            if let Some(ref mid) = message_id {
                let deleted = chat_repo.delete_messages_after(&chat_session.id, mid).await?;
                tracing::info!(chat_session_id = %chat_session.id, deleted = deleted, "Deleted chat messages after revert point (by message_id)");
            } else {
                // No message_id — use snapshot's created_at as cutoff for messages too
                let snapshots = chat_repo.get_snapshots(&chat_session.id).await.unwrap_or_default();
                if let Some(snap) = snapshots.iter().find(|s| s.commit_hash == commit_hash) {
                    let deleted = chat_repo.delete_messages_after_timestamp(&chat_session.id, &snap.created_at).await?;
                    tracing::info!(chat_session_id = %chat_session.id, deleted = deleted, "Deleted chat messages after revert point (by timestamp)");
                }
            }

            // Also clean snapshots + tool calls after the revert point (use snapshot created_at as cutoff)
            let snapshots = chat_repo.get_snapshots(&chat_session.id).await.unwrap_or_default();
            if let Some(snap) = snapshots.iter().find(|s| s.commit_hash == commit_hash) {
                let _ = chat_repo.delete_snapshots_after(&chat_session.id, &snap.created_at).await;
                let _ = chat_repo.delete_tool_calls_after(&chat_session.id, &snap.created_at).await;
            }
        }

        // 4. Emit events for frontend refresh
        use tauri::Emitter;
        let _ = app.emit("session:files-refreshed", serde_json::json!({
            "dev_session_id": dev_session_id,
        }));

        tracing::info!(dev_session_id = %dev_session_id, commit = %commit_hash, "Reverted session to snapshot");
        Ok(())
    }
    .await;
    result.into_state()
}

/// List branches: tries local git first, falls back to GitHub API if repo is linked.
/// Returns branches + whether the project is a local git repo (needed for session creation).
#[tauri::command]
pub async fn list_git_branches(
    request: ListBranchesRequest,
) -> StateCommandResult<ListBranchesResponse> {
    let path = Path::new(&request.project_path);

    // 1. Try local git
    let is_local_git = path.exists() && git_ops::is_git_repo(path);

    if is_local_git {
        let result = git_ops::list_local_branches(path);
        if let Ok(branches) = result {
            tracing::info!(count = branches.len(), "Listed local branches");
            return Ok(ListBranchesResponse { branches, is_local_git: true }).into_state();
        }
    }

    // 2. Fallback: GitHub API via detected repo
    tracing::info!(path = %request.project_path, "No local git, trying GitHub API fallback");
    let result: Result<ListBranchesResponse, VenoreError> = async {
        let (owner, repo_name) = gh_repo::detect_github_repo(path)?
            .ok_or_else(|| VenoreError::NotGitRepository(
                "Not a git repository and no GitHub remote detected".into(),
            ))?;

        let token = auth::resolve_token().await?
            .ok_or(VenoreError::GitHubAuthRequired)?;

        let client = GitHubClient::new(token);
        let branches = gh_branches::list_branches(&client, &owner, &repo_name).await?;
        tracing::info!(count = branches.len(), "Listed branches via GitHub API");
        Ok(ListBranchesResponse { branches, is_local_git: false })
    }
    .await;
    result.into_state()
}
