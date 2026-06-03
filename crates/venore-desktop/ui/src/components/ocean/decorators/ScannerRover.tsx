// =============================================================================
// ScannerRover — Horizontal radar disc with sweep and trail
// =============================================================================
// Redesign: the literal v1 port was a convergent cone with an orb on top,
// which looked like a teepee and didn't communicate "scanning". This version:
//   - Thin vertical mast from the floor up to the disc's height
//   - Horizontal disc with concentric rings (radar / sonar style)
//   - Sweep needle that rotates with a trail of fading-opacity leaves
//   - Central pivot point
//   - Status indicator (green = scan / amber = travel) above the disc
//
// Keeps the travel-between-`positions` logic to preserve the v1 "rover"
// metaphor, and the sweep speeds up while traveling between nodes.

import { useRef, useState, useMemo } from 'react'
import { useFrame } from '@react-three/fiber'
import * as THREE from 'three'
import { noRaycast } from '../nodes/types'

interface NodePosition {
  x: number
  z: number
  id?: string
}

interface ScannerRoverProps {
  positions: NodePosition[]
  color?: string
  scanDuration?: number
  travelSpeed?: number
  size?: number
  height?: number
  /** Number of trailing blades behind the leading sweep. Default 8. */
  linesPerSide?: number
}

export function ScannerRover({
  positions,
  color = '#60a5fa',
  scanDuration = 2,
  travelSpeed = 2,
  size = 1.9,
  height = 2.5,
  linesPerSide = 8,
}: ScannerRoverProps) {
  const groupRef = useRef<THREE.Group>(null)
  const sweepGroupRef = useRef<THREE.Group>(null)
  const timeRef = useRef(0)
  const scanTimeRef = useRef(0)

  const [currentIndex, setCurrentIndex] = useState(0)
  const [isScanning, setIsScanning] = useState(true)
  const [currentPos, setCurrentPos] = useState({
    x: positions[0]?.x ?? 0,
    z: positions[0]?.z ?? 0,
  })

  const radius = size * 0.5
  const trailCount = linesPerSide

  // One sweep blade = a thin sector. Trail blades sit behind the leading one
  // at small angle increments with decreasing opacity, faking motion blur.
  const blades = useMemo(() => {
    const arr: { startAngle: number; endAngle: number; opacity: number }[] = []
    const bladeArc = (Math.PI * 2) * 0.035 // ~12.6° per blade
    for (let i = 0; i < trailCount; i++) {
      arr.push({
        startAngle: -i * bladeArc,
        endAngle: -(i + 1) * bladeArc,
        opacity: 0.85 * (1 - i / trailCount),
      })
    }
    return arr
  }, [trailCount])

  /** Build a sector shape (filled wedge) in the X-Y plane between two angles. */
  const makeSectorShape = (rIn: number, rOut: number, aStart: number, aEnd: number) => {
    const shape = new THREE.Shape()
    const segments = 14
    for (let i = 0; i <= segments; i++) {
      const t = i / segments
      const a = aStart + (aEnd - aStart) * t
      const x = Math.cos(a) * rOut
      const y = Math.sin(a) * rOut
      if (i === 0) shape.moveTo(x, y)
      else shape.lineTo(x, y)
    }
    for (let i = segments; i >= 0; i--) {
      const t = i / segments
      const a = aStart + (aEnd - aStart) * t
      shape.lineTo(Math.cos(a) * rIn, Math.sin(a) * rIn)
    }
    return shape
  }

  const bladeShapes = useMemo(
    () => blades.map((b) => makeSectorShape(radius * 0.12, radius * 0.95, b.startAngle, b.endAngle)),
    [blades, radius],
  )

  // Concentric ring geometries (in X-Y so they lay flat after the disc group's
  // -PI/2 X rotation).
  const ringGeometries = useMemo(() => {
    return [0.4, 0.7, 0.95].map((scale) => {
      const r = radius * scale
      const points: THREE.Vector3[] = []
      const segments = 64
      for (let i = 0; i <= segments; i++) {
        const a = (i / segments) * Math.PI * 2
        points.push(new THREE.Vector3(Math.cos(a) * r, Math.sin(a) * r, 0))
      }
      return new THREE.BufferGeometry().setFromPoints(points)
    })
  }, [radius])

  useFrame((_, delta) => {
    if (positions.length === 0) return
    timeRef.current += delta

    const targetPos = positions[currentIndex]
    if (!targetPos) return

    if (isScanning) {
      scanTimeRef.current += delta
      if (scanTimeRef.current >= scanDuration) {
        scanTimeRef.current = 0
        setIsScanning(false)
      }
    } else {
      const nextIndex = (currentIndex + 1) % positions.length
      const nextPos = positions[nextIndex]
      const dx = nextPos.x - currentPos.x
      const dz = nextPos.z - currentPos.z
      const dist = Math.sqrt(dx * dx + dz * dz)
      if (dist < 0.05) {
        setCurrentPos({ x: nextPos.x, z: nextPos.z })
        setCurrentIndex(nextIndex)
        setIsScanning(true)
      } else {
        const moveAmount = delta * travelSpeed
        const ratio = Math.min(moveAmount / dist, 1)
        setCurrentPos({
          x: currentPos.x + dx * ratio,
          z: currentPos.z + dz * ratio,
        })
      }
    }

    if (groupRef.current) {
      groupRef.current.position.x = currentPos.x
      groupRef.current.position.z = currentPos.z
    }

    // Sweep rotates faster while traveling, slower while scanning. Local Z
    // rotation = world Y rotation after the disc's -PI/2 X parent rotation.
    if (sweepGroupRef.current) {
      const sweepSpeed = isScanning ? 2.4 : 4.5
      sweepGroupRef.current.rotation.z -= delta * sweepSpeed
    }
  })

  if (positions.length === 0) return null

  return (
    <group ref={groupRef} position={[currentPos.x, 0, currentPos.z]}>
      {/* Mast — keeps the disc visually anchored to the host node */}
      <mesh position={[0, height / 2, 0]} raycast={noRaycast}>
        <cylinderGeometry args={[0.025, 0.025, height, 8]} />
        <meshBasicMaterial color={color} transparent opacity={0.35} />
      </mesh>

      {/* Disc plane — laid flat on Y = height */}
      <group position={[0, height, 0]} rotation={[-Math.PI / 2, 0, 0]}>
        {/* Translucent disc base */}
        <mesh raycast={noRaycast}>
          <circleGeometry args={[radius, 48]} />
          <meshBasicMaterial color={color} transparent opacity={0.08} side={THREE.DoubleSide} />
        </mesh>

        {/* Concentric rings — lineLoop closes back to the start vertex */}
        {ringGeometries.map((geom, i) => (
          <lineLoop key={i} geometry={geom} position={[0, 0, 0.005 * (i + 1)]} raycast={noRaycast}>
            <lineBasicMaterial color={color} transparent opacity={0.35} />
          </lineLoop>
        ))}

        {/* Cross hair — two perpendicular lineSegments */}
        {[0, Math.PI / 2].map((angle) => {
          const points = [
            new THREE.Vector3(-radius * Math.cos(angle), -radius * Math.sin(angle), 0),
            new THREE.Vector3(radius * Math.cos(angle), radius * Math.sin(angle), 0),
          ]
          const geom = new THREE.BufferGeometry().setFromPoints(points)
          return (
            <lineSegments key={angle} geometry={geom} position={[0, 0, 0.01]} raycast={noRaycast}>
              <lineBasicMaterial color={color} transparent opacity={0.18} />
            </lineSegments>
          )
        })}

        {/* Sweep — leading blade + trail, all rotating together */}
        <group ref={sweepGroupRef}>
          {bladeShapes.map((shape, i) => (
            <mesh key={i} position={[0, 0, 0.02]} raycast={noRaycast}>
              <shapeGeometry args={[shape]} />
              <meshBasicMaterial
                color={color}
                transparent
                opacity={blades[i].opacity}
                side={THREE.DoubleSide}
              />
            </mesh>
          ))}
        </group>

        {/* Center pivot dot */}
        <mesh position={[0, 0, 0.05]} raycast={noRaycast}>
          <circleGeometry args={[0.07, 16]} />
          <meshBasicMaterial color={color} />
        </mesh>
      </group>

      {/* Status indicator — green while scanning, amber while traveling */}
      <mesh position={[0, height + 0.45, 0]} raycast={noRaycast}>
        <sphereGeometry args={[0.08, 12, 12]} />
        <meshBasicMaterial
          color={isScanning ? '#4ade80' : '#fbbf24'}
          transparent
          opacity={0.95}
        />
      </mesh>
    </group>
  )
}
