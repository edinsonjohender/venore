// =============================================================================
// ChatToolCall - Inline tool call display within assistant messages
// =============================================================================
// Shows tool name, arguments, status, and output preview.
// If the tool has a snapshot commit, shows a revert button.

import { type ReactNode, useState } from 'react'
import { Terminal, ChevronDown, ChevronRight, Loader2, Check, X, Ban, RotateCcw } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'
import type { ToolCallInfo, ToolCallStatus } from '@/stores/chatStore'
import { useChatStore } from '@/stores/chatStore'
import { useChatSessionStore } from '@/stores/chatSessionStore'

interface ChatToolCallProps {
  toolCall: ToolCallInfo
  messageId: string
  /** When true, renders without outer border/bg (for use inside overlay panels) */
  embedded?: boolean
}

function useStatusConfig(status: ToolCallStatus): { icon: ReactNode; label: string; color: string } {
  const { t } = useTranslation('chat')

  switch (status) {
    case 'running': return { icon: <Loader2 className="w-3 h-3 animate-spin" />, label: t('toolCall.running'), color: 'text-foreground-muted' }
    case 'completed': return { icon: <Check className="w-3 h-3 text-emerald-400/70" />, label: t('toolCall.completed'), color: 'text-foreground-subtle' }
    case 'denied': return { icon: <Ban className="w-3 h-3" />, label: t('toolCall.denied'), color: 'text-foreground-subtle' }
    case 'error': return { icon: <X className="w-3 h-3 text-red-400/70" />, label: t('toolCall.failed'), color: 'text-foreground-subtle' }
    default: return { icon: <Loader2 className="w-3 h-3 animate-spin" />, label: t('toolCall.pending'), color: 'text-foreground-subtle' }
  }
}

function formatArgs(args: Record<string, unknown>): string {
  const entries = Object.entries(args)
  if (entries.length === 0) return ''
  return entries
    .map(([k, v]) => `${k}: ${typeof v === 'string' ? v : JSON.stringify(v)}`)
    .join('\n')
}

export function ChatToolCall({ toolCall, messageId, embedded }: ChatToolCallProps) {
  const { t } = useTranslation('chat')
  const [expanded, setExpanded] = useState(false)
  const [showRevertConfirm, setShowRevertConfirm] = useState(false)
  const [reverting, setReverting] = useState(false)
  const status = useStatusConfig(toolCall.status)
  const hasOutput = toolCall.result && toolCall.result.length > 0

  const isStreaming = useChatStore((s) => s.isStreaming)
  const revertToSnapshot = useChatStore((s) => s.revertToSnapshot)
  const activeDevSessionId = useChatSessionStore((s) => s.activeDevSessionId)

  const canRevert = !!toolCall.commitHash && !!activeDevSessionId && !isStreaming

  const handleRevert = async () => {
    if (!toolCall.commitHash || !activeDevSessionId) return
    setReverting(true)
    try {
      await revertToSnapshot(activeDevSessionId, toolCall.commitHash, messageId)
    } finally {
      setReverting(false)
      setShowRevertConfirm(false)
    }
  }

  return (
    <div className={cn(
      'overflow-hidden',
      embedded
        ? 'rounded hover:bg-background-tertiary/30 transition-colors'
        : 'my-2 rounded-lg border border-border bg-background-secondary/50',
    )}>
      {/* Header */}
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className={cn(
          'w-full flex items-center gap-2 text-left transition-colors',
          embedded ? 'px-2 py-1.5' : 'px-3 py-2 hover:bg-background-tertiary/50',
        )}
      >
        <Terminal className="w-3.5 h-3.5 text-brand shrink-0" />
        <span className="text-xs font-mono font-medium text-foreground truncate">
          {toolCall.name}
        </span>
        <span className="flex-1" />
        {canRevert && (
          <button
            type="button"
            onClick={(e) => { e.stopPropagation(); setShowRevertConfirm(true) }}
            className="flex items-center gap-1 text-[10px] font-mono text-foreground-subtle hover:text-amber-400 transition-colors px-1.5 py-0.5 rounded hover:bg-background-tertiary/80"
            title={t('revert.button')}
          >
            <RotateCcw className="w-3 h-3" />
          </button>
        )}
        <span className={cn('flex items-center gap-1 text-[10px] font-mono', status.color)}>
          {status.icon}
          {status.label}
        </span>
        {expanded ? (
          <ChevronDown className="w-3 h-3 text-foreground-muted shrink-0" />
        ) : (
          <ChevronRight className="w-3 h-3 text-foreground-muted shrink-0" />
        )}
      </button>

      {/* Revert confirmation */}
      {showRevertConfirm && (
        <div className="border-t border-amber-500/30 bg-amber-500/5 px-3 py-2.5">
          <p className="text-xs text-foreground-muted mb-2">{t('revert.confirmMessage')}</p>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={handleRevert}
              disabled={reverting}
              className="flex items-center gap-1.5 text-[11px] font-medium px-3 py-1 rounded bg-amber-500/20 text-amber-400 hover:bg-amber-500/30 transition-colors disabled:opacity-50"
            >
              {reverting && <Loader2 className="w-3 h-3 animate-spin" />}
              {t('revert.confirm')}
            </button>
            <button
              type="button"
              onClick={() => setShowRevertConfirm(false)}
              className="text-[11px] font-medium px-3 py-1 rounded text-foreground-muted hover:bg-background-tertiary/80 transition-colors"
            >
              {t('revert.cancel')}
            </button>
          </div>
        </div>
      )}

      {/* Arguments preview (always visible for key info) */}
      {typeof toolCall.arguments.command === 'string' && (
        <div className="px-3 pb-2 -mt-0.5">
          <code className="text-[11px] font-mono text-foreground-muted">
            $ {toolCall.arguments.command}
          </code>
        </div>
      )}

      {/* Expanded details */}
      {expanded && (
        <div className="border-t border-border">
          {/* Full arguments */}
          <div className="px-3 py-2">
            <pre className="text-[10px] font-mono text-foreground-subtle whitespace-pre-wrap">
              {formatArgs(toolCall.arguments)}
            </pre>
          </div>

          {/* Output */}
          {hasOutput && (
            <div className="border-t border-border px-3 py-2">
              <span className="text-[9px] font-mono text-foreground-subtle uppercase tracking-wider">
                {t('toolCall.output')}
              </span>
              <pre className="mt-1 text-[11px] font-mono text-foreground-muted whitespace-pre-wrap max-h-[200px] overflow-y-auto">
                {toolCall.result}
              </pre>
            </div>
          )}
        </div>
      )}
    </div>
  )
}
