//! Currents runtime — passive workers that navigate the Ocean.
//!
//! A "current" is an independent worker that sweeps the ocean's nodes doing one
//! task (the first is the Index Current — keeping the logbook search index up to
//! date). Unlike the rover (a single cursor running N state-scanners per node),
//! each current has its OWN cursor and traversal queue, and emits its OWN
//! progress event keyed by `current_id`, so the frontend can render several
//! independent cursors moving at once.
//!
//! Design constraints honored here:
//!   - `ocean` must not depend on `rag`: a current's `visit` only emits
//!     serializable [`CurrentTask`] *intents*; the heavy work (embeddings, DB
//!     writes) is done by the desktop bridge that drains the event channel.
//!   - `visit` runs under the service lock and must be fast (no I/O, no awaits)
//!     — same rule as `Scanner::evaluate`.
//!   - One tokio task per project round-robins all currents (the service is
//!     behind a single Mutex, so N tasks would only contend on the lock).

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, warn};

use crate::ocean::currents::traversal::route_nearest_from;
use crate::ocean::service::with_service;
use crate::ocean::states::NodeStateInstance;
use crate::ocean::types::{GridCell, KnowledgeNodeData, LayoutEntry};

// =============================================================================
// Current trait + context + task intents
// =============================================================================

/// Read-only view passed to a current per node visit. Mirrors `ScanContext`.
pub struct VisitContext<'a> {
    pub node_id: &'a str,
    pub layout_entry: &'a LayoutEntry,
    pub knowledge: Option<&'a KnowledgeNodeData>,
    pub project_path: &'a str,
    pub now_ts: i64,
}

/// A serializable side-effect a current asks the desktop bridge to perform
/// AFTER the service lock is released. Keeps `ocean` free of `rag`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CurrentTask {
    /// Reindex one knowledge node's sections into the logbook search index.
    IndexLogbookNode { project_path: String, node_id: String },
    /// Re-hash one code module and compare against its stored fingerprint, so
    /// the bridge can flag drift. Spreads the (expensive) staleness detection
    /// that used to block project open across passive background ticks.
    CheckModuleStale {
        project_path: String,
        module_name: String,
        module_path: String,
    },
}

/// How a current picks the next node to visit. Orthogonal to *what* it does
/// per node — it's purely the navigation strategy over the Ocean.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Traversal {
    /// Plan a full route over EVERY node once per sweep (via `plan_route`), then
    /// drain it node by node. Re-sweeps everything on the periodic refresh.
    /// Right for tasks that must re-examine all nodes (indexing, drift checks).
    FullSweep,
    /// Drain the service's dirty queue — only nodes flagged dirty by a mutation
    /// (edit, move, connect, pending write…) or by the periodic `mark_all_dirty`.
    /// Incremental and prompt: a node mutated mid-session is picked up within a
    /// tick instead of waiting for the next full sweep. Right for node-state
    /// scanning, where a stale halo must react quickly to edits. `plan_route` is
    /// not used in this mode (the queue decides the order).
    DirtyQueue,
}

/// One passive worker. Registered once and reused across ticks. Must be
/// `Send + Sync` because the registry is `Arc`-shared with the runner task.
pub trait Current: Send + Sync {
    /// Stable identifier — event discriminator + frontend key (e.g. "index_current").
    fn id(&self) -> &'static str;
    /// Inspect one node and emit any work intents (effects OUTSIDE `ocean`, run
    /// by the desktop bridge). Runs under the service lock: must be fast, no I/O,
    /// no awaits. Return an empty vec for currents that only flag node state.
    fn visit(&self, ctx: &VisitContext<'_>) -> Vec<CurrentTask>;
    /// Optionally compute this node's state vector (an effect INSIDE `ocean`,
    /// applied to the service by the runner — unlike `visit`'s cross-module
    /// intents). `None` (default) = "I don't manage node state, leave it
    /// untouched". `Some(states)` = "set this node's states to exactly these"
    /// (an empty vec clears them). Only a state-scanning current overrides this;
    /// it's how the old rover's scanner pass folds into the Currents engine.
    fn evaluate_states(&self, _ctx: &VisitContext<'_>) -> Option<Vec<NodeStateInstance>> {
        None
    }
    /// How this current navigates the Ocean. Default: a full planned sweep.
    /// Override to `DirtyQueue` for prompt, incremental state scanning.
    fn traversal(&self) -> Traversal {
        Traversal::FullSweep
    }
    /// Plan the visiting ORDER for one full sweep, given every node + its cell.
    /// Called once per sweep (cheap vs recomputing per tick) — only in
    /// `FullSweep` traversal. Override to give a current a visibly distinct path
    /// — distinct currents should pick distinct strategies so their cursors
    /// don't trace the same line. Default: a greedy nearest-neighbor tour from
    /// the grid origin (see `traversal::route_*`).
    fn plan_route(&self, nodes: Vec<(String, GridCell)>) -> Vec<String> {
        route_nearest_from(&nodes, GridCell::new(0, 0))
    }
}

// =============================================================================
// Events
// =============================================================================

/// Where a current's cursor is and how much work it has left. One per tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentProgressEvent {
    pub project_path: String,
    pub current_id: String,
    pub current_cell: Option<GridCell>,
    pub target_cell: Option<GridCell>,
    pub queue_depth: usize,
    pub idle: bool,
}

/// A node's state vector flipped after a current's `evaluate_states` pass.
/// Mirrors the old rover `StateChangedEvent` exactly so the desktop bridge can
/// forward it on the same `ocean-state-changed` Tauri event with no frontend
/// change. Only emitted when the vector actually changed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentStateChange {
    pub project_path: String,
    pub node_id: String,
    pub states: Vec<NodeStateInstance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CurrentEvent {
    Progress(CurrentProgressEvent),
    Task(CurrentTask),
    StateChanged(CurrentStateChange),
}

/// Lightweight read-only view of a current's activity (per current_id).
#[derive(Debug, Clone, Default, Serialize)]
pub struct CurrentSnapshot {
    pub current_id: String,
    pub current_cell: Option<GridCell>,
    pub target_cell: Option<GridCell>,
    pub queue_depth: usize,
    pub idle: bool,
    pub last_step_at: i64,
}

// =============================================================================
// Singleton registry of running currents, keyed by project_path
// =============================================================================

struct RunningCurrents {
    stop: Arc<AtomicBool>,
    snapshots: Arc<Mutex<HashMap<String, CurrentSnapshot>>>,
    _join: tokio::task::JoinHandle<()>,
}

static CURRENTS: Lazy<Mutex<HashMap<String, RunningCurrents>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

// =============================================================================
// Tunables
// =============================================================================

/// Time between ticks. One node advanced per current per tick.
pub const CURRENTS_INTERVAL: Duration = Duration::from_millis(350);

/// How often each current re-enqueues every node (catches edits made between
/// sweeps; the per-section hash makes unchanged nodes a cheap no-op).
pub const CURRENTS_REFRESH: Duration = Duration::from_secs(5 * 60);

// =============================================================================
// Per-current traversal state (owned by the runner, NOT the service)
// =============================================================================

struct CurrentState {
    current: Arc<dyn Current>,
    /// Navigation mode, cached from `current.traversal()` at construction.
    mode: Traversal,
    /// The planned visiting order for this sweep, front = next node. Built once
    /// per sweep by `current.plan_route(...)`, so each current follows its own
    /// path. Drained one node per tick. Only used in `FullSweep` mode.
    route: VecDeque<String>,
    cursor: Option<GridCell>,
    /// When this current went idle (for the periodic refresh window).
    idle_since: Option<tokio::time::Instant>,
    /// Whether this current has completed at least one full sweep. Until then
    /// an empty `route` means "not seeded yet" (the layout may still be
    /// loading), so we retry seeding every tick instead of idling. Only
    /// meaningful in `FullSweep` mode (`DirtyQueue` relies on the dirty set).
    swept_once: bool,
    /// Whether the current active stretch has produced any work (a `Task` or a
    /// node-state change). Reset when a new active stretch begins; checked at
    /// sweep end to decide dormancy. Auto-gate: a current that sweeps a whole
    /// project without producing anything is irrelevant here and hides its
    /// scanner instead of gliding over nodes it never acts on.
    produced_work: bool,
    /// True once a completed sweep produced no work. While dormant the runner
    /// keeps sweeping silently (so new work is still detected) but emits no
    /// moving `Progress`, so the frontend marker stays hidden. The instant a
    /// visit produces work, the current wakes (`dormant = false`) and shows.
    dormant: bool,
}

impl CurrentState {
    fn new(current: Arc<dyn Current>) -> Self {
        let mode = current.traversal();
        Self {
            current,
            mode,
            route: VecDeque::new(),
            cursor: None,
            idle_since: None,
            swept_once: false,
            produced_work: false,
            dormant: false,
        }
    }
}

/// Gather every node id + its cell under the service lock — the input to
/// `plan_route`. Empty if the service/layout isn't available yet.
fn gather_nodes(project_path: &str) -> Vec<(String, GridCell)> {
    with_service(project_path, |svc| {
        svc.all_node_ids()
            .into_iter()
            .filter_map(|id| svc.peek_layout_entry(&id).map(|e| (id, e.cell)))
            .collect()
    })
    .unwrap_or_default()
}

// =============================================================================
// Public API
// =============================================================================

/// Idempotently spawn the Currents runner for `project_path`. Subsequent calls
/// with the same path are no-ops. `currents` is the set of workers to run
/// (e.g. `default_currents()`). `sender` is drained by the desktop bridge.
pub fn ensure_currents_started(
    project_path: &str,
    currents: Vec<Arc<dyn Current>>,
    sender: UnboundedSender<CurrentEvent>,
) {
    let mut map = match CURRENTS.lock() {
        Ok(m) => m,
        Err(e) => {
            warn!(error = %e, "CURRENTS lock poisoned, skipping spawn");
            return;
        }
    };
    if map.contains_key(project_path) {
        return;
    }

    let stop = Arc::new(AtomicBool::new(false));
    let snapshots = Arc::new(Mutex::new(HashMap::new()));

    let config = RunnerConfig {
        project_path: project_path.to_string(),
        sender,
        stop: stop.clone(),
        snapshots: snapshots.clone(),
        interval: CURRENTS_INTERVAL,
        refresh_interval: CURRENTS_REFRESH,
    };

    let join = tokio::spawn(currents_loop(config, currents));
    map.insert(
        project_path.to_string(),
        RunningCurrents { stop, snapshots, _join: join },
    );
    debug!(project_path, "Spawned ocean currents");
}

/// Read the latest snapshot for one current of a project, if running.
pub fn current_snapshot(project_path: &str, current_id: &str) -> Option<CurrentSnapshot> {
    let map = CURRENTS.lock().ok()?;
    let running = map.get(project_path)?;
    let snaps = running.snapshots.lock().ok()?;
    snaps.get(current_id).cloned()
}

/// Ask the currents runner for `project_path` to stop and forget it. Idempotent.
pub fn stop_currents(project_path: &str) {
    if let Ok(mut map) = CURRENTS.lock() {
        if let Some(running) = map.remove(project_path) {
            running.stop.store(true, Ordering::Relaxed);
        }
    }
}

// =============================================================================
// Internals
// =============================================================================

struct RunnerConfig {
    project_path: String,
    sender: UnboundedSender<CurrentEvent>,
    stop: Arc<AtomicBool>,
    snapshots: Arc<Mutex<HashMap<String, CurrentSnapshot>>>,
    interval: Duration,
    refresh_interval: Duration,
}

async fn currents_loop(config: RunnerConfig, currents: Vec<Arc<dyn Current>>) {
    let mut states: Vec<CurrentState> = currents.into_iter().map(CurrentState::new).collect();

    loop {
        if config.stop.load(Ordering::Relaxed) {
            debug!(project_path = %config.project_path, "Currents stopping");
            break;
        }
        tokio::time::sleep(config.interval).await;
        if config.stop.load(Ordering::Relaxed) {
            break;
        }

        for state in &mut states {
            // Seed-on-demand (FullSweep only): before the FIRST sweep, an empty
            // route means the layout wasn't ready when we spawned (race with
            // `initialize`). Retry planning each tick until nodes show up.
            // DirtyQueue currents need no seeding — `initialize` marks every
            // node dirty, so the queue is already populated.
            if state.mode == Traversal::FullSweep && !state.swept_once && state.route.is_empty() {
                let nodes = gather_nodes(&config.project_path);
                if !nodes.is_empty() {
                    state.route = state.current.plan_route(nodes).into();
                }
            }
            advance_one(&config, state);
        }
    }

    if let Ok(mut map) = CURRENTS.lock() {
        map.remove(&config.project_path);
    }
}

/// Advance one current by a single node (or handle its idle/refresh).
fn advance_one(config: &RunnerConfig, state: &mut CurrentState) {
    let now_ts = now_secs();

    // Resolve the next node + run visit/evaluate_states under the service lock.
    let outcome = with_service(&config.project_path, |svc| {
        loop {
            // Pick the next node id according to the traversal mode.
            let node_id = match state.mode {
                Traversal::FullSweep => match state.route.pop_front() {
                    Some(id) => id,
                    None => return StepOutcome::Idle,
                },
                Traversal::DirtyQueue => match svc.pop_next_dirty(state.cursor) {
                    Some((id, _cell)) => id,
                    None => return StepOutcome::Idle,
                },
            };
            // Node deleted since it was queued → skip, take the next.
            let entry = match svc.peek_layout_entry(&node_id) {
                Some(e) => e.clone(),
                None => continue,
            };
            let target_cell = entry.cell;
            let knowledge = svc.peek_knowledge_data(&node_id).cloned();

            // Build the read-only context, run the current. Scoped so the
            // borrow of `node_id`/`entry` ends before we mutate the service.
            let (tasks, state_result) = {
                let ctx = VisitContext {
                    node_id: &node_id,
                    layout_entry: &entry,
                    knowledge: knowledge.as_ref(),
                    project_path: &config.project_path,
                    now_ts,
                };
                (state.current.visit(&ctx), state.current.evaluate_states(&ctx))
            };

            // In-`ocean` effect: if this current manages node state, apply it.
            // `None` means "don't touch states"; `Some(_)` replaces them (empty
            // clears). We only ship a StateChanged event when it actually moved.
            let changed_states = match state_result {
                Some(results) => {
                    if svc.apply_scan_result(&node_id, results, now_ts) {
                        Some(svc.get_node_states(&node_id))
                    } else {
                        None
                    }
                }
                None => None,
            };

            let queue_depth = match state.mode {
                Traversal::FullSweep => state.route.len(),
                Traversal::DirtyQueue => svc.dirty_count(),
            };

            return StepOutcome::Visited {
                node_id,
                target_cell,
                tasks,
                changed_states,
                queue_depth,
            };
        }
    });

    let current_id = state.current.id().to_string();

    match outcome {
        Err(e) => {
            warn!(error = %e, project_path = %config.project_path, current_id = %current_id, "Current tick: service unavailable");
        }
        Ok(StepOutcome::Idle) => {
            // Reached idle with an empty queue → a full sweep is done (or there
            // was never anything to sweep). Stop the seed-on-demand retries; from
            // here only the periodic refresh re-enqueues.
            state.swept_once = true;
            // Auto-gate: a sweep that produced nothing means this current has no
            // work in this project → go dormant (hide its scanner). It keeps
            // sweeping silently and wakes the moment a visit produces work.
            state.dormant = !state.produced_work;
            // Periodic refresh: after being idle for refresh_interval, re-arm.
            let now = tokio::time::Instant::now();
            let trigger = match state.idle_since {
                None => { state.idle_since = Some(now); false }
                Some(since) => now.duration_since(since) >= config.refresh_interval,
            };
            if trigger {
                match state.mode {
                    // FullSweep: re-plan the whole route.
                    Traversal::FullSweep => {
                        let nodes = gather_nodes(&config.project_path);
                        state.route = state.current.plan_route(nodes).into();
                    }
                    // DirtyQueue: re-flag every node so time-sensitive states
                    // (future relative-time scanners, etc.) get a fresh look —
                    // exactly what the old rover's periodic refresh did.
                    Traversal::DirtyQueue => {
                        let _ = with_service(&config.project_path, |svc| svc.mark_all_dirty());
                    }
                }
                state.idle_since = Some(now);
            }
            update_snapshot(&config.snapshots, &current_id, state.cursor, None, 0, true);
            let _ = config.sender.send(CurrentEvent::Progress(CurrentProgressEvent {
                project_path: config.project_path.clone(),
                current_id,
                current_cell: state.cursor,
                target_cell: None,
                queue_depth: 0,
                idle: true,
            }));
        }
        Ok(StepOutcome::Visited { node_id, target_cell, tasks, changed_states, queue_depth }) => {
            // A new active stretch begins after being idle → reset the work flag.
            if state.idle_since.is_some() {
                state.produced_work = false;
            }
            state.idle_since = None;

            // Did this node give the current anything to do? A `Task` (effect
            // outside ocean) or a node-state change both count.
            let did_work = !tasks.is_empty() || changed_states.is_some();
            if did_work {
                state.produced_work = true;
                state.dormant = false; // wake immediately so the marker shows
            }

            // Work intents and state changes ALWAYS fire (they must execute even
            // if the marker was hidden) — but since `did_work` wakes the current,
            // a producing tick is never hidden.
            for task in tasks {
                let _ = config.sender.send(CurrentEvent::Task(task));
            }
            if let Some(states) = changed_states {
                let _ = config.sender.send(CurrentEvent::StateChanged(CurrentStateChange {
                    project_path: config.project_path.clone(),
                    node_id,
                    states,
                }));
            }

            // Moving Progress (what shows the scanner) is suppressed while
            // dormant — the sweep continues silently to keep detecting work.
            if !state.dormant {
                update_snapshot(&config.snapshots, &current_id, state.cursor, Some(target_cell), queue_depth, false);
                let _ = config.sender.send(CurrentEvent::Progress(CurrentProgressEvent {
                    project_path: config.project_path.clone(),
                    current_id: current_id.clone(),
                    current_cell: state.cursor,
                    target_cell: Some(target_cell),
                    queue_depth,
                    idle: false,
                }));
            }

            state.cursor = Some(target_cell);
        }
    }
}

enum StepOutcome {
    Idle,
    Visited {
        node_id: String,
        target_cell: GridCell,
        tasks: Vec<CurrentTask>,
        changed_states: Option<Vec<NodeStateInstance>>,
        queue_depth: usize,
    },
}

fn update_snapshot(
    snapshots: &Arc<Mutex<HashMap<String, CurrentSnapshot>>>,
    current_id: &str,
    current_cell: Option<GridCell>,
    target_cell: Option<GridCell>,
    queue_depth: usize,
    idle: bool,
) {
    if let Ok(mut map) = snapshots.lock() {
        map.insert(current_id.to_string(), CurrentSnapshot {
            current_id: current_id.to_string(),
            current_cell,
            target_cell,
            queue_depth,
            idle,
            last_step_at: now_secs(),
        });
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
