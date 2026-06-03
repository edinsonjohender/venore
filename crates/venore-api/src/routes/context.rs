//! Context generation API endpoints

use axum::{
    extract::State,
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
pub struct GenerateContextRequest {
    /// Island ID
    pub island_id: Uuid,

    /// Provider to use (anthropic, openai, ollama)
    pub provider: String,
}

#[derive(Serialize)]
pub struct GenerateContextResponse {
    /// Generated .context.md content
    pub content: String,

    /// Tokens used (if applicable)
    pub tokens_used: Option<u32>,
}

// ============================================================================
// HANDLERS
// ============================================================================

/// POST /api/context/generate
/// Generate a .context.md file for an island
pub async fn generate(
    State(_state): State<Arc<AppState>>,
    Json(_payload): Json<GenerateContextRequest>,
) -> Result<Json<GenerateContextResponse>, StatusCode> {
    // TODO: implement generation using venore-core
    // let generator = state.context_generator;
    // let island = state.island_repository.find_by_id(&payload.island_id).await?;
    // let content = generator.generate(&island).await?;

    Ok(Json(GenerateContextResponse {
        content: "# Generated context\n\nExample content".to_string(),
        tokens_used: Some(150),
    }))
}
