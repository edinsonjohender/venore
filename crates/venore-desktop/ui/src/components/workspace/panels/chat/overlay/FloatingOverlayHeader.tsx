// =============================================================================
// FloatingOverlayHeader - Shared header for floating overlays
// =============================================================================

import { X, ChevronUp, ChevronDown, type LucideIcon } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { AccentColor } from './FloatingOverlay'

// Badge keeps a subtle hint of accent — only color in the header
const badgeColorMap: Record<AccentColor, string> = {
  amber: 'bg-amber-500/10 text-amber-400/80',
  brand: 'bg-brand/10 text-brand/80',
  blue: 'bg-blue-500/10 text-blue-400/80',
  emerald: 'bg-emerald-500/10 text-emerald-400/80',
}

interface FloatingOverlayHeaderProps {
  icon: LucideIcon
  title: string
  accentColor: AccentColor
  onClose?: () => void
  /** Optional right-side badge content */
  badge?: string
  /** Collapse/expand support */
  isCollapsed?: boolean
  onToggleCollapse?: () => void
}

export function FloatingOverlayHeader({ icon: Icon, title, accentColor, onClose, badge, isCollapsed, onToggleCollapse }: FloatingOverlayHeaderProps) {
  return (
    <div
      className={cn(
        'flex items-center gap-2 px-3 py-2',
        !isCollapsed && 'border-b border-border',
        onToggleCollapse && 'cursor-pointer select-none',
      )}
      onClick={onToggleCollapse}
    >
      {/* Collapse chevron */}
      {onToggleCollapse && (
        <div className="h-5 w-5 flex items-center justify-center rounded text-foreground-subtle">
          {isCollapsed
            ? <ChevronUp className="w-3 h-3" />
            : <ChevronDown className="w-3 h-3" />
          }
        </div>
      )}
      <Icon className="w-3.5 h-3.5 shrink-0 text-foreground-muted" />
      <span className="text-xs font-medium text-foreground">{title}</span>
      {badge && (
        <span className={cn('rounded px-1.5 py-0.5 text-[10px] font-mono', badgeColorMap[accentColor])}>
          {badge}
        </span>
      )}
      <span className="flex-1" />
      {onClose && (
        <button
          type="button"
          onClick={(e) => { e.stopPropagation(); onClose() }}
          className="h-5 w-5 flex items-center justify-center rounded text-foreground-subtle hover:text-foreground-muted transition-colors"
        >
          <X className="w-3 h-3" />
        </button>
      )}
    </div>
  )
}
