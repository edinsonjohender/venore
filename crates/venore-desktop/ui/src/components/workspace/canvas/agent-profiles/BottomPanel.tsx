// =============================================================================
// BottomPanel — Console + Report tabs with resize/collapse
// =============================================================================

import { useState, useRef, useCallback, useLayoutEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { Terminal, BarChart3, ChevronUp, ChevronDown } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { PipelineStepDto, RunAnalysisContextDto } from '@/lib/tauri'
import type { ConsoleEntry, PipelineRun } from './types'
import { STAGE_COLORS } from './types'
import { ReportPanel } from './ReportPanel'

const MIN_HEIGHT = 80
const DEFAULT_RATIO = 0.6
const HEADER_HEIGHT = 32   // h-8

type BottomTab = 'console' | 'report'

interface BottomPanelProps {
  entries: ConsoleEntry[]
  steps: PipelineStepDto[] | null
  run: PipelineRun | null
  analysisContext?: RunAnalysisContextDto | null
  activeTab: BottomTab
  onTabChange: (tab: BottomTab) => void
}

export function BottomPanel({ entries, steps, run, analysisContext, activeTab, onTabChange }: BottomPanelProps) {
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
      {/* Resize handle */}
      {expanded && (
        <div
          onPointerDown={onPointerDown}
          className="h-1 shrink-0 cursor-row-resize hover:bg-brand/30 active:bg-brand/50 transition-colors"
        />
      )}

      {/* Header with tabs */}
      <div className="w-full h-8 shrink-0 px-3 flex items-center gap-0 bg-[#0c0c0e]">
        {/* Console tab */}
        <button
          onClick={() => onTabChange('console')}
          className={cn(
            'flex items-center gap-1.5 px-2.5 h-full text-[11px] font-medium transition-colors border-b-2',
            activeTab === 'console'
              ? 'text-foreground border-brand'
              : 'text-foreground-muted/60 border-transparent hover:text-foreground-muted',
          )}
        >
          <Terminal className="w-3.5 h-3.5" />
          {t('bottomPanel.console')}
          <span className="text-[10px] text-foreground-muted/50 ml-0.5">
            ({entries.length})
          </span>
        </button>

        {/* Report tab */}
        <button
          onClick={() => onTabChange('report')}
          className={cn(
            'flex items-center gap-1.5 px-2.5 h-full text-[11px] font-medium transition-colors border-b-2',
            activeTab === 'report'
              ? 'text-foreground border-brand'
              : 'text-foreground-muted/60 border-transparent hover:text-foreground-muted',
          )}
        >
          <BarChart3 className="w-3.5 h-3.5" />
          {t('bottomPanel.report')}
        </button>

        <div className="flex-1" />

        {/* Expand/collapse */}
        <button
          onClick={() => setExpanded(!expanded)}
          className="w-6 h-6 rounded flex items-center justify-center text-foreground-muted hover:text-foreground hover:bg-white/[0.06] transition-colors"
        >
          {expanded
            ? <ChevronDown className="w-3.5 h-3.5" />
            : <ChevronUp className="w-3.5 h-3.5" />
          }
        </button>
      </div>

      {/* Content */}
      {expanded && (
        <div className="flex-1 min-h-0 overflow-hidden">
          {activeTab === 'console' ? (
            <div className="h-full overflow-y-auto bg-[#09090b] font-mono text-[11px] p-2">
              {entries.length === 0 ? (
                <div className="h-full flex items-center justify-center text-foreground-muted/40 text-xs">
                  {t('bottomPanel.noActivity')}
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
          ) : (
            <div className="h-full bg-[#09090b]">
              <ReportPanel steps={steps} run={run} analysisContext={analysisContext ?? null} />
            </div>
          )}
        </div>
      )}
    </div>
  )
}
