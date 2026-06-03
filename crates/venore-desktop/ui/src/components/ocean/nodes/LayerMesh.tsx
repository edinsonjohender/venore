// =============================================================================
// LayerMesh — Single layer: body + type stripe + type indicator + edge wireframe
// =============================================================================

import { memo, useMemo } from 'react'
import { RoundedBox } from '@react-three/drei'
import * as THREE from 'three'
import { GRID_CONFIG } from '../ocean-config'
import { LAYER_COLORS, LAYER_STATUS_TINT, NODE_COLORS } from './colors'
import { noRaycast, type NodeLayer } from './types'

interface LayerMeshProps {
  layer: NodeLayer
  index: number
  totalLayers: number
  isHovered: boolean
}

const { nodeSize, layerHeight, layerGap } = GRID_CONFIG
const LAYER_HALF = (layerHeight / 2).toFixed(6)

// Shared EdgesGeometry — all LayerMesh use identical nodeSize, so one instance suffices
const _sharedBox = new THREE.BoxGeometry(nodeSize - 0.04, layerHeight - 0.02, nodeSize - 0.04)
const SHARED_EDGES_GEOMETRY = new THREE.EdgesGeometry(_sharedBox)
_sharedBox.dispose()

// Gradient overlay: color band at the base, fades to 0 at ~20% height
// t * 5.0 compresses the fade to the bottom portion only
const STRIPE_VERT = /* glsl */ `
  varying float vFade;
  void main() {
    float t = (position.y + ${LAYER_HALF}) / ${layerHeight.toFixed(6)};
    vFade = pow(clamp(1.0 - t * 5.0, 0.0, 1.0), 1.5) * 0.85;
    gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
  }
`
const STRIPE_FRAG = /* glsl */ `
  uniform vec3 uColor;
  varying float vFade;
  void main() {
    gl_FragColor = vec4(uColor * 1.6, vFade);
  }
`

export const LayerMesh = memo(function LayerMesh({ layer, index, totalLayers, isHovered }: LayerMeshProps) {
  const y = index * (layerHeight + layerGap) + layerHeight / 2
  const size = nodeSize
  const isTopLayer = index === totalLayers - 1

  // Stripe color: blend layer type color with status tint (30% tint for partial/in-progress)
  const baseColor = LAYER_COLORS[layer.type]
  const tint = LAYER_STATUS_TINT[layer.status as keyof typeof LAYER_STATUS_TINT]
  const typeColor = useMemo(() => {
    if (!tint) return baseColor
    const base = new THREE.Color(baseColor)
    const tintColor = new THREE.Color(tint)
    base.lerp(tintColor, 0.3)
    return '#' + base.getHexString()
  }, [baseColor, tint])

  const stripeUniforms = useMemo(
    () => ({ uColor: { value: new THREE.Color(typeColor) } }),
    [typeColor],
  )

  const bodyColor = NODE_COLORS.body
  const bodyHoverColor = NODE_COLORS.bodyLight

  return (
    <group position={[0, y, 0]}>
      {/* Body */}
      <RoundedBox args={[size, layerHeight, size]} radius={0.06} smoothness={2}>
        <meshStandardMaterial
          color={isHovered ? bodyHoverColor : bodyColor}
          metalness={0.1}
          roughness={0.8}
          polygonOffset
          polygonOffsetFactor={1}
          polygonOffsetUnits={1}
        />
      </RoundedBox>

      {/* Type stripe — gradient overlay, strong at base fading upward */}
      <RoundedBox raycast={noRaycast} args={[size, layerHeight, size]} radius={0.06} smoothness={2}>
        <shaderMaterial
          transparent
          depthWrite={false}
          uniforms={stripeUniforms}
          vertexShader={STRIPE_VERT}
          fragmentShader={STRIPE_FRAG}
        />
      </RoundedBox>

      {/* Type indicator — small square on top face (only on topmost layer) */}
      {isTopLayer && (
        <mesh raycast={noRaycast} position={[0, layerHeight / 2 + 0.01, 0]} rotation={[-Math.PI / 2, 0, 0]}>
          <planeGeometry args={[size * 0.22, size * 0.22]} />
          <meshStandardMaterial
            color={typeColor}
            emissive={typeColor}
            emissiveIntensity={0.3}
            transparent
            opacity={0.85}
            side={THREE.DoubleSide}
          />
        </mesh>
      )}

      {/* Edge wireframe — shared geometry across all LayerMesh instances */}
      <lineSegments raycast={noRaycast} geometry={SHARED_EDGES_GEOMETRY} renderOrder={1}>
        <lineBasicMaterial
          color={NODE_COLORS.edge}
          transparent
          opacity={isHovered ? 0.8 : 0.5}
        />
      </lineSegments>
    </group>
  )
})
