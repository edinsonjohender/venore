// =============================================================================
// StaleBadge — tilted ring above a node whose code drifted from the snapshot
// =============================================================================
// A slow, steady rotation in a muted amber to communicate "out of sync"
// without screaming for attention. Distinct visual rhythm from
// PendingWritesBadge (active/incoming) and OverflowHalo (urgent error): no
// scale breathing, no emissive pulse — just a calm rotating torus.
//
// Severity changes the color: `info` for "code was edited", `warning` for
// "module directory is gone from disk". Both surface the same shape so the
// glyph stays recognizable across both states.

import { useRef } from 'react'
import { useFrame } from '@react-three/fiber'
import * as THREE from 'three'
import { noRaycast } from '../nodes/types'

interface StaleBadgeProps {
  /** Y position above the node where the ring floats. */
  yOffset?: number
  /** 'info' = source changed, 'warning' = module directory missing. */
  severity?: 'info' | 'warning'
}

const COLOR_BY_SEVERITY: Record<'info' | 'warning', string> = {
  info: '#d97706', // amber-600 — muted, distinct from #fbbf24 (pending-writes amber)
  warning: '#b91c1c', // red-700 — darker than #ef4444 (overflow), so missing reads as "permanent gone"
}

export function StaleBadge({ yOffset = 1.0, severity = 'info' }: StaleBadgeProps) {
  const groupRef = useRef<THREE.Group>(null)
  const timeRef = useRef(0)
  const color = COLOR_BY_SEVERITY[severity]

  useFrame((_, delta) => {
    timeRef.current += delta
    if (groupRef.current) {
      // Slow steady spin around the world Y axis — no bob, no scale breathing.
      // The constant motion is the "this is out of date" signal.
      groupRef.current.rotation.y = timeRef.current * 0.35
    }
  })

  return (
    <group ref={groupRef} position={[0, yOffset, 0]} raycast={noRaycast}>
      {/* Tilted ring — torus axis points along Z, then group rotates around Y. */}
      <mesh rotation={[Math.PI / 2.5, 0, 0]} raycast={noRaycast}>
        <torusGeometry args={[0.32, 0.045, 12, 32]} />
        <meshStandardMaterial
          color={color}
          emissive={color}
          emissiveIntensity={0.5}
          metalness={0.2}
          roughness={0.5}
        />
      </mesh>
      {/* Faint soft halo so the ring still reads against dark backgrounds. */}
      <mesh raycast={noRaycast}>
        <sphereGeometry args={[0.52, 16, 16]} />
        <meshBasicMaterial
          color={color}
          transparent
          opacity={0.1}
          depthWrite={false}
        />
      </mesh>
    </group>
  )
}
