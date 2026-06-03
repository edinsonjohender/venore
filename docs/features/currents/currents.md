# Currents

> Core module: `crates/venore-core/src/ocean/currents/`
> Tauri bridge: `crates/venore-desktop/src/commands/currents.rs`
> UI: `crates/venore-desktop/ui/src/components/ocean/CurrentOverlay.tsx`

## What it is

A **current** is an autonomous worker that traverses the Ocean's nodes one at a time, in the
background, running a specific task on each. The traversal is visible: a cursor advances
across the grid. Multiple currents can run at once over the same project, each with its own
cursor, its own traversal queue, and its own purpose.

Currents is the **system** that provides the "navigate the Ocean node by node" mechanic; each
concrete current defines **what** to do per node. The system is agnostic to project type and
to the task: it only orchestrates traversal and emits events.

## Goal

Move per-node Ocean work into passive, incremental, visible tasks that don't block the UI. A
current replaces work that would otherwise run all at once (and possibly block) with a
node-by-node background sweep.

## Architecture

```
ocean/currents/
  runner.rs      Current trait, types, runner (tokio task), CURRENTS singleton
  traversal.rs   route strategies (route_*) + nearest_pending() primitive
  index.rs       IndexCurrent — logbook indexing current (knowledge nodes)
  staleness.rs   StalenessCurrent — code-drift detection current (module nodes)
  state.rs       StateCurrent — node-state scanner (overflow/pending-writes); the old rover
  mod.rs         default_currents() + re-exports
ocean/scanner.rs   Scanner trait + ScannerRegistry + ScanContext (no runtime — StateCurrent runs them)
ocean/scanners/    concrete scanners: saturation (overflow), pending_writes
```

> **The rover is gone.** Node-state scanning used to run in a standalone "rover"
> task (`scanner.rs::rover_loop`). That runtime was retired: `StateCurrent` runs
> the same `ScannerRegistry` inside the Currents engine. `scanner.rs` now only
> defines the `Scanner` rule abstraction.

Layering rule: `ocean` does **not** depend on `rag` or on Tauri. A current that needs effects
outside `ocean` (writing to an index, calling a repo) does **not** run them: it emits a
serializable intent (`CurrentTask`). The bridge in `venore-desktop` drains those intents and
executes them against the repos it knows about.

### The trait

```rust
pub trait Current: Send + Sync {
    fn id(&self) -> &'static str;                       // event discriminator + UI key
    fn visit(&self, ctx: &VisitContext<'_>) -> Vec<CurrentTask>;
    fn evaluate_states(&self, ctx: &VisitContext<'_>) -> Option<Vec<NodeStateInstance>> { None }
    fn traversal(&self) -> Traversal { Traversal::FullSweep }
    fn plan_route(&self, nodes: Vec<(String, GridCell)>) -> Vec<String> { /* default: nearest tour */ }
}
```

`visit` runs **under the Ocean service lock**: it must be fast, with no I/O and no `await`. It
inspects the node and returns zero or more work intents (effects **outside** `ocean`, run by the
bridge).

`evaluate_states` is the **in-`ocean`** counterpart: `None` (default) = "I don't manage node
state". `Some(states)` = "set this node's state vector to exactly these" (empty clears it). The
runner applies it to the service and emits `StateChanged` when it actually moved. Only
`StateCurrent` overrides it — this is how the old rover's scanner pass folds in.

`traversal` chooses how the current picks its next node: `FullSweep` (plan a full route per
sweep, default) or `DirtyQueue` (drain the service's dirty set — incremental + prompt, used by
`StateCurrent` so edits reflect within a tick).

`plan_route` decides the **order** a current visits nodes for one sweep — its *path*. It's
called once per sweep (cheaper than recomputing per tick) with every node + cell. The default is
a greedy nearest-neighbor tour from the origin; a current overrides it to pick a distinct flow so
two currents don't trace the same line. Ready-made strategies live in `traversal.rs`:
`route_nearest_from(start)`, `route_row_major`, `route_column_major`, `route_spiral`, plus
`far_corner` (a start anchor). The two default currents deliberately differ: **IndexCurrent** uses
`route_row_major` (a clean horizontal scan) and **StalenessCurrent** uses `route_nearest_from`
from `far_corner` (an organic flow fanning in from the opposite corner).

### Visit context

```rust
pub struct VisitContext<'a> {
    pub node_id: &'a str,
    pub layout_entry: &'a LayoutEntry,            // variant, cell, lighthouse_id…
    pub knowledge: Option<&'a KnowledgeNodeData>, // sections, if the node has any
    pub project_path: &'a str,
    pub now_ts: i64,
}
```

### Work intents

```rust
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CurrentTask {
    IndexLogbookNode { project_path: String, node_id: String },
    CheckModuleStale { project_path: String, module_name: String, module_path: String },
}
```

Each new current that requires an effect outside `ocean` adds its variant here and handles it
in the bridge.

> **Currents is project-type agnostic.** The runner hands EVERY node to EVERY current,
> regardless of project kind — nothing in the system says "this current is for code" or "for
> knowledge". Each current decides per node, in `visit`, whether it has work. A current with no
> relevant nodes in a given project simply emits nothing and goes idle. So every registered
> current runs on every project; the specialization lives in the current, not the substrate.

### Events

```rust
pub enum CurrentEvent {
    Progress(CurrentProgressEvent),    // per tick, per current
    Task(CurrentTask),                 // effect OUTSIDE ocean, executed in the bridge
    StateChanged(CurrentStateChange),  // node state flipped (in-ocean), forwarded to the UI
}

pub struct CurrentProgressEvent {
    pub project_path: String,
    pub current_id: String,          // identifies the current
    pub current_cell: Option<GridCell>,
    pub target_cell: Option<GridCell>,
    pub queue_depth: usize,
    pub idle: bool,
}

pub struct CurrentStateChange {       // mirrors the old rover StateChangedEvent
    pub project_path: String,
    pub node_id: String,
    pub states: Vec<NodeStateInstance>,
}
```

`CurrentEvent` travels over an mpsc channel from core to the bridge. The bridge forwards
`Progress` as `ocean-current-progress`, executes `Task`, and forwards `StateChanged` as
`ocean-state-changed` (the same event name + payload the rover used — so the decorator frontend
is unchanged).

Currents that flag node state can also emit their own Tauri events from the bridge. The Staleness
Current emits `ocean-stale-module` (one per drifted module, incremental) and
`ocean-stale-modules-reconciled` (the authoritative `keep_deepest`-filtered set, at sweep end).
The frontend fills drift badges from the former and replaces the whole set from the latter.

## Runner behavior

- **One tokio task per project**, round-robin across the registered currents. The Ocean
  service sits behind a single `Mutex`; N tasks would only contend on the lock, so a single
  task advances every current per tick.
- **Per-current state** (held in the runner, not the service): `mode: Traversal`, `route:
  VecDeque<String>` (the planned visit order for a `FullSweep`, front = next), `cursor:
  Option<GridCell>` (last cell), `swept_once: bool`.
- **Two traversal modes:**
  - `FullSweep` (Index, Staleness): the route is built **once per sweep** by
    `current.plan_route(nodes)`, then drained one node per tick. Distinct currents → distinct
    routes → visibly separate paths. While `swept_once == false` and the route is empty, each
    tick re-plans from `gather_nodes()` (absorbs the race with layout loading on open). On
    `CURRENTS_REFRESH` idle it re-plans (periodic re-sweep).
  - `DirtyQueue` (State): each tick pops the dirty node nearest the cursor via
    `svc.pop_next_dirty(cursor)`. Mutations (`mark_dirty`) make a node reappear within a tick →
    prompt, incremental. On `CURRENTS_REFRESH` idle it calls `mark_all_dirty()` (full re-eval).
- **Per tick, per current:** the next node is resolved (skipping any deleted), then `visit` and
  `evaluate_states` run under the lock. If `evaluate_states` returns `Some`, the runner applies
  it (`apply_scan_result`) and, when it changed, captures the new vector. Outside the lock it
  emits one `Progress`, one `Task` per intent, and a `StateChanged` if the vector moved.
- **Idle + refresh:** when the queue/route empties the current goes idle; after `CURRENTS_REFRESH`
  idle it re-arms per mode (re-plan route, or `mark_all_dirty`).
- **Auto-gate (dormancy):** a current tracks whether its current active stretch produced any work
  (a `Task` or a node-state change). If a whole sweep produces nothing, the current goes
  **dormant**: it keeps sweeping silently (so new work is still detected) but emits no moving
  `Progress`, so its scanner stays hidden. The instant a visit produces work it wakes and shows.
  This keeps the engine agnostic — nothing says "this current is for code/knowledge"; a current
  with no relevant nodes simply hides itself instead of gliding over nodes it never acts on (e.g.
  the Index Current on a pure-code project, the Staleness Current on a Knowledge project).

### Constants

| Constant | Value | Meaning |
|---|---|---|
| `CURRENTS_INTERVAL` | 350 ms | time between ticks (one node advanced per current per tick) |
| `CURRENTS_REFRESH` | 5 min | idle time before re-planning the route (periodic re-sweep) |

### Lifecycle

```rust
ensure_currents_started(project_path, currents, sender)  // idempotent per project_path
current_snapshot(project_path, current_id) -> Option<CurrentSnapshot>
stop_currents(project_path)
```

The `CURRENTS: Lazy<Mutex<HashMap<project_path, RunningCurrents>>>` singleton registers the
task, its stop flag, and the per-`current_id` snapshots. `RunningCurrents` holds the
`JoinHandle`.

## Registered currents

`default_currents()` returns the set every project runs.

### IndexCurrent (`id = "index_current"`)

`index.rs`. For each node with variant `KnowledgeNode` or `Lighthouse` that has sections, it
emits `CurrentTask::IndexLogbookNode`. Other variants and sectionless nodes emit nothing. The
bridge executes the intent: it re-reads the sections, reindexes them by hash into the
`LogbookRepository`, and triggers the embedding pass (coalesced per project). Indexing detail
in [logbook-rag](../logbook-rag/logbook-rag.md).

### StalenessCurrent (`id = "staleness_current"`)

`staleness.rs`. For each `Module` node (code projects), it emits `CurrentTask::CheckModuleStale`.
This replaces the staleness detection that used to run synchronously in `open_existing_project`
and froze the app for ~15s on large projects (it re-hashes every module's source subtree). Now
the work is spread across background ticks: the bridge hashes ONE module per intent, off the
service lock, on a dedicated serial worker (so a multi-second hash never stalls the cursor's
`Progress` events). Per drifted module it emits `ocean-stale-module` (incremental badge); when a
full sweep completes (the current goes idle) it applies `keep_deepest` over the accumulated set
and emits `ocean-stale-modules-reconciled` (authoritative, ancestors collapsed). The hashing
itself lives in `context::hash_storage::check_module_stale` / `filter_deepest_stale`.

**Stat short-circuit.** Before the SHA-256, `check_module_stale` computes a cheap stat-only
fingerprint (`file_count + total_size + max_mtime`, in `context::hash::calculate_module_fingerprint`)
and compares it to the stored one. If it matches, nothing changed → the content hash is skipped.
The SHA-256 stays the authoritative arbiter when the fingerprint differs (or the dir is missing →
"deleted"). The fingerprint is persisted in `code-hashes.json` (schema v2; v1 entries lack it and
serde-default to 0, forcing one content hash until re-snapshotted).

### StateCurrent (`id = "state_current"`)

`state.rs`. The old rover, folded in. It holds a `ScannerRegistry` (`scanners::default_registry`
— saturation → `Overflow`, pending-writes → `PendingWrites`) and, per node, runs every scanner in
`evaluate_states` (it emits **no** `CurrentTask` — its effect is in-`ocean` node state). Uses
`Traversal::DirtyQueue` so a node mutated mid-session (a new pending write, an edited section,
each of which calls `mark_dirty`) is re-scanned within a tick — preserving the rover's prompt
feedback. The runner applies the result via `apply_scan_result` and emits `StateChanged` →
`ocean-state-changed`. Adding a node-state scanner means registering it in
`scanners::default_registry`; no change to `StateCurrent` or the engine.

## Desktop bridge

`commands/currents.rs`:
- `ensure_currents_bridge(app) -> UnboundedSender<CurrentEvent>` — creates (idempotently) the
  task that drains the channel. `Progress` → `app.emit("ocean-current-progress", …)`. `Task` →
  executed. `StateChanged` → `app.emit("ocean-state-changed", …)` (the StateCurrent's node-state
  vector; same event the rover emitted).
- The `ocean → rag` coupling lives here: the bridge has access to `LogbookRepository` and the
  config store; `ocean` does not. Likewise the `ocean → context::hash_storage` coupling for
  `CheckModuleStale` lives here, on a dedicated serial worker plus an idle-transition watcher
  that drives the `keep_deepest` reconcile.
- Startup is wired in `initialize_ocean_layout` (`commands/ocean.rs`):
  `ensure_currents_bridge` + `ensure_currents_started(path, default_currents(), sender)`.

## Frontend

`CurrentOverlay.tsx` listens to `ocean-current-progress`, filters by `project_path` and
`current_id`, and animates a scanner marker (`ScannerRover`) that travels between cells along an
L-shaped path (X axis, then Z). One instance per `current_id`, each its own color. No business
logic: it only positions the marker from the event. Payload type: `OceanCurrentProgressEvent` in
`ui/src/lib/tauri.ts`. Each marker claims a distinct id in `roverActiveStore` (a SET of active
sources) so one current going idle never freezes another's still-moving marker.

Three overlays are mounted in `OceanNodes.tsx`: `index_current` (green), `staleness_current`
(amber) and `state_current` (blue — the node-state scanner). The `state_current` overlay wires
`onArriveAtCell` → `releasePendingForCell`, so a node-state halo lands exactly when the cursor
visually reaches the node (decoupled from backend tick timing), and its `idle` flag flushes any
buffered halos. There is no longer a `RoverOverlay`.

## Adding a new current

1. Implement `Current` in `ocean/currents/` (define `id` and `visit`; optionally override
   `plan_route` with a `traversal::route_*` strategy so it traces a distinct path).
2. If it needs an effect outside `ocean`, add a variant to `CurrentTask` and handle it in
   `commands/currents.rs`.
3. Register it in `default_currents()`.
4. (Optional) mount a `CurrentOverlay` with its `current_id` and color in the frontend.
