//! Tauri bridge for Ocean Currents.
//!
//! venore-core stays Tauri-agnostic: the currents runner pushes `CurrentEvent`
//! through an mpsc channel. This bridge drains it and:
//!   - forwards `Progress` events as the Tauri event `ocean-current-progress`
//!     (the frontend renders each current's cursor, keyed by `current_id`);
//!   - executes `Task(IndexLogbookNode)` by reindexing that node's sections
//!     into the `LogbookRepository`, then triggers a coalesced embedding pass.
//!
//! The ocean→rag coupling lives HERE (the desktop layer already owns both the
//! ocean service and the logbook repo) — `venore-core::ocean` never imports
//! `rag`.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};

use once_cell::sync::Lazy;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tracing::{debug, warn};

use venore_core::context::hash_storage::{self, StaleModule};
use venore_core::ocean::{CurrentEvent, CurrentTask};
use venore_core::rag::LogbookRepository;

use crate::state::LazyAppState;

const CURRENT_PROGRESS_EVENT: &str = "ocean-current-progress";

/// Node-state vector flipped for a node (overflow halo, pending-writes badge…).
/// Emitted by `StateCurrent` via `CurrentEvent::StateChanged`. Same event name
/// and payload shape the old rover used, so the frontend is unchanged.
const STATE_CHANGED_EVENT: &str = "ocean-state-changed";

/// Id of the Staleness Current — used to detect its sweep boundaries so the
/// bridge can run the `keep_deepest` reconcile pass once a full pass completes.
const STALENESS_CURRENT_ID: &str = "staleness_current";

/// Emitted once per drifted module as the Staleness Current sweeps (incremental
/// badge fill-in). Snake_case to match the other ocean current events.
const STALE_MODULE_EVENT: &str = "ocean-stale-module";

/// Emitted when a staleness sweep finishes: the authoritative, `keep_deepest`-
/// filtered set of drifted modules. The frontend REPLACES its badge map with
/// this (drops ancestors surfaced incrementally and modules no longer stale).
const STALE_RECONCILE_EVENT: &str = "ocean-stale-modules-reconciled";

static CURRENT_SENDER: Lazy<Mutex<Option<UnboundedSender<CurrentEvent>>>> =
    Lazy::new(|| Mutex::new(None));

/// Projects with an embedding pass already in flight — coalesces the bursts of
/// `IndexLogbookNode` tasks the first full sweep emits into one embed loop per
/// project instead of one per node.
static EMBEDDING_IN_FLIGHT: Lazy<Mutex<HashSet<String>>> =
    Lazy::new(|| Mutex::new(HashSet::new()));

// =============================================================================
// Staleness worker — serial off-loop hashing for the Staleness Current
// =============================================================================
//
// Module hashing reads + SHA-256s a whole source subtree (the work that used to
// freeze project open). It must NOT run on the bridge's main event loop, which
// also carries the cursor `Progress` events — a multi-second hash there would
// stall the cursor. So `CheckModuleStale` intents are forwarded to this
// dedicated worker, which hashes ONE module at a time (serial, no disk thrash)
// and emits badge events. The bridge loop only does a non-blocking `send`.

/// Work + lifecycle messages for the staleness worker.
enum StaleMsg {
    /// Hash one module and, if drifted, emit an incremental badge + accumulate it.
    Check { project_path: String, module_name: String, module_path: String },
    /// A new sweep started — reset the per-project accumulator.
    SweepStart { project_path: String },
    /// The sweep finished — apply `keep_deepest` and emit the authoritative set.
    Reconcile { project_path: String },
}

static STALE_SENDER: Lazy<Mutex<Option<UnboundedSender<StaleMsg>>>> =
    Lazy::new(|| Mutex::new(None));

/// Tracks the last `idle` flag seen per project for the staleness current, so
/// the bridge can fire `SweepStart`/`Reconcile` only on idle transitions (the
/// runner re-emits `idle: true` every tick while parked).
static STALENESS_LAST_IDLE: Lazy<Mutex<HashMap<String, bool>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Clone, Serialize)]
struct StaleModuleEvent {
    project_path: String,
    module_name: String,
    missing_on_disk: bool,
}

#[derive(Clone, Serialize)]
struct StaleModuleBadge {
    module_name: String,
    missing_on_disk: bool,
}

#[derive(Clone, Serialize)]
struct StaleReconcileEvent {
    project_path: String,
    modules: Vec<StaleModuleBadge>,
}

/// Idempotently spawn the staleness worker. Returns a sender clone.
fn ensure_stale_worker(app: &AppHandle) -> UnboundedSender<StaleMsg> {
    let mut guard = match STALE_SENDER.lock() {
        Ok(g) => g,
        Err(_) => {
            STALE_SENDER.clear_poison();
            STALE_SENDER.lock().expect("re-lock after clear_poison")
        }
    };
    if let Some(s) = guard.as_ref() {
        return s.clone();
    }

    let (tx, mut rx) = unbounded_channel::<StaleMsg>();
    *guard = Some(tx.clone());

    let app = app.clone();
    tokio::spawn(async move {
        // project_path -> (module_name -> StaleModule) found in the live sweep.
        let mut acc: HashMap<String, HashMap<String, StaleModule>> = HashMap::new();

        while let Some(msg) = rx.recv().await {
            match msg {
                StaleMsg::SweepStart { project_path } => {
                    acc.remove(&project_path);
                }
                StaleMsg::Check { project_path, module_name, module_path } => {
                    let pp = project_path.clone();
                    let mp = module_path.clone();
                    // Hashing is blocking I/O → off the async runtime threads.
                    let result = tokio::task::spawn_blocking(move || {
                        hash_storage::check_module_stale(Path::new(&pp), &mp)
                    })
                    .await;

                    match result {
                        Ok(Ok(Some(stale))) => {
                            let missing = stale.missing_on_disk;
                            acc.entry(project_path.clone())
                                .or_default()
                                .insert(module_name.clone(), stale);
                            if let Err(e) = app.emit(
                                STALE_MODULE_EVENT,
                                StaleModuleEvent { project_path, module_name, missing_on_disk: missing },
                            ) {
                                warn!(error = %e, "Failed to emit ocean-stale-module");
                            }
                        }
                        Ok(Ok(None)) => {
                            // Fresh now: drop any badge accumulated this sweep.
                            if let Some(map) = acc.get_mut(&project_path) {
                                map.remove(&module_name);
                            }
                        }
                        Ok(Err(e)) => warn!(error = %e, module = %module_name, "check_module_stale failed"),
                        Err(e) => warn!(error = %e, module = %module_name, "stale hash task panicked"),
                    }
                }
                StaleMsg::Reconcile { project_path } => {
                    let modules: Vec<StaleModule> = acc
                        .get(&project_path)
                        .map(|m| m.values().cloned().collect())
                        .unwrap_or_default();
                    let filtered = hash_storage::filter_deepest_stale(modules);
                    let badges: Vec<StaleModuleBadge> = filtered
                        .into_iter()
                        .map(|s| StaleModuleBadge {
                            module_name: s.module_name,
                            missing_on_disk: s.missing_on_disk,
                        })
                        .collect();
                    if let Err(e) = app.emit(
                        STALE_RECONCILE_EVENT,
                        StaleReconcileEvent { project_path, modules: badges },
                    ) {
                        warn!(error = %e, "Failed to emit ocean-stale-modules-reconciled");
                    }
                }
            }
        }
        debug!("Staleness worker: channel closed");
    });

    tx
}

/// Idempotently set up the bridge. Returns a sender clone for `ensure_currents_started`.
pub fn ensure_currents_bridge(app: &AppHandle) -> UnboundedSender<CurrentEvent> {
    let mut guard = match CURRENT_SENDER.lock() {
        Ok(g) => g,
        Err(e) => {
            warn!(error = %e, "CURRENT_SENDER lock poisoned, recreating");
            CURRENT_SENDER.clear_poison();
            CURRENT_SENDER.lock().expect("re-lock after clear_poison")
        }
    };
    if let Some(s) = guard.as_ref() {
        return s.clone();
    }

    let (tx, mut rx) = unbounded_channel::<CurrentEvent>();
    *guard = Some(tx.clone());

    let app_handle = app.clone();
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                CurrentEvent::Progress(p) => {
                    // The Staleness Current's sweep boundaries drive the
                    // keep_deepest reconcile. Detect idle transitions here.
                    if p.current_id == STALENESS_CURRENT_ID {
                        on_staleness_progress(&app_handle, &p);
                    }
                    if let Err(e) = app_handle.emit(CURRENT_PROGRESS_EVENT, p) {
                        warn!(error = %e, "Failed to emit ocean-current-progress");
                    }
                }
                CurrentEvent::StateChanged(sc) => {
                    // StateCurrent flagged/cleared a node's state vector. Reuse
                    // the rover's old Tauri event so the decorators react exactly
                    // as before (identical payload: project_path, node_id, states).
                    if let Err(e) = app_handle.emit(STATE_CHANGED_EVENT, sc) {
                        warn!(error = %e, "Failed to emit ocean-state-changed");
                    }
                }
                CurrentEvent::Task(task) => {
                    handle_task(&app_handle, task).await;
                }
            }
        }
        debug!("Currents bridge: channel closed");
    });

    tx
}

/// Execute one current task.
async fn handle_task(app: &AppHandle, task: CurrentTask) {
    match task {
        CurrentTask::IndexLogbookNode { project_path, node_id } => {
            if let Err(e) = index_one_node(app, &project_path, &node_id).await {
                warn!(error = %e, node_id = %node_id, "Logbook index task failed");
            }
        }
        CurrentTask::CheckModuleStale { project_path, module_name, module_path } => {
            // Non-blocking hand-off: the actual hashing runs serially on the
            // staleness worker so it never stalls this loop (cursor Progress
            // events share it).
            let tx = ensure_stale_worker(app);
            let _ = tx.send(StaleMsg::Check { project_path, module_name, module_path });
        }
    }
}

/// Fire `SweepStart`/`Reconcile` to the staleness worker on idle transitions of
/// the Staleness Current (the runner re-emits `idle: true` every tick while
/// parked, so we act only when the flag actually flips).
fn on_staleness_progress(app: &AppHandle, p: &venore_core::ocean::CurrentProgressEvent) {
    let tx = ensure_stale_worker(app);
    let mut last = match STALENESS_LAST_IDLE.lock() {
        Ok(g) => g,
        Err(_) => {
            STALENESS_LAST_IDLE.clear_poison();
            STALENESS_LAST_IDLE.lock().expect("re-lock after clear_poison")
        }
    };
    // Unknown project = treated as previously idle, so the first sweep's first
    // non-idle tick counts as a SweepStart.
    let was_idle = last.get(&p.project_path).copied().unwrap_or(true);
    if was_idle && !p.idle {
        let _ = tx.send(StaleMsg::SweepStart { project_path: p.project_path.clone() });
    } else if !was_idle && p.idle {
        let _ = tx.send(StaleMsg::Reconcile { project_path: p.project_path.clone() });
    }
    last.insert(p.project_path.clone(), p.idle);
}

/// Reindex one node's sections, then kick a coalesced embedding pass.
async fn index_one_node(
    app: &AppHandle,
    project_path: &str,
    node_id: &str,
) -> Result<(), venore_core::error::VenoreError> {
    let lazy = app.state::<LazyAppState>();

    // Repos + config store (cloned Arcs so we can use them across awaits).
    let (logbook_repo, config_store): (Arc<LogbookRepository>, _) = {
        let guard = lazy.get();
        let state = guard.as_ref().ok_or_else(|| {
            venore_core::error::VenoreError::NotFound("Backend not initialized".into())
        })?;
        (Arc::clone(&state.logbook_repository), Arc::clone(&state.config_store))
    };

    // Resolve project_id from disk identity (same as the chat layer).
    let project_id = venore_core::project::ProjectService::read_or_create_identity(
        std::path::Path::new(project_path),
    )
    .map(|identity| identity.id.to_string())
    .map_err(|e| venore_core::error::VenoreError::NotFound(format!("project identity: {}", e)))?;

    // Snapshot this node's sections from the ocean (in-memory / disk).
    let sections = venore_core::ocean::service::with_service(project_path, |svc| {
        svc.peek_knowledge_data(node_id).map(|d| d.sections.clone())
    })?;
    let sections = match sections {
        Some(s) => s,
        None => Vec::new(), // node has no content layer → diff will prune any stale chunks
    };

    let (upserted, deleted) =
        venore_core::rag::index_logbook_node(&logbook_repo, &project_id, node_id, &sections).await?;

    // Only run embeddings when something actually changed, and coalesce per project.
    if upserted > 0 {
        maybe_embed(logbook_repo, config_store, project_id);
    }

    if upserted > 0 || deleted > 0 {
        debug!(node_id, upserted, deleted, "Logbook node reindexed");
    }
    Ok(())
}

/// Spawn one embedding pass per project at a time (coalesced). No-op if a pass
/// is already in flight, or if no embedding provider/key is configured.
fn maybe_embed(
    logbook_repo: Arc<LogbookRepository>,
    config_store: Arc<venore_core::infrastructure::config::DefaultConfigStore>,
    project_id: String,
) {
    {
        let mut in_flight = match EMBEDDING_IN_FLIGHT.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        if in_flight.contains(&project_id) {
            return; // a pass is already running; it will pick up new chunks
        }
        in_flight.insert(project_id.clone());
    }

    tokio::spawn(async move {
        let (provider, api_key) =
            crate::commands::chat::helpers::resolve_embedding_provider(&config_store).await;

        if let Some(provider) = provider {
            let key = api_key.unwrap_or_default();
            if let Err(e) = venore_core::rag::embed_logbook_chunks(
                &logbook_repo,
                &project_id,
                provider.as_ref(),
                &key,
            ).await {
                warn!(error = %e, "Logbook embedding pass failed (FTS still works)");
            }
        } else {
            debug!("No embedding provider configured — logbook stays FTS-only");
        }

        if let Ok(mut in_flight) = EMBEDDING_IN_FLIGHT.lock() {
            in_flight.remove(&project_id);
        }
    });
}
