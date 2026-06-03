// =============================================================================
// ResizeHandle - Draggable divider between flex-row elements
// =============================================================================
// Vertical 4px bar with col-resize cursor. Emits deltaX on drag.
// Highlight on hover/drag. Forces cursor + user-select on body during drag.

import { useCallback, useRef, useState } from 'react'
import { cn } from '@/lib/utils'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface ResizeHandleProps {
  /** Called continuously during drag with horizontal pixel delta */
  onDrag: (deltaX: number) => void
  /** Called once when the user releases the mouse after dragging */
  onDragEnd?: () => void
  className?: string
}

// -----------------------------------------------------------------------------
// Main Component
// -----------------------------------------------------------------------------

export function ResizeHandle({ onDrag, onDragEnd, className }: ResizeHandleProps) {
  const [isDragging, setIsDragging] = useState(false)
  const startXRef = useRef(0)

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault()
      startXRef.current = e.clientX
      setIsDragging(true)

      const onMouseMove = (ev: MouseEvent) => {
        const deltaX = ev.clientX - startXRef.current
        startXRef.current = ev.clientX
        onDrag(deltaX)
      }

      const onMouseUp = () => {
        onDragEnd?.()
        setIsDragging(false)
        document.body.style.cursor = ''
        document.body.style.userSelect = ''
        document.removeEventListener('mousemove', onMouseMove)
        document.removeEventListener('mouseup', onMouseUp)
      }

      document.body.style.cursor = 'col-resize'
      document.body.style.userSelect = 'none'
      document.addEventListener('mousemove', onMouseMove)
      document.addEventListener('mouseup', onMouseUp)
    },
    [onDrag, onDragEnd],
  )

  return (
    <div
      onMouseDown={handleMouseDown}
      className={cn(
        'w-1 shrink-0 cursor-col-resize relative group',
        className,
      )}
    >
      {/* Visible line — highlights on hover or drag */}
      <div
        className={cn(
          'absolute inset-y-0 left-1/2 -translate-x-1/2 w-px transition-colors',
          isDragging
            ? 'bg-brand w-0.5'
            : 'bg-border group-hover:bg-brand group-hover:w-0.5',
        )}
      />
      {/* Wider invisible hit area */}
      <div className="absolute inset-y-0 -left-1 -right-1" />
    </div>
  )
}
