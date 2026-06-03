//! Knowledge Island Tauri commands
//!
//! CRUD for features, hexagons, and evidence.

use venore_core::error::VenoreError;
use venore_core::knowledge::{KnowledgeFeature, KnowledgeHexagon, KnowledgeEvidence};

use crate::state::{LazyAppState, get_state_field};
use crate::utils::{IntoStateCommandResult, StateCommandResult};
use super::dto::knowledge::*;

// =========================================================================
// Features
// =========================================================================

#[tauri::command]
pub async fn create_knowledge_feature(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: CreateFeatureRequest,
) -> StateCommandResult<FeatureResponse> {
    let repo = get_state_field!(lazy_state, knowledge_repository);
    let result: Result<FeatureResponse, VenoreError> = async {
        let repo = repo?;
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let feature = KnowledgeFeature {
            id,
            project_id: request.project_id,
            name: request.name,
            description: request.description,
            status: "active".to_string(),
            priority: "medium".to_string(),
            objective: request.objective.unwrap_or_else(|| "explore".to_string()),
            intensity: request.intensity.unwrap_or_else(|| "moderate".to_string()),
            max_hexagons_per_phase: request.max_hexagons_per_phase.unwrap_or(7),
            auto_advance: request.auto_advance.unwrap_or(false),
            tags: request.tags.unwrap_or_else(|| "[]".to_string()),
            created_at: now.clone(),
            updated_at: now,
        };
        repo.create_feature(&feature).await?;
        Ok(FeatureResponse::from(feature))
    }.await;
    result.into_state()
}

#[tauri::command]
pub async fn get_knowledge_feature(
    lazy_state: tauri::State<'_, LazyAppState>,
    id: String,
) -> StateCommandResult<FeatureResponse> {
    let repo = get_state_field!(lazy_state, knowledge_repository);
    let result: Result<FeatureResponse, VenoreError> = async {
        let repo = repo?;
        let feature = repo.get_feature(&id).await?
            .ok_or_else(|| VenoreError::NotFound(format!("Feature not found: {}", id)))?;
        Ok(FeatureResponse::from(feature))
    }.await;
    result.into_state()
}

#[tauri::command]
pub async fn list_knowledge_features(
    lazy_state: tauri::State<'_, LazyAppState>,
    project_id: String,
) -> StateCommandResult<Vec<FeatureResponse>> {
    let repo = get_state_field!(lazy_state, knowledge_repository);
    let result: Result<Vec<FeatureResponse>, VenoreError> = async {
        let repo = repo?;
        let features = repo.list_features_by_project(&project_id).await?;
        Ok(features.into_iter().map(FeatureResponse::from).collect())
    }.await;
    result.into_state()
}

#[tauri::command]
pub async fn update_knowledge_feature(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: UpdateFeatureRequest,
) -> StateCommandResult<FeatureResponse> {
    let repo = get_state_field!(lazy_state, knowledge_repository);
    let result: Result<FeatureResponse, VenoreError> = async {
        let repo = repo?;
        let now = chrono::Utc::now().to_rfc3339();
        let existing = repo.get_feature(&request.id).await?
            .ok_or_else(|| VenoreError::NotFound(format!("Feature not found: {}", request.id)))?;
        let feature = KnowledgeFeature {
            id: request.id,
            project_id: existing.project_id,
            name: request.name,
            description: request.description,
            status: request.status,
            priority: request.priority,
            objective: request.objective.unwrap_or(existing.objective),
            intensity: request.intensity.unwrap_or(existing.intensity),
            max_hexagons_per_phase: request.max_hexagons_per_phase.unwrap_or(existing.max_hexagons_per_phase),
            auto_advance: request.auto_advance.unwrap_or(existing.auto_advance),
            tags: request.tags.unwrap_or(existing.tags),
            created_at: existing.created_at,
            updated_at: now,
        };
        repo.update_feature(&feature).await?;
        Ok(FeatureResponse::from(feature))
    }.await;
    result.into_state()
}

#[tauri::command]
pub async fn delete_knowledge_feature(
    lazy_state: tauri::State<'_, LazyAppState>,
    id: String,
) -> StateCommandResult<()> {
    let repo = get_state_field!(lazy_state, knowledge_repository);
    let result: Result<(), VenoreError> = async {
        let repo = repo?;
        repo.delete_feature(&id).await?;
        Ok(())
    }.await;
    result.into_state()
}

// =========================================================================
// Hexagons
// =========================================================================

#[tauri::command]
pub async fn create_knowledge_hexagon(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: CreateHexagonRequest,
) -> StateCommandResult<HexagonResponse> {
    let repo = get_state_field!(lazy_state, knowledge_repository);
    let result: Result<HexagonResponse, VenoreError> = async {
        let repo = repo?;
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let hex = KnowledgeHexagon {
            id,
            feature_id: request.feature_id,
            title: request.title,
            description: request.description,
            phase: "discover".to_string(),
            percentage: 0,
            confidence: "low".to_string(),
            risk: "unknown".to_string(),
            priority: "medium".to_string(),
            is_dead_end: false,
            blocked_by: "[]".to_string(),
            notes_user: "".to_string(),
            agent_status: "idle".to_string(),
            created_at: now.clone(),
            updated_at: now,
        };
        repo.create_hexagon(&hex).await?;
        Ok(HexagonResponse::from(hex))
    }.await;
    result.into_state()
}

#[tauri::command]
pub async fn get_knowledge_hexagon(
    lazy_state: tauri::State<'_, LazyAppState>,
    id: String,
) -> StateCommandResult<HexagonResponse> {
    let repo = get_state_field!(lazy_state, knowledge_repository);
    let result: Result<HexagonResponse, VenoreError> = async {
        let repo = repo?;
        let hex = repo.get_hexagon(&id).await?
            .ok_or_else(|| VenoreError::NotFound(format!("Hexagon not found: {}", id)))?;
        Ok(HexagonResponse::from(hex))
    }.await;
    result.into_state()
}

#[tauri::command]
pub async fn list_knowledge_hexagons(
    lazy_state: tauri::State<'_, LazyAppState>,
    feature_id: String,
) -> StateCommandResult<Vec<HexagonResponse>> {
    let repo = get_state_field!(lazy_state, knowledge_repository);
    let result: Result<Vec<HexagonResponse>, VenoreError> = async {
        let repo = repo?;
        let hexagons = repo.list_hexagons_by_feature(&feature_id).await?;
        Ok(hexagons.into_iter().map(HexagonResponse::from).collect())
    }.await;
    result.into_state()
}

#[tauri::command]
pub async fn update_knowledge_hexagon(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: UpdateHexagonRequest,
) -> StateCommandResult<HexagonResponse> {
    let repo = get_state_field!(lazy_state, knowledge_repository);
    let result: Result<HexagonResponse, VenoreError> = async {
        let repo = repo?;
        let now = chrono::Utc::now().to_rfc3339();
        let existing = repo.get_hexagon(&request.id).await?
            .ok_or_else(|| VenoreError::NotFound(format!("Hexagon not found: {}", request.id)))?;
        let hex = KnowledgeHexagon {
            id: request.id,
            feature_id: existing.feature_id,
            title: request.title,
            description: request.description,
            phase: request.phase,
            percentage: request.percentage,
            confidence: request.confidence,
            risk: request.risk,
            priority: request.priority,
            is_dead_end: request.is_dead_end,
            blocked_by: request.blocked_by,
            notes_user: request.notes_user,
            agent_status: request.agent_status.unwrap_or(existing.agent_status),
            created_at: existing.created_at,
            updated_at: now,
        };
        repo.update_hexagon(&hex).await?;
        Ok(HexagonResponse::from(hex))
    }.await;
    result.into_state()
}

#[tauri::command]
pub async fn delete_knowledge_hexagon(
    lazy_state: tauri::State<'_, LazyAppState>,
    id: String,
) -> StateCommandResult<()> {
    let repo = get_state_field!(lazy_state, knowledge_repository);
    let result: Result<(), VenoreError> = async {
        let repo = repo?;
        repo.delete_hexagon(&id).await?;
        Ok(())
    }.await;
    result.into_state()
}

// =========================================================================
// Evidence
// =========================================================================

#[tauri::command]
pub async fn create_knowledge_evidence(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: CreateEvidenceRequest,
) -> StateCommandResult<EvidenceResponse> {
    let repo = get_state_field!(lazy_state, knowledge_repository);
    let result: Result<EvidenceResponse, VenoreError> = async {
        let repo = repo?;
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let ev = KnowledgeEvidence {
            id,
            hexagon_id: request.hexagon_id,
            content: request.content,
            source_url: request.source_url,
            source_type: request.source_type,
            confidence: request.confidence,
            created_at: now,
        };
        repo.create_evidence(&ev).await?;
        Ok(EvidenceResponse::from(ev))
    }.await;
    result.into_state()
}

#[tauri::command]
pub async fn list_knowledge_evidence(
    lazy_state: tauri::State<'_, LazyAppState>,
    hexagon_id: String,
) -> StateCommandResult<Vec<EvidenceResponse>> {
    let repo = get_state_field!(lazy_state, knowledge_repository);
    let result: Result<Vec<EvidenceResponse>, VenoreError> = async {
        let repo = repo?;
        let evidence = repo.list_evidence_by_hexagon(&hexagon_id).await?;
        Ok(evidence.into_iter().map(EvidenceResponse::from).collect())
    }.await;
    result.into_state()
}

#[tauri::command]
pub async fn delete_knowledge_evidence(
    lazy_state: tauri::State<'_, LazyAppState>,
    id: String,
) -> StateCommandResult<()> {
    let repo = get_state_field!(lazy_state, knowledge_repository);
    let result: Result<(), VenoreError> = async {
        let repo = repo?;
        repo.delete_evidence(&id).await?;
        Ok(())
    }.await;
    result.into_state()
}
