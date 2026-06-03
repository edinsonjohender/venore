// =============================================================================
// BuoyNode — Cluster of mini buildings for utilities / helpers / constants
// =============================================================================
// Direct port from v1 (`venore/src/components/canvas/ocean/nodes/BuoyNode.tsx`).
// Minor adaptations:
//   - GRID_CONFIG comes from `ocean-config` (same keys: nodeSize/layerHeight/layerGap).
//   - The status record changes from v1 (stable/critical/...) to v2 (fresh/stale/...).
//   - NODE_COLORS lives under `nodes/colors.ts` with the same body/bodyLight/edge fields.
// The tall tower stays at (0,0) so connections anchor to it.

import type { ThreeEvent } from '@react-three/fiber'
import * as THREE from 'three'
import { GRID_CONFIG } from '../ocean-config'
import { NODE_COLORS, STATUS_COLORS } from './colors'
import type { NodeStatus } from './types'

interface BuoyNodeProps {
  status: NodeStatus
  isSelected?: boolean
  isHovered?: boolean
  onClick?: (e: ThreeEvent<MouseEvent>) => void
  onPointerOver?: (e: ThreeEvent<PointerEvent>) => void
  onPointerOut?: () => void
}

// Layout of the 3 mini buildings. Tall tower at exact center (0,0) so
// connections anchor to the main building's status.
const BUILDINGS = [
  { x: -0.28, z: 0.22, layers: 1 }, // small — back-left
  { x: 0.28, z: 0.22, layers: 2 },  // medium — back-right
  { x: 0, z: 0, layers: 3 },        // tall — center (connections anchor here)
]

const SCALE = 0.7

export function BuoyNode({
  status,
  isSelected = false,
  isHovered = false,
  onClick,
  onPointerOver,
  onPointerOut,
}: BuoyNodeProps) {
  const { nodeSize, layerHeight, layerGap } = GRID_CONFIG
  const statusColor = STATUS_COLORS[status]
  const bodyColor = isHovered || isSelected ? NODE_COLORS.bodyLight : NODE_COLORS.body

  const buildingSize = nodeSize * 0.28
  const scaledLayerHeight = layerHeight * SCALE
  const scaledLayerGap = layerGap * SCALE

  return (
    <group>
      {BUILDINGS.map((building, buildingIndex) => {
        const isMainBuilding = buildingIndex === BUILDINGS.length - 1

        return (
          <group key={buildingIndex} position={[building.x, 0, building.z]}>
            {Array.from({ length: building.layers }).map((_, layerIndex) => {
              const y = layerIndex * (scaledLayerHeight + scaledLayerGap) + scaledLayerHeight / 2
              const isTopLayer = layerIndex === building.layers - 1

              return (
                <group key={layerIndex} position={[0, y, 0]}>
                  <mesh
                    onClick={onClick}
                    onPointerOver={onPointerOver}
                    onPointerOut={onPointerOut}
                    castShadow
                    receiveShadow
                    userData={{ baseOpacity: 1 }}
                  >
                    <boxGeometry args={[buildingSize, scaledLayerHeight, buildingSize]} />
                    <meshStandardMaterial
                      color={bodyColor}
                      metalness={0.1}
                      roughness={0.8}
                      transparent
                      opacity={1}
                    />
                  </mesh>

                  {isMainBuilding && isTopLayer && (
                    <mesh
                      position={[0, scaledLayerHeight / 2 + 0.02, 0]}
                      rotation={[-Math.PI / 2, 0, 0]}
                      userData={{ baseOpacity: 0.85 }}
                    >
                      <planeGeometry args={[buildingSize * 0.6, buildingSize * 0.6]} />
                      <meshStandardMaterial
                        color={statusColor}
                        emissive={statusColor}
                        emissiveIntensity={isHovered || isSelected ? 0.5 : 0.3}
                        metalness={0.2}
                        roughness={0.3}
                        transparent
                        opacity={0.85}
                      />
                    </mesh>
                  )}

                  <lineSegments userData={{ baseOpacity: isHovered || isSelected ? 0.8 : 0.5 }}>
                    <edgesGeometry args={[new THREE.BoxGeometry(buildingSize, scaledLayerHeight, buildingSize)]} />
                    <lineBasicMaterial
                      color={NODE_COLORS.edge}
                      transparent
                      opacity={isHovered || isSelected ? 0.8 : 0.5}
                    />
                  </lineSegments>
                </group>
              )
            })}
          </group>
        )
      })}
    </group>
  )
}

/** Cluster height — useful for anchoring connections to the tall building's roof. */
export function calculateBuoyHeight(): number {
  const { layerHeight, layerGap } = GRID_CONFIG
  const maxLayers = Math.max(...BUILDINGS.map((b) => b.layers))
  return maxLayers * (layerHeight * SCALE + layerGap * SCALE)
}
