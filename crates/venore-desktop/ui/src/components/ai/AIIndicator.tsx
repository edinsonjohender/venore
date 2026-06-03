// =============================================================================
// AIIndicator - Sparkles icon in CanvasHeader with rainbow border when active
// =============================================================================

import { Sparkles } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'
import { useAIConnectionStore } from '@/stores/aiConnectionStore'

export function AIIndicator({ className }: { className?: string }) {
  const { t } = useTranslation('workspace')
  const hasActive = useAIConnectionStore((s) =>
    Object.values(s.connections).some((c) => c.active),
  )
  const disconnectAll = useAIConnectionStore((s) => s.disconnectAll)

  return (
    <div
      data-ai-indicator
      className={cn(hasActive && 'rainbow-border cursor-pointer', className)}
      onClick={hasActive ? disconnectAll : undefined}
    >
      <div
        className={cn(
          'flex items-center justify-center w-7 h-7 rounded-lg transition-all duration-300',
          hasActive
            ? 'text-foreground bg-background-secondary'
            : 'text-foreground-muted hover:text-foreground hover:bg-background-tertiary',
        )}
        title={hasActive ? t('aiIndicator.disconnectAll') : t('aiIndicator.label')}
      >
        <Sparkles className="w-3.5 h-3.5" />
      </div>
    </div>
  )
}
