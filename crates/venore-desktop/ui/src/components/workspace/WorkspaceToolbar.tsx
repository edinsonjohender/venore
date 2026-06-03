// =============================================================================
// WorkspaceToolbar - Canvas-only floating toolbar
// =============================================================================
// Floating bar on the canvas with canvas-specific actions only.
// Panel toggles, terminal, search, settings → moved to ActivityBar.

import { useSyncExternalStore } from 'react'
import { Hand, Move, ChevronDown } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { FloatingToolbar } from '@/components/ui/floating-toolbar'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { cn } from '@/lib/utils'
import {
  type OceanMode,
  getOceanMode,
  setOceanMode,
  subscribeOceanMode,
} from '@/components/ocean/ocean-mode'

// -----------------------------------------------------------------------------
// CanvasModeDropdown — Navigate / Move Node selector
// -----------------------------------------------------------------------------

const MODE_ICON: Record<OceanMode, typeof Hand> = {
  'navigate': Hand,
  'move-node': Move,
}

const CANVAS_MODE_KEYS: { value: OceanMode; labelKey: string; icon: typeof Hand; shortcut: string }[] = [
  { value: 'navigate', labelKey: 'toolbar.navigate', icon: Hand, shortcut: 'H' },
  { value: 'move-node', labelKey: 'toolbar.moveNode', icon: Move, shortcut: 'N' },
]

function CanvasModeDropdown() {
  const { t } = useTranslation('workspace')
  const mode = useSyncExternalStore(subscribeOceanMode, getOceanMode)
  const ActiveIcon = MODE_ICON[mode]

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          className={cn(
            'flex items-center gap-1 h-8 px-2.5 rounded-md transition-colors',
            'text-foreground-muted hover:bg-background-tertiary hover:text-foreground',
            'data-[state=open]:bg-background-tertiary data-[state=open]:text-foreground',
          )}
          title={t('toolbar.canvasMode')}
        >
          <ActiveIcon className="w-4 h-4" />
          <span className="text-xs">{mode === 'navigate' ? t('toolbar.navigate') : t('toolbar.move')}</span>
          <ChevronDown className="w-3 h-3" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent side="top" align="start" sideOffset={12}>
        {CANVAS_MODE_KEYS.map(({ value, labelKey, icon: Icon, shortcut }) => (
          <DropdownMenuItem
            key={value}
            className={cn(
              'gap-3',
              mode === value
                ? 'bg-background-tertiary text-foreground'
                : 'text-foreground-muted',
            )}
            onSelect={() => setOceanMode(value)}
          >
            <Icon className="w-4 h-4" />
            <span className="flex-1 text-sm">{t(labelKey)}</span>
            <span className="text-xs text-foreground-subtle">{shortcut}</span>
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

// -----------------------------------------------------------------------------
// Main Component
// -----------------------------------------------------------------------------

export function WorkspaceToolbar() {
  return (
    <FloatingToolbar>
      <CanvasModeDropdown />
    </FloatingToolbar>
  )
}
