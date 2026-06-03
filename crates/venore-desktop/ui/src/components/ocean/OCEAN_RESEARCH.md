# Ocean Canvas — Research Notes (Feb 2026)

## V1 Position System: How It Worked

### Algorithm: "BFS con esteroides"

1. **Group nodes by island** — `island` field in `.context.md` frontmatter
2. **Main island** = largest group, positioned at origin (0,0)
3. **Root node** = highest connection weight (most imports/exports)
4. **BFS from root** → layer 0 (center), layer 1 (ring), layer 2 (outer ring)...
5. **Ring positions** = Manhattan distance from center = layer number
6. **Sub-islands** positioned around main in 8 directions (right, bottom-right, etc.)
7. **Collision resolution** = spiral outward until empty cell found
8. **Lighthouses** at island edges as gateway nodes

### Key files in v1

| File | Purpose |
|------|---------|
| `LayoutCalculator.ts` | Stateless facade |
| `bfs.ts` | Main algorithm — island grouping + layout |
| `bfs-grid.ts` | BFS layers + ring position generation |
| `positioning.ts` | Sub-island offset calculations |
| `grid/math.ts` | `gridToWorld()` coordinate conversion |
| `OceanMapperService.ts` | Sub-island detection (NOT positioning) |

---

## V1 Position Bugs: Why Positions Were Lost

### Bug 1: No merge logic

When reopening a project, if a new `.context.md` was added, there was no code
to merge saved positions with new nodes. New nodes got calculated positions,
old nodes kept saved positions → inconsistent layout.

### Bug 2: Rescan destroys everything

`rescanProject()` called `LayoutCalculator.calculate()` which always runs full
BFS from scratch. All user-dragged positions overwritten.

### Bug 3: LayoutManager memory-only

On app reopen, LayoutManager's in-memory map was empty. If user dragged a node,
`updateNodeLayout()` got an empty map and overwrote `layouts.json` with `[]`.

### Bug 4: No file validation

`layouts.json` had no schema version, no checksum, no atomic writes. A crash
during save = corrupted JSON = all positions lost.

### Bug 5: No cleanup of deleted nodes

Positions for deleted `.context.md` files stayed as ghosts in `layouts.json`.

### Bug 6: Always full recalculation

`LayoutCalculator.calculate()` is pure — always BFS everything. No incremental
mode to add/remove single nodes.

### Bug 7: No dirty flag

Saved even when nothing changed, increasing corruption risk.

---

## What Is an Island

An island is a **logical grouping of nodes**, NOT a directory.

Determined by the `island` field in each `.context.md` frontmatter:

```yaml
---
name: Auth Service
island: "auth"          # ← this determines grouping
nodeType: module
---
```

- `island: null` or missing → belongs to "main" island
- `island: "auth"` → belongs to "auth" sub-island
- One node belongs to exactly ONE island

### Island composition

Each island contains:

| Node type | Visual | Purpose |
|-----------|--------|---------|
| **module** | Stacked cube layers | Core business logic |
| **buoy** | 3 mini-buildings | Utilities, helpers, constants |
| **cylinder** | Tall cylinder | External services, APIs |
| **lighthouse** | Tall pillar (7x height) | Entry point / gateway |

### How sub-islands are detected (OceanMapperService)

Analyzes each project folder with 6 weighted criteria (score 0-100):

| Criteria | Max pts | What it measures |
|----------|---------|-----------------|
| File count (`.context.md`) | 20 | How many context files |
| Has `index.ts` | 15 | Entry point exists |
| Pattern match | 20 | Architectural patterns (services, features, api) |
| Internal cohesion | 25 | Ratio of internal vs external imports |
| Depth | 10 | Folder depth (shallow = better) |
| Export count | 10 | Public API surface |

Score >= threshold (default 30) → proposed as sub-island.
User accepts/rejects. On accept: `island: "name"` propagated to all `.context.md` in folder.

---

## Full Pipeline: Project → Ocean Visualization

```
Project on disk
  ↓ ContextScanner (reads .context.md files)
ContextNode[] (identity + metadata + island field)
  ↓ ConnectionResolver (name → ID, case-insensitive)
ResolvedConnection[] (from, to, type)
  ↓ OceanEngine.extractIslandGroups()
Island groups: ["main", "auth", "api"]
  ↓ LayoutCalculator (BFS "Army Formation")
GridPosition[] per node ({col, row})
  ↓ OceanEngine.toRenderNode()
RenderNode[] (id, gridPos, island, visual, layers, metadata)
  ↓ routeConnectionsThroughLighthouses()
OceanConnection[] (direct / to-port / inter-island / from-port)
  ↓ snapshot() → IPC event → frontend
OceanSnapshot → store → 3D components
```

---

## V2 Design: How to Fix Position Management

### Principle: Backend owns ALL state

Frontend sends intents, backend validates and responds. See `OCEAN_ARCHITECTURE.md`.

### Fast startup (3 phases)

```
Phase 1 (0-100ms): Show skeleton
  - Read layout.json from disk (<10KB)
  - Show gray placeholder nodes at saved positions

Phase 2 (100-500ms): Hydrate
  - Render real nodes with labels and status colors
  - User can already navigate

Phase 3 (500ms+, background): Reconcile
  - Detect modules in background (Tauri command)
  - Compare module set hash with saved hash
  - If unchanged: done
  - If changed: diff → reconcile
```

### Reconciliation (instead of full recalculation)

```
current_modules vs saved_positions
  ↓
KEEP   = exists in both → restore saved position
NEW    = exists now but not before → auto-place
DELETE = saved but no longer exists → discard, free cell
```

New node placement:
1. Has connections to KEEP nodes → centroid of neighbors, snap to nearest free cell
2. No connections → next free cell in spiral from origin

### Persistence format

```
project-root/
  .venore/
    layout.json
```

```json
{
  "version": 1,
  "moduleSetHash": "abc123",
  "camera": { "x": 30, "z": 30, "zoom": 30 },
  "positions": {
    "module-id-1": { "col": 0, "row": 0, "userPlaced": true },
    "module-id-2": { "col": 3, "row": 5, "userPlaced": false }
  }
}
```

- `userPlaced: true` = user dragged it → NEVER auto-move
- `userPlaced: false` = auto-placed → can be repositioned if topology changes

### Occupancy grid (body block in backend)

Backend maintains:

```rust
struct OceanLayoutService {
    positions: HashMap<ModuleId, GridCell>,     // module → position
    occupancy: HashMap<(i32, i32), ModuleId>,   // cell → module
}
```

#### Move validation

```
Frontend: "move node-A to (3, 5)"
Backend:
  occupancy.get((3, 5))
    FREE → update both maps → emit event → save
    OCCUPIED → reject → emit rejection event
```

#### Auto-placement

```
For each new module:
  1. Calculate ideal position (centroid of neighbors or BFS)
  2. Cell occupied? → spiral outward for free cell
  3. Insert into both maps
```

#### Deletion

```
Module removed:
  pos = positions[module_id]
  occupancy.remove((pos.col, pos.row))   // free the cell
  positions.remove(module_id)
```

### Key rules

1. **Never recalculate everything** — only place new nodes
2. **Never move userPlaced nodes** automatically
3. **Hash for fast detection** — skip if module set unchanged
4. **Save on every drag-end** — dirty flag to avoid unnecessary writes
5. **Atomic writes** — write to temp file, then rename (prevents corruption)

---

## References

### Node editors
- React Flow: `snapToGrid` + `snapGrid` props, helper lines for alignment
- yFiles: incremental mode inserts space without redrawing existing nodes
- Figma: WebGPU renderer, smart guides, delta-based multiplayer sync

### Games
- Factorio: AABB collision, chunk-based spatial partitioning, lazy activation,
  registration-based tracking, delta compression for multiplayer
- Satisfactory: world-aligned foundation grid, Ctrl to snap, 2m/4m grid units
- Cities Skylines 2: road-relative zoning grid, snap options
- Minecraft: Anvil region format (32x32 chunks), empty sections not loaded

### Performance (for scaling to 100+ nodes)
- InstancedMesh: 1 draw call for all identical nodes
- Viewport culling with spatial index (RBush): 20x speedup for 20k objects
- Refs for drag state, setState only on commit
- Factorio: differential rendering, sprite atlas batching

### Position persistence
- VS Code: DOM snapshot on close, lazy code loading, progressive enhancement
- Figma: delta sync over WebSocket, central authority server
- Zed Editor: SQLite sidecar for workspace data
- Obsidian: persistent graph plugin saves node positions

### Incremental layout
- D3-force: pin/place pattern (fx/fy for fixed nodes)
- Centroid placement: new node at average position of connected neighbors
- Cola.js: constraint-based with pinning support
- fCoSE: incremental constraints on existing layout

### Conflict resolution
- Three-way merge: classify as KEEP/NEW/DELETE
- Stable IDs (content fingerprint) survive renames
- userPlaced flag prevents auto-move of user-positioned nodes
