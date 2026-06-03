// =============================================================================
// PendingWritesBadge — pulsing sparkle above a node with N pending AI writes
// =============================================================================
// Floats a small octahedron above the node, slowly rotating + pulsing in
// amber. Drives the `pending_writes` NodeStateKind. The count from the
// scanner payload modulates intensity slightly (more pendings → brighter)
// but the badge stays compact: it's a "look here" hint, not a dashboard.

import { useMemo, useRef } from 'react'
import { useFrame } from '@react-three/fiber'
import * as THREE from 'three'
import { noRaycast } from '../nodes/types'

interface PendingWritesBadgeProps {
  /** Y position above the node where the badge floats. */
  yOffset?: number
  /** How many pendings are queued (drives intensity). */
  count?: number
  /** Hex color for the sparkle. Amber by default. */
  color?: string
}

export function PendingWritesBadge({
  yOffset = 1.0,
  count = 1,
  color = '#fbbf24',
}: PendingWritesBadgeProps) {
  const groupRef = useRef<THREE.Group>(null)
  const meshRef = useRef<THREE.Mesh>(null)
  const matRef = useRef<THREE.MeshStandardMaterial>(null)
  const timeRef = useRef(0)

  // Brighter base + tighter bob the more pendings stack up — but cap so a
  // node with 14 pendings doesn't blind the canvas.
  const intensityCap = useMemo(() => Math.min(1.6, 0.8 + count * 0.08), [count])

  useFrame((_, delta) => {
    timeRef.current += delta
    if (groupRef.current) {
      // Slow rotation + gentle vertical bob — the bob amplitude widens
      // slightly with count, again capped.
      groupRef.current.rotation.y = timeRef.current * 0.6
      const bobAmplitude = Math.min(0.18, 0.08 + count * 0.012)
      groupRef.current.position.y = yOffset + Math.sin(timeRef.current * 1.4) * bobAmplitude
    }
    if (meshRef.current) {
      // Subtle scale breathing.
      const s = 1 + Math.sin(timeRef.current * 2.1) * 0.08
      meshRef.current.scale.setScalar(s)
    }
    if (matRef.current) {
      // Pulse emissive between ~0.4 and intensityCap so it stays visible
      // even at the trough.
      const pulse = 0.4 + (Math.sin(timeRef.current * 2.1) * 0.5 + 0.5) * (intensityCap - 0.4)
      matRef.current.emissiveIntensity = pulse
    }
  })

  return (
    <group ref={groupRef} position={[0, yOffset, 0]} raycast={noRaycast}>
      <mesh ref={meshRef} raycast={noRaycast}>
        <octahedronGeometry args={[0.22, 0]} />
        <meshStandardMaterial
          ref={matRef}
          color={color}
          emissive={color}
          emissiveIntensity={0.8}
          metalness={0.1}
          roughness={0.4}
        />
      </mesh>
      {/* Soft halo billboard so the sparkle reads on dark backgrounds. */}
      <mesh raycast={noRaycast}>
        <sphereGeometry args={[0.42, 16, 16]} />
        <meshBasicMaterial
          color={color}
          transparent
          opacity={0.18}
          depthWrite={false}
        />
      </mesh>
    </group>
  )
}
