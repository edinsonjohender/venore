//! DTOs for Ocean Canvas commands

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// =============================================================================
// Requests
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeOceanLayoutRequest {
    pub project_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveOceanNodeRequest {
    pub project_path: String,
    pub node_id: String,
    pub target_col: i32,
    pub target_row: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveOceanCameraRequest {
    pub project_path: String,
    pub x: f64,
    pub z: f64,
    pub zoom: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateKnowledgeNodeRequest {
    pub project_path: String,
    pub name: String,
    pub col: i32,
    pub row: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateLighthouseRequest {
    pub project_path: String,
    pub name: String,
    pub col: i32,
    pub row: i32,
}

/// Lighthouse creation reuses the same accepted/rejected shape as knowledge nodes.
pub type CreateLighthouseResponse = CreateKnowledgeNodeResponse;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteOceanNodeRequest {
    pub project_path: String,
    pub node_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameOceanNodeRequest {
    pub project_path: String,
    pub node_id: String,
    pub new_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OceanNodeMutationResponse {
    pub ok: bool,
    pub node_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetNodeLighthouseRequest {
    pub project_path: String,
    pub node_id: String,
    /// `None` to detach the node from its current lighthouse cluster.
    pub lighthouse_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetNodeLighthouseResponse {
    pub accepted: bool,
    pub node_id: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighthouseClusterRequest {
    pub project_path: String,
    pub lighthouse_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighthouseClusterResponse {
    pub ok: bool,
    pub lighthouse_id: String,
    /// Number of nodes affected (1 for dissolve = the lighthouse itself; for
    /// delete_cluster it includes the lighthouse + every child that was removed).
    pub affected_nodes: u32,
}

// =============================================================================
// Knowledge node content layer
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SourceAttributionDto {
    User,
    Ai { model: String, timestamp: i64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSectionDto {
    pub id: String,
    pub name: String,
    pub content_markdown: String,
    pub source: SourceAttributionDto,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeNodeDataResponse {
    pub node_id: String,
    /// "concept" | "feature" | "decision" | "finding" | "question"
    pub subtype: String,
    pub sections: Vec<NodeSectionDto>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetKnowledgeNodeRequest {
    pub project_path: String,
    pub node_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateNodeSubtypeRequest {
    pub project_path: String,
    pub node_id: String,
    /// One of: "concept" | "feature" | "decision" | "finding" | "question"
    pub subtype: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddNodeSectionRequest {
    pub project_path: String,
    pub node_id: String,
    pub name: String,
    pub content_markdown: String,
    pub source: SourceAttributionDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddNodeSectionResponse {
    pub ok: bool,
    pub section: Option<NodeSectionDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateNodeSectionRequest {
    pub project_path: String,
    pub node_id: String,
    pub section_id: String,
    pub name: Option<String>,
    pub content_markdown: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteNodeSectionRequest {
    pub project_path: String,
    pub node_id: String,
    pub section_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReorderNodeSectionsRequest {
    pub project_path: String,
    pub node_id: String,
    pub ordered_section_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromoteToLighthouseRequest {
    pub project_path: String,
    pub node_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromoteToLighthouseResponse {
    pub accepted: bool,
    pub node_id: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractSectionToNodeRequest {
    pub project_path: String,
    pub source_node_id: String,
    pub section_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractSectionToNodeResponse {
    pub accepted: bool,
    pub new_node_id: String,
    pub col: i32,
    pub row: i32,
    pub name: String,
    pub reason: Option<String>,
}

/// Generic mutation response for boolean ops (update_*, delete_*).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeFieldMutationResponse {
    pub ok: bool,
}

// =============================================================================
// Responses
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OceanConnectionDto {
    pub id: String,
    pub from_id: String,
    pub to_id: String,
    /// "dependency" (derived from code analysis) or "manual" (user-drawn).
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOceanConnectionRequest {
    pub project_path: String,
    pub from_id: String,
    pub to_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOceanConnectionResponse {
    pub accepted: bool,
    pub connection: Option<OceanConnectionDto>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteOceanConnectionRequest {
    pub project_path: String,
    pub connection_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OceanLayoutResponse {
    pub nodes: Vec<NodePosition>,
    pub connections: Vec<OceanConnectionDto>,
    pub camera: Option<CameraStateDto>,
    /// Per-lighthouse color overrides (`#RRGGBB`). Lighthouses without an
    /// entry fall back to the deterministic palette on the frontend.
    pub lighthouse_colors: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetLighthouseColorRequest {
    pub project_path: String,
    pub lighthouse_id: String,
    /// `#RRGGBB`. Pass `None` to clear the override.
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetLighthouseColorResponse {
    pub accepted: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePosition {
    pub module_id: String,
    pub module_name: String,
    pub module_path: String,
    pub col: i32,
    pub row: i32,
    pub user_placed: bool,
    pub layers: Vec<NodeLayerDto>,
    pub node_status: String,
    /// "module", "knowledge_node" or "lighthouse" — drives node-kind-specific UI
    pub node_variant: String,
    /// Lighthouse this node is clustered under, if any
    pub lighthouse_id: Option<String>,
    /// Number of logbook sections for knowledge nodes / lighthouses.
    /// Drives the stack height in the 3D ocean. 0 for module nodes.
    pub section_count: u32,
    /// Subtype string for knowledge nodes / lighthouses (concept, feature,
    /// decision, finding, question). None for module variants.
    pub subtype: Option<String>,
    /// Active runtime states emitted by registered scanners (e.g. overflow).
    /// Always present; empty when the rover hasn't visited the node yet or
    /// no scanner flagged anything. Updated incrementally via the
    /// `ocean-state-changed` event so the frontend doesn't refetch.
    #[serde(default)]
    pub states: Vec<NodeStateDto>,
}

/// Mirror of `venore_core::ocean::NodeStateInstance` for the frontend.
/// `kind` is the snake_case string id of `NodeStateKind`; `severity` is one
/// of `info` / `warning` / `severe`; `payload` is a free-form bag the
/// scanner shipped (e.g. `{ score, sections, total_chars }` for overflow).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStateDto {
    pub kind: String,
    pub severity: String,
    pub computed_at: i64,
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeLayerDto {
    #[serde(rename = "type")]
    pub layer_type: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraStateDto {
    pub x: f64,
    pub z: f64,
    pub zoom: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeLayerUpdate {
    pub module_id: String,
    pub layers: Vec<NodeLayerDto>,
    pub node_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveOceanNodeResponse {
    pub accepted: bool,
    pub node_id: String,
    pub col: i32,
    pub row: i32,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveOceanNodesEntry {
    pub node_id: String,
    pub target_col: i32,
    pub target_row: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveOceanNodesRequest {
    pub project_path: String,
    pub moves: Vec<MoveOceanNodesEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveOceanNodesResponse {
    pub all_accepted: bool,
    pub results: Vec<MoveOceanNodeResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateKnowledgeNodeResponse {
    pub accepted: bool,
    pub node_id: String,
    pub col: i32,
    pub row: i32,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetModuleDetailsRequest {
    pub project_path: String,
    pub module_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDetailsResponse {
    pub name: String,
    pub path: String,
    pub file_count: usize,
    pub entry_point: Option<String>,
    pub files: Vec<String>,
    pub dependencies: Vec<String>,
    pub dependents: Vec<String>,
    pub external_deps: Vec<String>,
    pub exports: Vec<SymbolInfoDto>,
    pub layers: Vec<NodeLayerDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInfoDto {
    pub name: String,
    pub kind: String,
    pub file: String,
}

// =============================================================================
// Node states + rover status
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetNodeStatesRequest {
    pub project_path: String,
    pub node_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetNodeStatesResponse {
    pub node_id: String,
    pub states: Vec<NodeStateDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DismissNodeStateRequest {
    pub project_path: String,
    pub node_id: String,
    /// Stable kind id (e.g. "overflow"). Must match `NodeStateKind::as_str()`.
    pub kind: String,
    /// Unix-epoch seconds. Pass `0` (or any non-positive value) to clear the
    /// dismissal entirely.
    pub until_ts: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridCellDto {
    pub col: i32,
    pub row: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetRoverStatusRequest {
    pub project_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoverStatusResponse {
    pub current_cell: Option<GridCellDto>,
    pub target_cell: Option<GridCellDto>,
    pub queue_depth: u32,
    pub idle: bool,
    pub last_step_at: i64,
    /// `false` if no rover is registered for this project (e.g. the canvas
    /// hasn't been opened yet). Other fields are zeroed in that case.
    pub running: bool,
}
