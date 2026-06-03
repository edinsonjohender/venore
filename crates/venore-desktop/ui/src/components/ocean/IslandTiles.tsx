// =============================================================================
// IslandTiles — Decorative tiles that paint the territory of one island.
// =============================================================================
// Each tile is a flat (zero-volume) rounded square laid horizontally on the
// grid floor. Tiles do NOT represent nodes — they only signal "this stretch
// of the Ocean belongs to this island". Built with a 2D Shape extruded into
// a plane geometry so it sits truly flat and aligned with the grid.

import { memo, useMemo } from 'react'
import * as THREE from 'three'

import { GRID_CONFIG, cellToWorld } from './ocean-config'
import {
  computeIslandTiles,
  islandColor,
  type NodePosition,
} from './island-utils'
import { useHighlightStore, DIM_FACTOR } from '@/stores/highlightStore'
import { useLighthouseColorsStore } from '@/stores/lighthouseColorsStore'

interface IslandTilesProps {
  lighthouseId: string
  nodes: NodePosition[]
}

const TILE_PADDING_RATIO = 0.05 // 5% gap — tiles nearly fill the cell
const TILE_CORNER_RATIO = 0.08 // corner radius — gentle, not pillow-shaped
const TILE_Y = 0.01 // sit on the grid plane (Y=0); below tiles in isometric view drift down-right
const NODE_TILE_OPACITY = 0.4
const PATH_TILE_OPACITY = 0.16

/** Build a 2D rounded-square Shape centered at origin. */
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

export const IslandTiles = memo(function IslandTiles({ lighthouseId, nodes }: IslandTilesProps) {
  const tiles = useMemo(
    () => computeIslandTiles(lighthouseId, nodes),
    [lighthouseId, nodes],
  )
  const overrides = useLighthouseColorsStore((s) => s.overrides)
  const color = useMemo(
    () => islandColor(lighthouseId, overrides),
    [lighthouseId, overrides],
  )

  // When some other island is highlighted, dim our tiles to match how the
  // nodes themselves are dimmed. Subscribed reactively because the change
  // is discrete (selection toggled), not continuous.
  const elevations = useHighlightStore((s) => s.elevations)
  const isHighlightActive = elevations.size > 0
  const isThisIslandElevated = elevations.has(lighthouseId)
  const islandDim = isHighlightActive && !isThisIslandElevated ? DIM_FACTOR : 1

  // Pre-build the rounded-square Shape once. We hand it to a fresh
  // <shapeGeometry> per tile so each mesh gets its own bounding box and
  // frustum culling stays accurate (sharing a single ShapeGeometry across
  // many positions caused tiles to be culled together as if they sat at
  // the first mesh's position).
  const shape = useMemo(() => {
    const tileSize = GRID_CONFIG.cellSize * (1 - TILE_PADDING_RATIO * 2)
    const radius = tileSize * TILE_CORNER_RATIO
    return roundedSquareShape(tileSize, radius)
  }, [])

  if (tiles.length === 0) return null

  return (
    <group>
      {tiles.map((tile) => {
        const [x, , z] = cellToWorld(tile.col, tile.row)
        const baseOpacity = tile.kind === 'node' ? NODE_TILE_OPACITY : PATH_TILE_OPACITY
        const opacity = baseOpacity * islandDim
        return (
          <mesh
            key={`${tile.col}-${tile.row}`}
            position={[x, TILE_Y, z]}
            rotation={[-Math.PI / 2, 0, 0]}
            frustumCulled={false}
            renderOrder={-1}
          >
            <shapeGeometry args={[shape]} />
            <meshBasicMaterial
              color={color}
              transparent
              opacity={opacity}
              depthWrite
              side={THREE.DoubleSide}
            />
          </mesh>
        )
      })}
    </group>
  )
})
