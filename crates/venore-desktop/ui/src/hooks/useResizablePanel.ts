// =============================================================================
// useResizablePanel - Encapsulates resize logic for a single panel
// =============================================================================
// Tracks width, clamps between floor/max, and signals close-pending state.
// Actual close happens on drag end (close-on-release), not during drag.

import { useCallback, useRef, useState } from 'react'

// -----------------------------------------------------------------------------
// Constants
// -----------------------------------------------------------------------------

/** Minimum visual width during drag — prevents panel from disappearing entirely */
const COLLAPSE_FLOOR = 60

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface UseResizablePanelOptions {
  /** Starting width in px (also used when re-opening) */
  initialWidth: number
  /** Below this → close pending; on release → auto-close */
  minWidth: number
  /** Upper bound */
  maxWidth?: number
  /** Determines drag direction: left panels grow with +deltaX, right with -deltaX */
  side: 'left' | 'right'
  /** Called when panel is released below minWidth */
  onClose: () => void
}

interface UseResizablePanelReturn {
  /** Current panel width in px */
  width: number
  /** True when width < minWidth during drag — panel will close on release */
  closePending: boolean
  /** Pass this as onDrag to ResizeHandle */
  handleDrag: (deltaX: number) => void
  /** Pass this as onDragEnd to ResizeHandle — decides whether to close */
  handleDragEnd: () => void
  /** Reset width to initialWidth (call when re-opening) */
  reset: () => void
}

// -----------------------------------------------------------------------------
// Hook
// -----------------------------------------------------------------------------

export function useResizablePanel({
  initialWidth,
  minWidth,
  maxWidth = 600,
  side,
  onClose,
}: UseResizablePanelOptions): UseResizablePanelReturn {
  const [width, setWidth] = useState(initialWidth)
  const onCloseRef = useRef(onClose)
  onCloseRef.current = onClose

  const closePending = width < minWidth

  const handleDrag = useCallback(
    (deltaX: number) => {
      setWidth(prev => {
        // Left panels: dragging right (+deltaX) grows the panel
        // Right panels: dragging left (-deltaX) grows the panel
        const adjusted = side === 'left' ? prev + deltaX : prev - deltaX
        return Math.max(COLLAPSE_FLOOR, Math.min(adjusted, maxWidth))
      })
    },
    [side, maxWidth],
  )

  const handleDragEnd = useCallback(() => {
    setWidth(prev => {
      if (prev < minWidth) {
        onCloseRef.current()
        return initialWidth
      }
      return prev
    })
  }, [minWidth, initialWidth])

  const reset = useCallback(() => {
    setWidth(initialWidth)
  }, [initialWidth])

  return { width, closePending, handleDrag, handleDragEnd, reset }
}
