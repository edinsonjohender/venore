// =============================================================================
// NodeSelectionOutline — Thin square outline marking a selected node's cell.
// =============================================================================
// Sits in between the cell border (100%) and the island tile (90%) so it
// reads as a clear "this cell is selected" marker without overlapping
// either. Color is supplied by the parent (island color or generic edge gray).

import { memo, useMemo } from 'react'
import * as THREE from 'three'

import { GRID_CONFIG, cellToWorld } from './ocean-config'

interface NodeSelectionOutlineProps {
  col: number
  row: number
  color: string
}

const OUTLINE_PADDING_RATIO = 0.03 // 3% padding per side → outline at ~94% of cellSize
const OUTLINE_Y = 0.02 // just above the island tiles (which sit at 0.01)

export const NodeSelectionOutline = memo(function NodeSelectionOutline({
  col,
  row,
  color,
}: NodeSelectionOutlineProps) {
  const [x, , z] = cellToWorld(col, row)

  const geometry = useMemo(() => {
    const size = GRID_CONFIG.cellSize * (1 - OUTLINE_PADDING_RATIO * 2)
    const half = size / 2
    // Pairs of points → each pair is a segment; four segments form the square.
    const points = [
      new THREE.Vector3(-half, 0, -half), new THREE.Vector3(half, 0, -half),
      new THREE.Vector3(half, 0, -half), new THREE.Vector3(half, 0, half),
      new THREE.Vector3(half, 0, half), new THREE.Vector3(-half, 0, half),
      new THREE.Vector3(-half, 0, half), new THREE.Vector3(-half, 0, -half),
    ]
    return new THREE.BufferGeometry().setFromPoints(points)
  }, [])

  return (
    <lineSegments
      position={[x, OUTLINE_Y, z]}
      frustumCulled={false}
      renderOrder={2}
    >
      <primitive object={geometry} attach="geometry" />
      <lineBasicMaterial color={color} transparent opacity={0.95} depthWrite={false} />
    </lineSegments>
  )
})
