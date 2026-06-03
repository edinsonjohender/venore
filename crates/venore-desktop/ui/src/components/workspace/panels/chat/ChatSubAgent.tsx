// =============================================================================
// ChatSubAgent - Inline card for spawn_agent tool calls
// =============================================================================
// Shows sub-agent type, task, and collapsible result.

import { useState } from 'react'
import { Bot, ChevronDown, ChevronRight, Loader2, Check, X } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'
import type { SubAgentPayload } from '@/stores/chatStore'

interface ChatSubAgentProps {
  payload: SubAgentPayload
  /** When true, renders without outer border/bg (for use inside overlay panels) */
  embedded?: boolean
}

export function ChatSubAgent({ payload, embedded }: ChatSubAgentProps) {
  const { t } = useTranslation('chat')
  const [expanded, setExpanded] = useState(false)
  const isRunning = payload.status === 'started'
  const isDone = payload.status === 'completed'
  const isFailed = payload.status === 'failed'

  const agentLabel = t(`subAgent.${payload.agent_type}`, { defaultValue: payload.agent_type })

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
        <Bot className="w-3.5 h-3.5 text-foreground-muted shrink-0" />
        <span className="text-xs font-mono font-medium text-foreground">
          {t('subAgent.agentLabel', { type: agentLabel })}
        </span>
        <span className="flex-1 text-xs text-foreground-muted truncate">
          {payload.task}
        </span>
        {isRunning && <Loader2 className="w-3 h-3 text-foreground-muted animate-spin shrink-0" />}
        {isDone && <Check className="w-3 h-3 text-emerald-400/70 shrink-0" />}
        {isFailed && <X className="w-3 h-3 text-red-400/70 shrink-0" />}
        {payload.result && (
          expanded
            ? <ChevronDown className="w-3 h-3 text-foreground-muted shrink-0" />
            : <ChevronRight className="w-3 h-3 text-foreground-muted shrink-0" />
        )}
      </button>

      {/* Expanded result */}
      {expanded && payload.result && (
        <div className="border-t border-border px-3 py-2">
          <pre className="text-[11px] font-mono text-foreground-muted whitespace-pre-wrap max-h-[200px] overflow-y-auto">
            {payload.result}
          </pre>
        </div>
      )}
    </div>
  )
}
