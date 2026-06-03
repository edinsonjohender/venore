// =============================================================================
// LighthouseBody — Tall slim box + status-colored light at the top
// =============================================================================
// Port of v1's LighthouseNode (canvas/ocean/nodes/LighthouseNode.tsx). The
// lighthouse is the anchor of an island. A slim, tall, plain box (no rounded
// corners) with a horizontal emissive plane on top whose color reflects the
// node's status — exactly the same status semantics as a regular node, but
// scaled up vertically so it reads as the cluster's gateway.

import { memo, useMemo } from 'react'
import * as THREE from 'three'
import { GRID_CONFIG } from '../ocean-config'
import { NODE_COLORS, STATUS_COLORS } from './colors'
import { noRaycast, type NodeStatus } from './types'

interface LighthouseBodyProps {
  status: NodeStatus
  isHovered: boolean
}

const { nodeSize, layerHeight, layerGap } = GRID_CONFIG

// Dimensions — direct port of v1's LIGHTHOUSE_CONFIG
const WIDTH_RATIO = 0.35
const DEPTH_RATIO = 0.35
const HEIGHT_MULTIPLIER = 7

const TOWER_WIDTH = nodeSize * WIDTH_RATIO
const TOWER_DEPTH = nodeSize * DEPTH_RATIO
export const LIGHTHOUSE_BODY_HEIGHT =
  HEIGHT_MULTIPLIER * (layerHeight + layerGap)

// Total height including the small status-color light plane on top.
// Used by OceanNode to position the floating label above the lantern.
export const LIGHTHOUSE_TOTAL_HEIGHT = LIGHTHOUSE_BODY_HEIGHT + 0.04

// Stripe gradient — same idea as LayerMesh but tuned for a tall body:
// visible only in the bottom ~12% of the tower, low opacity, lower intensity.
// Goal: a subtle color hint at the base, NOT a neon stripe.
const TOWER_HALF = (LIGHTHOUSE_BODY_HEIGHT / 2).toFixed(6)
const TOWER_FULL = LIGHTHOUSE_BODY_HEIGHT.toFixed(6)

const STRIPE_VERT = /* glsl */ `
  varying float vFade;
  void main() {
    float t = (position.y + ${TOWER_HALF}) / ${TOWER_FULL};
    vFade = pow(clamp(1.0 - t * 8.0, 0.0, 1.0), 2.0) * 0.45;
    gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
  }
`

const STRIPE_FRAG = /* glsl */ `
  uniform vec3 uColor;
  varying float vFade;
  void main() {
    gl_FragColor = vec4(uColor * 1.1, vFade);
  }
`

export const LighthouseBody = memo(function LighthouseBody({
  status,
  isHovered,
}: LighthouseBodyProps) {
  const bodyColor = isHovered ? NODE_COLORS.bodyLight : NODE_COLORS.body
  const statusColor = STATUS_COLORS[status]
  const glowIntensity = isHovered ? 0.5 : 0.3
  const edgeOpacity = isHovered ? 0.8 : 0.5

  const stripeUniforms = useMemo(
    () => ({ uColor: { value: new THREE.Color(statusColor) } }),
    [statusColor],
  )

  return (
    <group>
      {/* Tower body — plain rectangular box, slim and tall */}
      <mesh position={[0, LIGHTHOUSE_BODY_HEIGHT / 2, 0]} castShadow receiveShadow>
        <boxGeometry args={[TOWER_WIDTH, LIGHTHOUSE_BODY_HEIGHT, TOWER_DEPTH]} />
        <meshStandardMaterial
          color={bodyColor}
          metalness={0.1}
          roughness={0.8}
          polygonOffset
          polygonOffsetFactor={1}
          polygonOffsetUnits={1}
        />
      </mesh>

      {/* Stripe gradient — status color strong at the base, fades upward */}
      <mesh position={[0, LIGHTHOUSE_BODY_HEIGHT / 2, 0]} raycast={noRaycast}>
        <boxGeometry args={[TOWER_WIDTH, LIGHTHOUSE_BODY_HEIGHT, TOWER_DEPTH]} />
        <shaderMaterial
          transparent
          depthWrite={false}
          uniforms={stripeUniforms}
          vertexShader={STRIPE_VERT}
          fragmentShader={STRIPE_FRAG}
        />
      </mesh>

      {/* Status-colored light at the top — horizontal plane, ~80% of the body face */}
      <mesh
        position={[0, LIGHTHOUSE_BODY_HEIGHT + 0.02, 0]}
        rotation={[-Math.PI / 2, 0, 0]}
        raycast={noRaycast}
      >
        <planeGeometry args={[TOWER_WIDTH * 0.8, TOWER_DEPTH * 0.8]} />
        <meshStandardMaterial
          color={statusColor}
          emissive={statusColor}
          emissiveIntensity={glowIntensity}
          metalness={0.2}
          roughness={0.3}
          transparent
          opacity={0.85}
          side={THREE.DoubleSide}
        />
      </mesh>

      {/* Edge wireframe — outlines the tower against dark backgrounds */}
      <lineSegments
        position={[0, LIGHTHOUSE_BODY_HEIGHT / 2, 0]}
        raycast={noRaycast}
      >
        <edgesGeometry args={[new THREE.BoxGeometry(TOWER_WIDTH, LIGHTHOUSE_BODY_HEIGHT, TOWER_DEPTH)]} />
        <lineBasicMaterial
          color={NODE_COLORS.edge}
          transparent
          opacity={edgeOpacity}
        />
      </lineSegments>
    </group>
  )
})
