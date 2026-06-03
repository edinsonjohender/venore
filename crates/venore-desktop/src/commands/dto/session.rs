//! Session DTOs for Tauri IPC

use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct CreateSessionRequest {
    pub name: String,
    pub objective: String,
    pub project_path: String,
    pub project_id: String,
    pub base_branch: String,
    pub branch_name: String,
}

#[derive(Serialize)]
pub struct SessionDto {
    pub id: String,
    pub name: String,
    pub objective: String,
    pub project_id: String,
    pub base_branch: String,
    pub session_branch: String,
    pub worktree_path: String,
    pub status: String,
    pub files_changed: u32,
    pub additions: u32,
    pub deletions: u32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize)]
pub struct SessionDiffFileDto {
    pub filename: String,
    pub status: String,
    pub additions: u32,
    pub deletions: u32,
    pub patch: Option<String>,
}

#[derive(Serialize)]
pub struct SessionCommitDto {
    pub hash: String,
    pub short_hash: String,
    pub message: String,
    pub author: String,
    pub date: String,
}

#[derive(Deserialize)]
pub struct SessionDiffRequest {
    pub session_id: String,
    pub project_path: String,
}

#[derive(Deserialize)]
pub struct ListBranchesRequest {
    pub project_path: String,
}

#[derive(Serialize)]
pub struct ListBranchesResponse {
    pub branches: Vec<String>,
    pub is_local_git: bool,
}
