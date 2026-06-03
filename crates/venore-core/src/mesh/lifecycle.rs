//! Mesh Lifecycle — atomic init/stop for the entire mesh subsystem
//!
//! `mesh_init()` performs the full startup sequence atomically:
//! register → start transport → set handler → auto-connect → background loop.
//!
//! `mesh_stop()` performs the reverse: stop loop → shutdown transport → unregister.
//!
//! The `MeshEventEmitter` trait decouples core from Tauri so events can be
//! emitted without depending on the desktop crate.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use once_cell::sync::Lazy;

use crate::error::{Result, VenoreError};
use crate::mesh::discovery::MeshDiscovery;
use crate::mesh::handler::MeshRequestHandler;
use crate::mesh::transport::MeshTransport;
use crate::mesh::types::PeerInfo;

/// Flag to signal the background auto-connect loop to stop.
static LOOP_RUNNING: Lazy<Arc<AtomicBool>> = Lazy::new(|| Arc::new(AtomicBool::new(false)));

/// Handle for the background auto-connect task.
static LOOP_HANDLE: Lazy<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>> =
    Lazy::new(|| tokio::sync::Mutex::new(None));

// =============================================================================
// MeshEventEmitter trait
// =============================================================================

/// Trait for emitting mesh events to the frontend.
///
/// Decouples venore-core from Tauri so the lifecycle module can push events
/// without importing the desktop crate.
pub trait MeshEventEmitter: Send + Sync + 'static {
    /// Emitted when the peer list changes (auto-connect found new peers, etc.)
    fn emit_peers_updated(&self, peers: Vec<PeerInfo>);

    /// Emitted with current mesh status (running, port, connected peers)
    fn emit_mesh_status(&self, running: bool, port: u16, connected_peers: Vec<String>);

    /// Emitted when an error occurs in the background loop
    fn emit_mesh_error(&self, message: String);
}

// =============================================================================
// mesh_init — atomic startup
// =============================================================================

/// Initialize the mesh subsystem for `project_id` atomically. Safe to
/// call multiple times within the same process for different projects:
/// each open project adds its own peer entry and request handler.
///
/// 1. Register this project in mesh discovery (writes `.json`)
/// 2. Ensure WebSocket transport is running (idempotent — server is
///    per-process, not per-peer) and stamp its port into this project's
///    registration file
/// 3. Install this project's request handler under `project_id`
/// 4. Auto-connect to already-discovered peers
/// 5. Emit initial status to the frontend
/// 6. Spawn the background auto-connect + touch loop (idempotent)
///
/// If any step fails, previously completed steps for THIS project are
/// rolled back — other projects in the same process are left intact.
pub async fn mesh_init(
    project_id: &str,
    project_name: &str,
    project_path: &str,
    handler: Arc<dyn MeshRequestHandler>,
    emitter: Arc<dyn MeshEventEmitter>,
) -> Result<()> {
    // Step 1: Register this project
    {
        let mesh = MeshDiscovery::global();
        let mut guard = mesh.lock().map_err(|e| {
            VenoreError::MeshError(format!("Discovery mutex poisoned: {}", e))
        })?;
        guard.register(project_id, project_name, project_path)?;
    }
    tracing::info!(project_id, "Mesh: registered");

    // Step 2: Start (or reuse) transport, then stamp its port on this
    // project's registration. `start()` is idempotent at the singleton
    // level — a second mesh_init from a sibling project just gets the
    // existing port back.
    let port = {
        let transport = MeshTransport::global();
        let mut t = transport.lock().await;
        match t.start().await {
            Ok(p) => p,
            Err(e) => {
                let mesh = MeshDiscovery::global();
                if let Ok(mut m) = mesh.lock() {
                    let _ = m.unregister(project_id);
                }
                return Err(e);
            }
        }
    };
    if port > 0 {
        let mesh = MeshDiscovery::global();
        let result = mesh.lock();
        if let Ok(mut m) = result {
            let _ = m.update_port(project_id, port);
        }
    }
    tracing::info!(port, "Mesh: transport ready");

    // Step 3: Install this project's request handler
    crate::mesh::set_request_handler(project_id, handler).await;

    // Step 4: Auto-connect to peers
    {
        let transport = MeshTransport::global();
        let mut t = transport.lock().await;
        if let Err(e) = t.auto_connect().await {
            tracing::warn!(error = %e, "Mesh: initial auto-connect failed (non-fatal)");
        }
    }

    // Step 5: Emit initial status
    emit_current_state(&emitter).await;

    // Step 6: Spawn background loop (no-op if already running)
    spawn_auto_connect_loop(emitter);

    tracing::info!(project_id, "Mesh: project initialized");
    Ok(())
}

// =============================================================================
// mesh_stop — atomic shutdown
// =============================================================================

/// Stop the mesh subsystem atomically — full process shutdown path.
///
/// 1. Stop the background auto-connect / touch loop
/// 2. Shutdown transport (disconnect all peers)
/// 3. Drop every locally-registered peer's handler
/// 4. Unregister every local peer (remove all `.json` files)
pub async fn mesh_stop() {
    stop_auto_connect_loop().await;

    {
        let transport = MeshTransport::global();
        let mut t = transport.lock().await;
        t.shutdown().await;
    }
    tracing::info!("Mesh: transport shut down");

    crate::mesh::clear_request_handlers().await;

    {
        let mesh = MeshDiscovery::global();
        let result = mesh.lock();
        if let Ok(mut m) = result {
            let _ = m.unregister_all();
        }
    }
    tracing::info!("Mesh: unregistered all peers — lifecycle stopped");
}

// =============================================================================
// Background auto-connect loop
// =============================================================================

/// Spawn a background task that auto-connects to new peers every 30 seconds
/// and emits `mesh:peers-updated` when the peer list changes.
fn spawn_auto_connect_loop(emitter: Arc<dyn MeshEventEmitter>) {
    // If already running, don't spawn another
    if LOOP_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    let flag = Arc::clone(&LOOP_RUNNING);
    let handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        // Skip the first tick (we already auto-connected in mesh_init)
        interval.tick().await;

        while flag.load(Ordering::SeqCst) {
            interval.tick().await;

            if !flag.load(Ordering::SeqCst) {
                break;
            }

            // Refresh `last_seen` on all local peers so other processes'
            // TTL-based liveness check doesn't sweep us. Cheap — just
            // rewrites the JSON files we own.
            {
                let mesh = MeshDiscovery::global();
                let result = mesh.lock();
                if let Ok(mut m) = result {
                    if let Err(e) = m.touch_local() {
                        tracing::debug!(error = %e, "Mesh touch_local failed");
                    }
                }
            }

            // Auto-connect to new peers
            {
                let transport = MeshTransport::global();
                let mut t = transport.lock().await;
                if let Err(e) = t.auto_connect().await {
                    tracing::debug!(error = %e, "Mesh background auto-connect failed");
                    emitter.emit_mesh_error(format!("Auto-connect error: {}", e));
                    continue;
                }
            }

            // Emit updated state
            emit_current_state(&emitter).await;
        }

        tracing::debug!("Mesh auto-connect loop stopped");
    });

    // Store handle (fire-and-forget store, we abort it on stop)
    tokio::spawn(async move {
        let mut guard = LOOP_HANDLE.lock().await;
        *guard = Some(handle);
    });
}

/// Stop the background auto-connect loop.
async fn stop_auto_connect_loop() {
    LOOP_RUNNING.store(false, Ordering::SeqCst);

    let mut guard = LOOP_HANDLE.lock().await;
    if let Some(handle) = guard.take() {
        handle.abort();
        let _ = handle.await;
    }
}

// =============================================================================
// Helpers
// =============================================================================

/// Emit the current mesh state (peers + transport status) via the emitter.
async fn emit_current_state(emitter: &Arc<dyn MeshEventEmitter>) {
    // Get peers
    let peers = {
        let mesh = MeshDiscovery::global();
        let result = mesh.lock();
        match result {
            Ok(m) => m.discover_peers().unwrap_or_default(),
            Err(_) => vec![],
        }
    };

    // Get transport status
    let (running, port, connected) = {
        let transport = MeshTransport::global();
        let t = transport.lock().await;
        (t.is_running(), t.port(), t.connected_peers())
    };

    emitter.emit_peers_updated(peers);
    emitter.emit_mesh_status(running, port, connected);
}
