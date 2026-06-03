// =============================================================================
// StatusBar - VS Code-style status bar at the bottom of the window
// =============================================================================
// Shows git branch, errors/warnings, terminal toggle, updater badge, and project name.

import { useEffect } from 'react'
import { GitBranch, AlertCircle, AlertTriangle, Terminal } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'
import { useTerminalStore } from '@/stores/terminalStore'
import { LanguageIndicator } from '@/components/LanguageSelector'
import { UpdateNotification } from './panels/updater/UpdateNotification'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface StatusBarProps {
  projectPath: string
  onShowUpdater?: () => void
}

// -----------------------------------------------------------------------------
// Main Component
// -----------------------------------------------------------------------------

export function StatusBar({ projectPath, onShowUpdater }: StatusBarProps) {
  const { t } = useTranslation('workspace')
  const { isOpen, toggle } = useTerminalStore()

  // Extract project name from path
  const projectName = projectPath.split(/[/\\]/).filter(Boolean).pop() ?? 'Project'

  // Keyboard shortcut: Ctrl+` to toggle terminal (like VS Code)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key === '`') {
        e.preventDefault()
        toggle()
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [toggle])

  return (
    <div className="flex items-center h-6 bg-background px-3 border-t border-border shrink-0 select-none">
      {/* Left side: git branch + diagnostics */}
      <div className="flex items-center gap-3 text-[11px] text-foreground-muted">
        <span className="flex items-center gap-1">
          <GitBranch className="w-3.5 h-3.5" />
          <span>main</span>
        </span>

        <span className="flex items-center gap-1">
          <AlertCircle className="w-3.5 h-3.5" />
          <span>0</span>
        </span>

        <span className="flex items-center gap-1">
          <AlertTriangle className="w-3.5 h-3.5" />
          <span>0</span>
        </span>

        {/* Context updater notification */}
        {onShowUpdater && <UpdateNotification onClick={onShowUpdater} />}
      </div>

      {/* Spacer */}
      <div className="flex-1" />

      {/* Right side: terminal toggle + project name */}
      <div className="flex items-center gap-3 text-[11px] text-foreground-muted">
        {/* Terminal toggle button — like VS Code */}
        <button
          onClick={toggle}
          className={cn(
            'flex items-center gap-1 px-1.5 h-full cursor-pointer transition-colors rounded-sm',
            'hover:text-foreground hover:bg-background-tertiary',
            isOpen && 'text-foreground',
          )}
          title={t('statusBar.toggleTerminal')}
        >
          <Terminal className="w-3.5 h-3.5" />
          <span>{t('statusBar.terminal')}</span>
        </button>

        <LanguageIndicator />

        <span className="truncate max-w-[200px]">
          {projectName}
        </span>
      </div>
    </div>
  )
}
