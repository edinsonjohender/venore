// =============================================================================
// ActivityBar - VS Code-style vertical icon sidebar
// =============================================================================
// Fixed left bar with icon-only buttons. Each toggles its panel (docked/closed).
// Top: reorderable panel icons + terminal. Bottom: agents.
// Drag-to-reorder is supported for the middle group — order persists via Zustand.

import { useCallback, useRef, useState, useEffect, useMemo } from 'react'
import { Terminal, Bot } from 'lucide-react'
import { AuthButton } from './AuthButton'
import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'
import { PANEL_MAP, PANEL_REGISTRY } from './panels'
import type { PanelDefinition } from './panels'
import { usePanelMode, usePanelStore, DEFAULT_ACTIVITY_BAR_ORDER } from '@/stores/panelStore'
import { useTerminalStore } from '@/stores/terminalStore'
import { useCanvasTabStore } from '@/stores/canvasTabStore'
import { useWorkspaceFeatureStore, FEATURE_MATRIX } from '@/stores/workspaceFeatureStore'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface ActivityBarProps {}

// All possible reorderable IDs: panel IDs from registry + terminal + agents + prompts
const ALL_ACTIVITY_IDS = new Set([...PANEL_REGISTRY.map((d) => d.id), 'terminal', 'ai'])

// -----------------------------------------------------------------------------
// PanelButton — one icon per registered panel
// -----------------------------------------------------------------------------

function PanelButton({ def, isDragging }: { def: PanelDefinition; isDragging?: boolean }) {
  const { t } = useTranslation()
  const mode = usePanelMode(def.id)
  const { setMode } = usePanelStore()
  const isActive = mode === 'docked' || mode === 'floating'
  const restMode = def.collapsedContent ? 'collapsed' : 'closed'
  const Icon = def.icon

  const handleClick = useCallback(() => {
    if (isDragging) return
    if (isActive) {
      setMode(def.id, restMode)
    } else {
      setMode(def.id, 'docked')
    }
  }, [isActive, def.id, restMode, setMode, isDragging])

  return (
    <button
      className={cn(
        'relative flex items-center justify-center w-full h-10 transition-colors',
        'text-foreground-subtle hover:text-foreground',
        isActive && 'text-foreground',
      )}
      title={t(def.titleKey)}
      onClick={handleClick}
    >
      {/* Active indicator — left accent bar */}
      {isActive && (
        <div className="absolute left-0 top-1.5 bottom-1.5 w-0.5 bg-brand rounded-r" />
      )}
      <Icon className="w-[18px] h-[18px]" />
    </button>
  )
}

// -----------------------------------------------------------------------------
// TerminalButton
// -----------------------------------------------------------------------------

function TerminalButton({ isDragging }: { isDragging?: boolean }) {
  const { t } = useTranslation('workspace')
  const { isOpen, toggle } = useTerminalStore()

  const handleClick = useCallback(() => {
    if (isDragging) return
    toggle()
  }, [toggle, isDragging])

  return (
    <button
      className={cn(
        'relative flex items-center justify-center w-full h-10 transition-colors',
        'text-foreground-subtle hover:text-foreground',
        isOpen && 'text-foreground',
      )}
      title={t('activityBar.terminal')}
      onClick={handleClick}
    >
      {isOpen && (
        <div className="absolute left-0 top-1.5 bottom-1.5 w-0.5 bg-brand rounded-r" />
      )}
      <Terminal className="w-[18px] h-[18px]" />
    </button>
  )
}

// -----------------------------------------------------------------------------
// AgentsButton
// -----------------------------------------------------------------------------

function AIButton({ isDragging }: { isDragging?: boolean }) {
  const { t } = useTranslation('workspace')
  const activeTabId = useCanvasTabStore((s) => s.activeTabId)
  const openAI = useCanvasTabStore((s) => s.openAI)
  const isActive = activeTabId === 'ai'

  const handleClick = useCallback(() => {
    if (isDragging) return
    openAI()
  }, [openAI, isDragging])

  return (
    <button
      className={cn(
        'relative flex items-center justify-center w-full h-10 transition-colors',
        'text-foreground-subtle hover:text-foreground',
        isActive && 'text-foreground',
      )}
      title={t('activityBar.ai', 'AI')}
      onClick={handleClick}
    >
      {isActive && (
        <div className="absolute left-0 top-1.5 bottom-1.5 w-0.5 bg-brand rounded-r" />
      )}
      <Bot className="w-[18px] h-[18px]" />
    </button>
  )
}

// -----------------------------------------------------------------------------
// useActivityBarDrag — drag-to-reorder hook
// -----------------------------------------------------------------------------

const DRAG_THRESHOLD = 4

function useActivityBarDrag(
  order: string[],
  setOrder: (order: string[]) => void,
  validIds: Set<string>,
  defaultOrder: string[],
) {
  const containerRef = useRef<HTMLDivElement>(null)
  const [dragId, setDragId] = useState<string | null>(null)
  const [dropIndex, setDropIndex] = useState<number | null>(null)
  const startY = useRef(0)
  const isDragging = useRef(false)
  const dragIdRef = useRef<string | null>(null)
  // Keep refs to latest values so the effect closure is always current
  const validIdsRef = useRef(validIds)
  validIdsRef.current = validIds
  const defaultOrderRef = useRef(defaultOrder)
  defaultOrderRef.current = defaultOrder

  const onDragStart = useCallback((id: string, e: React.MouseEvent) => {
    e.preventDefault()
    startY.current = e.clientY
    dragIdRef.current = id
    isDragging.current = false
  }, [])

  useEffect(() => {
    function onMouseMove(e: MouseEvent) {
      if (dragIdRef.current === null) return

      // Check threshold before entering drag mode
      if (!isDragging.current) {
        if (Math.abs(e.clientY - startY.current) < DRAG_THRESHOLD) return
        isDragging.current = true
        setDragId(dragIdRef.current)
        document.body.style.cursor = 'grabbing'
        document.body.style.userSelect = 'none'
      }

      // Calculate drop index from cursor Y position
      const container = containerRef.current
      if (!container) return

      const children = container.querySelectorAll<HTMLElement>('[data-reorderable]')
      const count = children.length
      let targetIdx = count
      for (let i = 0; i < count; i++) {
        const rect = children[i].getBoundingClientRect()
        const midY = rect.top + rect.height / 2
        if (e.clientY < midY) {
          targetIdx = i
          break
        }
      }
      setDropIndex(targetIdx)
    }

    function onMouseUp() {
      if (dragIdRef.current !== null && isDragging.current) {
        // Compute new order
        setDropIndex((currentDropIndex) => {
          if (currentDropIndex !== null) {
            const currentOrder = usePanelStore.getState().activityBarOrder
            const reconciledOrder = reconcileOrder(currentOrder, validIdsRef.current, defaultOrderRef.current)
            const fromIdx = reconciledOrder.indexOf(dragIdRef.current!)
            if (fromIdx !== -1) {
              const without = [...reconciledOrder]
              without.splice(fromIdx, 1)
              // Adjust target if it was after the removed item
              const adjustedIdx = currentDropIndex > fromIdx
                ? currentDropIndex - 1
                : currentDropIndex
              without.splice(adjustedIdx, 0, dragIdRef.current!)
              if (without.join(',') !== reconciledOrder.join(',')) {
                setOrder(without)
              }
            }
          }
          return null
        })
      }
      // Cleanup
      dragIdRef.current = null
      isDragging.current = false
      setDragId(null)
      document.body.style.cursor = ''
      document.body.style.userSelect = ''
    }

    document.addEventListener('mousemove', onMouseMove)
    document.addEventListener('mouseup', onMouseUp)
    return () => {
      document.removeEventListener('mousemove', onMouseMove)
      document.removeEventListener('mouseup', onMouseUp)
    }
  }, [setOrder])

  return { containerRef, dragId, dropIndex, onDragStart, isDragging: isDragging.current }
}

// -----------------------------------------------------------------------------
// Reconciliation — keep persisted order in sync with current registry
// -----------------------------------------------------------------------------

function reconcileOrder(persisted: string[], validIds: Set<string>, defaultOrder: string[]): string[] {
  // Filter out stale IDs and IDs not valid for this project type
  const filtered = persisted.filter((id) => validIds.has(id))
  // Append any new IDs not already present
  for (const id of defaultOrder) {
    if (!filtered.includes(id)) {
      filtered.push(id)
    }
  }
  return filtered
}

// -----------------------------------------------------------------------------
// DropIndicator — visual line showing where the item will land
// -----------------------------------------------------------------------------

function DropIndicator() {
  return (
    <div className="flex items-center justify-center py-0.5">
      <div className="h-0.5 w-6 rounded-full bg-brand shadow-[0_0_6px_var(--color-brand)]" />
    </div>
  )
}

// -----------------------------------------------------------------------------
// Main Component
// -----------------------------------------------------------------------------

export function ActivityBar(_props: ActivityBarProps) {
  const { t } = useTranslation('workspace')
  const projectType = useWorkspaceFeatureStore((s) => s.projectType)
  const activityBarOrder = usePanelStore((s) => s.activityBarOrder)
  const setActivityBarOrder = usePanelStore((s) => s.setActivityBarOrder)

  // Compute valid IDs and default order for the current project type
  const validIds = useMemo(() => {
    const features = FEATURE_MATRIX[projectType]
    const ids = new Set<string>()
    for (const id of ALL_ACTIVITY_IDS) {
      if (features.has(id as any)) ids.add(id)
    }
    return ids
  }, [projectType])

  const defaultOrder = useMemo(
    () => DEFAULT_ACTIVITY_BAR_ORDER.filter((id) => validIds.has(id)),
    [validIds],
  )

  const order = useMemo(
    () => reconcileOrder(activityBarOrder, validIds, defaultOrder),
    [activityBarOrder, validIds, defaultOrder],
  )
  const { containerRef, dragId, dropIndex, onDragStart } = useActivityBarDrag(
    order,
    setActivityBarOrder,
    validIds,
    defaultOrder,
  )

  return (
    <div className="shrink-0 w-11 flex flex-col border-r border-border bg-background-secondary">
      {/* Reorderable panel toggles */}
      <div className="flex flex-col mt-1" ref={containerRef}>
        {order.map((id, idx) => (
          <div key={id}>
            {/* Drop indicator — before this item */}
            {dragId !== null && dropIndex === idx && <DropIndicator />}
            <div
              data-reorderable="true"
              className={cn(
                'cursor-grab transition-all duration-150',
                dragId !== null && dragId === id && 'scale-110 opacity-80 z-10',
                dragId !== null && dragId !== id && 'opacity-40',
              )}
              onMouseDown={(e) => onDragStart(id, e)}
            >
              {id === 'terminal' ? (
                <TerminalButton isDragging={dragId !== null} />
              ) : id === 'ai' ? (
                <AIButton isDragging={dragId !== null} />
              ) : (
                (() => {
                  const def = PANEL_MAP.get(id)
                  return def ? (
                    <PanelButton def={def} isDragging={dragId !== null} />
                  ) : null
                })()
              )}
            </div>
          </div>
        ))}
        {/* Drop indicator — after last item */}
        {dragId !== null && dropIndex === order.length && <DropIndicator />}
      </div>

      {/* Spacer */}
      <div className="flex-1" />

      {/* Cloud auth (bottom) */}
      <div className="mb-1">
        <AuthButton />
      </div>
    </div>
  )
}
