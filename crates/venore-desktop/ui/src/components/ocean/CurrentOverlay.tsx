// =============================================================================
// CurrentOverlay — passive-worker cursor for an Ocean Current
// =============================================================================
// Listens to `ocean-current-progress` events for ONE current (matched by
// `currentId`) and animates a ScannerRover from cell to cell along the grid
// axes. Multiple CurrentOverlay instances (one per current_id) can run at once,
// each tracking its own independent cursor — this is the visible "passive
// worker navigating the Ocean".
//
// Every Ocean cursor is a CurrentOverlay, including the node-state scanner (the
// old rover, now `state_current`); instances differ only by the current_id
// filter, the marker color, and whether they wire `onArriveAtCell`.

import { useCallback, useEffect, useRef, useState } from 'react'
import { useFrame, useThree } from '@react-three/fiber'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import * as THREE from 'three'
import { ScannerRover } from './decorators/ScannerRover'
import { cellToWorld, worldToCell } from './ocean-config'
import type { GridCellDto, OceanCurrentProgressEvent } from '@/lib/tauri'

interface CurrentOverlayProps {
  projectPath: string
  /** Which current to track (e.g. "index_current"). Events for other
   *  currents are ignored so each overlay owns exactly one cursor. */
  currentId: string
  /** Marker color — lets distinct currents read as distinct on the canvas. */
  color?: string
  /** Bumped when this current becomes visible/idle so the parent can keep
   *  the canvas frameloop alive while the cursor is moving. */
  onActiveChange?: (active: boolean) => void
  /** Fired when the cursor physically reaches a cell (visual landing). Used by
   *  the state current so node-state halos appear exactly when the scanner
   *  arrives, decoupled from backend tick timing — mirrors the old rover. */
  onArriveAtCell?: (cell: GridCellDto) => void
}

const TRAVEL_SPEED = 24
const SNAP_THRESHOLD = 0.05

export function CurrentOverlay({
  projectPath,
  currentId,
  color = '#34d399',
  onActiveChange,
  onArriveAtCell,
}: CurrentOverlayProps) {
  const groupRef = useRef<THREE.Group | null>(null)
  const pathRef = useRef<Array<{ x: number; z: number }>>([])
  const currentPosRef = useRef<{ x: number; z: number } | null>(null)
  // Latest callback in a ref so useFrame doesn't capture a stale closure.
  const onArriveRef = useRef<typeof onArriveAtCell>(onArriveAtCell)
  useEffect(() => {
    onArriveRef.current = onArriveAtCell
  }, [onArriveAtCell])
  const [visible, setVisible] = useState(false)
  const { invalidate } = useThree()

  const setGroupRef = useCallback((node: THREE.Group | null) => {
    groupRef.current = node
    if (node && currentPosRef.current) {
      node.position.set(currentPosRef.current.x, 0, currentPosRef.current.z)
    }
  }, [])

  useEffect(() => {
    let unlisten: UnlistenFn | null = null
    let cancelled = false

    listen<OceanCurrentProgressEvent>('ocean-current-progress', (event) => {
      if (cancelled) return
      const p = event.payload
      if (p.project_path !== projectPath) return
      if (p.current_id !== currentId) return

      if (p.idle || !p.target_cell) {
        pathRef.current = []
        setVisible(false)
        onActiveChange?.(false)
        invalidate()
        return
      }

      if (!currentPosRef.current) {
        const seed = p.current_cell ?? p.target_cell
        const [sx, , sz] = cellToWorld(seed.col, seed.row)
        currentPosRef.current = { x: sx, z: sz }
      }

      pathRef.current = buildElbowPath(currentPosRef.current, p.target_cell)
      setVisible(true)
      onActiveChange?.(true)
      // First-event seed lands directly on the target → empty path, so useFrame
      // won't fire the arrival. Emit it here so buffered state still releases.
      if (pathRef.current.length === 0) {
        onArriveRef.current?.(p.target_cell)
      }
      invalidate()
    })
      .then((fn) => {
        if (cancelled) {
          fn()
          return
        }
        unlisten = fn
      })
      .catch((err) => {
        console.error('Failed to listen for current progress:', err)
      })

    return () => {
      cancelled = true
      if (unlisten) unlisten()
    }
  }, [projectPath, currentId, onActiveChange, invalidate])

  useFrame((_, delta) => {
    if (!groupRef.current || !currentPosRef.current) return
    if (pathRef.current.length === 0) return

    const next = pathRef.current[0]
    const dx = next.x - currentPosRef.current.x
    const dz = next.z - currentPosRef.current.z
    const dist = Math.hypot(dx, dz)

    if (dist < SNAP_THRESHOLD) {
      currentPosRef.current.x = next.x
      currentPosRef.current.z = next.z
      pathRef.current.shift()
      // Final waypoint consumed → cursor physically arrived. Notify so
      // consumers can sync visual state (halo apply) to the actual landing.
      if (pathRef.current.length === 0) {
        const arrivedCell = worldToCell(
          currentPosRef.current.x,
          currentPosRef.current.z,
        )
        onArriveRef.current?.(arrivedCell)
      }
    } else {
      const step = Math.min(delta * TRAVEL_SPEED, dist)
      currentPosRef.current.x += (dx / dist) * step
      currentPosRef.current.z += (dz / dist) * step
    }

    groupRef.current.position.set(
      currentPosRef.current.x,
      0,
      currentPosRef.current.z,
    )
  })

  if (!visible) return null

  return (
    <group ref={setGroupRef}>
      <ScannerRover positions={[{ x: 0, z: 0 }]} color={color} />
    </group>
  )
}

/** L-shaped path from a world coord to a target cell: travel along X first,
 *  then Z — keeps the cursor grid-aligned in the isometric projection. */
function buildElbowPath(
  from: { x: number; z: number },
  toCell: GridCellDto,
): Array<{ x: number; z: number }> {
  const target = cellToWorld(toCell.col, toCell.row)
  const finalPoint = { x: target[0], z: target[2] }

  if (
    Math.abs(finalPoint.x - from.x) < SNAP_THRESHOLD &&
    Math.abs(finalPoint.z - from.z) < SNAP_THRESHOLD
  ) {
    return []
  }

  const path: Array<{ x: number; z: number }> = []
  if (
    Math.abs(finalPoint.x - from.x) > SNAP_THRESHOLD &&
    Math.abs(finalPoint.z - from.z) > SNAP_THRESHOLD
  ) {
    path.push({ x: finalPoint.x, z: from.z })
  }
  path.push(finalPoint)
  return path
}
