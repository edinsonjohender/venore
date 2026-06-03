// =============================================================================
// TerminalTabBar - Tab bar for terminal panel
// =============================================================================
// Shows tabs with names, + button for new tab, X per tab, close panel button.

import { useState, useRef, useEffect, useCallback } from 'react'
import { Plus, X, Terminal as TerminalIcon, ChevronDown, GitBranch, Copy, ClipboardPaste, Eraser } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'
import { useTerminalStore, type TerminalTab } from '@/stores/terminalStore'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface TerminalTabBarProps {
  onNewTab: () => void
  onCloseTab: (terminalId: string) => void
  onClosePanel: () => void
  onCopy?: () => Promise<void>
  onPaste?: () => Promise<void>
  onClear?: () => void
}

// -----------------------------------------------------------------------------
// Main Component
// -----------------------------------------------------------------------------

export function TerminalTabBar({ onNewTab, onCloseTab, onClosePanel, onCopy, onPaste, onClear }: TerminalTabBarProps) {
  const { t } = useTranslation('workspace')
  const { tabs, activeTabId, setActiveTab } = useTerminalStore()

  return (
    <div className="flex items-center h-9 bg-background-secondary/50 border-b border-border px-2 shrink-0 gap-2">
      {/* Section label */}
      <div className="flex items-center gap-1.5 text-foreground-subtle shrink-0 select-none">
        <TerminalIcon className="w-3.5 h-3.5" />
        <span className="text-[11px] font-medium uppercase tracking-wider">{t('terminalTabBar.terminal')}</span>
      </div>

      {/* Divider */}
      <div className="w-px h-4 bg-border shrink-0" />

      {/* Tabs */}
      <div className="flex items-center gap-1 flex-1 min-w-0 overflow-x-auto">
        {tabs.map((tab) => (
          <TabItem
            key={tab.id}
            tab={tab}
            isActive={tab.id === activeTabId}
            onSelect={() => setActiveTab(tab.id)}
            onClose={() => onCloseTab(tab.id)}
          />
        ))}

        {/* New tab button */}
        <button
          onClick={onNewTab}
          className="flex items-center justify-center w-6 h-6 rounded text-foreground-subtle hover:text-foreground hover:bg-background-tertiary transition-colors shrink-0"
          title={t('terminalTabBar.newTerminal')}
        >
          <Plus className="w-3.5 h-3.5" />
        </button>
      </div>

      {/* Action buttons */}
      <div className="flex items-center gap-0.5 shrink-0">
        <button
          onClick={() => onCopy?.()}
          disabled={!onCopy}
          className="flex items-center justify-center w-6 h-6 rounded text-foreground-subtle hover:text-foreground hover:bg-background-tertiary transition-colors disabled:opacity-30 disabled:pointer-events-none"
          title={t('terminalTabBar.copy')}
        >
          <Copy className="w-3.5 h-3.5" />
        </button>
        <button
          onClick={() => onPaste?.()}
          disabled={!onPaste}
          className="flex items-center justify-center w-6 h-6 rounded text-foreground-subtle hover:text-foreground hover:bg-background-tertiary transition-colors disabled:opacity-30 disabled:pointer-events-none"
          title={t('terminalTabBar.paste')}
        >
          <ClipboardPaste className="w-3.5 h-3.5" />
        </button>
        <button
          onClick={onClear}
          disabled={!onClear}
          className="flex items-center justify-center w-6 h-6 rounded text-foreground-subtle hover:text-foreground hover:bg-background-tertiary transition-colors disabled:opacity-30 disabled:pointer-events-none"
          title={t('terminalTabBar.clear')}
        >
          <Eraser className="w-3.5 h-3.5" />
        </button>
      </div>

      {/* Divider */}
      <div className="w-px h-4 bg-border shrink-0" />

      {/* Close panel button */}
      <button
        onClick={onClosePanel}
        className="flex items-center justify-center w-6 h-6 rounded text-foreground-subtle hover:text-foreground hover:bg-background-tertiary transition-colors shrink-0"
        title={t('terminalTabBar.closeTerminalPanel')}
      >
        <ChevronDown className="w-3.5 h-3.5" />
      </button>
    </div>
  )
}

// -----------------------------------------------------------------------------
// TabItem
// -----------------------------------------------------------------------------

function TabItem({
  tab,
  isActive,
  onSelect,
  onClose,
}: {
  tab: TerminalTab
  isActive: boolean
  onSelect: () => void
  onClose: () => void
}) {
  const { renameTab } = useTerminalStore()
  const [isEditing, setIsEditing] = useState(false)
  const [draft, setDraft] = useState(tab.name)
  const inputRef = useRef<HTMLInputElement>(null)

  const isSession = !!tab.devSessionId

  // Focus + select all when entering edit mode
  useEffect(() => {
    if (isEditing) {
      inputRef.current?.focus()
      inputRef.current?.select()
    }
  }, [isEditing])

  const commitRename = useCallback(() => {
    setIsEditing(false)
    renameTab(tab.id, draft)
  }, [tab.id, draft, renameTab])

  const cancelRename = useCallback(() => {
    setIsEditing(false)
    setDraft(tab.name)
  }, [tab.name])

  const handleDoubleClick = useCallback((e: React.MouseEvent) => {
    if (isSession) return // Session tabs are not renameable
    e.stopPropagation()
    setDraft(tab.name)
    setIsEditing(true)
  }, [tab.name, isSession])

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      commitRename()
    } else if (e.key === 'Escape') {
      cancelRename()
    }
  }, [commitRename, cancelRename])

  return (
    <div
      className={cn(
        'group relative flex items-center gap-1.5 h-6 pl-2.5 pr-1 rounded text-xs cursor-pointer transition-colors shrink-0',
        isActive
          ? 'bg-background-tertiary text-foreground'
          : 'text-foreground-subtle hover:text-foreground-muted hover:bg-background-tertiary/50',
      )}
      onClick={onSelect}
      onDoubleClick={handleDoubleClick}
    >
      {/* Session icon prefix */}
      {isSession && (
        <GitBranch className="w-3 h-3 shrink-0 text-blue-400" />
      )}

      {isEditing ? (
        <input
          ref={inputRef}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={commitRename}
          onKeyDown={handleKeyDown}
          className="w-[80px] bg-transparent border border-brand rounded px-1 text-xs text-foreground outline-none"
          onClick={(e) => e.stopPropagation()}
        />
      ) : (
        <span className="truncate max-w-[100px]">{tab.name}</span>
      )}

      {/* Hide close button for session-bound tabs */}
      {!isSession && (
        <button
          onClick={(e) => {
            e.stopPropagation()
            onClose()
          }}
          className={cn(
            'flex items-center justify-center w-4 h-4 rounded transition-colors',
            isActive
              ? 'text-foreground-muted hover:text-foreground hover:bg-background-secondary'
              : 'opacity-0 group-hover:opacity-100 text-foreground-subtle hover:text-foreground hover:bg-background-secondary',
          )}
        >
          <X className="w-3 h-3" />
        </button>
      )}
    </div>
  )
}
