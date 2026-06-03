//! Project-related API endpoints

use axum::{
    extract::{Path, State},
    Json,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;

// ============================================================================
// REQUEST/RESPONSE TYPES
// ============================================================================

#[derive(Deserialize)]
pub struct AnalyzeRequest {
    /// Absolute path to the project
    pub path: String,
}

#[derive(Serialize)]
pub struct AnalyzeResponse {
    pub project_id: Uuid,
    pub name: String,
    pub islands_count: usize,
}

#[derive(Serialize)]
pub struct ProjectResponse {
    pub id: Uuid,
    pub name: String,
    pub path: String,
}

// ============================================================================
// HANDLERS
// ============================================================================

/// POST /api/projects/analyze
/// Analyze a project and return its structure
pub async fn analyze(
    State(_state): State<Arc<AppState>>,
    Json(_payload): Json<AnalyzeRequest>,
) -> Result<Json<AnalyzeResponse>, StatusCode> {
    // TODO: implement analysis using venore-core
    // let analyzer = state.project_analyzer;
    // let project = analyzer.analyze(&payload.path).await?;

    // Placeholder response for now
    Ok(Json(AnalyzeResponse {
        project_id: Uuid::new_v4(),
        name: "example-project".to_string(),
        islands_count: 0,
    }))
}

/// GET /api/projects/:id
/// Fetch a project by ID
pub async fn get_by_id(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ProjectResponse>, StatusCode> {
    // TODO: query the database
    Ok(Json(ProjectResponse {
        id,
        name: "example".to_string(),
        path: "/path/to/project".to_string(),
    }))
}

/// GET /api/projects
/// List all projects
pub async fn list(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<Vec<ProjectResponse>>, StatusCode> {
    // TODO: list from the database
    Ok(Json(Vec::new()))
}
