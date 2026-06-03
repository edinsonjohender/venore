// =============================================================================
// HexPanelContent — Content for hexagon floating panel
// =============================================================================
// Shows: status controls, phase/progress, badges, description, notes,
// blocked-by, evidence, and a direct input to the agent.

import { useState } from 'react'
import { Play, Pause, Square, RotateCcw, Send } from 'lucide-react'
import { cn } from '@/lib/utils'
import { toast } from 'sonner'
import { useChatSessionStore } from '@/stores/chatSessionStore'
import { useChatStore } from '@/stores/chatStore'
import { usePanelStore } from '@/stores/panelStore'
import { useCanvasTabStore } from '@/stores/canvasTabStore'
import { PHASE_COLORS, PHASE_LABELS, fillOpacityForPercentage } from './hex-colors'
import type { HexPanelData } from '@/stores/hexFloatingStore'

type HexStatus = 'idle' | 'running' | 'paused' | 'completed' | 'cancelled'

interface HexPanelContentProps {
  data: HexPanelData
}

export function HexPanelContent({ data }: HexPanelContentProps) {
  const { hex, evidence } = data
  const color = hex.isDeadEnd ? PHASE_COLORS['dead-end'] : PHASE_COLORS[hex.phase]

  const [status, setStatus] = useState<HexStatus>(hex.percentage >= 85 ? 'completed' : 'running')
  const [message, setMessage] = useState('')
  const [sending, setSending] = useState(false)

  const handleSend = async () => {
    if (!message.trim() || sending) return
    const text = message.trim()
    setMessage('')
    setSending(true)

    try {
      const { tabs, activeTabId } = useCanvasTabStore.getState()
      const tab = tabs.find((t) => t.id === activeTabId)
      const featureId = tab?.type === 'knowledge' ? tab.data?.featureId : undefined

      const sessionId = await useChatSessionStore.getState().getOrCreateSendableSession()
      await useChatStore.getState().sendMessage(
        `Regarding hexagon "${hex.title}" (${hex.id}): ${text}`,
        sessionId, null, undefined, undefined, undefined, featureId,
      )
      usePanelStore.getState().setMode('chat', 'docked')
    } catch (err) {
      toast.error('Failed to send message')
    } finally {
      setSending(false)
    }
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      handleSend()
    }
  }

  return (
    <div className="flex-1 flex flex-col min-h-0">
      <div className="flex-1 overflow-y-auto">
        {/* Status controls */}
        <section className="px-3 py-2 border-b border-border">
          <div className="flex items-center gap-1.5">
            {status === 'idle' && (
              <button
                onClick={() => setStatus('running')}
                className="flex items-center gap-1 px-2 py-0.5 rounded text-[10px] font-medium bg-brand/15 text-brand hover:bg-brand/25 transition-colors"
              >
                <Play className="w-2.5 h-2.5" />
                Start
              </button>
            )}
            {status === 'running' && (
              <>
                <button
                  onClick={() => setStatus('paused')}
                  className="flex items-center gap-1 px-2 py-0.5 rounded text-[10px] font-medium bg-amber-500/15 text-amber-400 hover:bg-amber-500/25 transition-colors"
                >
                  <Pause className="w-2.5 h-2.5" />
                  Pause
                </button>
                <button
                  onClick={() => setStatus('cancelled')}
                  className="flex items-center gap-1 px-2 py-0.5 rounded text-[10px] font-medium bg-red-500/10 text-red-400 hover:bg-red-500/20 transition-colors"
                >
                  <Square className="w-2.5 h-2.5" />
                  Cancel
                </button>
              </>
            )}
            {status === 'paused' && (
              <>
                <button
                  onClick={() => setStatus('running')}
                  className="flex items-center gap-1 px-2 py-0.5 rounded text-[10px] font-medium bg-brand/15 text-brand hover:bg-brand/25 transition-colors"
                >
                  <Play className="w-2.5 h-2.5" />
                  Resume
                </button>
                <button
                  onClick={() => setStatus('cancelled')}
                  className="flex items-center gap-1 px-2 py-0.5 rounded text-[10px] font-medium bg-red-500/10 text-red-400 hover:bg-red-500/20 transition-colors"
                >
                  <Square className="w-2.5 h-2.5" />
                  Cancel
                </button>
              </>
            )}
            {status === 'completed' && (
              <span className="text-[10px] font-medium text-emerald-400">Completed</span>
            )}
            {status === 'cancelled' && (
              <button
                onClick={() => setStatus('running')}
                className="flex items-center gap-1 px-2 py-0.5 rounded text-[10px] font-medium bg-brand/15 text-brand hover:bg-brand/25 transition-colors"
              >
                <RotateCcw className="w-2.5 h-2.5" />
                Restart
              </button>
            )}

            <div className="flex-1" />

            {/* Status dot */}
            <span className={cn(
              'w-1.5 h-1.5 rounded-full',
              status === 'running' && 'bg-brand animate-pulse',
              status === 'paused' && 'bg-amber-400',
              status === 'completed' && 'bg-emerald-400',
              status === 'cancelled' && 'bg-red-400',
              status === 'idle' && 'bg-foreground-subtle',
            )} />
          </div>
        </section>

        {/* Phase + Progress */}
        <section className="px-3 py-2 border-b border-border">
          <div className="flex items-center gap-2 mb-2">
            <span
              className="w-2 h-2 rounded-full shrink-0"
              style={{ backgroundColor: color }}
            />
            <span className="text-[10px] font-medium text-foreground-muted uppercase tracking-wider">
              {PHASE_LABELS[hex.phase]}
            </span>
          </div>
          <div className="flex items-center gap-2">
            <div className="h-1.5 flex-1 rounded-full bg-white/5 overflow-hidden">
              <div
                className="h-full rounded-full transition-all"
                style={{
                  width: `${hex.percentage}%`,
                  backgroundColor: color,
                  opacity: fillOpacityForPercentage(hex.percentage) + 0.3,
                }}
              />
            </div>
            <span className="text-[10px] text-foreground-subtle w-8 text-right">{hex.percentage}%</span>
          </div>
        </section>

        {/* Badges */}
        <section className="px-3 py-2 border-b border-border">
          <div className="flex flex-wrap gap-1.5">
            <span className={cn(
              'px-1.5 py-0.5 rounded text-[10px] font-medium capitalize',
              hex.confidence === 'high' ? 'bg-emerald-500/15 text-emerald-400'
                : hex.confidence === 'medium' ? 'bg-amber-500/15 text-amber-400'
                  : 'bg-red-500/15 text-red-400',
            )}>
              {hex.confidence} confidence
            </span>
            <span className={cn(
              'px-1.5 py-0.5 rounded text-[10px] font-medium capitalize',
              hex.risk === 'low' ? 'bg-emerald-500/15 text-emerald-400'
                : hex.risk === 'medium' ? 'bg-amber-500/15 text-amber-400'
                  : 'bg-red-500/15 text-red-400',
            )}>
              {hex.risk} risk
            </span>
            {hex.isDeadEnd && (
              <span className="px-1.5 py-0.5 rounded text-[10px] font-medium bg-red-500/15 text-red-400">
                Dead End
              </span>
            )}
          </div>
        </section>

        {/* Description */}
        <section className="px-3 py-2 border-b border-border">
          <p className="text-[11px] text-foreground-muted leading-relaxed">{hex.description}</p>
          {hex.notes && (
            <p className="text-[10px] text-foreground-subtle italic mt-1.5">{hex.notes}</p>
          )}
        </section>

        {/* Blocked by */}
        {hex.blockedBy.length > 0 && (
          <section className="px-3 py-2 border-b border-border">
            <span className="text-[10px] text-foreground-subtle font-medium uppercase tracking-wider">
              Blocked by
            </span>
            <div className="mt-1 flex flex-col gap-0.5">
              {hex.blockedBy.map((id) => (
                <button
                  key={id}
                  className="text-[10px] text-foreground-muted hover:text-foreground truncate text-left transition-colors"
                >
                  {id}
                </button>
              ))}
            </div>
          </section>
        )}

        {/* Evidence */}
        {evidence.length > 0 && (
          <section className="px-3 py-2">
            <span className="text-[10px] text-foreground-subtle font-medium uppercase tracking-wider">
              Evidence ({evidence.length})
            </span>
            <div className="mt-1 flex flex-col gap-1">
              {evidence.map((ev) => (
                <div key={ev.id} className="rounded bg-white/5 px-2 py-1.5">
                  <span className="text-[10px] text-foreground-muted block">{ev.title}</span>
                  {ev.url && (
                    <span className="text-[9px] text-foreground-subtle truncate block mt-0.5">{ev.url}</span>
                  )}
                  {ev.content && (
                    <span className="text-[9px] text-foreground-subtle block mt-0.5">{ev.content}</span>
                  )}
                </div>
              ))}
            </div>
          </section>
        )}
      </div>

      {/* Direct input to agent */}
      <div className="shrink-0 border-t border-border px-2 py-1.5">
        <div className="flex items-center gap-1.5">
          <input
            type="text"
            value={message}
            onChange={(e) => setMessage(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Send instruction to agent..."
            className="flex-1 bg-transparent text-[11px] text-foreground placeholder:text-foreground-subtle outline-none"
          />
          <button
            onClick={handleSend}
            disabled={!message.trim() || sending}
            className={cn(
              'shrink-0 p-1 rounded transition-colors',
              message.trim() && !sending
                ? 'text-brand hover:bg-brand/15'
                : 'text-foreground-subtle',
            )}
          >
            <Send className="w-3 h-3" />
          </button>
        </div>
      </div>
    </div>
  )
}
