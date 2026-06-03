//! Mesh Tauri commands — peer discovery, transport, and lifecycle
//!
//! The frontend calls `mesh_init` once on project load. The backend handles
//! the full lifecycle atomically (register → transport → handler → auto-connect
//! → background loop). Events flow back via Tauri event emitter.

use std::sync::Arc;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::state::LazyAppState;
use crate::utils::{CommandResult, StateCommandResult};
use venore_core::llm::LlmGateway;
use venore_core::mesh::{AgentHandler, MeshDiscovery, MeshEventEmitter, MeshTransport, PeerInfo};
use venore_core::rag::RagRepository;
use venore_core::VenoreError;

// =============================================================================
// TauriMeshEmitter — bridges core MeshEventEmitter to Tauri events
// =============================================================================

struct TauriMeshEmitter {
    app: AppHandle,
}

impl MeshEventEmitter for TauriMeshEmitter {
    fn emit_peers_updated(&self, peers: Vec<PeerInfo>) {
        let _ = self.app.emit("mesh:peers-updated", &peers);
    }

    fn emit_mesh_status(&self, running: bool, port: u16, connected_peers: Vec<String>) {
        let payload = MeshStatusPayload { running, port, connected_peers };
        let _ = self.app.emit("mesh:status-updated", &payload);
    }

    fn emit_mesh_error(&self, message: String) {
        let _ = self.app.emit("mesh:error", serde_json::json!({ "message": message }));
    }
}

/// Payload for `mesh:status-updated` event
#[derive(Serialize, Clone)]
struct MeshStatusPayload {
    running: bool,
    port: u16,
    connected_peers: Vec<String>,
}

// =============================================================================
// Lifecycle command — replaces register + start_transport + setup_handler +
//                     auto_connect (all merged into one atomic init)
// =============================================================================

/// Initialize the mesh subsystem atomically.
///
/// One call from the frontend triggers: register → transport → handler →
/// auto-connect → background loop. Events push status/peers/errors back.
#[tauri::command]
pub async fn mesh_init(
    app: AppHandle,
    project_id: String,
    project_name: String,
    project_path: String,
    lazy_state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<()> {
    let rag_repo = get_rag_repo(&lazy_state);
    let llm_gateway = get_llm_gateway(&lazy_state);

    let result: Result<(), VenoreError> = async {
        let rag_repo = rag_repo?;
        let llm_gateway = llm_gateway?;

        let handler = Arc::new(AgentHandler::new(
            project_id.clone(),
            project_path.clone(),
            rag_repo,
            llm_gateway,
        ));

        let emitter: Arc<dyn MeshEventEmitter> = Arc::new(TauriMeshEmitter { app });

        venore_core::mesh::lifecycle::mesh_init(
            &project_id,
            &project_name,
            &project_path,
            handler,
            emitter,
        )
        .await
    }
    .await;

    Ok(result.into())
}

// =============================================================================
// Remaining commands (5 total: mesh_init + these 4)
// =============================================================================

/// Get all live peers (excluding self)
#[tauri::command]
pub async fn mesh_get_peers() -> CommandResult<Vec<PeerInfo>> {
    let result: Result<Vec<PeerInfo>, VenoreError> = (|| {
        let mesh = MeshDiscovery::global();
        let guard = mesh.lock().map_err(|e| {
            VenoreError::MeshError(format!("Mesh mutex poisoned: {}", e))
        })?;
        guard.discover_peers()
    })();
    result.into()
}

/// Transport status returned to the frontend
#[derive(Serialize)]
pub struct MeshTransportStatus {
    pub running: bool,
    pub port: u16,
    pub connected_peers: Vec<String>,
}

/// Get transport status (running, port, connected peers)
#[tauri::command]
pub async fn mesh_transport_status() -> CommandResult<MeshTransportStatus> {
    let transport = MeshTransport::global();
    let t = transport.lock().await;
    let status = MeshTransportStatus {
        running: t.is_running(),
        port: t.port(),
        connected_peers: t.connected_peers(),
    };
    CommandResult::ok(status)
}

/// Connect to a peer by project_id
#[tauri::command]
pub async fn mesh_connect_peer(project_id: String) -> CommandResult<()> {
    let transport = MeshTransport::global();
    let mut t = transport.lock().await;
    let result = t.connect_to_peer(&project_id).await;
    result.into()
}

/// Disconnect from a specific peer
#[tauri::command]
pub async fn mesh_disconnect_peer(project_id: String) -> CommandResult<()> {
    let transport = MeshTransport::global();
    let mut t = transport.lock().await;
    t.disconnect_peer(&project_id).await;
    CommandResult::ok(())
}

/// Tear down a single project's mesh presence without stopping the
/// shared transport. Called from the frontend when a window is about to
/// close or when it switches to a different project: removes this
/// project's request handler and deletes its `.json` registration so
/// other peers stop seeing it almost immediately, instead of waiting on
/// the TTL sweep.
#[tauri::command]
pub async fn mesh_unregister_project(project_id: String) -> CommandResult<()> {
    venore_core::mesh::unset_request_handler(&project_id).await;
    let result: Result<(), VenoreError> = (|| {
        let mesh = MeshDiscovery::global();
        let mut guard = mesh.lock().map_err(|e| {
            VenoreError::MeshError(format!("Discovery mutex poisoned: {}", e))
        })?;
        guard.unregister(&project_id)
    })();
    result.into()
}

// =============================================================================
// Helpers
// =============================================================================

fn get_rag_repo(lazy: &LazyAppState) -> Result<Arc<RagRepository>, VenoreError> {
    let guard = lazy.get();
    match guard.as_ref() {
        Some(state) => Ok(Arc::clone(&state.rag_repository)),
        None => Err(VenoreError::NotFound("Backend not initialized".into())),
    }
}

fn get_llm_gateway(lazy: &LazyAppState) -> Result<Arc<LlmGateway>, VenoreError> {
    let guard = lazy.get();
    match guard.as_ref() {
        Some(state) => Ok(Arc::clone(&state.llm_gateway)),
        None => Err(VenoreError::NotFound("Backend not initialized".into())),
    }
}
