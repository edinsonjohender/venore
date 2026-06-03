// =============================================================================
// WorkspacePanel - Base panel container with header and close button
// =============================================================================
// Reusable shell for any panel (left, right, bottom).
// Renders a header bar with title + close, and a scrollable content area.
// Supports undocking via header button or drag-to-undock (>15px threshold).

import { useCallback, useRef, type ReactNode } from 'react'
import { Maximize2, X } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

// Constants
const UNDOCK_DRAG_THRESHOLD = 15

// Types
interface WorkspacePanelProps {
  /** Panel title shown in the header */
  title: string
  /** Optional icon element rendered before the title */
  icon?: ReactNode
  /** Custom action buttons rendered between title and undock/close */
  headerActions?: ReactNode
  /** Called when the close button is clicked */
  onClose?: () => void
  /** Called when the undock button is clicked (only shown when provided) */
  onUndock?: () => void
  /** Called when user drags the header beyond threshold — receives mouse position */
  onUndockDrag?: (mouseX: number, mouseY: number) => void
  /** Panel content */
  children?: ReactNode
  /** Additional className on the outer container */
  className?: string
  /** Which side — affects border direction */
  side?: 'left' | 'right'
}

// Main Component
export function WorkspacePanel({
  title,
  icon,
  headerActions,
  onClose,
  onUndock,
  onUndockDrag,
  children,
  className,
  side,
}: WorkspacePanelProps) {
  const { t } = useTranslation('workspace')
  const dragStartRef = useRef<{ x: number; y: number } | null>(null)

  const handleHeaderMouseDown = useCallback(
    (e: React.MouseEvent) => {
      if (!onUndockDrag) return
      // Don't interfere with button clicks
      if ((e.target as HTMLElement).closest('button')) return

      const startX = e.clientX
      const startY = e.clientY
      dragStartRef.current = { x: startX, y: startY }

      const onMouseMove = (ev: MouseEvent) => {
        if (!dragStartRef.current) return
        const dx = ev.clientX - dragStartRef.current.x
        const dy = ev.clientY - dragStartRef.current.y
        if (Math.sqrt(dx * dx + dy * dy) > UNDOCK_DRAG_THRESHOLD) {
          dragStartRef.current = null
          document.removeEventListener('mousemove', onMouseMove)
          document.removeEventListener('mouseup', onMouseUp)
          onUndockDrag(ev.clientX, ev.clientY)
        }
      }

      const onMouseUp = () => {
        dragStartRef.current = null
        document.removeEventListener('mousemove', onMouseMove)
        document.removeEventListener('mouseup', onMouseUp)
      }

      document.addEventListener('mousemove', onMouseMove)
      document.addEventListener('mouseup', onMouseUp)
    },
    [onUndockDrag],
  )

  return (
    <div
      className={cn(
        'flex flex-col bg-background-secondary h-full',
        side === 'left' && 'border-r border-border',
        side === 'right' && 'border-l border-border',
        className,
      )}
    >
      {/* Header */}
      <div
        className={cn(
          'flex items-center h-9 px-3 border-b border-border shrink-0',
          onUndockDrag && 'cursor-grab active:cursor-grabbing',
        )}
        onMouseDown={handleHeaderMouseDown}
      >
        {icon && (
          <span className="text-foreground-muted mr-2 flex items-center">{icon}</span>
        )}
        <span className="text-xs font-medium text-foreground-muted uppercase tracking-wider flex-1 truncate">
          {title}
        </span>
        {headerActions}
        {onUndock && (
          <Button
            variant="ghost"
            size="icon"
            className="h-6 w-6 shrink-0"
            title={t('panel.undockPanel')}
            onClick={onUndock}
          >
            <Maximize2 className="w-3.5 h-3.5" />
          </Button>
        )}
        {onClose && (
          <Button
            variant="ghost"
            size="icon"
            className="h-6 w-6 shrink-0"
            onClick={onClose}
          >
            <X className="w-3.5 h-3.5" />
          </Button>
        )}
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto">
        {children}
      </div>
    </div>
  )
}
