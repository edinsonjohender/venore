// =============================================================================
// CylinderNode — Stacked cylinders for external services / APIs / DBs
// =============================================================================
// Direct port from v1 (`venore/src/components/canvas/ocean/nodes/CylinderNode.tsx`).
// Minor adaptations:
//   - GRID_CONFIG comes from `ocean-config`.
//   - The status record changes from v1 (stable/critical/...) to v2 (fresh/stale/...).
//   - NODE_COLORS and the field names match v2 1:1.

import type { ThreeEvent } from '@react-three/fiber'
import * as THREE from 'three'
import { GRID_CONFIG } from '../ocean-config'
import { NODE_COLORS, STATUS_COLORS } from './colors'
import type { NodeStatus } from './types'

interface CylinderNodeProps {
  status: NodeStatus
  isSelected?: boolean
  isHovered?: boolean
  onClick?: (e: ThreeEvent<MouseEvent>) => void
  onPointerOver?: (e: ThreeEvent<PointerEvent>) => void
  onPointerOut?: () => void
}

const LAYER_COUNT = 2
const SCALE = 0.75

export function CylinderNode({
  status,
  isSelected = false,
  isHovered = false,
  onClick,
  onPointerOver,
  onPointerOut,
}: CylinderNodeProps) {
  const { nodeSize, layerHeight, layerGap } = GRID_CONFIG
  const statusColor = STATUS_COLORS[status]
  const bodyColor = isHovered || isSelected ? NODE_COLORS.bodyLight : NODE_COLORS.body

  const radius = (nodeSize * 0.5) * SCALE
  const scaledLayerHeight = layerHeight * SCALE
  const scaledLayerGap = layerGap * SCALE

  return (
    <group>
      {Array.from({ length: LAYER_COUNT }).map((_, layerIndex) => {
        const y = layerIndex * (scaledLayerHeight + scaledLayerGap) + scaledLayerHeight / 2
        const isTopLayer = layerIndex === LAYER_COUNT - 1

        return (
          <group key={layerIndex} position={[0, y, 0]}>
            {/* Cuerpo del cilindro */}
            <mesh
              onClick={onClick}
              onPointerOver={onPointerOver}
              onPointerOut={onPointerOut}
              castShadow
              receiveShadow
              userData={{ baseOpacity: 1 }}
            >
              <cylinderGeometry args={[radius, radius, scaledLayerHeight, 32]} />
              <meshStandardMaterial
                color={bodyColor}
                metalness={0.1}
                roughness={0.8}
                transparent
                opacity={1}
              />
            </mesh>

            {/* Anillo superior (suaviza el borde del cilindro) */}
            <mesh
              position={[0, scaledLayerHeight / 2, 0]}
              rotation={[-Math.PI / 2, 0, 0]}
              userData={{ baseOpacity: isHovered || isSelected ? 0.4 : 0.2 }}
            >
              <ringGeometry args={[radius * 0.85, radius, 32]} />
              <meshStandardMaterial
                color={NODE_COLORS.edge}
                transparent
                opacity={isHovered || isSelected ? 0.4 : 0.2}
                side={THREE.DoubleSide}
              />
            </mesh>

            {/* Status disc en la capa superior */}
            {isTopLayer && (
              <mesh
                position={[0, scaledLayerHeight / 2 + 0.02, 0]}
                rotation={[-Math.PI / 2, 0, 0]}
                userData={{ baseOpacity: 0.85 }}
              >
                <circleGeometry args={[radius * 0.5, 32]} />
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

            {/* Edges verticales del cilindro */}
            <lineSegments userData={{ baseOpacity: isHovered || isSelected ? 0.8 : 0.5 }}>
              <edgesGeometry args={[new THREE.CylinderGeometry(radius, radius, scaledLayerHeight, 32)]} />
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
}

/** Cylinder height — useful for anchoring connections to the top disc. */
export function calculateCylinderHeight(): number {
  const { layerHeight, layerGap } = GRID_CONFIG
  return LAYER_COUNT * (layerHeight * SCALE + layerGap * SCALE)
}
