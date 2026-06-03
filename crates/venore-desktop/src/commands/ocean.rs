//! Tauri commands for Ocean Canvas layout.
//!
//! Thin wrappers that delegate to venore-core's OceanLayoutService.
//! Module data is pulled from WizardSessionManager cache — the frontend
//! only sends project_path.

use tracing::{debug, info, warn};

use std::path::Path;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::utils::CommandResult;
use std::collections::HashSet;

use super::dto::ocean::{
    AddNodeSectionRequest, AddNodeSectionResponse, CameraStateDto, CreateKnowledgeNodeRequest,
    CreateKnowledgeNodeResponse, CreateLighthouseRequest, CreateLighthouseResponse,
    CreateOceanConnectionRequest, CreateOceanConnectionResponse, DeleteNodeSectionRequest,
    DeleteOceanConnectionRequest, DeleteOceanNodeRequest, DismissNodeStateRequest,
    ExtractSectionToNodeRequest, ExtractSectionToNodeResponse, GetKnowledgeNodeRequest,
    GetModuleDetailsRequest, GetNodeStatesRequest, GetNodeStatesResponse, GetRoverStatusRequest,
    GridCellDto, InitializeOceanLayoutRequest, KnowledgeFieldMutationResponse,
    KnowledgeNodeDataResponse, LighthouseClusterRequest, LighthouseClusterResponse,
    ModuleDetailsResponse, MoveOceanNodeRequest, MoveOceanNodeResponse, MoveOceanNodesRequest,
    MoveOceanNodesResponse, NodeLayerDto, NodeLayerUpdate, NodePosition, NodeSectionDto,
    NodeStateDto, OceanConnectionDto, OceanLayoutResponse, OceanNodeMutationResponse,
    PromoteToLighthouseRequest, PromoteToLighthouseResponse, RenameOceanNodeRequest,
    ReorderNodeSectionsRequest, RoverStatusResponse, SaveOceanCameraRequest,
    SetLighthouseColorRequest, SetLighthouseColorResponse, SetNodeLighthouseRequest,
    SetNodeLighthouseResponse, SourceAttributionDto, SymbolInfoDto, UpdateNodeSectionRequest,
    UpdateNodeSubtypeRequest,
};
use venore_core::analysis::AnalysisOutput;
use venore_core::error::VenoreError;
use venore_core::layers::{self, LayerStatus, ModuleConnectionInfo, ModuleLayerAnalysis};
use venore_core::ocean::{
    current_snapshot, CameraState, GridCell, KnowledgeNodeSubtype,
    ModuleInfo, MoveResult, NodeStateInstance, NodeStateKind, NodeVariant,
    SourceAttribution,
};
use venore_core::wizard::WizardSessionManager;

// =============================================================================
// Node-state DTO mapping
// =============================================================================
//
// Node-state scanning is no longer a standalone rover: it runs as the
// `StateCurrent` inside the Currents engine (spawned in `initialize_ocean_layout`
// alongside the other currents). The bridge that forwards its
// `ocean-state-changed` events lives in `commands::currents`. This module only
// maps node states into the layout DTO and exposes `get_rover_status` (now
// backed by the StateCurrent's snapshot).

/// Convert a core state instance to its DTO. Used both by the layout mapper
/// (initial fetch) and by the explicit `get_node_states` command.
fn state_instance_to_dto(state: &NodeStateInstance) -> NodeStateDto {
    NodeStateDto {
        kind: state.kind.as_str().to_string(),
        severity: severity_to_str(state.severity).to_string(),
        computed_at: state.computed_at,
        payload: state.payload.clone(),
    }
}

fn severity_to_str(severity: venore_core::ocean::Severity) -> &'static str {
    use venore_core::ocean::Severity::*;
    match severity {
        Info => "info",
        Warning => "warning",
        Severe => "severe",
    }
}

fn state_kind_from_str(s: &str) -> Option<NodeStateKind> {
    match s {
        "overflow" => Some(NodeStateKind::Overflow),
        _ => None,
    }
}

// =============================================================================
// Commands
// =============================================================================

/// Initialize ocean layout for a project.
///
/// Pulls module data (with dependencies) from the WizardSession cache.
/// If no cache exists, tries to restore layout from disk.
/// If nothing is available, returns an empty layout.
///
/// Side effects: spawns (idempotently) the project's rover scanner so node
/// states (overflow, etc.) start being computed in the background. Events
/// flow back to the frontend on `ocean-rover-progress` and `ocean-state-changed`.
#[tauri::command]
pub async fn initialize_ocean_layout(
    app: AppHandle,
    request: InitializeOceanLayoutRequest,
) -> CommandResult<OceanLayoutResponse> {
    info!(project_path = %request.project_path, "Initializing ocean layout");

    let modules = extract_modules_from_session(&request.project_path);
    debug!(modules_count = modules.len(), "Extracted modules from session");

    // Spawn the Currents runner. `default_currents()` includes the StateCurrent
    // (node-state halos/badges — the old rover, now folded in), the Index
    // Current (logbook search index) and the Staleness Current (code-drift
    // badges). Each sweeps the Ocean node-by-node and self-gates per node.
    let currents_sender = crate::commands::currents::ensure_currents_bridge(&app);
    venore_core::ocean::ensure_currents_started(
        &request.project_path,
        venore_core::ocean::default_currents(),
        currents_sender,
    );

    // Snapshot the current node_states map (in-memory only) so we can ship
    // any already-computed states alongside the freshly-loaded layout. The
    // StateCurrent may not have walked everything yet at this point — empty
    // states[] is fine for unscanned nodes; updates trickle in via events.
    let node_states_snapshot = venore_core::ocean::service::with_service(
        &request.project_path,
        |service| service.all_node_states().clone(),
    ).unwrap_or_default();

    let result: Result<OceanLayoutResponse, VenoreError> = (|| {
        let layout = venore_core::ocean::service::with_service(&request.project_path, |service| {
            service.initialize(&modules)
        })?;

        // Return nodes with empty layers — layer analysis runs separately via compute_ocean_layers.
        // Knowledge nodes don't have layers (no code to analyze) so they get "fresh" immediately;
        // their stack height is driven by section_count instead.
        let nodes: Vec<NodePosition> = layout
            .positions
            .values()
            .map(|entry| {
                // Buoy / Cylinder behave like Module for status + content
                // semantics — they're code-representational stubs awaiting
                // analysis, not knowledge containers.
                let node_status = match entry.node_variant {
                    NodeVariant::Module | NodeVariant::Buoy | NodeVariant::Cylinder => "loading",
                    NodeVariant::KnowledgeNode | NodeVariant::Lighthouse => "fresh",
                };
                let section_count = match entry.node_variant {
                    NodeVariant::Module | NodeVariant::Buoy | NodeVariant::Cylinder => 0,
                    NodeVariant::KnowledgeNode | NodeVariant::Lighthouse => layout
                        .knowledge_data
                        .get(&entry.module_id)
                        .map(|d| d.sections.len() as u32)
                        .unwrap_or(0),
                };
                let subtype = match entry.node_variant {
                    NodeVariant::Module | NodeVariant::Buoy | NodeVariant::Cylinder => None,
                    NodeVariant::KnowledgeNode | NodeVariant::Lighthouse => layout
                        .knowledge_data
                        .get(&entry.module_id)
                        .map(|d| subtype_to_str(d.subtype).to_string()),
                };
                let states = node_states_snapshot
                    .get(&entry.module_id)
                    .map(|v| v.iter().map(state_instance_to_dto).collect())
                    .unwrap_or_default();
                NodePosition {
                    module_id: entry.module_id.clone(),
                    module_name: entry.module_name.clone(),
                    module_path: entry.module_path.clone(),
                    col: entry.cell.col,
                    row: entry.cell.row,
                    user_placed: entry.user_placed,
                    layers: Vec::new(),
                    node_status: node_status.to_string(),
                    node_variant: node_variant_to_str(entry.node_variant).to_string(),
                    lighthouse_id: entry.lighthouse_id.clone(),
                    section_count,
                    subtype,
                    states,
                }
            })
            .collect();

        let camera = layout.camera.map(|c| CameraStateDto {
            x: c.x,
            z: c.z,
            zoom: c.zoom,
        });

        let node_ids: HashSet<String> = nodes.iter().map(|n| n.module_id.clone()).collect();
        let mut connections = build_ocean_connections(&modules, &node_ids);
        // Append user-drawn connections, filtering out any with missing endpoints.
        for conn in &layout.manual_connections {
            if !node_ids.contains(&conn.from_id) || !node_ids.contains(&conn.to_id) {
                continue;
            }
            connections.push(OceanConnectionDto {
                id: conn.id.clone(),
                from_id: conn.from_id.clone(),
                to_id: conn.to_id.clone(),
                kind: "manual".to_string(),
            });
        }

        info!(nodes_count = nodes.len(), connections_count = connections.len(), "Ocean layout initialized");
        Ok(OceanLayoutResponse {
            nodes,
            connections,
            camera,
            lighthouse_colors: layout.lighthouse_colors,
        })
    })();

    result.into()
}

/// Compute layer analysis for all ocean nodes.
///
/// Runs OUTSIDE any mutex lock — safe to call after initialize_ocean_layout
/// without blocking the UI. Returns per-node layer updates that the frontend
/// merges into its existing nodes state.
///
/// **Source of truth**: if the wizard already persisted layers to the
/// `module_layers` table (via `wizard_index_project`), we read those. The
/// wizard has richer info (real `rag_module_deps` for the connections layer,
/// full module list, etc.) and avoids the canvas recomputing from a sparse
/// in-memory analysis cache. We only recompute as a fallback when nothing is
/// persisted yet (e.g. a project opened without running the wizard).
#[tauri::command]
pub async fn compute_ocean_layers(
    lazy_state: tauri::State<'_, crate::state::LazyAppState>,
    request: InitializeOceanLayoutRequest,
) -> crate::utils::StateCommandResult<Vec<NodeLayerUpdate>> {
    use crate::utils::IntoStateCommandResult;

    info!(project_path = %request.project_path, "Computing ocean layers (background)");

    // 1. Resolve project identity (best-effort — fall back to recompute if missing).
    let project_path = Path::new(&request.project_path);
    let project_id = venore_core::project::ProjectService::read_or_create_identity(project_path)
        .ok()
        .map(|i| i.id.to_string());

    // 2. Try persisted layers from DB.
    if let Some(pid) = project_id.as_deref() {
        let ctx_repo = {
            let guard = lazy_state.get();
            guard.as_ref().map(|s| std::sync::Arc::clone(&s.context_repository))
        };
        if let Some(repo) = ctx_repo {
            // File-first: portable `.venore/module-layers.json` wins over the
            // DB. If only the DB has data (project predates portable layers),
            // the call below also silently migrates it to the file so
            // subsequent reads stay fast and the snapshot travels with the
            // repo from now on.
            match venore_core::context::file_storage::load_layers_file_first(
                project_path, pid, &repo,
            ).await {
                Ok(rows) if !rows.is_empty() => {
                    let updates = layers_db_rows_to_updates(rows);
                    info!(updates_count = updates.len(), "Ocean layers loaded (file-first)");
                    return Ok::<Vec<NodeLayerUpdate>, VenoreError>(updates).into_state();
                }
                Ok(_) => debug!(project_id = pid, "No persisted layers — falling back to recompute"),
                Err(e) => warn!(error = %e, "Failed to read persisted layers — falling back to recompute"),
            }
        }
    }

    // 3. Fallback: recompute from analysis cache.
    let modules = extract_modules_from_session(&request.project_path);
    let layers_config = get_layers_to_generate(&request.project_path);
    let connection_map = build_connection_map(&modules);

    let updates: Vec<NodeLayerUpdate> = modules
        .iter()
        .map(|m| {
            let conn_info = connection_map.get(&m.name);
            let analysis = layers::analyze_module_layers(
                project_path,
                &m.path,
                conn_info,
                &layers_config,
            );
            let layers = convert_layer_analysis(&analysis);
            let node_status = compute_node_status(&analysis);
            NodeLayerUpdate {
                module_id: m.id.clone(),
                layers,
                node_status,
            }
        })
        .collect();

    info!(updates_count = updates.len(), "Ocean layers recomputed (fallback)");
    Ok::<Vec<NodeLayerUpdate>, VenoreError>(updates).into_state()
}

/// Group DB layer rows by module and produce one NodeLayerUpdate per module.
/// Missing layers are filtered out (canvas just renders a shorter node).
/// The node_status comes from the context layer's `freshness` detail field
/// when present — same semantics as `compute_node_status`.
fn layers_db_rows_to_updates(
    rows: Vec<venore_core::context::repository::ModuleLayerRecord>,
) -> Vec<NodeLayerUpdate> {
    use std::collections::HashMap;
    let mut by_module: HashMap<String, Vec<venore_core::context::repository::ModuleLayerRecord>> = HashMap::new();
    for r in rows {
        by_module.entry(r.module_name.clone()).or_default().push(r);
    }

    by_module
        .into_iter()
        .map(|(module_name, mod_rows)| {
            let mut layers: Vec<NodeLayerDto> = Vec::new();
            let mut has_complete = false;
            let mut has_partial = false;
            let mut has_missing = false;

            for r in &mod_rows {
                match r.status.as_str() {
                    "complete" => has_complete = true,
                    "partial" => has_partial = true,
                    "missing" => {
                        has_missing = true;
                        continue; // not surfaced as a layer in the UI
                    }
                    _ => {}
                }
                let details = serde_json::from_str::<std::collections::HashMap<String, serde_json::Value>>(&r.details_json).ok();
                layers.push(NodeLayerDto {
                    layer_type: r.layer_type.clone(),
                    status: r.status.clone(),
                    details: details.filter(|d| !d.is_empty()),
                });
            }

            let node_status = if has_complete && !has_partial && !has_missing {
                "fresh".to_string()
            } else if has_complete || has_partial {
                "stale".to_string()
            } else {
                "missing".to_string()
            };

            NodeLayerUpdate {
                module_id: module_name,
                layers,
                node_status,
            }
        })
        .collect()
}

/// Move a node to a target grid cell.
/// Returns accepted/rejected with reason.
#[tauri::command]
pub async fn move_ocean_node(
    request: MoveOceanNodeRequest,
) -> CommandResult<MoveOceanNodeResponse> {
    debug!(node_id = %request.node_id, col = request.target_col, row = request.target_row, "Move ocean node");

    let target = GridCell::new(request.target_col, request.target_row);

    let result: Result<MoveOceanNodeResponse, VenoreError> = (|| {
        let move_result = venore_core::ocean::service::with_service(&request.project_path, |service| {
            service.move_node(&request.node_id, target)
        })?;

        match move_result {
            MoveResult::Accepted { node_id, cell } => Ok(MoveOceanNodeResponse {
                accepted: true,
                node_id,
                col: cell.col,
                row: cell.row,
                reason: None,
            }),
            MoveResult::Rejected { node_id, reason } => Ok(MoveOceanNodeResponse {
                accepted: false,
                node_id,
                col: request.target_col,
                row: request.target_row,
                reason: Some(reason),
            }),
        }
    })();

    result.into()
}

/// Move multiple nodes atomically (group move). Either every move is
/// accepted or none — never leaves the layout half-applied.
#[tauri::command]
pub async fn move_ocean_nodes(
    request: MoveOceanNodesRequest,
) -> CommandResult<MoveOceanNodesResponse> {
    debug!(project_path = %request.project_path, count = request.moves.len(), "Move ocean nodes (atomic)");

    let result: Result<MoveOceanNodesResponse, VenoreError> = (|| {
        let moves: Vec<(String, GridCell)> = request
            .moves
            .iter()
            .map(|m| (m.node_id.clone(), GridCell::new(m.target_col, m.target_row)))
            .collect();

        let outcomes = venore_core::ocean::service::with_service(&request.project_path, |service| {
            service.move_nodes_atomic(moves)
        })?;

        let mut all_accepted = true;
        let results: Vec<MoveOceanNodeResponse> = outcomes
            .into_iter()
            .zip(request.moves.iter())
            .map(|(outcome, requested)| match outcome {
                MoveResult::Accepted { node_id, cell } => MoveOceanNodeResponse {
                    accepted: true,
                    node_id,
                    col: cell.col,
                    row: cell.row,
                    reason: None,
                },
                MoveResult::Rejected { node_id, reason } => {
                    all_accepted = false;
                    MoveOceanNodeResponse {
                        accepted: false,
                        node_id,
                        col: requested.target_col,
                        row: requested.target_row,
                        reason: Some(reason),
                    }
                }
            })
            .collect();

        Ok(MoveOceanNodesResponse {
            all_accepted,
            results,
        })
    })();

    result.into()
}

/// Create a new user-curated knowledge node at a target grid cell.
/// Returns accepted with the generated UUID, or rejected with reason.
#[tauri::command]
pub async fn create_knowledge_node(
    request: CreateKnowledgeNodeRequest,
) -> CommandResult<CreateKnowledgeNodeResponse> {
    debug!(project_path = %request.project_path, name = %request.name, col = request.col, row = request.row, "Create knowledge node");

    let target = GridCell::new(request.col, request.row);

    let result: Result<CreateKnowledgeNodeResponse, VenoreError> = (|| {
        let move_result = venore_core::ocean::service::with_service(&request.project_path, |service| {
            service.create_knowledge_node(request.name.clone(), target)
        })?;

        match move_result {
            MoveResult::Accepted { node_id, cell } => Ok(CreateKnowledgeNodeResponse {
                accepted: true,
                node_id,
                col: cell.col,
                row: cell.row,
                reason: None,
            }),
            MoveResult::Rejected { node_id, reason } => Ok(CreateKnowledgeNodeResponse {
                accepted: false,
                node_id,
                col: request.col,
                row: request.row,
                reason: Some(reason),
            }),
        }
    })();

    result.into()
}

/// Create a new lighthouse (anchor of an island) at a target grid cell.
/// Returns accepted with the generated UUID, or rejected with reason.
#[tauri::command]
pub async fn create_lighthouse(
    request: CreateLighthouseRequest,
) -> CommandResult<CreateLighthouseResponse> {
    debug!(project_path = %request.project_path, name = %request.name, col = request.col, row = request.row, "Create lighthouse");

    let target = GridCell::new(request.col, request.row);

    let result: Result<CreateLighthouseResponse, VenoreError> = (|| {
        let move_result = venore_core::ocean::service::with_service(&request.project_path, |service| {
            service.create_lighthouse(request.name.clone(), target)
        })?;

        match move_result {
            MoveResult::Accepted { node_id, cell } => Ok(CreateKnowledgeNodeResponse {
                accepted: true,
                node_id,
                col: cell.col,
                row: cell.row,
                reason: None,
            }),
            MoveResult::Rejected { node_id, reason } => Ok(CreateKnowledgeNodeResponse {
                accepted: false,
                node_id,
                col: request.col,
                row: request.row,
                reason: Some(reason),
            }),
        }
    })();

    result.into()
}

/// Delete a node from the ocean layout.
#[tauri::command]
pub async fn delete_ocean_node(
    request: DeleteOceanNodeRequest,
) -> CommandResult<OceanNodeMutationResponse> {
    debug!(project_path = %request.project_path, node_id = %request.node_id, "Delete ocean node");

    let result: Result<OceanNodeMutationResponse, VenoreError> = (|| {
        let ok = venore_core::ocean::service::with_service(&request.project_path, |service| {
            service.delete_node(&request.node_id)
        })?;

        Ok(OceanNodeMutationResponse {
            ok,
            node_id: request.node_id.clone(),
        })
    })();

    result.into()
}

/// Rename a node in the ocean layout.
#[tauri::command]
pub async fn rename_ocean_node(
    request: RenameOceanNodeRequest,
) -> CommandResult<OceanNodeMutationResponse> {
    debug!(project_path = %request.project_path, node_id = %request.node_id, new_name = %request.new_name, "Rename ocean node");

    let result: Result<OceanNodeMutationResponse, VenoreError> = (|| {
        let ok = venore_core::ocean::service::with_service(&request.project_path, |service| {
            service.rename_node(&request.node_id, request.new_name.clone())
        })?;

        Ok(OceanNodeMutationResponse {
            ok,
            node_id: request.node_id.clone(),
        })
    })();

    result.into()
}

/// Assign a node to a lighthouse cluster (or detach with `lighthouse_id = null`).
#[tauri::command]
pub async fn set_node_lighthouse(
    request: SetNodeLighthouseRequest,
) -> CommandResult<SetNodeLighthouseResponse> {
    debug!(project_path = %request.project_path, node_id = %request.node_id, ?request.lighthouse_id, "Set node lighthouse");

    let result: Result<SetNodeLighthouseResponse, VenoreError> = (|| {
        let outcome = venore_core::ocean::service::with_service(&request.project_path, |service| {
            service.set_node_lighthouse(&request.node_id, request.lighthouse_id.clone())
        })?;

        match outcome {
            Ok(()) => Ok(SetNodeLighthouseResponse {
                accepted: true,
                node_id: request.node_id.clone(),
                reason: None,
            }),
            Err(reason) => Ok(SetNodeLighthouseResponse {
                accepted: false,
                node_id: request.node_id.clone(),
                reason: Some(reason),
            }),
        }
    })();

    result.into()
}

/// Dissolve a lighthouse cluster: demote the lighthouse to a regular knowledge
/// node and detach all its children. Nothing is deleted.
#[tauri::command]
pub async fn dissolve_lighthouse(
    request: LighthouseClusterRequest,
) -> CommandResult<LighthouseClusterResponse> {
    debug!(project_path = %request.project_path, lighthouse_id = %request.lighthouse_id, "Dissolve lighthouse");

    let result: Result<LighthouseClusterResponse, VenoreError> = (|| {
        let ok = venore_core::ocean::service::with_service(&request.project_path, |service| {
            service.dissolve_lighthouse(&request.lighthouse_id)
        })?;

        Ok(LighthouseClusterResponse {
            ok,
            lighthouse_id: request.lighthouse_id.clone(),
            affected_nodes: if ok { 1 } else { 0 },
        })
    })();

    result.into()
}

/// Delete a lighthouse and every node whose lighthouse_id points to it.
#[tauri::command]
pub async fn delete_lighthouse_cluster(
    request: LighthouseClusterRequest,
) -> CommandResult<LighthouseClusterResponse> {
    debug!(project_path = %request.project_path, lighthouse_id = %request.lighthouse_id, "Delete lighthouse cluster");

    let result: Result<LighthouseClusterResponse, VenoreError> = (|| {
        let count = venore_core::ocean::service::with_service(&request.project_path, |service| {
            service.delete_lighthouse_cluster(&request.lighthouse_id)
        })?;

        Ok(LighthouseClusterResponse {
            ok: count > 0,
            lighthouse_id: request.lighthouse_id.clone(),
            affected_nodes: count,
        })
    })();

    result.into()
}

// =============================================================================
// Knowledge node content layer — read + mutate
// =============================================================================

/// Payload broadcast whenever a node's logbook content changes. Listened by
/// every NodeLogbook instance (in-app and pop-out) and by OceanNodes to keep
/// section_count visuals in sync.
#[derive(Debug, Clone, Serialize)]
struct LogbookChangedPayload {
    project_path: String,
    node_id: String,
}

const LOGBOOK_CHANGED_EVENT: &str = "ocean-knowledge-changed";

fn emit_logbook_changed(app: &AppHandle, project_path: &str, node_id: &str) {
    let _ = app.emit(
        LOGBOOK_CHANGED_EVENT,
        LogbookChangedPayload {
            project_path: project_path.to_string(),
            node_id: node_id.to_string(),
        },
    );
}

#[derive(Debug, Clone, Serialize)]
struct ConnectionsChangedPayload {
    project_path: String,
}

const CONNECTIONS_CHANGED_EVENT: &str = "ocean-connections-changed";

fn emit_connections_changed(app: &AppHandle, project_path: &str) {
    let _ = app.emit(
        CONNECTIONS_CHANGED_EVENT,
        ConnectionsChangedPayload {
            project_path: project_path.to_string(),
        },
    );
}

/// Read the full content layer for a knowledge node (or lighthouse).
/// Returns 404-style error if the node has no content data yet.
#[tauri::command]
pub async fn get_knowledge_node(
    request: GetKnowledgeNodeRequest,
) -> CommandResult<KnowledgeNodeDataResponse> {
    debug!(project_path = %request.project_path, node_id = %request.node_id, "Get knowledge node");

    let result: Result<KnowledgeNodeDataResponse, VenoreError> = (|| {
        let data = venore_core::ocean::service::with_service(&request.project_path, |service| {
            service.get_knowledge_data(&request.node_id)
        })?;

        let data = data.ok_or_else(|| {
            VenoreError::NotFound(format!("Knowledge node '{}' has no content layer yet", request.node_id))
        })?;

        Ok(KnowledgeNodeDataResponse {
            node_id: request.node_id.clone(),
            subtype: subtype_to_str(data.subtype).to_string(),
            sections: data
                .sections
                .into_iter()
                .map(|s| NodeSectionDto {
                    id: s.id,
                    name: s.name,
                    content_markdown: s.content_markdown,
                    source: source_to_dto(s.source),
                    created_at: s.created_at,
                    updated_at: s.updated_at,
                })
                .collect(),
            created_at: data.created_at,
            updated_at: data.updated_at,
        })
    })();

    result.into()
}

#[tauri::command]
pub async fn update_node_subtype(
    app: AppHandle,
    request: UpdateNodeSubtypeRequest,
) -> CommandResult<KnowledgeFieldMutationResponse> {
    debug!(node_id = %request.node_id, subtype = %request.subtype, "Update node subtype");
    let result: Result<KnowledgeFieldMutationResponse, VenoreError> = (|| {
        let subtype = subtype_from_str(&request.subtype)
            .ok_or_else(|| VenoreError::InvalidParams(format!("Unknown subtype '{}'", request.subtype)))?;
        let ok = venore_core::ocean::service::with_service(&request.project_path, |service| {
            service.update_node_subtype(&request.node_id, subtype)
        })?;
        if ok {
            emit_logbook_changed(&app, &request.project_path, &request.node_id);
        }
        Ok(KnowledgeFieldMutationResponse { ok })
    })();
    result.into()
}

#[tauri::command]
pub async fn add_node_section(
    app: AppHandle,
    request: AddNodeSectionRequest,
) -> CommandResult<AddNodeSectionResponse> {
    debug!(node_id = %request.node_id, name = %request.name, "Add node section");
    let result: Result<AddNodeSectionResponse, VenoreError> = (|| {
        let source = source_from_dto(request.source.clone());
        let section = venore_core::ocean::service::with_service(&request.project_path, |service| {
            service.add_node_section(
                &request.node_id,
                request.name.clone(),
                request.content_markdown.clone(),
                source,
                None,
                None,
            )
        })?;
        if section.is_some() {
            emit_logbook_changed(&app, &request.project_path, &request.node_id);
        }
        Ok(AddNodeSectionResponse {
            ok: section.is_some(),
            section: section.map(|s| NodeSectionDto {
                id: s.id,
                name: s.name,
                content_markdown: s.content_markdown,
                source: source_to_dto(s.source),
                created_at: s.created_at,
                updated_at: s.updated_at,
            }),
        })
    })();
    result.into()
}

#[tauri::command]
pub async fn update_node_section(
    app: AppHandle,
    request: UpdateNodeSectionRequest,
) -> CommandResult<KnowledgeFieldMutationResponse> {
    debug!(node_id = %request.node_id, section_id = %request.section_id, "Update node section");
    let result: Result<KnowledgeFieldMutationResponse, VenoreError> = (|| {
        let ok = venore_core::ocean::service::with_service(&request.project_path, |service| {
            service.update_node_section(
                &request.node_id,
                &request.section_id,
                request.name.clone(),
                request.content_markdown.clone(),
            )
        })?;
        if ok {
            emit_logbook_changed(&app, &request.project_path, &request.node_id);
        }
        Ok(KnowledgeFieldMutationResponse { ok })
    })();
    result.into()
}

#[tauri::command]
pub async fn delete_node_section(
    app: AppHandle,
    request: DeleteNodeSectionRequest,
) -> CommandResult<KnowledgeFieldMutationResponse> {
    debug!(node_id = %request.node_id, section_id = %request.section_id, "Delete node section");
    let result: Result<KnowledgeFieldMutationResponse, VenoreError> = (|| {
        let ok = venore_core::ocean::service::with_service(&request.project_path, |service| {
            service.delete_node_section(&request.node_id, &request.section_id)
        })?;
        if ok {
            emit_logbook_changed(&app, &request.project_path, &request.node_id);
        }
        Ok(KnowledgeFieldMutationResponse { ok })
    })();
    result.into()
}

#[tauri::command]
pub async fn promote_to_lighthouse(
    app: AppHandle,
    request: PromoteToLighthouseRequest,
) -> CommandResult<PromoteToLighthouseResponse> {
    debug!(node_id = %request.node_id, "Promote node to lighthouse");
    let result: Result<PromoteToLighthouseResponse, VenoreError> = (|| {
        let outcome = venore_core::ocean::service::with_service(&request.project_path, |service| {
            service.promote_to_lighthouse(&request.node_id)
        })?;
        match outcome {
            Ok(()) => {
                // Treat the variant flip as a logbook-relevant change so the
                // ocean canvas refreshes (visual switches box-stack → pillar).
                emit_logbook_changed(&app, &request.project_path, &request.node_id);
                Ok(PromoteToLighthouseResponse {
                    accepted: true,
                    node_id: request.node_id.clone(),
                    reason: None,
                })
            }
            Err(reason) => Ok(PromoteToLighthouseResponse {
                accepted: false,
                node_id: request.node_id.clone(),
                reason: Some(reason),
            }),
        }
    })();
    result.into()
}

#[tauri::command]
pub async fn extract_section_to_node(
    app: AppHandle,
    request: ExtractSectionToNodeRequest,
) -> CommandResult<ExtractSectionToNodeResponse> {
    debug!(source = %request.source_node_id, section = %request.section_id, "Extract section to node");
    let result: Result<ExtractSectionToNodeResponse, VenoreError> = (|| {
        let outcome = venore_core::ocean::service::with_service(&request.project_path, |service| {
            service.extract_section_to_node(&request.source_node_id, &request.section_id)
        })?;

        match outcome {
            Ok((new_node_id, cell, name)) => {
                // Both sides change: source loses a section, new node appears.
                emit_logbook_changed(&app, &request.project_path, &request.source_node_id);
                emit_logbook_changed(&app, &request.project_path, &new_node_id);
                Ok(ExtractSectionToNodeResponse {
                    accepted: true,
                    new_node_id,
                    col: cell.col,
                    row: cell.row,
                    name,
                    reason: None,
                })
            }
            Err(reason) => Ok(ExtractSectionToNodeResponse {
                accepted: false,
                new_node_id: String::new(),
                col: 0,
                row: 0,
                name: String::new(),
                reason: Some(reason),
            }),
        }
    })();
    result.into()
}

#[tauri::command]
pub async fn reorder_node_sections(
    app: AppHandle,
    request: ReorderNodeSectionsRequest,
) -> CommandResult<KnowledgeFieldMutationResponse> {
    debug!(node_id = %request.node_id, count = request.ordered_section_ids.len(), "Reorder node sections");
    let result: Result<KnowledgeFieldMutationResponse, VenoreError> = (|| {
        let ok = venore_core::ocean::service::with_service(&request.project_path, |service| {
            service.reorder_node_sections(&request.node_id, request.ordered_section_ids.clone())
        })?;
        if ok {
            emit_logbook_changed(&app, &request.project_path, &request.node_id);
        }
        Ok(KnowledgeFieldMutationResponse { ok })
    })();
    result.into()
}

// =============================================================================
// Manual connections
// =============================================================================

#[tauri::command]
pub async fn create_ocean_connection(
    app: AppHandle,
    request: CreateOceanConnectionRequest,
) -> CommandResult<CreateOceanConnectionResponse> {
    debug!(from = %request.from_id, to = %request.to_id, "Create ocean connection");
    let result: Result<CreateOceanConnectionResponse, VenoreError> = (|| {
        let outcome = venore_core::ocean::service::with_service(&request.project_path, |service| {
            service.create_connection(&request.from_id, &request.to_id)
        })?;
        match outcome {
            Ok(conn) => {
                emit_connections_changed(&app, &request.project_path);
                Ok(CreateOceanConnectionResponse {
                    accepted: true,
                    connection: Some(OceanConnectionDto {
                        id: conn.id,
                        from_id: conn.from_id,
                        to_id: conn.to_id,
                        kind: "manual".to_string(),
                    }),
                    reason: None,
                })
            }
            Err(reason) => Ok(CreateOceanConnectionResponse {
                accepted: false,
                connection: None,
                reason: Some(reason),
            }),
        }
    })();
    result.into()
}

#[tauri::command]
pub async fn delete_ocean_connection(
    app: AppHandle,
    request: DeleteOceanConnectionRequest,
) -> CommandResult<KnowledgeFieldMutationResponse> {
    debug!(connection_id = %request.connection_id, "Delete ocean connection");
    let result: Result<KnowledgeFieldMutationResponse, VenoreError> = (|| {
        let ok = venore_core::ocean::service::with_service(&request.project_path, |service| {
            service.delete_connection(&request.connection_id)
        })?;
        if ok {
            emit_connections_changed(&app, &request.project_path);
        }
        Ok(KnowledgeFieldMutationResponse { ok })
    })();
    result.into()
}

/// Set or clear the per-lighthouse color override. `None` color clears it.
#[tauri::command]
pub async fn set_lighthouse_color(
    app: AppHandle,
    request: SetLighthouseColorRequest,
) -> CommandResult<SetLighthouseColorResponse> {
    debug!(lighthouse_id = %request.lighthouse_id, color = ?request.color, "Set lighthouse color");
    let result: Result<SetLighthouseColorResponse, VenoreError> = (|| {
        let outcome = venore_core::ocean::service::with_service(&request.project_path, |service| {
            match request.color.clone() {
                Some(color) => service.set_lighthouse_color(&request.lighthouse_id, color),
                None => {
                    service.clear_lighthouse_color(&request.lighthouse_id);
                    Ok(())
                }
            }
        })?;
        match outcome {
            Ok(()) => {
                // Reuse the connections-changed event to trigger a layout refetch
                // on the frontend — the color lives at layout level alongside
                // connections so a single channel keeps the canvas in sync.
                emit_connections_changed(&app, &request.project_path);
                Ok(SetLighthouseColorResponse { accepted: true, reason: None })
            }
            Err(reason) => Ok(SetLighthouseColorResponse {
                accepted: false,
                reason: Some(reason),
            }),
        }
    })();
    result.into()
}

/// Save camera state for a project's ocean layout.
#[tauri::command]
pub async fn save_ocean_camera(request: SaveOceanCameraRequest) -> CommandResult<()> {
    debug!(project_path = %request.project_path, "Saving ocean camera");

    let result: Result<(), VenoreError> = (|| {
        venore_core::ocean::service::with_service(&request.project_path, |service| {
            service.save_camera(CameraState {
                x: request.x,
                z: request.z,
                zoom: request.zoom,
            });
        })?;

        Ok(())
    })();

    result.into()
}

/// Get detailed module information.
///
/// Tries in-memory WizardSession first, then falls back to the persisted
/// analysis file on disk (`.venore/analysis-output.json`).
#[tauri::command]
pub async fn get_module_details(
    request: GetModuleDetailsRequest,
) -> CommandResult<ModuleDetailsResponse> {
    debug!(project_path = %request.project_path, module = %request.module_name, "Getting module details");

    let layers_config = get_layers_to_generate(&request.project_path);

    let result: Result<ModuleDetailsResponse, VenoreError> = (|| {
        // 1) Try in-memory session cache first
        let from_session = try_get_analysis_from_session(&request.project_path);

        // 2) Fall back to disk
        let analysis = match from_session {
            Some(a) => a,
            None => {
                debug!("No session cache, loading analysis from disk");
                AnalysisOutput::load_from_disk(Path::new(&request.project_path))
                    .map_err(|e| VenoreError::FileReadError(format!("{}", e)))?
                    .ok_or_else(|| VenoreError::NotFound(
                        format!("Analysis for '{}' (run the wizard first)", request.project_path)
                    ))?
            }
        };

        let module = analysis
            .modules
            .iter()
            .find(|m| m.name == request.module_name)
            .ok_or_else(|| {
                VenoreError::NotFound(format!("Module '{}'", request.module_name))
            })?;

        let exports: Vec<SymbolInfoDto> = module
            .symbols
            .exports
            .iter()
            .map(|s| SymbolInfoDto {
                name: s.name.clone(),
                kind: s.kind.clone(),
                file: s.file.clone(),
            })
            .collect();

        let project_path = Path::new(&request.project_path);
        let conn_info = ModuleConnectionInfo {
            dependencies: module.architecture.dependencies.clone(),
            dependents: module.architecture.dependents.clone(),
        };
        let analysis = layers::analyze_module_layers(
            project_path,
            &module.path,
            Some(&conn_info),
            &layers_config,
        );
        let layers = convert_layer_analysis(&analysis);

        Ok(ModuleDetailsResponse {
            name: module.name.clone(),
            path: module.path.clone(),
            file_count: module.file_count,
            entry_point: module.entry_point.clone(),
            files: module.files.clone(),
            dependencies: module.architecture.dependencies.clone(),
            dependents: module.architecture.dependents.clone(),
            external_deps: module.architecture.external_deps.clone(),
            exports,
            layers,
        })
    })();

    result.into()
}

/// Try to get AnalysisOutput from the in-memory WizardSession.
/// Returns None if no session or no cached analysis (never errors).
fn try_get_analysis_from_session(project_path: &str) -> Option<AnalysisOutput> {
    let session_mgr = WizardSessionManager::global();
    let guard = session_mgr.lock().ok()?;
    let session = guard.get(project_path)?;
    session.get_cached_analysis().ok().cloned()
}

// =============================================================================
// Node states + rover status
// =============================================================================

/// Read the active state list for a node. Used by the node-detail panel /
/// any UI surface that needs the current vector outside of the streaming
/// `ocean-state-changed` events.
#[tauri::command]
pub async fn get_node_states(
    request: GetNodeStatesRequest,
) -> CommandResult<GetNodeStatesResponse> {
    debug!(project_path = %request.project_path, node_id = %request.node_id, "Get node states");

    let result: Result<GetNodeStatesResponse, VenoreError> = (|| {
        let states = venore_core::ocean::service::with_service(&request.project_path, |service| {
            service.get_node_states(&request.node_id)
        })?;

        Ok(GetNodeStatesResponse {
            node_id: request.node_id.clone(),
            states: states.iter().map(state_instance_to_dto).collect(),
        })
    })();

    result.into()
}

/// Persist a "ignore until X" dismissal for a state kind on a node. Pass
/// `until_ts <= 0` to clear the dismissal entirely. Triggers a re-scan so
/// the suppression (or its removal) propagates immediately.
#[tauri::command]
pub async fn dismiss_node_state(
    request: DismissNodeStateRequest,
) -> CommandResult<KnowledgeFieldMutationResponse> {
    debug!(
        project_path = %request.project_path,
        node_id = %request.node_id,
        kind = %request.kind,
        until_ts = request.until_ts,
        "Dismiss node state"
    );

    let result: Result<KnowledgeFieldMutationResponse, VenoreError> = (|| {
        let kind = state_kind_from_str(&request.kind).ok_or_else(|| {
            VenoreError::InvalidParams(format!("Unknown state kind '{}'", request.kind))
        })?;
        let ok = venore_core::ocean::service::with_service(&request.project_path, |service| {
            service.dismiss_state(&request.node_id, kind, request.until_ts)
        })?;
        Ok(KnowledgeFieldMutationResponse { ok })
    })();

    result.into()
}

/// Read the node-state scanner's current activity (which cell, what's next,
/// queue depth). Backed by the `StateCurrent` snapshot now that the rover is
/// folded into the Currents engine. Returns `running: false` when no current
/// has been spawned for this project yet.
#[tauri::command]
pub async fn get_rover_status(
    request: GetRoverStatusRequest,
) -> CommandResult<RoverStatusResponse> {
    let snap = current_snapshot(&request.project_path, "state_current");
    let response = match snap {
        Some(s) => RoverStatusResponse {
            current_cell: s.current_cell.map(|c| GridCellDto { col: c.col, row: c.row }),
            target_cell: s.target_cell.map(|c| GridCellDto { col: c.col, row: c.row }),
            queue_depth: s.queue_depth as u32,
            idle: s.idle,
            last_step_at: s.last_step_at,
            running: true,
        },
        None => RoverStatusResponse {
            current_cell: None,
            target_cell: None,
            queue_depth: 0,
            idle: true,
            last_step_at: 0,
            running: false,
        },
    };
    CommandResult::ok(response)
}

// =============================================================================
// Helpers
// =============================================================================

/// Convert NodeVariant to the string the frontend expects.
/// Kept in sync with the snake_case serde rename on the enum.
fn node_variant_to_str(variant: NodeVariant) -> &'static str {
    match variant {
        NodeVariant::Module => "module",
        NodeVariant::KnowledgeNode => "knowledge_node",
        NodeVariant::Lighthouse => "lighthouse",
        NodeVariant::Buoy => "buoy",
        NodeVariant::Cylinder => "cylinder",
    }
}

fn subtype_to_str(subtype: KnowledgeNodeSubtype) -> &'static str {
    match subtype {
        KnowledgeNodeSubtype::Concept => "concept",
        KnowledgeNodeSubtype::Feature => "feature",
        KnowledgeNodeSubtype::Decision => "decision",
        KnowledgeNodeSubtype::Finding => "finding",
        KnowledgeNodeSubtype::Question => "question",
    }
}

fn subtype_from_str(s: &str) -> Option<KnowledgeNodeSubtype> {
    match s {
        "concept" => Some(KnowledgeNodeSubtype::Concept),
        "feature" => Some(KnowledgeNodeSubtype::Feature),
        "decision" => Some(KnowledgeNodeSubtype::Decision),
        "finding" => Some(KnowledgeNodeSubtype::Finding),
        "question" => Some(KnowledgeNodeSubtype::Question),
        _ => None,
    }
}

fn source_to_dto(source: SourceAttribution) -> SourceAttributionDto {
    match source {
        SourceAttribution::User => SourceAttributionDto::User,
        SourceAttribution::Ai { model, timestamp } => SourceAttributionDto::Ai { model, timestamp },
    }
}

fn source_from_dto(dto: SourceAttributionDto) -> SourceAttribution {
    match dto {
        SourceAttributionDto::User => SourceAttribution::User,
        SourceAttributionDto::Ai { model, timestamp } => SourceAttribution::Ai { model, timestamp },
    }
}

/// Default layers when no wizard config is available.
const DEFAULT_LAYERS: &[&str] = &["context"];

/// Get layers_to_generate from wizard config (session or checkpoint).
/// Falls back to ["context"] if nothing is available.
fn get_layers_to_generate(project_path: &str) -> Vec<String> {
    // Try in-memory session first
    let session_mgr = WizardSessionManager::global();
    if let Ok(guard) = session_mgr.lock() {
        if let Some(session) = guard.get(project_path) {
            if let Ok(config) = session.get_wizard_config() {
                if !config.layers_to_generate.is_empty() {
                    return config.layers_to_generate.clone();
                }
            }
        }
    }

    // Try persisted layers config (saved by build_wizard_config)
    let layers_path = Path::new(project_path).join(".venore/layers-config.json");
    if layers_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&layers_path) {
            if let Ok(layers) = serde_json::from_str::<Vec<String>>(&content) {
                if !layers.is_empty() {
                    return layers;
                }
            }
        }
    }

    // Try checkpoint on disk
    let checkpoint_path = Path::new(project_path).join(".venore/context-checkpoint.json");
    if checkpoint_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&checkpoint_path) {
            if let Ok(checkpoint) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(layers) = checkpoint
                    .get("wizard_config")
                    .and_then(|c| c.get("layers_to_generate"))
                    .and_then(|l| l.as_array())
                {
                    let result: Vec<String> = layers
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect();
                    if !result.is_empty() {
                        return result;
                    }
                }
            }
        }
    }

    DEFAULT_LAYERS.iter().map(|s| s.to_string()).collect()
}

/// Convert core LayerAnalysis results to NodeLayerDto for the frontend.
/// Missing layers are filtered out — the node will simply be shorter.
fn convert_layer_analysis(analysis: &ModuleLayerAnalysis) -> Vec<NodeLayerDto> {
    analysis
        .layers
        .iter()
        .filter(|la| la.status != LayerStatus::Missing)
        .map(|la| NodeLayerDto {
            layer_type: la.layer_type.as_str().to_string(),
            status: la.status.as_str().to_string(),
            details: if la.details.is_empty() {
                None
            } else {
                Some(la.details.clone())
            },
        })
        .collect()
}

/// Compute node status from the layer mix — drives the glow color on the 3D node.
///
/// Until the new flow eliminated per-module `.context.md`, status came from
/// the context layer's `freshness`. With context layer gone, we derive health
/// from the analyzed layers as a whole:
///   - "fresh"   = at least one Complete and zero Missing
///   - "stale"   = any Partial (or Complete + Missing mix)
///   - "missing" = all layers Missing (or no layers)
fn compute_node_status(analysis: &ModuleLayerAnalysis) -> String {
    let mut has_complete = false;
    let mut has_partial = false;
    let mut has_missing = false;
    for la in &analysis.layers {
        match la.status {
            LayerStatus::Complete => has_complete = true,
            LayerStatus::Partial => has_partial = true,
            LayerStatus::Missing => has_missing = true,
        }
    }
    if has_complete && !has_partial && !has_missing {
        "fresh".to_string()
    } else if has_complete || has_partial {
        "stale".to_string()
    } else {
        "missing".to_string()
    }
}

/// Build a map of module_name → ModuleConnectionInfo from the extracted modules.
fn build_connection_map(modules: &[ModuleInfo]) -> std::collections::HashMap<String, ModuleConnectionInfo> {
    modules
        .iter()
        .map(|m| {
            (
                m.name.clone(),
                ModuleConnectionInfo {
                    dependencies: m.dependencies.clone(),
                    dependents: m.dependents.clone(),
                },
            )
        })
        .collect()
}

/// Build directed connections from module dependencies.
/// Only emits connections where both endpoints exist in the layout.
/// The frontend handles dedup and bidirectional detection.
fn build_ocean_connections(modules: &[ModuleInfo], node_ids: &HashSet<String>) -> Vec<OceanConnectionDto> {
    let mut connections = Vec::new();
    let mut id_counter = 0u32;
    for module in modules {
        for dep in &module.dependencies {
            if !node_ids.contains(&module.id) || !node_ids.contains(dep) {
                continue;
            }
            id_counter += 1;
            connections.push(OceanConnectionDto {
                id: format!("conn-{}", id_counter),
                from_id: module.id.clone(),
                to_id: dep.clone(),
                kind: "dependency".to_string(),
            });
        }
    }
    connections
}

/// Extract ModuleInfo list from WizardSession's cached analysis.
///
/// Tries in-memory WizardSessionManager first, falls back to
/// `.venore/analysis-output.json` on disk if no session is cached.
fn extract_modules_from_session(project_path: &str) -> Vec<ModuleInfo> {
    // 1. Try in-memory session cache
    let from_session = try_get_analysis_from_session(project_path);

    // 2. Fall back to disk
    let analysis = match from_session {
        Some(a) => a,
        None => {
            debug!(project_path, "No session cache, loading analysis from disk");
            match AnalysisOutput::load_from_disk(Path::new(project_path)) {
                Ok(Some(a)) => a,
                Ok(None) => {
                    debug!(project_path, "No analysis on disk either");
                    return Vec::new();
                }
                Err(e) => {
                    warn!(project_path, error = %e, "Failed to load analysis from disk");
                    return Vec::new();
                }
            }
        }
    };

    analysis
        .modules
        .iter()
        .map(|m| ModuleInfo {
            id: m.name.clone(),
            name: m.name.clone(),
            path: m.path.clone(),
            dependencies: m.architecture.dependencies.clone(),
            dependents: m.architecture.dependents.clone(),
        })
        .collect()
}
