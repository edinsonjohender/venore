// =============================================================================
// SecurityPerimeter — "DO NOT CROSS"-style perimeter (v1 port)
// =============================================================================
// Posts in the 4 corners + holographic ribbons with chevrons and text,
// shader with scanlines and pulse. Designed to wrap a node and signal
// "attention".
//
// Source: venore (v1) `src/components/_experiments/SecurityPerimeter.tsx`.
// Unchanged — the component had no project-internal imports.

import { useRef, useMemo } from 'react'
import { useFrame } from '@react-three/fiber'
import * as THREE from 'three'

interface SecurityPerimeterProps {
  size?: number
  /** Tape background color (e.g. yellow for warning, red for error, blue for in-use). */
  color?: string
  /** Text rendered twice across the tape. Keep it short (≤12 chars). */
  text?: string
  /** Foreground color for the text + text-box backgrounds. Default black works on
   *  most light/medium tape colors; pass white for very dark tapes. */
  textColor?: string
  postHeight?: number
  tapeCount?: number
  baseHeight?: number
  spacing?: number
  speed?: number
  intensity?: number
}

export function SecurityPerimeter({
  size = 1.9,
  color = '#fbbf24',
  text = 'DO NOT CROSS',
  textColor = '#000000',
  postHeight = 2,
  tapeCount = 2,
  baseHeight = 0.4,
  spacing = 0.6,
  speed = 1,
  intensity = 1,
}: SecurityPerimeterProps) {
  const groupRef = useRef<THREE.Group>(null)
  const materialsRef = useRef<THREE.ShaderMaterial[]>([])
  const timeRef = useRef(0)

  const half = size / 2

  const postPositions = useMemo(() => [
    { x: -half, z: -half },
    { x: half, z: -half },
    { x: half, z: half },
    { x: -half, z: half },
  ], [half])

  const tapeData = useMemo(() => {
    const data: { y: number }[] = []
    for (let i = 0; i < tapeCount; i++) {
      data.push({ y: baseHeight + i * spacing })
    }
    return data
  }, [tapeCount, baseHeight, spacing])

  const tapeTexture = useMemo(() => {
    const canvas = document.createElement('canvas')
    canvas.width = 1024
    canvas.height = 128
    const ctx = canvas.getContext('2d')!

    // Tape background — driven by the `color` prop so the same shader can
    // serve "warning yellow", "error red", "in-use blue", etc.
    ctx.fillStyle = color
    ctx.fillRect(0, 0, 1024, 128)

    // Cut transparent chevrons out of the background so you can see through
    // the tape between text blocks.
    ctx.globalCompositeOperation = 'destination-out'
    const chevronWidth = 60
    const gap = 20

    for (let i = 1024; i > 0; i -= chevronWidth + gap) {
      ctx.beginPath()
      ctx.moveTo(i, 10)
      ctx.lineTo(i - chevronWidth * 0.7, 64)
      ctx.lineTo(i, 118)
      ctx.lineTo(i - 20, 118)
      ctx.lineTo(i - chevronWidth * 0.7 - 20, 64)
      ctx.lineTo(i - 20, 10)
      ctx.closePath()
      ctx.fill()
    }

    ctx.globalCompositeOperation = 'source-over'

    // Two opaque text panels — same color as the tape so they look like
    // continuous tape with the chevrons paused around the message.
    ctx.fillStyle = color
    ctx.fillRect(180, 25, 320, 78)
    ctx.fillRect(680, 25, 320, 78)

    // Message — drawn twice across the canvas length so the texture reads
    // continuously when wrapped around the perimeter.
    ctx.fillStyle = textColor
    ctx.font = 'bold 44px Arial Black, sans-serif'
    ctx.textAlign = 'center'
    ctx.textBaseline = 'middle'
    ctx.fillText(text, 340, 64)
    ctx.fillText(text, 840, 64)

    // Top + bottom solid borders (same color, no chevrons) for a clean edge.
    ctx.fillStyle = color
    ctx.fillRect(0, 0, 1024, 6)
    ctx.fillRect(0, 122, 1024, 6)

    const texture = new THREE.CanvasTexture(canvas)
    texture.wrapS = THREE.RepeatWrapping
    texture.wrapT = THREE.ClampToEdgeWrapping
    texture.repeat.set(1, 1)
    return texture
  }, [color, text, textColor])

  const hologramShader = useMemo(() => ({
    uniforms: {
      uTime: { value: 0 },
      uColor: { value: new THREE.Color(color) },
      uIntensity: { value: intensity },
      uTexture: { value: tapeTexture },
    },
    vertexShader: `
      varying vec2 vUv;
      void main() {
        vUv = uv;
        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
      }
    `,
    fragmentShader: `
      uniform float uTime;
      uniform vec3 uColor;
      uniform float uIntensity;
      uniform sampler2D uTexture;
      varying vec2 vUv;

      void main() {
        vec2 scrollUv = vec2(vUv.x + uTime * 0.15, vUv.y);
        vec4 texColor = texture2D(uTexture, scrollUv);
        float pulse = 0.85 + sin(uTime * 2.0) * 0.15;
        float scanline = 0.95 + sin(vUv.y * 30.0 + uTime * 8.0) * 0.05;
        vec3 finalColor = texColor.rgb * pulse * scanline;
        float alpha = 0.45 * uIntensity;
        gl_FragColor = vec4(finalColor, alpha);
      }
    `,
    transparent: true,
    side: THREE.FrontSide,
    depthWrite: false,
  }), [color, intensity, tapeTexture])

  useFrame((_, delta) => {
    timeRef.current += delta * speed

    materialsRef.current.forEach((mat) => {
      if (mat.uniforms) {
        mat.uniforms.uTime.value = timeRef.current
      }
    })

    if (groupRef.current) {
      groupRef.current.traverse((child) => {
        if (child.userData.isPostLight && child instanceof THREE.Mesh) {
          const pulse = 0.7 + Math.sin(timeRef.current * 3) * 0.3
          if (child.material instanceof THREE.MeshBasicMaterial) {
            child.material.opacity = pulse * intensity
          }
        }
      })
    }
  })

  const createTapeGeometry = (from: { x: number; z: number }, to: { x: number; z: number }) => {
    const dx = to.x - from.x
    const dz = to.z - from.z
    const length = Math.sqrt(dx * dx + dz * dz)
    const angle = Math.atan2(dz, dx)
    return { length, angle, midX: (from.x + to.x) / 2, midZ: (from.z + to.z) / 2 }
  }

  const sides = [
    createTapeGeometry(postPositions[0], postPositions[1]),
    createTapeGeometry(postPositions[1], postPositions[2]),
    createTapeGeometry(postPositions[2], postPositions[3]),
    createTapeGeometry(postPositions[3], postPositions[0]),
  ]

  // postHeight is part of the v1 prop API but only the tapes are rendered today
  // (the post mesh path was scaffolded then removed). Keep the prop accepted
  // so consumers can still pass it without errors when we re-introduce posts.
  void postHeight

  return (
    <group ref={groupRef}>
      {tapeData.map((tape, tapeIndex) => (
        <group key={`tape-level-${tapeIndex}`}>
          {sides.map((side, sideIndex) => {
            const material1 = new THREE.ShaderMaterial(hologramShader)
            const material2 = new THREE.ShaderMaterial(hologramShader)
            materialsRef.current.push(material1, material2)

            return (
              <group key={`tape-${tapeIndex}-${sideIndex}`}>
                <mesh
                  position={[side.midX, tape.y, side.midZ]}
                  rotation={[0, -side.angle, 0]}
                  material={material1}
                >
                  <planeGeometry args={[side.length, 0.3]} />
                </mesh>
                <mesh
                  position={[side.midX, tape.y, side.midZ]}
                  rotation={[0, -side.angle + Math.PI, 0]}
                  scale={[-1, 1, 1]}
                  material={material2}
                >
                  <planeGeometry args={[side.length, 0.3]} />
                </mesh>
              </group>
            )
          })}
        </group>
      ))}
    </group>
  )
}
