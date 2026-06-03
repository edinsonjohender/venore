// =============================================================================
// OceanNodes — Backend-driven nodes for Ocean Canvas
// =============================================================================
// Calls initializeOceanLayout on mount, renders nodes from backend positions.
// Drag-end calls moveOceanNode — backend validates occupancy.
// Restores saved camera position from backend on mount.
// No business logic here: just fetch, render, and forward intents.

import { useState, useCallback, useEffect, useRef } from 'react'
import { useThree, type ThreeEvent } from '@react-three/fiber'
import { Select } from '@react-three/drei'
import * as THREE from 'three'
import { listen } from '@tauri-apps/api/event'
import { OceanNode, type NodeStatus, type NodeLayer } from './OceanNode'
import { OceanConnections } from './OceanConnections'
import { IslandTiles } from './IslandTiles'
import { NodeSelectionOutline } from './NodeSelectionOutline'
import { DragPreviewTiles } from './DragPreviewTiles'
import { NodeDecoratorOverlay } from './NodeDecoratorOverlay'
import { CurrentOverlay } from './CurrentOverlay'
import { islandColor, type NodePosition as IslandNode } from './island-utils'
import { NODE_COLORS } from './nodes/colors'
import { DEFAULT_LAYERS } from './nodes/types'
import { CAMERA_CONFIG, cellToWorld, worldToCell } from './ocean-config'
import {
  tauriApi,
  type OceanNodePosition,
  type OceanConnectionDto,
  type NodeLayerDto,
  type OceanCurrentProgressEvent,
  type OceanStateChangedEvent,
  type OceanStaleModuleEvent,
  type OceanStaleReconcileEvent,
} from '@/lib/tauri'
import { useNodeFloatingStore } from '@/stores/nodeFloatingStore'
import { focusNodePanel, useHasOpenNodes } from '@/stores/openNodes'
import { useSelectedNodesStore } from '@/stores/selectedNodesStore'
import { useHighlightStore } from '@/stores/highlightStore'
import { useLighthouseColorsStore } from '@/stores/lighthouseColorsStore'
import { useRoverActiveStore } from '@/stores/roverActiveStore'

export interface LighthouseOption {
  id: string
  name: string
}

interface OceanNodesProps {
  projectPath: string
  /** Bumped by the parent to force a refetch of the layout (e.g. after creating a node). */
  reloadKey?: number
  /** Right-click on a node. Parent gets the full node + the list of available
   *  lighthouses (so it can build a "Move to lighthouse..." submenu). */
  onNodeContextMenu?: (
    node: OceanNodePosition,
    event: ThreeEvent<MouseEvent>,
    lighthouses: LighthouseOption[],
  ) => void
}

/** Map backend layer DTOs to NodeLayer format. Falls back to defaults if empty. */
function mapLayers(dtos: NodeLayerDto[]): NodeLayer[] {
  if (!dtos || dtos.length === 0) return DEFAULT_LAYERS
  return dtos.map((d) => ({
    type: d.type as NodeLayer['type'],
    status: d.status as NodeLayer['status'],
    details: d.details,
  }))
}

/** Knowledge nodes don't have code-derived layers — their stack height comes
 *  from the logbook's section count. Each section becomes one synthetic
 *  "layer" so the visual stack scales with content. Clamped to >=1 so nodes
 *  with zero sections still have a footprint. */
function synthesizeKnowledgeLayers(sectionCount: number): NodeLayer[] {
  const n = Math.max(1, sectionCount)
  return Array.from({ length: n }, () => ({
    type: 'context' as const,
    status: 'complete' as const,
  }))
}

function layersForNode(node: OceanNodePosition): NodeLayer[] {
  if (node.node_variant === 'knowledge_node') {
    return synthesizeKnowledgeLayers(node.section_count)
  }
  return mapLayers(node.layers)
}

export function OceanNodes({ projectPath, reloadKey = 0, onNodeContextMenu }: OceanNodesProps) {
  const [nodes, setNodes] = useState<OceanNodePosition[]>([])
  const [connections, setConnections] = useState<OceanConnectionDto[]>([])
  // Map module_name -> 'info' (code changed) | 'warning' (module missing).
  // Synthesized into each node's `states` at render time so the decorator
  // registry handles the rendering. Empty for projects without a committed
  // `.venore/code-hashes.json` snapshot.
  const [staleByModuleName, setStaleByModuleName] = useState<Map<string, 'info' | 'warning'>>(
    () => new Map(),
  )
  const { camera, controls } = useThree()
  const cameraRef = useRef(camera)
  cameraRef.current = camera
  const controlsRef = useRef(controls)
  controlsRef.current = controls

  // Fetch layout from backend on mount, restore camera if saved.
  // Layer analysis runs separately after nodes render (instant canvas).
  useEffect(() => {
    tauriApi
      .initializeOceanLayout({ project_path: projectPath })
      .then((response) => {
        setNodes(response.nodes)
        setConnections(response.connections)
        useLighthouseColorsStore.getState().setOverrides(response.lighthouse_colors)

        // Restore saved camera position + sync MapControls target
        // so the viewing angle stays the same (offset [30, 40, 30])
        const cam = cameraRef.current
        const ctrl = controlsRef.current
        if (response.camera) {
          cam.position.set(response.camera.x, cam.position.y, response.camera.z)
          if (cam instanceof THREE.OrthographicCamera) {
            cam.zoom = response.camera.zoom
            cam.updateProjectionMatrix()
          }
          if (ctrl && 'target' in ctrl) {
            const mapCtrl = ctrl as unknown as { target: THREE.Vector3; update: () => void }
            mapCtrl.target.set(
              response.camera.x - CAMERA_CONFIG.position[0],
              0,
              response.camera.z - CAMERA_CONFIG.position[2],
            )
            mapCtrl.update()
          }
        }

        // Chain: compute layers in background, then merge into nodes
        tauriApi
          .computeOceanLayers({ project_path: projectPath })
          .then((updates) => {
            setNodes((prev) => {
              const updateMap = new Map(updates.map((u) => [u.module_id, u]))
              return prev.map((n) => {
                const update = updateMap.get(n.module_id)
                if (!update) return n
                return { ...n, layers: update.layers, node_status: update.node_status }
              })
            })
          })
          .catch((err) => {
            console.error('Failed to compute ocean layers:', err)
          })

        // Drift detection is no longer a synchronous walk here (it froze open
        // for ~15s on large projects). The Staleness Current sweeps module
        // nodes in the background and feeds `staleByModuleName` incrementally
        // via the `ocean-stale-module` / `ocean-stale-modules-reconciled`
        // events — see the dedicated listener effect below.
      })
      .catch((err) => {
        console.error('Failed to initialize ocean layout:', err)
      })
  }, [projectPath, reloadKey])

  // Staleness Current feed. The background current sweeps module nodes and
  // emits one `ocean-stale-module` per drifted module (incremental fill-in),
  // then an authoritative `ocean-stale-modules-reconciled` when the sweep ends
  // (collapses ancestors via keep_deepest, drops modules no longer stale). The
  // incremental events let badges appear as the cursor moves; the reconcile
  // replaces the whole map so it converges to the same result the old
  // synchronous walk produced — without blocking open.
  useEffect(() => {
    let cancelled = false
    const unlistenIncremental = listen<OceanStaleModuleEvent>('ocean-stale-module', (event) => {
      if (cancelled) return
      const p = event.payload
      if (p.project_path !== projectPath) return
      setStaleByModuleName((prev) => {
        const next = new Map(prev)
        next.set(p.module_name, p.missing_on_disk ? 'warning' : 'info')
        return next
      })
    })
    const unlistenReconcile = listen<OceanStaleReconcileEvent>(
      'ocean-stale-modules-reconciled',
      (event) => {
        if (cancelled) return
        const p = event.payload
        if (p.project_path !== projectPath) return
        const map = new Map<string, 'info' | 'warning'>()
        for (const m of p.modules) {
          map.set(m.module_name, m.missing_on_disk ? 'warning' : 'info')
        }
        setStaleByModuleName(map)
      },
    )
    return () => {
      cancelled = true
      unlistenIncremental.then((fn) => fn())
      unlistenReconcile.then((fn) => fn())
    }
  }, [projectPath])

  // Auto-refresh node colors after the context updater finishes
  useEffect(() => {
    let cancelled = false
    const unlisten = listen('context-update-complete', () => {
      if (cancelled) return
      tauriApi
        .computeOceanLayers({ project_path: projectPath })
        .then((updates) => {
          setNodes((prev) => {
            const updateMap = new Map(updates.map((u) => [u.module_id, u]))
            return prev.map((n) => {
              const update = updateMap.get(n.module_id)
              if (!update) return n
              return { ...n, layers: update.layers, node_status: update.node_status }
            })
          })
        })
        .catch((err) => console.error('Failed to refresh ocean layers:', err))

      // Re-check drift too: a context-update typically means new commits
      // landed, which is exactly the situation where staleness flips.
      tauriApi
        .getStaleModules(projectPath)
        .then((stale) => {
          const map = new Map<string, 'info' | 'warning'>()
          for (const s of stale) {
            map.set(s.moduleName, s.missingOnDisk ? 'warning' : 'info')
          }
          setStaleByModuleName(map)
        })
        .catch((err) => console.error('Failed to refresh stale modules:', err))
    })
    return () => {
      cancelled = true
      unlisten.then((fn) => fn())
    }
  }, [projectPath])

  // Logbook content changed (any window): full refetch so new nodes (from
  // extract), removed nodes, variant flips, and section_count updates all
  // surface. We preserve the layer analysis cached in current state so module
  // nodes don't flash to "loading" while computeOceanLayers re-runs.
  useEffect(() => {
    let cancelled = false
    const unlisten = listen<{ project_path: string; node_id: string }>(
      'ocean-knowledge-changed',
      (event) => {
        if (cancelled) return
        const p = event.payload
        if (p.project_path !== projectPath) return
        tauriApi
          .initializeOceanLayout({ project_path: projectPath })
          .then((response) => {
            if (cancelled) return
            setNodes((prev) => {
              const cached = new Map(
                prev.map((n) => [n.module_id, { layers: n.layers, node_status: n.node_status }]),
              )
              return response.nodes.map((n) => {
                const c = cached.get(n.module_id)
                return c ? { ...n, layers: c.layers, node_status: c.node_status } : n
              })
            })
            setConnections(response.connections)
          })
          .catch((err) => console.error('Failed to refresh after logbook change:', err))
      },
    )
    return () => {
      cancelled = true
      unlisten.then((fn) => fn())
    }
  }, [projectPath])

  // Rover scanned a node and its state vector flipped: BUFFER the change so
  // the halo doesn't pop in before the rover visually arrives at the cell.
  //
  // Backend emits `state-changed` and `rover-progress` together on each
  // tick (target=B). The next tick's `progress` arrives ~350ms later with
  // `current_cell=B`, by which point the visual rover has reached B too.
  // Releasing the pending change at THAT moment syncs the halo to the
  // rover's arrival without coupling timing parameters between layers.
  const pendingStatesRef = useRef<Map<string, OceanStateChangedEvent['states']>>(
    new Map(),
  )

  useEffect(() => {
    let cancelled = false
    const unlisten = listen<OceanStateChangedEvent>('ocean-state-changed', (event) => {
      if (cancelled) return
      if (event.payload.project_path !== projectPath) return
      pendingStatesRef.current.set(event.payload.node_id, event.payload.states)
    })
    return () => {
      cancelled = true
      unlisten.then((fn) => fn())
    }
  }, [projectPath])

  // Helper: apply any pending state-change for the node sitting at (col,row).
  const releasePendingForCell = useCallback((col: number, row: number) => {
    const node = nodesRef.current.find((n) => n.col === col && n.row === row)
    if (!node) return
    const states = pendingStatesRef.current.get(node.module_id)
    if (!states) return
    pendingStatesRef.current.delete(node.module_id)
    setNodes((prev) =>
      prev.map((n) => (n.module_id === node.module_id ? { ...n, states } : n)),
    )
  }, [])

  // Helper: drain everything pending — used when the rover reports idle
  // (no more dirty nodes) or when this OceanNodes unmounts. Keeps the UI
  // eventually consistent even if the rover never visits some cell again.
  const flushAllPending = useCallback(() => {
    if (pendingStatesRef.current.size === 0) return
    const updates = new Map(pendingStatesRef.current)
    pendingStatesRef.current.clear()
    setNodes((prev) =>
      prev.map((n) => {
        const states = updates.get(n.module_id)
        return states ? { ...n, states } : n
      }),
    )
  }, [])

  // Listen to the state current's progress only to detect `idle` — when its
  // queue empties, drain any leftover pending so straggler halos still appear.
  // The actual "cursor finished this cell" signal comes from CurrentOverlay's
  // onArriveAtCell callback (visual landing), not from the backend tick,
  // so far-cell hops stay synchronized regardless of travel distance.
  useEffect(() => {
    let cancelled = false
    const unlisten = listen<OceanCurrentProgressEvent>('ocean-current-progress', (event) => {
      if (cancelled) return
      if (event.payload.project_path !== projectPath) return
      if (event.payload.current_id !== 'state_current') return
      if (event.payload.idle) flushAllPending()
    })
    return () => {
      cancelled = true
      unlisten.then((fn) => fn())
    }
  }, [projectPath, flushAllPending])

  // Fail-safe: if a pending state never gets released (rover redirected
  // before reaching the previous target, or the user moved the node
  // mid-flight, etc.), drain whatever's left every 3s so the UI doesn't
  // stay out of date indefinitely.
  useEffect(() => {
    const id = setInterval(() => {
      if (pendingStatesRef.current.size === 0) return
      flushAllPending()
    }, 3_000)
    return () => clearInterval(id)
  }, [flushAllPending])

  // Manual connections or lighthouse-color changed (any window): refetch
  // layout-level state without disturbing nodes. Same channel covers both
  // since the changes live alongside each other.
  useEffect(() => {
    let cancelled = false
    const unlisten = listen<{ project_path: string }>(
      'ocean-connections-changed',
      (event) => {
        if (cancelled) return
        if (event.payload.project_path !== projectPath) return
        tauriApi
          .initializeOceanLayout({ project_path: projectPath })
          .then((response) => {
            if (cancelled) return
            setConnections(response.connections)
            useLighthouseColorsStore.getState().setOverrides(response.lighthouse_colors)
          })
          .catch((err) => console.error('Failed to refresh connections:', err))
      },
    )
    return () => {
      cancelled = true
      unlisten.then((fn) => fn())
    }
  }, [projectPath])

  // Highlight coordinator: when the selection changes (or nodes update),
  // compute which nodes should sit at level 1 (elevated) and write the map
  // to the highlight store. Today: a selected node lifts its whole island
  // (all nodes that share its lighthouse_id), or itself if it's loose.
  const selectedIds = useSelectedNodesStore((s) => s.ids)
  useEffect(() => {
    const elevations = new Map<string, number>()
    if (selectedIds.size === 0) {
      useHighlightStore.getState().setElevations(elevations)
      return
    }
    const elevatedIslands = new Set<string>()
    const elevatedLoose = new Set<string>()
    for (const id of selectedIds) {
      const node = nodes.find((n) => n.module_id === id)
      if (!node) continue
      if (node.node_variant === 'lighthouse') {
        elevatedIslands.add(node.module_id)
      } else if (node.lighthouse_id) {
        elevatedIslands.add(node.lighthouse_id)
      } else {
        elevatedLoose.add(node.module_id)
      }
    }
    for (const n of nodes) {
      const memberOfElevatedIsland =
        (n.node_variant === 'lighthouse' && elevatedIslands.has(n.module_id)) ||
        (!!n.lighthouse_id && elevatedIslands.has(n.lighthouse_id))
      if (memberOfElevatedIsland || elevatedLoose.has(n.module_id)) {
        elevations.set(n.module_id, 1)
      }
    }
    useHighlightStore.getState().setElevations(elevations)
  }, [selectedIds, nodes])

  // Drag-end: ask backend to validate the move.
  // Returns true if accepted, false if rejected/error.
  // OceanNode handles visual reset on rejection imperatively.
  // If the dragged node belongs to a multi-selection, every selected node
  // moves with the same delta atomically (group move).
  const handleMove = useCallback(
    async (id: string, newPos: [number, number, number]): Promise<boolean> => {
      // newPos is already snapped to grid by BaseNode
      const { col, row } = worldToCell(newPos[0], newPos[2])

      const selected = useSelectedNodesStore.getState().ids
      const isGroupMove = selected.has(id) && selected.size > 1

      if (!isGroupMove) {
        try {
          const result = await tauriApi.moveOceanNode({
            project_path: projectPath,
            node_id: id,
            target_col: col,
            target_row: row,
          })
          if (result.accepted) {
            setNodes((prev) =>
              prev.map((n) =>
                n.module_id === id
                  ? { ...n, col: result.col, row: result.row, user_placed: true }
                  : n,
              ),
            )
            return true
          }
          return false
        } catch {
          return false
        }
      }

      // Group move: compute the delta from the anchor's old/new cell, apply to
      // every selected node, then send the whole batch atomically.
      const anchor = nodesRef.current.find((n) => n.module_id === id)
      if (!anchor) return false
      const dCol = col - anchor.col
      const dRow = row - anchor.row
      const groupNodes = nodesRef.current.filter((n) => selected.has(n.module_id))
      const moves = groupNodes.map((n) => ({
        node_id: n.module_id,
        target_col: n.col + dCol,
        target_row: n.row + dRow,
      }))

      try {
        const result = await tauriApi.moveOceanNodes({
          project_path: projectPath,
          moves,
        })
        if (result.all_accepted) {
          const targetById = new Map(
            result.results.map((r) => [r.node_id, { col: r.col, row: r.row }]),
          )
          setNodes((prev) =>
            prev.map((n) => {
              const target = targetById.get(n.module_id)
              return target
                ? { ...n, col: target.col, row: target.row, user_placed: true }
                : n
            }),
          )
          return true
        }
        return false
      } catch {
        return false
      }
    },
    [projectPath],
  )

  const nodesRef = useRef(nodes)
  nodesRef.current = nodes

  // Enrich the context-menu callback with the full node + the list of
  // available lighthouses, derived from the current state.
  const handleNodeContextMenuInternal = useCallback(
    (id: string, event: ThreeEvent<MouseEvent>, _label: string) => {
      if (!onNodeContextMenu) return
      const node = nodesRef.current.find((n) => n.module_id === id)
      if (!node) return
      const lighthouses: LighthouseOption[] = nodesRef.current
        .filter((n) => n.node_variant === 'lighthouse')
        .map((n) => ({ id: n.module_id, name: n.module_name }))
      onNodeContextMenu(node, event, lighthouses)
    },
    [onNodeContextMenu],
  )

  const handleClick = useCallback(
    (id: string) => {
      const node = nodesRef.current.find((n) => n.module_id === id)
      if (!node) return
      // Routed via the unified helper so clicking a node that's currently
      // popped out focuses the existing OS window instead of spawning a
      // duplicate floating panel in-app.
      focusNodePanel({
        projectPath,
        moduleId: node.module_id,
        moduleName: node.module_name,
        modulePath: node.module_path,
        nodeVariant: node.node_variant,
      })
    },
    [projectPath],
  )

  // Project node state into the shape island-utils expects (id/col/row/lighthouseId).
  // Lighthouse ids = the ones to render tiles for.
  const islandNodes: IslandNode[] = nodes.map((n) => ({
    id: n.module_id,
    col: n.col,
    row: n.row,
    lighthouseId: n.lighthouse_id,
  }))
  const lighthouseIds = nodes
    .filter((n) => n.node_variant === 'lighthouse')
    .map((n) => n.module_id)

  // drei <Select multiple box> reports the leaf meshes hit by the rectangle.
  // Each ocean node is a Group with userData.oceanNodeId — walk up the parent
  // chain from the leaf to find that id.
  // Stable callback BaseNode uses to read the current node positions during
  // a group drag (collision detection + initial-position snapshot).
  const getAllNodes = useCallback(
    () => nodesRef.current.map((n) => ({ id: n.module_id, col: n.col, row: n.row })),
    [],
  )

  const handleSelectChange = useCallback((items: THREE.Object3D[]) => {
    const ids = new Set<string>()
    for (const obj of items) {
      let current: THREE.Object3D | null = obj
      while (current) {
        const id = current.userData?.oceanNodeId
        if (typeof id === 'string') {
          ids.add(id)
          break
        }
        current = current.parent
      }
    }
    if (ids.size === 0) return
    useSelectedNodesStore.getState().add(Array.from(ids))
  }, [])

  // Per-cursor frameloop claims (rover + each current). Distinct ids so one
  // cursor going idle doesn't stop the frame loop while another is gliding.
  const setActiveSource = useRoverActiveStore((s) => s.setActiveSource)
  const setStateCurrentActive = useCallback(
    (a: boolean) => setActiveSource('state_current', a),
    [setActiveSource],
  )
  const setIndexCurrentActive = useCallback(
    (a: boolean) => setActiveSource('index_current', a),
    [setActiveSource],
  )
  const setStalenessCurrentActive = useCallback(
    (a: boolean) => setActiveSource('staleness_current', a),
    [setActiveSource],
  )
  const setHasAnimatedDecorators = useRoverActiveStore((s) => s.setHasAnimatedDecorators)

  // Whenever a node's `states[]` flips, recompute whether *any* node has
  // active states and bubble that up. The flag drives `frameloop="always"`
  // so animated decorators keep ticking. We account for three sources:
  //   - backend states shipped via `ocean-state-changed`,
  //   - client-side stale badges (synthesized in the overlay),
  //   - the open-panel `SecurityPerimeter` (its tape pulses via useFrame).
  // Missing any of these would freeze the corresponding animation at ~1fps.
  const hasOpenNodes = useHasOpenNodes()
  useEffect(() => {
    const hasBackendStates = nodes.some((n) => n.states && n.states.length > 0)
    const hasStaleStates = staleByModuleName.size > 0
    setHasAnimatedDecorators(hasBackendStates || hasStaleStates || hasOpenNodes)
  }, [nodes, staleByModuleName, hasOpenNodes, setHasAnimatedDecorators])

  // Reset the flag if this OceanNodes unmounts (e.g. project switch) so
  // the canvas drops back to demand-frameloop.
  useEffect(() => {
    return () => {
      setHasAnimatedDecorators(false)
      setStateCurrentActive(false)
      setIndexCurrentActive(false)
      setStalenessCurrentActive(false)
    }
  }, [setHasAnimatedDecorators, setStateCurrentActive, setIndexCurrentActive, setStalenessCurrentActive])

  return (
    <>
      {lighthouseIds.map((id) => (
        <IslandTiles key={id} lighthouseId={id} nodes={islandNodes} />
      ))}
      <DragPreviewTiles />
      <SelectionOutlines nodes={nodes} />
      <OceanConnections nodes={nodes} connections={connections} />
      <Select multiple box onChange={handleSelectChange}>
        {nodes.map((n) => (
          <OceanNode
            key={n.module_id}
            id={n.module_id}
            position={cellToWorld(n.col, n.row)}
            label={n.module_name}
            status={(n.node_status || 'loading') as NodeStatus}
            layers={layersForNode(n)}
            variant={n.node_variant}
            onMove={handleMove}
            onDoubleClick={handleClick}
            onContextMenu={handleNodeContextMenuInternal}
            getAllNodes={getAllNodes}
          />
        ))}
      </Select>
      {/* Decorator overlays — siblings of <Select> so they're not picked by
          box-select and so the BaseNode dim/elevation animation doesn't
          fade an active alert when another node is highlighted.
          Mounted for every node: the overlay decides internally whether
          to render anything (backend states OR client-side "panel open"
          perimeter). Skipping on `!states` here would prevent the open-panel
          decorator from ever showing on a clean node. */}
      {nodes.map((n) => (
        <NodeDecoratorOverlay
          key={`deco-${n.module_id}`}
          node={n}
          layers={layersForNode(n)}
          staleSeverity={staleByModuleName.get(n.module_name)}
        />
      ))}
      {/* State Current — node-state scanner (overflow halos, pending-writes
          badges), the old rover folded into the Currents engine. Blue cursor;
          drains the dirty queue so edits reflect promptly. `onArriveAtCell` is
          how state-changes get released into the UI: each halo waits for the
          cursor to physically reach its node before showing up. */}
      <CurrentOverlay
        projectPath={projectPath}
        currentId="state_current"
        color="#60a5fa"
        onActiveChange={setStateCurrentActive}
        onArriveAtCell={(cell) => releasePendingForCell(cell.col, cell.row)}
      />
      {/* Index Current — passive worker keeping the logbook search index
          fresh. Independent cursor from the rover; reuses the same
          frameloop-active flag so movement keeps animating. */}
      <CurrentOverlay
        projectPath={projectPath}
        currentId="index_current"
        onActiveChange={setIndexCurrentActive}
      />
      {/* Staleness Current — passive worker re-hashing code modules to flag
          drift. Amber to read as the code-drift badge color; independent cursor
          from the index current and the rover. */}
      <CurrentOverlay
        projectPath={projectPath}
        currentId="staleness_current"
        color="#fbbf24"
        onActiveChange={setStalenessCurrentActive}
      />
    </>
  )
}

/** Reads the global selection set and renders an outline at every selected
 *  node's cell. Color is derived per node: island color if the node belongs
 *  to a lighthouse cluster, otherwise the generic edge gray. */
function SelectionOutlines({ nodes }: { nodes: OceanNodePosition[] }) {
  const selectedIds = useSelectedNodesStore((s) => s.ids)
  const overrides = useLighthouseColorsStore((s) => s.overrides)
  if (selectedIds.size === 0) return null
  return (
    <>
      {nodes
        .filter((n) => selectedIds.has(n.module_id))
        .map((n) => {
          const color = n.lighthouse_id ? islandColor(n.lighthouse_id, overrides) : NODE_COLORS.edge
          return (
            <NodeSelectionOutline
              key={n.module_id}
              col={n.col}
              row={n.row}
              color={color}
            />
          )
        })}
    </>
  )
}
