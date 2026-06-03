// =============================================================================
// ConnectionLine — Bezier curve between two ocean nodes
// =============================================================================
// Two visual variants driven by `kind`:
//   - "dependency" → quiet, low-opacity hint of code deps (white/gray)
//   - "manual"     → user-drawn intent: emerald, higher opacity, arrow head

import { memo, useMemo } from 'react'
import { Line } from '@react-three/drei'
import * as THREE from 'three'
import type { OceanConnectionKind } from '@/lib/tauri'

interface ConnectionLineProps {
  from: { x: number; z: number; height: number }
  to: { x: number; z: number; height: number }
  kind: OceanConnectionKind
  /** Only relevant for `dependency` kind — manual connections are always
   *  rendered directionally regardless of whether the inverse exists. */
  isBidirectional?: boolean
}

const SEGMENTS = 16
const MANUAL_COLOR = '#10b981' // emerald-500
const MANUAL_OPACITY = 0.6
const DEP_OPACITY = 0.15

export const ConnectionLine = memo(function ConnectionLine({
  from,
  to,
  kind,
  isBidirectional = false,
}: ConnectionLineProps) {
  const { points, arrow } = useMemo(() => {
    const start = new THREE.Vector3(from.x, from.height, from.z)
    const end = new THREE.Vector3(to.x, to.height, to.z)

    const dist = start.distanceTo(end)
    const arcHeight = Math.max(2.5, Math.min(dist * 0.6, 6))

    const mid = new THREE.Vector3()
      .addVectors(start, end)
      .multiplyScalar(0.5)
    mid.y = Math.max(start.y, end.y) + arcHeight

    const curve = new THREE.QuadraticBezierCurve3(start, mid, end)
    const pts = curve.getPoints(SEGMENTS)

    // Tangent direction at the end of the curve, used to orient the arrow.
    const tangent = curve.getTangentAt(1).normalize()

    return { points: pts, arrow: { position: end.clone(), tangent } }
  }, [from.x, from.z, from.height, to.x, to.z, to.height])

  if (kind === 'manual') {
    // Position the cone tip slightly back from the node so the geometry isn't
    // hidden inside the box. The cone's default axis is +Y; rotate to face
    // along the tangent.
    const ARROW_LEN = 0.6
    const ARROW_RADIUS = 0.22
    const tipBackOffset = ARROW_LEN * 0.4
    const arrowPos = arrow.position
      .clone()
      .addScaledVector(arrow.tangent, -tipBackOffset)

    const up = new THREE.Vector3(0, 1, 0)
    const quat = new THREE.Quaternion().setFromUnitVectors(up, arrow.tangent)

    // Always-on-top: lineWidth > 1 makes drei use Line2 (mesh-quad), which is
    // subject to depth test → from some camera angles a node was getting drawn
    // after the line and occluding it. Disabling depth test plus a high
    // renderOrder keeps the manual edge visible regardless of viewing angle.
    return (
      <group renderOrder={10}>
        <Line
          points={points}
          color={MANUAL_COLOR}
          transparent
          opacity={MANUAL_OPACITY}
          lineWidth={1.6}
          depthWrite={false}
          depthTest={false}
        />
        <mesh position={arrowPos} quaternion={quat} renderOrder={10}>
          <coneGeometry args={[ARROW_RADIUS, ARROW_LEN, 12]} />
          <meshBasicMaterial
            color={MANUAL_COLOR}
            transparent
            opacity={MANUAL_OPACITY}
            depthWrite={false}
            depthTest={false}
          />
        </mesh>
      </group>
    )
  }

  // Dependency: subtle, with bidir dedup color hint
  const color = isBidirectional ? '#e0e0e0' : '#ffffff'
  return (
    <Line
      points={points}
      color={color}
      transparent
      opacity={DEP_OPACITY}
      lineWidth={1}
      depthWrite={false}
    />
  )
})
