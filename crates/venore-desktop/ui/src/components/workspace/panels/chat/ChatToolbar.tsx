// =============================================================================
// ChatToolbar - Bottom toolbar with attach, context, model selector, send/stop
// =============================================================================

import { Plus, AtSign, SendHorizontal, Square } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { ModelSelector } from './ModelSelector'

interface ChatToolbarProps {
  onAttach: () => void
  onToggleContext: () => void
  contextActive: boolean
  canSend: boolean
  isStreaming: boolean
  onSend: () => void
  onStop: () => void
}

export function ChatToolbar({
  onAttach,
  onToggleContext,
  contextActive,
  canSend,
  isStreaming,
  onSend,
  onStop,
}: ChatToolbarProps) {
  const { t } = useTranslation('chat')

  return (
    <div className="flex items-center justify-between px-2 pb-2 pt-0.5">
      {/* Left group */}
      <div className="flex items-center gap-0.5">
        <button
          type="button"
          onClick={onAttach}
          className="h-7 w-7 flex items-center justify-center rounded-lg text-foreground-muted hover:text-foreground hover:bg-background-secondary transition-colors"
          title={t('input.addAttachment')}
        >
          <Plus className="w-3.5 h-3.5" />
        </button>
        <button
          type="button"
          onClick={onToggleContext}
          className={`h-7 w-7 flex items-center justify-center rounded-lg transition-colors ${
            contextActive
              ? 'text-brand bg-brand/10'
              : 'text-foreground-muted hover:text-foreground hover:bg-background-secondary'
          }`}
          title={t('input.addContext')}
        >
          <AtSign className="w-3.5 h-3.5" />
        </button>
        <ModelSelector />
      </div>

      {/* Right group */}
      <div className="flex items-center gap-1.5">
        {isStreaming ? (
          <button
            type="button"
            onClick={onStop}
            className="h-7 w-7 flex items-center justify-center rounded-lg bg-red-500/80 text-white transition-colors hover:bg-red-500"
            title={t('input.stopTitle')}
          >
            <Square className="w-3 h-3" />
          </button>
        ) : (
          <button
            type="button"
            onClick={onSend}
            disabled={!canSend}
            className="h-7 w-7 flex items-center justify-center rounded-lg bg-brand text-background transition-colors hover:bg-brand-hover disabled:opacity-30 disabled:pointer-events-none"
            title={t('input.sendTitle')}
          >
            <SendHorizontal className="w-3.5 h-3.5" />
          </button>
        )}
      </div>
    </div>
  )
}
