// =============================================================================
// MeshIndicator — Network icon in CanvasHeader with connected peer badge
// =============================================================================

import { useTranslation } from 'react-i18next'
import { Network } from 'lucide-react'
import { cn } from '@/lib/utils'
import { useMeshStore } from '@/stores/meshStore'

export function MeshIndicator({ className }: { className?: string }) {
  const { t } = useTranslation('workspace')
  const togglePanel = useMeshStore((s) => s.togglePanel)
  const connectedCount = useMeshStore((s) => s.connectedPeerIds.length)
  const panelOpen = useMeshStore((s) => s.panelOpen)

  return (
    <button
      onClick={togglePanel}
      className={cn(
        'relative flex items-center justify-center w-7 h-7 rounded-lg transition-all duration-300',
        panelOpen || connectedCount > 0
          ? 'text-foreground bg-background-secondary'
          : 'text-foreground-muted hover:text-foreground hover:bg-background-tertiary',
        className,
      )}
      title={connectedCount > 0 ? t('mesh.indicatorTitle', { count: connectedCount }) : t('mesh.indicatorTitleEmpty')}
    >
      <Network className="w-3.5 h-3.5" />
      {connectedCount > 0 && (
        <span className="absolute -top-0.5 -right-0.5 w-3 h-3 rounded-full bg-emerald-500
                         text-[8px] font-bold text-white flex items-center justify-center">
          {connectedCount}
        </span>
      )}
    </button>
  )
}
