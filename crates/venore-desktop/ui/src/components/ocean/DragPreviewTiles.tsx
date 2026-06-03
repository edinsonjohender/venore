// =============================================================================
// DragPreviewTiles — Translucent tiles shown at the destination cells while
// a group of nodes is being dragged.
// =============================================================================
// Green if the cell is free, red if it's blocked by a node outside the group.
// Subscribes to dragPreviewStore so it only renders when a drag is active.

import { memo, useMemo } from 'react'
import * as THREE from 'three'

import { GRID_CONFIG, cellToWorld } from './ocean-config'
import { useDragPreviewStore } from '@/stores/dragPreviewStore'

const TILE_PADDING_RATIO = 0.05
const TILE_CORNER_RATIO = 0.08
const TILE_Y = 0.015 // sits just above island tiles (0.01) and below the outline (0.02)
const COLOR_OK = '#22c55e' // emerald 500 — free cell
const COLOR_BLOCKED = '#ef4444' // red 500 — collision
const OPACITY = 0.45

function roundedSquareShape(size: number, radius: number): THREE.Shape {
  const half = size / 2
  const r = Math.min(radius, half)
  const s = new THREE.Shape()
  s.moveTo(-half + r, -half)
  s.lineTo(half - r, -half)
  s.quadraticCurveTo(half, -half, half, -half + r)
  s.lineTo(half, half - r)
  s.quadraticCurveTo(half, half, half - r, half)
  s.lineTo(-half + r, half)
  s.quadraticCurveTo(-half, half, -half, half - r)
  s.lineTo(-half, -half + r)
  s.quadraticCurveTo(-half, -half, -half + r, -half)
  return s
}

export const DragPreviewTiles = memo(function DragPreviewTiles() {
  const isActive = useDragPreviewStore((s) => s.isActive)
  const cells = useDragPreviewStore((s) => s.cells)

  const shape = useMemo(() => {
    const tileSize = GRID_CONFIG.cellSize * (1 - TILE_PADDING_RATIO * 2)
    const radius = tileSize * TILE_CORNER_RATIO
    return roundedSquareShape(tileSize, radius)
  }, [])

  if (!isActive || cells.length === 0) return null

  return (
    <group>
      {cells.map((cell, i) => {
        const [x, , z] = cellToWorld(cell.col, cell.row)
        const color = cell.ok ? COLOR_OK : COLOR_BLOCKED
        return (
          <mesh
            // eslint-disable-next-line react/no-array-index-key
            key={`${cell.col}-${cell.row}-${i}`}
            position={[x, TILE_Y, z]}
            rotation={[-Math.PI / 2, 0, 0]}
            frustumCulled={false}
            renderOrder={0}
          >
            <shapeGeometry args={[shape]} />
            <meshBasicMaterial
              color={color}
              transparent
              opacity={OPACITY}
              depthWrite
              side={THREE.DoubleSide}
            />
          </mesh>
        )
      })}
    </group>
  )
})
