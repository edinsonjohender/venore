// =============================================================================
// OceanGrid — Floor plane + grid lines
// =============================================================================

import type { ThreeEvent } from '@react-three/fiber'
import { GRID_CONFIG, GRID_TOTAL_SIZE, OCEAN_COLORS } from './ocean-config'

const FLOOR_SIZE = GRID_TOTAL_SIZE * 2 // Floor extends beyond grid lines

interface OceanGridProps {
  /** Forwarded to the floor mesh — captures double clicks on empty space. */
  onFloorDoubleClick?: (event: ThreeEvent<MouseEvent>) => void
  /** Forwarded to the floor mesh — captures right-clicks on empty space. */
  onFloorContextMenu?: (event: ThreeEvent<MouseEvent>) => void
  /** Forwarded to the floor mesh — captures clicks on empty space (used to clear selection). */
  onFloorClick?: (event: ThreeEvent<MouseEvent>) => void
}

export function OceanGrid({
  onFloorDoubleClick,
  onFloorContextMenu,
  onFloorClick,
}: OceanGridProps = {}) {
  const divisions = GRID_CONFIG.oceanGridSize

  return (
    <group>
      {/* Floor plane — larger than grid to avoid edge visibility */}
      <mesh
        rotation-x={-Math.PI / 2}
        position-y={-0.1}
        onDoubleClick={onFloorDoubleClick}
        onContextMenu={onFloorContextMenu}
        onClick={onFloorClick}
      >
        <planeGeometry args={[FLOOR_SIZE, FLOOR_SIZE]} />
        <meshStandardMaterial
          color={OCEAN_COLORS.floorColor}
          transparent
          opacity={OCEAN_COLORS.floorOpacity}
        />
      </mesh>

      {/* Grid lines */}
      <gridHelper
        args={[GRID_TOTAL_SIZE, divisions, OCEAN_COLORS.gridLineColor, OCEAN_COLORS.gridLineColor]}
        position-y={0}
        material-transparent
        material-opacity={OCEAN_COLORS.gridLineOpacity}
      />
    </group>
  )
}
