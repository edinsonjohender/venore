// =============================================================================
// BaseNode — Interaction wrapper for all ocean node variants
// =============================================================================
// Handles: drag (move-node mode), click (navigate mode), hover state,
// floating label, and status indicator glow.
// Body geometry is provided via render prop — variants only define shape.
//
// Drag uses custom implementation (R3F pointerdown + DOM pointermove/up +
// raycast against Y=0 plane) instead of drei DragControls. This gives us
// full control over event propagation — stopPropagation on pointerdown
// prevents the raycaster from activating nodes behind in orthographic view.

import { memo, useRef, useState, useCallback, useEffect, useSyncExternalStore } from 'react'
import { useThree, useFrame, type ThreeEvent } from '@react-three/fiber'
import { Text } from '@react-three/drei'
import * as THREE from 'three'
import { GRID_CONFIG, cellToWorld, snapToCell, worldToCell } from '../ocean-config'
import { getOceanMode, subscribeOceanMode } from '../ocean-mode'
import { STATUS_COLORS } from './colors'
import { noRaycast, type NodeStatus } from './types'
import { useSelectedNodesStore } from '@/stores/selectedNodesStore'
import { useNodeDragStateStore } from '@/stores/nodeDragStateStore'
import { groupDragRegistry } from '@/stores/groupDragRegistry'
import { useDragPreviewStore, type DragPreviewCell } from '@/stores/dragPreviewStore'
import { useHighlightStore, LEVEL_HEIGHTS, DIM_FACTOR } from '@/stores/highlightStore'

interface BaseNodeSnapshot {
  id: string
  col: number
  row: number
}

interface BaseNodeProps {
  id: string
  position: [number, number, number]
  label: string
  status: NodeStatus
  contentHeight: number
  /** Hide the small status-color square at the top — useful for variants like
   *  the lighthouse whose own geometry already carries the status meaning. */
  hideStatusIndicator?: boolean
  onMove?: (id: string, newPos: [number, number, number]) => Promise<boolean>
  /** Single click without modifiers fires this AFTER the selection update —
   *  caller can use it for side effects (rare). With Ctrl/Meta we toggle the
   *  selection and skip onClick. */
  onClick?: (id: string) => void
  /** Double click handler — typically used to open a detail panel. */
  onDoubleClick?: (id: string) => void
  /** Right-click handler — receives the underlying DOM event + the node's current label. */
  onContextMenu?: (id: string, event: ThreeEvent<MouseEvent>, label: string) => void
  /** Returns a fresh snapshot of every node currently on the canvas. Used by
   *  group drag to compute initial positions of siblings and detect collisions. */
  getAllNodes?: () => BaseNodeSnapshot[]
  children: (state: { isHovered: boolean }) => React.ReactNode
}

const { nodeSize } = GRID_CONFIG

// Shared across instances — raycast helpers for drag
// Safe: JS is single-threaded and only one node drags at a time (stopPropagation).
const _raycaster = new THREE.Raycaster()
const _plane = new THREE.Plane(new THREE.Vector3(0, 1, 0), 0) // Y = 0
const _intersection = new THREE.Vector3()
const _ndcCoords = new THREE.Vector2()

export const BaseNode = memo(function BaseNode({
  id,
  position,
  label,
  status,
  contentHeight,
  hideStatusIndicator = false,
  onMove,
  onClick,
  onDoubleClick,
  onContextMenu,
  getAllNodes,
  children,
}: BaseNodeProps) {
  const groupRef = useRef<THREE.Group>(null!)
  const isDragging = useRef(false)
  const dragOffset = useRef({ x: 0, z: 0 })
  const dragCleanup = useRef<(() => void) | null>(null)
  const { invalidate, camera, gl } = useThree()
  const [isHovered, setIsHovered] = useState(false)

  // Cleanup DOM listeners if component unmounts during drag
  useEffect(() => {
    return () => { dragCleanup.current?.() }
  }, [])

  // Register this node's group ref so other nodes can move it imperatively
  // during a group drag. Cleanup on unmount.
  useEffect(() => {
    groupDragRegistry.register(id, groupRef)
    return () => { groupDragRegistry.unregister(id) }
  }, [id])

  // Animate elevation Y + opacity dimming.
  // - Y lerps toward LEVEL_HEIGHTS[currentLevel].
  // - Each mesh's material.opacity is multiplied by an animated dim factor:
  //   1.0 when nothing is highlighted or this node IS elevated, DIM_FACTOR
  //   when something is highlighted and this node is NOT elevated.
  // Reads the store imperatively to avoid re-rendering on every change.
  const baseOpacities = useRef<Map<THREE.Object3D, number>>(new Map())
  const baseTransparencies = useRef<Map<THREE.Object3D, boolean>>(new Map())
  const opacitiesCaptured = useRef(false)
  const currentDim = useRef(1)

  useFrame((_, delta) => {
    if (!groupRef.current) return

    // Capture each mesh's original opacity once. After this we always
    // multiply that base value by `currentDim` instead of writing a new
    // base, so per-frame updates compose cleanly.
    if (!opacitiesCaptured.current) {
      groupRef.current.traverse((child) => {
        const m = (child as { material?: THREE.Material }).material
        if (!m || !('opacity' in m)) return
        baseOpacities.current.set(child, (m as { opacity: number }).opacity)
        baseTransparencies.current.set(child, (m as { transparent: boolean }).transparent)
      })
      opacitiesCaptured.current = true
    }

    // Elevation animation (skip while dragging — drag owns position.y).
    if (!isDragging.current) {
      const level = useHighlightStore.getState().elevations.get(id) ?? 0
      const targetY = LEVEL_HEIGHTS[level] ?? 0
      const currentY = groupRef.current.position.y
      const yDelta = Math.abs(currentY - targetY)
      if (yDelta > 0.001) {
        groupRef.current.position.y = THREE.MathUtils.lerp(currentY, targetY, Math.min(delta * 8, 1))
        invalidate()
      } else if (yDelta > 0) {
        groupRef.current.position.y = targetY
        invalidate()
      }
    }

    // Dimming animation.
    const elevations = useHighlightStore.getState().elevations
    const isHighlightActive = elevations.size > 0
    const isElevated = elevations.has(id)
    const targetDim = isHighlightActive && !isElevated ? DIM_FACTOR : 1
    if (Math.abs(currentDim.current - targetDim) > 0.001) {
      currentDim.current = THREE.MathUtils.lerp(currentDim.current, targetDim, Math.min(delta * 8, 1))
      groupRef.current.traverse((child) => {
        const m = (child as { material?: THREE.Material & { opacity?: number; transparent?: boolean } }).material
        if (!m || !('opacity' in m)) return
        const base = baseOpacities.current.get(child) ?? 1
        ;(m as { opacity: number }).opacity = base * currentDim.current
        ;(m as { transparent: boolean }).transparent =
          (baseTransparencies.current.get(child) ?? false) || currentDim.current < 1
      })
      invalidate()
    }
  })

  const mode = useSyncExternalStore(subscribeOceanMode, getOceanMode)

  const statusColor = STATUS_COLORS[status]

  // --- Helpers: screen → world via plane raycast ---

  const screenToWorld = useCallback(
    (clientX: number, clientY: number): THREE.Vector3 | null => {
      const rect = gl.domElement.getBoundingClientRect()
      _ndcCoords.set(
        ((clientX - rect.left) / rect.width) * 2 - 1,
        -((clientY - rect.top) / rect.height) * 2 + 1,
      )
      _raycaster.setFromCamera(_ndcCoords, camera)
      return _raycaster.ray.intersectPlane(_plane, _intersection) ? _intersection : null
    },
    [camera, gl],
  )

  // --- Drag (move-node mode) via R3F pointerdown + DOM move/up ---

  const handlePointerDown = useCallback(
    (e: ThreeEvent<PointerEvent>) => {
      // Shift+drag is reserved for box-select — let it bubble to drei <Select>.
      if (e.nativeEvent.shiftKey) return

      // Drag-to-move is only available in move-node mode (Figma's "V" tool).
      // In navigate mode (Figma's "H" hand) the canvas pans instead.
      if (mode !== 'move-node') return

      e.stopPropagation()

      isDragging.current = true
      useNodeDragStateStore.getState().setDragging(true)

      // Offset between hit point on Y=0 plane and the anchor's center
      dragOffset.current = {
        x: groupRef.current.position.x - e.point.x,
        z: groupRef.current.position.z - e.point.z,
      }

      // Decide single vs group drag and snapshot initial state.
      const selected = useSelectedNodesStore.getState().ids
      const isGroup = selected.has(id) && selected.size > 1
      const allNodes = getAllNodes ? getAllNodes() : []
      const groupMembers = isGroup
        ? allNodes.filter((n) => selected.has(n.id))
        : []
      // Map of memberId → { initialWorld: {x,z}, initialCell: {col,row} }
      const initialState = new Map<
        string,
        { worldX: number; worldZ: number; col: number; row: number }
      >()
      for (const n of groupMembers) {
        const [wx, , wz] = cellToWorld(n.col, n.row)
        initialState.set(n.id, { worldX: wx, worldZ: wz, col: n.col, row: n.row })
      }

      if (isGroup) {
        useDragPreviewStore.getState().setActive(true)
      }

      const canvas = gl.domElement
      const previewStore = useDragPreviewStore

      const onPointerMove = (domEvent: PointerEvent) => {
        const hit = screenToWorld(domEvent.clientX, domEvent.clientY)
        if (!hit) return

        // Move the anchor visually first
        const anchorX = hit.x + dragOffset.current.x
        const anchorZ = hit.z + dragOffset.current.z
        groupRef.current.position.set(anchorX, 0, anchorZ)

        if (!isGroup) {
          invalidate()
          return
        }

        // Group: replicate the anchor's delta from its initial world position
        // onto every other group member.
        const anchorInitial = initialState.get(id)
        if (!anchorInitial) {
          invalidate()
          return
        }
        const deltaX = anchorX - anchorInitial.worldX
        const deltaZ = anchorZ - anchorInitial.worldZ

        for (const member of groupMembers) {
          if (member.id === id) continue
          const start = initialState.get(member.id)
          const ref = groupDragRegistry.get(member.id)
          if (!start || !ref?.current) continue
          ref.current.position.set(start.worldX + deltaX, 0, start.worldZ + deltaZ)
        }

        // Compute target cells for each group member + collision check.
        const groupIds = new Set(groupMembers.map((n) => n.id))
        const cells: DragPreviewCell[] = groupMembers.map((member) => {
          const start = initialState.get(member.id)!
          const targetX = snapToCell(start.worldX + deltaX)
          const targetZ = snapToCell(start.worldZ + deltaZ)
          const { col, row } = worldToCell(targetX, targetZ)
          const blocker = allNodes.find(
            (n) => !groupIds.has(n.id) && n.col === col && n.row === row,
          )
          return { col, row, ok: !blocker }
        })
        previewStore.getState().setCells(cells)

        invalidate()
      }

      const cleanup = () => {
        isDragging.current = false
        useNodeDragStateStore.getState().setDragging(false)
        if (isGroup) useDragPreviewStore.getState().clear()
        dragCleanup.current = null
        canvas.removeEventListener('pointermove', onPointerMove)
        canvas.removeEventListener('pointerup', onPointerUp)
      }
      dragCleanup.current = cleanup

      const onPointerUp = () => {
        cleanup()

        // Snap anchor to grid and read its final cell.
        const snappedX = snapToCell(groupRef.current.position.x)
        const snappedZ = snapToCell(groupRef.current.position.z)
        groupRef.current.position.set(snappedX, position[1], snappedZ)

        // Snap every other group member visually (so they don't sit at
        // sub-cell pixel positions even if backend rejects).
        if (isGroup) {
          const anchorInitial = initialState.get(id)
          if (anchorInitial) {
            const deltaX = snappedX - anchorInitial.worldX
            const deltaZ = snappedZ - anchorInitial.worldZ
            for (const member of groupMembers) {
              if (member.id === id) continue
              const start = initialState.get(member.id)
              const ref = groupDragRegistry.get(member.id)
              if (!start || !ref?.current) continue
              ref.current.position.set(start.worldX + deltaX, 0, start.worldZ + deltaZ)
            }
          }
        }
        invalidate()

        const moved = snappedX !== position[0] || snappedZ !== position[2]
        if (!moved) return

        onMove?.(id, [snappedX, position[1], snappedZ])?.then((accepted) => {
          if (accepted) return
          // Rejected: spring everything back to its initial position.
          groupRef.current.position.set(position[0], position[1], position[2])
          if (isGroup) {
            for (const member of groupMembers) {
              if (member.id === id) continue
              const start = initialState.get(member.id)
              const ref = groupDragRegistry.get(member.id)
              if (!start || !ref?.current) continue
              ref.current.position.set(start.worldX, 0, start.worldZ)
            }
          }
          invalidate()
        })
      }

      canvas.addEventListener('pointermove', onPointerMove)
      canvas.addEventListener('pointerup', onPointerUp)
    },
    [mode, id, position, onMove, gl, invalidate, screenToWorld, getAllNodes],
  )

  // --- Click (navigate mode) — selection logic ---
  // Without modifiers → replace selection with this node.
  // Ctrl/Meta → toggle this node in/out of the selection.

  const handleClick = useCallback(
    (e: ThreeEvent<MouseEvent>) => {
      // Selection only happens in move-node (Figma's "V"). In navigate (hand)
      // the canvas is meant for panning, not for picking.
      if (mode !== 'move-node') return
      e.stopPropagation()
      const ctrl = e.nativeEvent.ctrlKey || e.nativeEvent.metaKey
      const store = useSelectedNodesStore.getState()
      if (ctrl) {
        store.toggle(id)
      } else {
        store.set([id])
        onClick?.(id)
      }
    },
    [mode, id, onClick],
  )

  // --- Double click — typically opens a detail panel ---

  const handleDoubleClick = useCallback(
    (e: ThreeEvent<MouseEvent>) => {
      if (!onDoubleClick) return
      e.stopPropagation()
      onDoubleClick(id)
    },
    [id, onDoubleClick],
  )

  // --- Context menu (right-click) — works in any mode ---

  const handleContextMenu = useCallback(
    (e: ThreeEvent<MouseEvent>) => {
      if (!onContextMenu) return
      e.stopPropagation()
      e.nativeEvent.preventDefault()
      onContextMenu(id, e, label)
    },
    [id, label, onContextMenu],
  )

  // --- Hover (stopPropagation prevents nodes behind from highlighting) ---

  const handlePointerOver = useCallback((e: ThreeEvent<PointerEvent>) => {
    e.stopPropagation()
    setIsHovered(true)
  }, [])

  const handlePointerOut = useCallback((e: ThreeEvent<PointerEvent>) => {
    e.stopPropagation()
    setIsHovered(false)
  }, [])

  return (
    <group
      ref={groupRef}
      position={position}
      userData={{ oceanNodeId: id }}
      onPointerDown={handlePointerDown}
      onClick={handleClick}
      onDoubleClick={handleDoubleClick}
      onContextMenu={handleContextMenu}
      onPointerOver={handlePointerOver}
      onPointerOut={handlePointerOut}
    >
      {children({ isHovered })}

      {/* Status indicator — glow on top of content (hidden for variants whose
          own geometry carries the status meaning, e.g. the lighthouse lantern) */}
      {!hideStatusIndicator && (
        <mesh
          raycast={noRaycast}
          position={[0, contentHeight + 0.02, 0]}
          rotation={[-Math.PI / 2, 0, 0]}
        >
          <planeGeometry args={[nodeSize * 0.35, nodeSize * 0.35]} />
          <meshStandardMaterial
            color={statusColor}
            emissive={statusColor}
            emissiveIntensity={isHovered ? 0.5 : 0.3}
            transparent
            opacity={0.85}
            side={THREE.DoubleSide}
          />
        </mesh>
      )}

      {/* Floating label */}
      <Text
        raycast={noRaycast}
        position={[0, contentHeight + 0.18, 0]}
        fontSize={0.22}
        color="#e2e8f0"
        anchorX="center"
        anchorY="bottom"
      >
        {label}
      </Text>
    </group>
  )
})
