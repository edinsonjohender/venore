//! Knowledge DTOs for Tauri IPC

use serde::{Deserialize, Serialize};
use venore_core::knowledge::{KnowledgeFeature, KnowledgeHexagon, KnowledgeEvidence};

// =========================================================================
// Feature DTOs
// =========================================================================

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateFeatureRequest {
    pub project_id: String,
    pub name: String,
    pub description: String,
    pub objective: Option<String>,
    pub intensity: Option<String>,
    pub max_hexagons_per_phase: Option<i32>,
    pub auto_advance: Option<bool>,
    pub tags: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFeatureRequest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub objective: Option<String>,
    pub intensity: Option<String>,
    pub max_hexagons_per_phase: Option<i32>,
    pub auto_advance: Option<bool>,
    pub tags: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureResponse {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub objective: String,
    pub intensity: String,
    pub max_hexagons_per_phase: i32,
    pub auto_advance: bool,
    pub tags: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<KnowledgeFeature> for FeatureResponse {
    fn from(f: KnowledgeFeature) -> Self {
        Self {
            id: f.id,
            project_id: f.project_id,
            name: f.name,
            description: f.description,
            status: f.status,
            priority: f.priority,
            objective: f.objective,
            intensity: f.intensity,
            max_hexagons_per_phase: f.max_hexagons_per_phase,
            auto_advance: f.auto_advance,
            tags: f.tags,
            created_at: f.created_at,
            updated_at: f.updated_at,
        }
    }
}

// =========================================================================
// Hexagon DTOs
// =========================================================================

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateHexagonRequest {
    pub feature_id: String,
    pub title: String,
    pub description: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateHexagonRequest {
    pub id: String,
    pub title: String,
    pub description: String,
    pub phase: String,
    pub percentage: i32,
    pub confidence: String,
    pub risk: String,
    pub priority: String,
    pub is_dead_end: bool,
    pub blocked_by: String,
    pub notes_user: String,
    pub agent_status: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HexagonResponse {
    pub id: String,
    pub feature_id: String,
    pub title: String,
    pub description: String,
    pub phase: String,
    pub percentage: i32,
    pub confidence: String,
    pub risk: String,
    pub priority: String,
    pub is_dead_end: bool,
    pub blocked_by: String,
    pub notes_user: String,
    pub agent_status: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<KnowledgeHexagon> for HexagonResponse {
    fn from(h: KnowledgeHexagon) -> Self {
        Self {
            id: h.id,
            feature_id: h.feature_id,
            title: h.title,
            description: h.description,
            phase: h.phase,
            percentage: h.percentage,
            confidence: h.confidence,
            risk: h.risk,
            priority: h.priority,
            is_dead_end: h.is_dead_end,
            blocked_by: h.blocked_by,
            notes_user: h.notes_user,
            agent_status: h.agent_status,
            created_at: h.created_at,
            updated_at: h.updated_at,
        }
    }
}

// =========================================================================
// Evidence DTOs
// =========================================================================

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateEvidenceRequest {
    pub hexagon_id: String,
    pub content: String,
    pub source_url: String,
    pub source_type: String,
    pub confidence: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceResponse {
    pub id: String,
    pub hexagon_id: String,
    pub content: String,
    pub source_url: String,
    pub source_type: String,
    pub confidence: String,
    pub created_at: String,
}

impl From<KnowledgeEvidence> for EvidenceResponse {
    fn from(e: KnowledgeEvidence) -> Self {
        Self {
            id: e.id,
            hexagon_id: e.hexagon_id,
            content: e.content,
            source_url: e.source_url,
            source_type: e.source_type,
            confidence: e.confidence,
            created_at: e.created_at,
        }
    }
}
