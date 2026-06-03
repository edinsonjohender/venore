//! DTOs for file editor commands

use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct ReadFileRequest {
    pub project_path: String,
    pub relative_path: String,
}

#[derive(Serialize)]
pub struct ReadFileResponse {
    pub content: String,
    pub size: u64,
}

#[derive(Deserialize)]
pub struct WriteFileRequest {
    pub project_path: String,
    pub relative_path: String,
    pub content: String,
}
