// =============================================================================
// useFloatingPanel - Drag & resize logic for floating (detached) panels
// =============================================================================
// Manages position + size for an absolutely-positioned panel.
// Clamps to a bounding container so the panel never escapes the canvas.
// Supports drag (via header mousedown) and resize from 8 directions.

import { useCallback, useRef, useState } from 'react'

// Types
export type ResizeDirection = 'n' | 's' | 'e' | 'w' | 'ne' | 'nw' | 'se' | 'sw'

export interface UseFloatingPanelOptions {
  initialSize: { width: number; height: number }
  initialPosition?: { x: number; y: number }
  minSize?: { width: number; height: number }
  boundsRef: React.RefObject<HTMLElement | null>
  onDragEnd?: (finalPos: { x: number; y: number }, size: { width: number; height: number }) => void
  onResizeEnd?: (finalPos: { x: number; y: number }, size: { width: number; height: number }) => void
}

export interface UseFloatingPanelReturn {
  position: { x: number; y: number }
  size: { width: number; height: number }
  isDragging: boolean
  isResizing: boolean
  handleDragStart: (e: React.MouseEvent) => void
  handleResizeStart: (e: React.MouseEvent, dir: ResizeDirection) => void
  setPosition: (pos: { x: number; y: number }) => void
}

// Helpers
function clamp(value: number, min: number, max: number) {
  return Math.max(min, Math.min(value, max))
}

function getBounds(ref: React.RefObject<HTMLElement | null>) {
  if (!ref.current) return { width: 9999, height: 9999 }
  const r = ref.current.getBoundingClientRect()
  return { width: r.width, height: r.height }
}

// Hook
export function useFloatingPanel({
  initialSize,
  initialPosition,
  minSize = { width: 200, height: 150 },
  boundsRef,
  onDragEnd,
  onResizeEnd,
}: UseFloatingPanelOptions): UseFloatingPanelReturn {
  const [position, setPositionState] = useState(initialPosition ?? { x: 40, y: 40 })
  const [size, setSize] = useState(initialSize)
  const [isDragging, setIsDragging] = useState(false)
  const [isResizing, setIsResizing] = useState(false)

  // Refs for live values during mousemove (avoids stale closures)
  const posRef = useRef(position)
  const sizeRef = useRef(size)
  const onDragEndRef = useRef(onDragEnd)
  onDragEndRef.current = onDragEnd
  const onResizeEndRef = useRef(onResizeEnd)
  onResizeEndRef.current = onResizeEnd

  const setPosition = useCallback((pos: { x: number; y: number }) => {
    posRef.current = pos
    setPositionState(pos)
  }, [])

  // --- Drag (header) ---
  const handleDragStart = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault()
      const offsetX = e.clientX - posRef.current.x
      const offsetY = e.clientY - posRef.current.y
      setIsDragging(true)

      const onMouseMove = (ev: MouseEvent) => {
        const bounds = getBounds(boundsRef)
        const s = sizeRef.current
        const x = clamp(ev.clientX - offsetX, 0, bounds.width - s.width)
        const y = clamp(ev.clientY - offsetY, 0, bounds.height - s.height)
        posRef.current = { x, y }
        setPositionState({ x, y })
      }

      const onMouseUp = () => {
        setIsDragging(false)
        document.body.style.cursor = ''
        document.body.style.userSelect = ''
        document.removeEventListener('mousemove', onMouseMove)
        document.removeEventListener('mouseup', onMouseUp)
        onDragEndRef.current?.(posRef.current, sizeRef.current)
      }

      document.body.style.cursor = 'grabbing'
      document.body.style.userSelect = 'none'
      document.addEventListener('mousemove', onMouseMove)
      document.addEventListener('mouseup', onMouseUp)
    },
    [boundsRef],
  )

  // --- Resize (edges/corners) ---
  const handleResizeStart = useCallback(
    (e: React.MouseEvent, dir: ResizeDirection) => {
      e.preventDefault()
      e.stopPropagation()

      const startX = e.clientX
      const startY = e.clientY
      const startPos = { ...posRef.current }
      const startSize = { ...sizeRef.current }

      setIsResizing(true)

      const cursorMap: Record<ResizeDirection, string> = {
        n: 'ns-resize', s: 'ns-resize',
        e: 'ew-resize', w: 'ew-resize',
        ne: 'nesw-resize', sw: 'nesw-resize',
        nw: 'nwse-resize', se: 'nwse-resize',
      }

      const onMouseMove = (ev: MouseEvent) => {
        const dx = ev.clientX - startX
        const dy = ev.clientY - startY
        const bounds = getBounds(boundsRef)

        let newX = startPos.x
        let newY = startPos.y
        let newW = startSize.width
        let newH = startSize.height

        // Horizontal
        if (dir.includes('e')) {
          newW = clamp(startSize.width + dx, minSize.width, bounds.width - startPos.x)
        }
        if (dir.includes('w')) {
          const maxDx = startSize.width - minSize.width
          const clampedDx = clamp(dx, -startPos.x, maxDx)
          newX = startPos.x + clampedDx
          newW = startSize.width - clampedDx
        }

        // Vertical
        if (dir.includes('s')) {
          newH = clamp(startSize.height + dy, minSize.height, bounds.height - startPos.y)
        }
        if (dir === 'n' || dir === 'nw' || dir === 'ne') {
          const maxDy = startSize.height - minSize.height
          const clampedDy = clamp(dy, -startPos.y, maxDy)
          newY = startPos.y + clampedDy
          newH = startSize.height - clampedDy
        }

        posRef.current = { x: newX, y: newY }
        sizeRef.current = { width: newW, height: newH }
        setPositionState({ x: newX, y: newY })
        setSize({ width: newW, height: newH })
      }

      const onMouseUp = () => {
        setIsResizing(false)
        document.body.style.cursor = ''
        document.body.style.userSelect = ''
        document.removeEventListener('mousemove', onMouseMove)
        document.removeEventListener('mouseup', onMouseUp)
        onResizeEndRef.current?.(posRef.current, sizeRef.current)
      }

      document.body.style.cursor = cursorMap[dir]
      document.body.style.userSelect = 'none'
      document.addEventListener('mousemove', onMouseMove)
      document.addEventListener('mouseup', onMouseUp)
    },
    [boundsRef, minSize.width, minSize.height],
  )

  return { position, size, isDragging, isResizing, handleDragStart, handleResizeStart, setPosition }
}
