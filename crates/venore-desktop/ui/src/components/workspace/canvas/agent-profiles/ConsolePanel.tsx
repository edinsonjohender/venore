// =============================================================================
// ConsolePanel — Collapsible & resizable log panel for pipeline activity
// =============================================================================

import { useState, useRef, useCallback, useLayoutEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { Terminal, ChevronUp, ChevronDown } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { ConsoleEntry } from './types'
import { STAGE_COLORS } from './types'

const MIN_HEIGHT = 80
const DEFAULT_RATIO = 0.6
const HEADER_HEIGHT = 32   // h-8
const HANDLE_HEIGHT = 4    // h-1

interface ConsolePanelProps {
  entries: ConsoleEntry[]
}

export function ConsolePanel({ entries }: ConsolePanelProps) {
  const { t } = useTranslation('agents')
  const [expanded, setExpanded] = useState(true)
  const [totalHeight, setTotalHeight] = useState<number | null>(null)
  const containerRef = useRef<HTMLDivElement>(null)
  const dragging = useRef(false)
  const startY = useRef(0)
  const startH = useRef(0)

  // Measure parent on first expand to set 60% height
  useLayoutEffect(() => {
    if (expanded && totalHeight === null && containerRef.current) {
      const parentH = containerRef.current.parentElement?.clientHeight ?? 400
      setTotalHeight(Math.round(parentH * DEFAULT_RATIO))
    }
  }, [expanded, totalHeight])

  const onPointerDown = useCallback((e: React.PointerEvent) => {
    if (totalHeight === null) return
    e.preventDefault()
    dragging.current = true
    startY.current = e.clientY
    startH.current = totalHeight

    const onPointerMove = (ev: PointerEvent) => {
      if (!dragging.current) return
      const delta = startY.current - ev.clientY
      const next = startH.current + delta
      setTotalHeight(Math.max(MIN_HEIGHT, next))
    }

    const onPointerUp = () => {
      dragging.current = false
      document.removeEventListener('pointermove', onPointerMove)
      document.removeEventListener('pointerup', onPointerUp)
    }

    document.addEventListener('pointermove', onPointerMove)
    document.addEventListener('pointerup', onPointerUp)
  }, [totalHeight])

  // When collapsed: just the header, shrink-0
  // When expanded: desired height but can shrink via flex (no shrink-0)
  const outerStyle = expanded
    ? { height: totalHeight ?? 200, minHeight: MIN_HEIGHT }
    : undefined

  return (
    <div
      ref={containerRef}
      className={cn(
        'border-t border-border relative z-10',
        expanded ? 'flex flex-col min-h-0' : 'shrink-0',
      )}
      style={outerStyle}
    >
      {/* Resize handle — only visible when expanded */}
      {expanded && (
        <div
          onPointerDown={onPointerDown}
          className="h-1 shrink-0 cursor-row-resize hover:bg-brand/30 active:bg-brand/50 transition-colors"
        />
      )}

      {/* Header */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full h-8 shrink-0 px-3 flex items-center gap-2 bg-[#0c0c0e] hover:bg-[#111114] transition-colors"
      >
        <Terminal className="w-3.5 h-3.5 text-foreground-muted" />
        <span className="text-[11px] font-medium uppercase tracking-wider text-foreground-muted">
          {t('console.title')}
        </span>
        <span className="text-[10px] text-foreground-muted/50 ml-1">
          ({entries.length})
        </span>
        <div className="flex-1" />
        {expanded ? (
          <ChevronDown className="w-3.5 h-3.5 text-foreground-muted" />
        ) : (
          <ChevronUp className="w-3.5 h-3.5 text-foreground-muted" />
        )}
      </button>

      {/* Content — flex-1 so it fills remaining space and shrinks with parent */}
      {expanded && (
        <div className="flex-1 min-h-0 overflow-y-auto bg-[#09090b] font-mono text-[11px] p-2">
          {entries.length === 0 ? (
            <div className="h-full flex items-center justify-center text-foreground-muted/40 text-xs">
              {t('console.noActivity')}
            </div>
          ) : (
            <div className="space-y-0.5">
              {entries.map((entry, i) => {
                const colors = STAGE_COLORS[entry.stage as keyof typeof STAGE_COLORS] ?? STAGE_COLORS.specialist
                const time = new Date(entry.timestamp).toLocaleTimeString('en-US', {
                  hour12: false,
                  hour: '2-digit',
                  minute: '2-digit',
                  second: '2-digit',
                })
                return (
                  <div key={i} className="flex gap-2 leading-relaxed">
                    <span className="text-foreground-muted/50">[{time}]</span>
                    <span className={cn('font-medium', colors.text)}>
                      [{entry.agentName}]
                    </span>
                    <span className="text-foreground-muted">{entry.message}</span>
                  </div>
                )
              })}
            </div>
          )}
        </div>
      )}
    </div>
  )
}
