// =============================================================================
// CanvasHeader - Browser-style tab bar above the canvas
// =============================================================================
// Active tab merges with content below (no bottom border, same bg).
// Inactive tabs are recessed with subtle transparency. Sharp corners throughout.

import { useRef, useState, useCallback, useEffect } from 'react'
import { X, Waves, Hexagon, GitPullRequest, CircleDot, FileCode, Bot, GitBranch, ChevronLeft, ChevronRight } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'
import { AIIndicator } from '@/components/ai'
import { MeshIndicator } from './MeshIndicator'
import { useCanvasTabStore, type CanvasTab } from '@/stores/canvasTabStore'

// -----------------------------------------------------------------------------
// Tab icon per type
// -----------------------------------------------------------------------------

const TAB_ICONS: Record<CanvasTab['type'], typeof Waves> = {
  ocean: Waves,
  knowledge: Hexagon,
  pr: GitPullRequest,
  issue: CircleDot,
  file: FileCode,
  ai: Bot,
  session: GitBranch,
}

// -----------------------------------------------------------------------------
// Main Component
// -----------------------------------------------------------------------------

const SCROLL_STEP = 160

export function CanvasHeader() {
  const { t } = useTranslation('workspace')
  const tabs = useCanvasTabStore((s) => s.tabs)
  const activeTabId = useCanvasTabStore((s) => s.activeTabId)
  const setActiveTab = useCanvasTabStore((s) => s.setActiveTab)
  const closeTab = useCanvasTabStore((s) => s.closeTab)

  const scrollRef = useRef<HTMLDivElement>(null)
  const [canScrollLeft, setCanScrollLeft] = useState(false)
  const [canScrollRight, setCanScrollRight] = useState(false)

  const checkOverflow = useCallback(() => {
    const el = scrollRef.current
    if (!el) return
    setCanScrollLeft(el.scrollLeft > 0)
    setCanScrollRight(el.scrollLeft + el.clientWidth < el.scrollWidth - 1)
  }, [])

  useEffect(() => {
    checkOverflow()
    const el = scrollRef.current
    if (!el) return
    const ro = new ResizeObserver(checkOverflow)
    ro.observe(el)
    return () => ro.disconnect()
  }, [checkOverflow, tabs.length])

  const scroll = useCallback((dir: -1 | 1) => {
    scrollRef.current?.scrollBy({ left: dir * SCROLL_STEP, behavior: 'smooth' })
  }, [])

  const showArrows = canScrollLeft || canScrollRight

  return (
    <div className="flex items-center h-9 bg-[hsl(var(--canvas-tab-bar))] shrink-0 relative">
      {/* Scroll left */}
      {canScrollLeft && (
        <button
          onClick={() => scroll(-1)}
          className="shrink-0 w-7 h-full flex items-center justify-center text-foreground-subtle hover:text-foreground transition-colors"
        >
          <ChevronLeft className="w-3.5 h-3.5" />
        </button>
      )}

      {/* Tabs */}
      <div
        ref={scrollRef}
        onScroll={checkOverflow}
        className="flex items-center h-full overflow-x-hidden min-w-0"
      >
        {tabs.map((tab, i) => {
          const isActive = tab.id === activeTabId
          const prevActive = i > 0 && tabs[i - 1].id === activeTabId
          const Icon = TAB_ICONS[tab.type]
          const showDivider = i > 0 && !isActive && !prevActive

          return (
            <div key={tab.id} className="flex items-center h-full">
              {showDivider && (
                <div className="w-px h-4 bg-border shrink-0" />
              )}
              <button
                onClick={() => setActiveTab(tab.id)}
                className={cn(
                  'group h-full px-3.5 text-xs flex items-center gap-2 shrink-0 max-w-[220px] transition-colors relative',
                  isActive
                    ? 'bg-background-tertiary text-foreground z-[1]'
                    : 'text-foreground-subtle hover:text-foreground-muted hover:bg-white/[0.04]',
                )}
              >
                <Icon className="w-3.5 h-3.5 shrink-0 opacity-50" />
                <span className="truncate">{tab.label}</span>
                {tab.data?.isDirty && (
                  <span className="w-1.5 h-1.5 rounded-full bg-foreground/60 shrink-0" />
                )}
                {tab.type !== 'ocean' && (
                  <span
                    onClick={(e) => {
                      e.stopPropagation()
                      if (tab.data?.isDirty) {
                        if (!window.confirm(t('canvasHeader.unsavedChanges', { label: tab.label }))) return
                      }
                      closeTab(tab.id)
                    }}
                    className={cn(
                      'shrink-0 w-4 h-4 flex items-center justify-center transition-opacity',
                      'hover:bg-foreground/10',
                      isActive ? 'opacity-40 hover:opacity-80' : 'opacity-0 group-hover:opacity-40 hover:!opacity-80',
                    )}
                  >
                    <X className="w-3 h-3" />
                  </span>
                )}
              </button>
            </div>
          )
        })}
      </div>

      {/* Scroll right */}
      {canScrollRight && (
        <button
          onClick={() => scroll(1)}
          className="shrink-0 w-7 h-full flex items-center justify-center text-foreground-subtle hover:text-foreground transition-colors"
        >
          <ChevronRight className="w-3.5 h-3.5" />
        </button>
      )}

      <div className="flex-1" />

      {/* Right: Mesh + AI Indicator */}
      <MeshIndicator className="mr-1" />
      <AIIndicator className="mr-2" />

      {/* Bottom line — visible under inactive tabs, active tab covers it */}
      <div className="absolute bottom-0 left-0 right-0 h-px bg-border" />
    </div>
  )
}
