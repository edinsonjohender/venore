// =============================================================================
// HorizontalResizeHandle - Draggable divider between flex-col elements
// =============================================================================
// Horizontal 4px bar with row-resize cursor. Emits deltaY on drag.
// Highlight on hover/drag. Forces cursor + user-select on body during drag.

import { useCallback, useRef, useState } from 'react'
import { cn } from '@/lib/utils'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface HorizontalResizeHandleProps {
  onDrag: (deltaY: number) => void
  onDragEnd?: () => void
  className?: string
}

// -----------------------------------------------------------------------------
// Main Component
// -----------------------------------------------------------------------------

export function HorizontalResizeHandle({ onDrag, onDragEnd, className }: HorizontalResizeHandleProps) {
  const [isDragging, setIsDragging] = useState(false)
  const startYRef = useRef(0)

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault()
      startYRef.current = e.clientY
      setIsDragging(true)

      const onMouseMove = (ev: MouseEvent) => {
        const deltaY = ev.clientY - startYRef.current
        startYRef.current = ev.clientY
        onDrag(deltaY)
      }

      const onMouseUp = () => {
        onDragEnd?.()
        setIsDragging(false)
        document.body.style.cursor = ''
        document.body.style.userSelect = ''
        document.removeEventListener('mousemove', onMouseMove)
        document.removeEventListener('mouseup', onMouseUp)
      }

      document.body.style.cursor = 'row-resize'
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
        'h-1 shrink-0 cursor-row-resize relative group',
        className,
      )}
    >
      {/* Visible line — highlights on hover or drag */}
      <div
        className={cn(
          'absolute inset-x-0 top-1/2 -translate-y-1/2 h-px transition-colors',
          isDragging
            ? 'bg-brand h-0.5'
            : 'bg-border group-hover:bg-brand group-hover:h-0.5',
        )}
      />
      {/* Wider invisible hit area */}
      <div className="absolute inset-x-0 -top-1 -bottom-1" />
    </div>
  )
}
