// =============================================================================
// FloatingPanelWrapper - Absolutely-positioned container for detached panels
// =============================================================================
// Renders a floating panel with a draggable header, dock/close buttons,
// and 8 invisible resize grips (4 edges + 4 corners).

import type { ReactNode } from 'react'
import { Minimize2, X } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import type { ResizeDirection } from '@/hooks/useFloatingPanel'

// Types
interface FloatingPanelWrapperProps {
  title: string
  icon?: ReactNode
  headerActions?: ReactNode
  children?: ReactNode
  left: number
  top: number
  width: number
  height: number
  zIndex: number
  hideDock?: boolean
  animStyle?: React.CSSProperties
  onFocus: () => void
  onDragStart: (e: React.MouseEvent) => void
  onResizeStart: (e: React.MouseEvent, dir: ResizeDirection) => void
  onDock: () => void
  onClose: () => void
}

// Resize grip definitions (inline styles to avoid dynamic Tailwind classes)
const EDGE = 4
const CORNER = 10

const grips: { dir: ResizeDirection; style: React.CSSProperties }[] = [
  // Edges
  { dir: 'n', style: { position: 'absolute', top: 0, left: CORNER, right: CORNER, height: EDGE, cursor: 'ns-resize' } },
  { dir: 's', style: { position: 'absolute', bottom: 0, left: CORNER, right: CORNER, height: EDGE, cursor: 'ns-resize' } },
  { dir: 'w', style: { position: 'absolute', left: 0, top: CORNER, bottom: CORNER, width: EDGE, cursor: 'ew-resize' } },
  { dir: 'e', style: { position: 'absolute', right: 0, top: CORNER, bottom: CORNER, width: EDGE, cursor: 'ew-resize' } },
  // Corners
  { dir: 'nw', style: { position: 'absolute', top: 0, left: 0, width: CORNER, height: CORNER, cursor: 'nwse-resize' } },
  { dir: 'ne', style: { position: 'absolute', top: 0, right: 0, width: CORNER, height: CORNER, cursor: 'nesw-resize' } },
  { dir: 'sw', style: { position: 'absolute', bottom: 0, left: 0, width: CORNER, height: CORNER, cursor: 'nesw-resize' } },
  { dir: 'se', style: { position: 'absolute', bottom: 0, right: 0, width: CORNER, height: CORNER, cursor: 'nwse-resize' } },
]

// Main Component
export function FloatingPanelWrapper({
  title,
  icon,
  headerActions,
  children,
  left,
  top,
  width,
  height,
  zIndex,
  hideDock,
  animStyle,
  onFocus,
  onDragStart,
  onResizeStart,
  onDock,
  onClose,
}: FloatingPanelWrapperProps) {
  const { t } = useTranslation('workspace')
  return (
    <div
      className="absolute flex flex-col rounded-lg border border-border
                 bg-background-secondary shadow-xl overflow-hidden"
      style={{ left, top, width, height, zIndex, ...animStyle }}
      onMouseDown={onFocus}
    >
      {/* Header — draggable */}
      <div
        className="h-9 flex items-center px-3 border-b border-border
                   cursor-grab active:cursor-grabbing shrink-0 select-none"
        onMouseDown={onDragStart}
      >
        {icon && (
          <span className="text-foreground-muted mr-2 flex items-center">{icon}</span>
        )}
        <span className="text-xs font-medium text-foreground-muted uppercase tracking-wider flex-1 truncate">
          {title}
        </span>
        {headerActions && (
          <div onMouseDown={e => e.stopPropagation()}>
            {headerActions}
          </div>
        )}
        {!hideDock && (
          <Button
            variant="ghost"
            size="icon"
            className="h-6 w-6 shrink-0"
            title={t('floatingPanel.dockPanel')}
            onMouseDown={e => e.stopPropagation()}
            onClick={onDock}
          >
            <Minimize2 className="w-3.5 h-3.5" />
          </Button>
        )}
        <Button
          variant="ghost"
          size="icon"
          className="h-6 w-6 shrink-0"
          title={t('floatingPanel.closePanel')}
          onMouseDown={e => e.stopPropagation()}
          onClick={onClose}
        >
          <X className="w-3.5 h-3.5" />
        </Button>
      </div>

      {/* Content */}
      <div className="flex-1 min-h-0 flex flex-col">
        {children}
      </div>

      {/* Resize grips — invisible hit areas */}
      {grips.map(({ dir, style }) => (
        <div
          key={dir}
          style={style}
          onMouseDown={e => onResizeStart(e, dir)}
        />
      ))}
    </div>
  )
}
