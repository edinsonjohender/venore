// =============================================================================
// PresencePin — User-presence pin floating above a node (v1 port)
// =============================================================================
// Camera-following billboard: circle with border + initials (rendered only
// inside SHOW_TEXT_DISTANCE so we don't pay text render cost from far away).
// `level === 'editing'` forces the amber color; everything else uses the
// `color` passed in. `PresencePinStack` stacks multiple pins with a vertical
// offset and an overflow `+N` pin when users exceed `maxVisible`.
//
// Source: venore (v1) `src/components/_experiments/PresencePin.tsx`.

import { useRef, useMemo, memo, useState } from 'react'
import { useFrame, useThree } from '@react-three/fiber'
import { Vector3, type Group } from 'three'
import { Billboard, Text } from '@react-three/drei'

type PresenceLevel = 'hover' | 'viewing' | 'editing'

interface PresencePinProps {
  initials?: string
  color?: string
  height?: number
  radius?: number
  level?: PresenceLevel
}

export interface PresenceUser {
  id: string
  name: string
  color: string
  level?: PresenceLevel
}

interface PresencePinStackProps {
  users: PresenceUser[]
  maxVisible?: number
  baseHeight?: number
  heightStep?: number
}

const EDITING_COLOR = '#f59e0b'
const OVERFLOW_COLOR = '#71717a'
const SEGMENTS = 24
const SHOW_TEXT_DISTANCE = 60

const getInitials = (name: string): string => {
  const parts = name.trim().split(/\s+/)
  return parts.length >= 2
    ? parts[0][0] + parts[1][0]
    : name.slice(0, 2)
}

// Reused vector to avoid allocations every frame.
const worldPos = new Vector3()

export const PresencePin = memo(function PresencePin({
  initials,
  color = '#01e8a2',
  height = 1.5,
  radius = 0.25,
  level = 'viewing',
}: PresencePinProps) {
  const groupRef = useRef<Group>(null)
  const scaleRef = useRef(0)
  const [showText, setShowText] = useState(false)
  const { camera } = useThree()

  const effectiveColor = level === 'editing' ? EDITING_COLOR : color

  const ringArgs = useMemo(
    () => [radius - 0.04, radius, SEGMENTS] as [number, number, number],
    [radius],
  )

  useFrame((_, delta) => {
    if (scaleRef.current < 1 && groupRef.current) {
      scaleRef.current = Math.min(1, scaleRef.current + delta * 4)
      groupRef.current.scale.setScalar(scaleRef.current)
    }

    if (groupRef.current && initials) {
      groupRef.current.getWorldPosition(worldPos)
      const distance = camera.position.distanceTo(worldPos)
      const shouldShow = distance < SHOW_TEXT_DISTANCE
      if (shouldShow !== showText) {
        setShowText(shouldShow)
      }
    }
  })

  return (
    <group ref={groupRef} scale={0}>
      <Billboard position={[0, height, 0]} follow>
        <mesh>
          <circleGeometry args={[radius, SEGMENTS]} />
          <meshBasicMaterial color={effectiveColor} />
        </mesh>

        <mesh position={[0, 0, 0.02]}>
          <ringGeometry args={ringArgs} />
          <meshBasicMaterial color="#000000" />
        </mesh>

        {showText && initials && (
          <Text
            position={[0, 0, 0.03]}
            fontSize={radius * 0.8}
            color="#000000"
            anchorX="center"
            anchorY="middle"
            fontWeight={700}
          >
            {initials.slice(0, 2).toUpperCase()}
          </Text>
        )}
      </Billboard>
    </group>
  )
})

export const PresencePinStack = memo(function PresencePinStack({
  users,
  maxVisible = 3,
  baseHeight = 1.2,
  heightStep = 0.5,
}: PresencePinStackProps) {
  const visibleUsers = useMemo(() => users.slice(0, maxVisible), [users, maxVisible])
  const hiddenCount = Math.max(0, users.length - maxVisible)

  return (
    <group>
      {visibleUsers.map((user, index) => (
        <PresencePin
          key={user.id}
          initials={getInitials(user.name)}
          color={user.color}
          height={baseHeight + index * heightStep}
          level={user.level}
        />
      ))}

      {hiddenCount > 0 && (
        <PresencePin
          initials={`+${hiddenCount}`}
          color={OVERFLOW_COLOR}
          height={baseHeight + visibleUsers.length * heightStep}
          level="hover"
        />
      )}
    </group>
  )
})
