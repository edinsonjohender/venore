// =============================================================================
// OverflowHalo — Horizontal rings wrapping a saturated node
// =============================================================================
// Stacked horizontal squares that surround the pillar, animated rising and
// falling like a sine wave. The first decorator wired to the state contract —
// applied when a knowledge_node enters `NodeStateKind::Overflow` (too many
// sections / chars). Originally ported from v1 as "BoxingRing"; renamed to
// reflect the intent (perimeter overflow alert, not sports analogy).
//
// Source: venore (v1) `src/components/_experiments/BoxingRing.tsx`.

import { useRef, useMemo } from 'react'
import { useFrame } from '@react-three/fiber'
import * as THREE from 'three'
import { noRaycast } from '../nodes/types'

interface OverflowHaloProps {
  size?: number
  color?: string
  ringCount?: number
  baseHeight?: number
  spacing?: number
  speed?: number
  intensity?: number
}

export function OverflowHalo({
  size = 2.4,
  color = '#f87171',
  ringCount = 4,
  baseHeight = 0.5,
  spacing = 0.6,
  speed = 2,
  intensity = 1,
}: OverflowHaloProps) {
  const linesRef = useRef<THREE.LineLoop[]>([])
  const timeRef = useRef(0)

  const ringData = useMemo(() => {
    const data: { baseY: number; phaseOffset: number }[] = []
    for (let i = 0; i < ringCount; i++) {
      const baseY = baseHeight + (i * spacing)
      const phaseOffset = (i / ringCount) * Math.PI
      data.push({ baseY, phaseOffset })
    }
    return data
  }, [ringCount, baseHeight, spacing])

  const squareGeometry = useMemo(() => {
    const half = size / 2
    const points = [
      new THREE.Vector3(-half, 0, -half),
      new THREE.Vector3(half, 0, -half),
      new THREE.Vector3(half, 0, half),
      new THREE.Vector3(-half, 0, half),
    ]
    return new THREE.BufferGeometry().setFromPoints(points)
  }, [size])

  useFrame((_, delta) => {
    timeRef.current += delta * speed

    linesRef.current.forEach((line, index) => {
      if (line && ringData[index]) {
        const { baseY, phaseOffset } = ringData[index]

        const oscillation = Math.sin(timeRef.current + phaseOffset) * 0.15
        line.position.y = baseY + oscillation

        const pulse = 0.6 + Math.sin(timeRef.current * 1.5 + phaseOffset) * 0.3
        if (line.material instanceof THREE.LineBasicMaterial) {
          line.material.opacity = pulse * intensity
        }
      }
    })
  })

  return (
    <group>
      {ringData.map(({ baseY }, index) => (
        <lineLoop
          key={index}
          ref={(el) => { if (el) linesRef.current[index] = el }}
          geometry={squareGeometry}
          position={[0, baseY, 0]}
          raycast={noRaycast}
        >
          <lineBasicMaterial color={color} transparent opacity={0.7 * intensity} />
        </lineLoop>
      ))}
    </group>
  )
}
