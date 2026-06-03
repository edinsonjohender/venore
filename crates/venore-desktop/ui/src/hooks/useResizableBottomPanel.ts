// =============================================================================
// useResizableBottomPanel - Height-based resize for bottom panels
// =============================================================================
// Mirror of useResizablePanel but tracks height instead of width.
// Dragging up (-deltaY) grows the panel. Close-on-release when below minHeight.

import { useCallback, useRef, useState } from 'react'

// -----------------------------------------------------------------------------
// Constants
// -----------------------------------------------------------------------------

const COLLAPSE_FLOOR = 60

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface UseResizableBottomPanelOptions {
  initialHeight: number
  minHeight: number
  maxHeight?: number
  onClose: () => void
}

interface UseResizableBottomPanelReturn {
  height: number
  closePending: boolean
  handleDrag: (deltaY: number) => void
  handleDragEnd: () => void
  reset: () => void
}

// -----------------------------------------------------------------------------
// Hook
// -----------------------------------------------------------------------------

export function useResizableBottomPanel({
  initialHeight,
  minHeight,
  maxHeight = 600,
  onClose,
}: UseResizableBottomPanelOptions): UseResizableBottomPanelReturn {
  const [height, setHeight] = useState(initialHeight)
  const onCloseRef = useRef(onClose)
  onCloseRef.current = onClose

  const closePending = height < minHeight

  const handleDrag = useCallback(
    (deltaY: number) => {
      setHeight((prev) => {
        // Dragging up (-deltaY) grows the panel
        const adjusted = prev - deltaY
        return Math.max(COLLAPSE_FLOOR, Math.min(adjusted, maxHeight))
      })
    },
    [maxHeight],
  )

  const handleDragEnd = useCallback(() => {
    setHeight((prev) => {
      if (prev < minHeight) {
        onCloseRef.current()
        return initialHeight
      }
      return prev
    })
  }, [minHeight, initialHeight])

  const reset = useCallback(() => {
    setHeight(initialHeight)
  }, [initialHeight])

  return { height, closePending, handleDrag, handleDragEnd, reset }
}
