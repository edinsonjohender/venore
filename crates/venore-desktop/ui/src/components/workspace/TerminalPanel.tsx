// =============================================================================
// TerminalPanel - Docked bottom panel with embedded terminal
// =============================================================================
// Sits at the bottom of the canvas zone. HorizontalResizeHandle on top,
// TerminalTabBar, and xterm.js instances per tab.
// xterm instances are ALWAYS mounted (never unmounted on minimize) so that
// running processes and scrollback survive panel open/close.

import { useCallback, useRef, useEffect, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { cn } from '@/lib/utils'
import { tauriApi, type TerminalAiSpawnedPayload, type TerminalDeadPayload, type TerminalSessionSpawnedPayload } from '@/lib/tauri'
import { HorizontalResizeHandle } from '@/components/ui/horizontal-resize-handle'
import { TerminalTabBar } from './TerminalTabBar'
import { useTerminalStore } from '@/stores/terminalStore'
import { useTerminal } from '@/hooks/useTerminal'
import { useResizableBottomPanel } from '@/hooks/useResizableBottomPanel'

// -----------------------------------------------------------------------------
// Constants
// -----------------------------------------------------------------------------

const PANEL_ANIM_DURATION = 200
const INITIAL_HEIGHT = 250
const MIN_HEIGHT = 100
const MAX_HEIGHT = 600

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

export interface TerminalMethods {
  clear: () => void
  copySelection: () => Promise<void>
  paste: () => Promise<void>
  focus: () => void
}

interface TerminalPanelProps {
  projectPath: string
}

// -----------------------------------------------------------------------------
// TerminalTabContent - Single xterm instance with ResizeObserver
// -----------------------------------------------------------------------------

function TerminalTabContent({
  terminalId,
  isActive,
  isPanelOpen,
  onMethodsReady,
}: {
  terminalId: string
  isActive: boolean
  isPanelOpen: boolean
  onMethodsReady: (methods: TerminalMethods | null) => void
}) {
  const containerRef = useRef<HTMLDivElement>(null)
  const { fit, clear, copySelection, paste, focus } = useTerminal({ terminalId, containerRef })

  // Report methods when this tab becomes active, null when inactive
  useEffect(() => {
    if (isActive) {
      onMethodsReady({ clear, copySelection, paste, focus })
    } else {
      onMethodsReady(null)
    }
  }, [isActive, clear, copySelection, paste, focus]) // eslint-disable-line react-hooks/exhaustive-deps

  // Re-fit on container resize
  useEffect(() => {
    const el = containerRef.current
    if (!el) return

    const observer = new ResizeObserver(() => {
      if (isActive && isPanelOpen) fit()
    })
    observer.observe(el)
    return () => observer.disconnect()
  }, [isActive, isPanelOpen, fit])

  // Fit when becoming active or when panel reopens
  useEffect(() => {
    if (isActive && isPanelOpen) {
      requestAnimationFrame(() => fit())
    }
  }, [isActive, isPanelOpen, fit])

  return (
    <div
      ref={containerRef}
      className={cn(
        'absolute inset-0 px-3 py-1',
        !isActive && 'invisible',
      )}
    />
  )
}

// -----------------------------------------------------------------------------
// Main Component
// -----------------------------------------------------------------------------

export function TerminalPanel({ projectPath }: TerminalPanelProps) {
  const { isOpen, tabs, activeTabId, close, addTab, removeTab } = useTerminalStore()
  const [activeTerminalMethods, setActiveTerminalMethods] = useState<TerminalMethods | null>(null)

  const resizable = useResizableBottomPanel({
    initialHeight: INITIAL_HEIGHT,
    minHeight: MIN_HEIGHT,
    maxHeight: MAX_HEIGHT,
    onClose: close,
  })

  // Auto-spawn first terminal when opening with no tabs
  const hasAutoSpawned = useRef(false)
  useEffect(() => {
    if (isOpen && tabs.length === 0 && !hasAutoSpawned.current) {
      hasAutoSpawned.current = true
      spawnNewTab(projectPath, addTab)
    }
    if (!isOpen) {
      hasAutoSpawned.current = false
    }
  }, [isOpen, tabs.length, projectPath, addTab])

  // Listen for AI auto-spawned terminals — open panel + add tab
  useEffect(() => {
    const unlisten = listen<TerminalAiSpawnedPayload>('terminal:ai-spawned', (event) => {
      const { terminal_id } = event.payload
      const { tabs, addTab, open } = useTerminalStore.getState()
      // Only add if not already tracked
      if (!tabs.some((t) => t.id === terminal_id)) {
        addTab(terminal_id)
      }
      open()
    })
    return () => { unlisten.then((fn) => fn()) }
  }, [])

  // Listen for session-bound terminal spawns — open panel + add session tab
  useEffect(() => {
    const unlisten = listen<TerminalSessionSpawnedPayload>('terminal:session-spawned', (event) => {
      const { terminal_id, dev_session_id, label } = event.payload
      const { addSessionTab } = useTerminalStore.getState()
      addSessionTab(terminal_id, dev_session_id, label)
    })
    return () => { unlisten.then((fn) => fn()) }
  }, [])

  // Listen for dead terminals — remove their tab automatically
  useEffect(() => {
    const unlisten = listen<TerminalDeadPayload>('terminal:dead', (event) => {
      const { terminal_id } = event.payload
      const { removeTab } = useTerminalStore.getState()
      removeTab(terminal_id)
    })
    return () => { unlisten.then((fn) => fn()) }
  }, [])

  const handleNewTab = useCallback(() => {
    spawnNewTab(projectPath, addTab)
  }, [projectPath, addTab])

  const handleCloseTab = useCallback(
    (terminalId: string) => {
      tauriApi.killTerminal({ terminal_id: terminalId }).catch(() => {})
      removeTab(terminalId)
    },
    [removeTab],
  )

  // Nothing to render if no tabs exist and panel is closed
  if (tabs.length === 0 && !isOpen) return null

  const targetHeight = resizable.height + 4 // +4 for resize handle

  return (
    <div
      className="shrink-0 flex flex-col overflow-hidden"
      style={{
        height: isOpen ? targetHeight : 0,
        transition: `height ${PANEL_ANIM_DURATION}ms ease-out`,
      }}
    >
      <HorizontalResizeHandle onDrag={resizable.handleDrag} onDragEnd={resizable.handleDragEnd} />

      <div
        className={cn(
          'flex-1 flex flex-col min-h-0',
          resizable.closePending && 'opacity-40 transition-opacity',
        )}
      >
        <TerminalTabBar
          onNewTab={handleNewTab}
          onCloseTab={handleCloseTab}
          onClosePanel={close}
          onCopy={activeTerminalMethods?.copySelection}
          onPaste={activeTerminalMethods?.paste}
          onClear={activeTerminalMethods?.clear}
        />

        {/* Terminal content area — always mounted to preserve xterm instances */}
        <div className="flex-1 relative bg-[#09090b]">
          {tabs.map((tab) => (
            <TerminalTabContent
              key={tab.id}
              terminalId={tab.id}
              isActive={tab.id === activeTabId}
              isPanelOpen={isOpen}
              onMethodsReady={(methods) => {
                if (tab.id === activeTabId) setActiveTerminalMethods(methods)
              }}
            />
          ))}
        </div>
      </div>
    </div>
  )
}

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

async function spawnNewTab(projectPath: string, addTab: (id: string) => void) {
  try {
    const label = projectPath.split(/[/\\]/).filter(Boolean).pop()
    const response = await tauriApi.spawnTerminal({ cwd: projectPath, label })
    addTab(response.terminal_id)
  } catch (e) {
    console.error('Failed to spawn terminal:', e)
  }
}
