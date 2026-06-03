// =============================================================================
// TitleBarMenus - IDE-style menu bar for the main window title bar
// =============================================================================
// Extracted from TitleBar so the shell stays hook-free. This component owns
// all menu-specific state (translations, modals, auth) and should only be
// rendered inside <TitleBar> in the main workspace window.

import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import { basename } from '@tauri-apps/api/path'
import { Window } from '@tauri-apps/api/window'
import { toast } from 'sonner'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from './ui/dropdown-menu'
import { RAGIndexModal } from './rag'
import { AuthModal } from './cloud/AuthModal'
import { LanguageMenuItems } from './LanguageSelector'
import { OnboardingWizardModal } from './onboarding/OnboardingWizardModal'
import { AIConfigModal } from './ai-config/AIConfigModal'
import { SettingsModal } from './settings/SettingsModal'
import { useAuthStore } from '@/stores/authStore'
import { useAppPhaseStore } from '@/stores/appPhaseStore'
import { useSettingsStore } from '@/stores/settingsStore'
import { tauriApi } from '@/lib/tauri'
import { resetAllWizardStores } from '@/lib/wizard/resetAllStores'
import type { WizardResult } from '@/lib/wizard/types'

// -----------------------------------------------------------------------------
// Shared styles for compact IDE menu
// -----------------------------------------------------------------------------

const menuContentClass =
  'min-w-[180px] bg-background-tertiary border-border rounded-md p-1 shadow-xl shadow-black/50'

const menuItemClass =
  'text-xs px-3 py-1.5 rounded-sm text-foreground-muted cursor-default select-none outline-none focus:bg-background-secondary focus:text-foreground'

const menuShortcutClass = 'ml-auto pl-6 text-[10px] tracking-wide text-foreground-subtle'

const menuSeparatorClass = '-mx-1 my-1 h-px bg-border'

const menuSubTriggerClass =
  'text-xs px-3 py-1.5 rounded-sm text-foreground-muted cursor-default select-none outline-none focus:bg-background-secondary focus:text-foreground data-[state=open]:bg-background-secondary'

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

interface TitleBarMenusProps {
  projectPath?: string
  projectId?: string
}

/** Same shape as LauncherScreen's recent project entry. Duplicated here to
 *  avoid a circular import (LauncherScreen will also gradually move to the
 *  store-based open flow). */
interface RecentProject {
  id?: string
  path: string
  name: string
  lastOpened: number
  projectType?: string
}

const RECENTS_STORAGE_KEY = 'venore-recent-projects'
const RECENTS_MENU_LIMIT = 8

function readRecents(): RecentProject[] {
  try {
    const raw = localStorage.getItem(RECENTS_STORAGE_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw) as RecentProject[]
    return parsed
      .filter((p) => typeof p.path === 'string' && p.path.length > 0)
      .sort((a, b) => (b.lastOpened ?? 0) - (a.lastOpened ?? 0))
      .slice(0, RECENTS_MENU_LIMIT)
  } catch (e) {
    console.warn('[TitleBarMenus] Failed to read recents from localStorage:', e)
    return []
  }
}

export function TitleBarMenus({ projectPath, projectId }: TitleBarMenusProps) {
  const { t } = useTranslation('menu')
  const [showRAGModal, setShowRAGModal] = useState(false)
  const [showAuthModal, setShowAuthModal] = useState(false)
  const [showWizard, setShowWizard] = useState(false)
  const [showAIConfigModal, setShowAIConfigModal] = useState(false)
  const [resnapshotting, setResnapshotting] = useState(false)
  const [recents, setRecents] = useState<RecentProject[]>(() => readRecents())
  const { authenticated } = useAuthStore()
  const goToLauncher = useAppPhaseStore((s) => s.goToLauncher)
  const openProject = useAppPhaseStore((s) => s.openProject)
  const openSettings = useSettingsStore((s) => s.openModal)
  // Run Wizard and Re-analyze are codebase-only operations — knowledge
  // projects don't have a source tree to walk or modules to layer-analyze.
  // The store carries the active project's type so we can gate both items.
  const projectType = useAppPhaseStore((s) => s.currentProjectType)
  const isCodeProject = projectType === 'code'

  // Re-read recents from localStorage when the user is about to open the
  // submenu via the File menu. Cheap (parses one localStorage key) and
  // avoids stale entries after the user opens projects from this same menu.
  const refreshRecents = () => setRecents(readRecents())

  // Debounce for New Window: open_main_window has no "focus existing"
  // branch (each call creates a new window by design), so a double-click
  // or held-Enter would spawn duplicates. 500ms is well above accidental
  // repeat-click latency while staying invisible to deliberate use.
  const lastNewWindowAt = useRef(0)
  const handleNewWindow = () => {
    const now = Date.now()
    if (now - lastNewWindowAt.current < 500) return
    lastNewWindowAt.current = now
    void tauriApi.openMainWindow().catch((err) => {
      const message = err instanceof Error ? err.message : String(err)
      toast.error(
        t('newWindowFailed', { defaultValue: 'Could not open a new window' }),
        { description: message },
      )
    })
  }

  // Pick a folder and try to open it strictly (must have `.venore/`). On
  // success, the store flips phase to workspace; on a non-Venore folder we
  // surface the same toast the launcher does so the UX is consistent.
  const handleOpenProjectDialog = async () => {
    let selected: string | null = null
    try {
      const result = await openDialog({
        directory: true,
        multiple: false,
        title: t('selectProjectFolder', { defaultValue: 'Select Project Folder' }),
      })
      if (typeof result === 'string') selected = result
    } catch (e) {
      console.error('[TitleBarMenus] dialog error:', e)
      return
    }
    if (!selected) return

    try {
      const report = await tauriApi.openExistingProject(selected)
      await openProject(report.project.path, 'code', report.project.id)
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      const isNotVenore =
        message.includes('.venore/project.json') ||
        message.toLowerCase().includes('not a venore project') ||
        message.toLowerCase().includes('not found')
      const fallbackName = await basename(selected).catch(() => selected)
      toast.error(
        isNotVenore
          ? t('openNotVenore', { defaultValue: 'This folder is not a Venore project yet.' })
          : t('openFailed', { defaultValue: 'Failed to open project' }),
        {
          description: isNotVenore ? fallbackName : message,
        },
      )
    }
  }

  // Re-onboard the active project. Same modal LauncherScreen mounts, just
  // pointed at the current path. The modal calls registerProject and
  // saveProjectMemory internally; `wizard_index_project` emits
  // `context-update-complete`, so canvas and dashboard refresh themselves
  // when the wizard finishes — no manual refetch needed here.
  const handleRunWizard = () => {
    if (!projectPath || !isCodeProject) return
    setShowWizard(true)
  }

  const handleWizardComplete = (_result: WizardResult) => {
    setShowWizard(false)
    resetAllWizardStores()
    toast.success(
      t('wizardCompleted', { defaultValue: 'Wizard completed' }),
      { description: t('wizardCompletedHint', { defaultValue: 'Snapshot regenerated from the wizard.' }) },
    )
    // No navigation: we're already inside this project's workspace and the
    // backend emitted `context-update-complete`. The canvas + ProjectPanel
    // listeners do the refresh.
  }

  const handleWizardClose = (open: boolean) => {
    setShowWizard(open)
    if (!open) {
      // Match LauncherScreen: reset only when the user fully dismisses the
      // modal. Persisted wizard drafts in zustand would otherwise leak
      // across project re-onboards.
      resetAllWizardStores()
    }
  }

  // Re-analyze: same call as the ProjectPanel refresh button. Re-walks the
  // source tree, re-indexes RAG, rewrites `.venore/{module-layers,code-hashes,
  // analysis-output}.json`. No LLM, no memory regen — the wizard handles
  // that case.
  const handleReanalyze = async () => {
    if (!projectPath || !isCodeProject || resnapshotting) return
    setResnapshotting(true)
    try {
      const report = await tauriApi.resnapshotProject(projectPath)
      toast.success(
        t('reanalyzeSuccess', { defaultValue: 'Snapshot refreshed' }),
        {
          description: [
            t('reanalyzeModules', { n: report.modules, defaultValue: '{{n}} modules' }),
            t('reanalyzeLayers', { n: report.layersWritten, defaultValue: '{{n}} layers' }),
            t('reanalyzeHashes', { n: report.hashesWritten, defaultValue: '{{n}} hashes' }),
          ].join(', '),
        },
      )
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      toast.error(
        t('reanalyzeFailed', { defaultValue: 'Re-analyze failed' }),
        { description: message },
      )
    } finally {
      setResnapshotting(false)
    }
  }

  // Wire keyboard shortcuts Ctrl+B (back to launcher) and Ctrl+O (open).
  // Lives here so the shortcuts only fire while the workspace title bar is
  // mounted — the launcher screen has its own affordances.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!e.ctrlKey || e.shiftKey || e.altKey) return
      if (e.key === 'b' || e.key === 'B') {
        e.preventDefault()
        goToLauncher()
      } else if (e.key === 'o' || e.key === 'O') {
        e.preventDefault()
        void handleOpenProjectDialog()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
    // handleOpenProjectDialog closes over translation + store, but those are
    // stable across renders for this purpose. Re-binding on every render is
    // cheap and avoids stale-closure foot-guns.
  })

  return (
    <>
      <div className="flex items-center h-full shrink-0">
        {/* File */}
        <DropdownMenu onOpenChange={(open) => { if (open) refreshRecents() }}>
          <DropdownMenuTrigger asChild>
            <button className="text-xs px-2 h-full text-foreground-muted hover:bg-background-secondary transition-colors outline-none select-none">
              {t('file')}
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" sideOffset={0} className={menuContentClass}>
            <DropdownMenuItem className={menuItemClass} onSelect={goToLauncher}>
              {t('backToLauncher')}
              <span className={menuShortcutClass}>Ctrl+B</span>
            </DropdownMenuItem>
            <DropdownMenuSeparator className={menuSeparatorClass} />
            <DropdownMenuItem className={menuItemClass} onSelect={() => { void handleOpenProjectDialog() }}>
              {t('openProject')}
              <span className={menuShortcutClass}>Ctrl+O</span>
            </DropdownMenuItem>
            <DropdownMenuItem
              className={menuItemClass}
              onSelect={handleNewWindow}
            >
              {t('newWindow', { defaultValue: 'New Window' })}
            </DropdownMenuItem>
            <DropdownMenuSub>
              <DropdownMenuSubTrigger className={menuSubTriggerClass}>
                {t('recentProjects')}
              </DropdownMenuSubTrigger>
              <DropdownMenuSubContent className={menuContentClass}>
                {recents.length === 0 ? (
                  <DropdownMenuItem disabled className={menuItemClass}>
                    {t('noRecentProjects')}
                  </DropdownMenuItem>
                ) : (
                  recents.map((p) => (
                    <DropdownMenuItem
                      key={p.path}
                      className={menuItemClass}
                      onSelect={() => {
                        void openProject(p.path, p.projectType ?? 'code', p.id)
                      }}
                      title={p.path}
                    >
                      <span className="truncate max-w-[260px]">{p.name || p.path}</span>
                    </DropdownMenuItem>
                  ))
                )}
              </DropdownMenuSubContent>
            </DropdownMenuSub>
            <DropdownMenuSeparator className={menuSeparatorClass} />
            <DropdownMenuItem
              className={menuItemClass}
              onSelect={() => {
                void Window.getCurrent().close()
              }}
            >
              {t('exit')}
              <span className={menuShortcutClass}>Alt+F4</span>
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>

        {/* Tools */}
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <button className="text-xs px-2 h-full text-foreground-muted hover:bg-background-secondary transition-colors outline-none select-none">
              {t('tools')}
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" sideOffset={0} className={menuContentClass}>
            <DropdownMenuItem
              className={menuItemClass}
              disabled={!isCodeProject || !projectPath}
              onSelect={handleRunWizard}
            >
              {t('runWizard')}
            </DropdownMenuItem>
            <DropdownMenuItem
              className={menuItemClass}
              disabled={!isCodeProject || !projectPath || resnapshotting}
              onSelect={() => { void handleReanalyze() }}
            >
              {t('reAnalyzeProject')}
            </DropdownMenuItem>
            <DropdownMenuSeparator className={menuSeparatorClass} />
            <DropdownMenuItem
              className={menuItemClass}
              disabled={!isCodeProject || !projectPath}
              onSelect={() => setShowRAGModal(true)}
            >
              {t('indexCodeRag')}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>

        {/* Settings */}
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <button className="text-xs px-2 h-full text-foreground-muted hover:bg-background-secondary transition-colors outline-none select-none">
              {t('settings')}
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" sideOffset={0} className={menuContentClass}>
            <DropdownMenuItem
              className={menuItemClass}
              onSelect={() => setShowAIConfigModal(true)}
            >
              {t('aiConfiguration')}
            </DropdownMenuItem>
            <DropdownMenuSeparator className={menuSeparatorClass} />
            <DropdownMenuSub>
              <DropdownMenuSubTrigger className={menuSubTriggerClass}>
                {t('language')}
              </DropdownMenuSubTrigger>
              <DropdownMenuSubContent className={menuContentClass}>
                <LanguageMenuItems />
              </DropdownMenuSubContent>
            </DropdownMenuSub>
            <DropdownMenuSeparator className={menuSeparatorClass} />
            <DropdownMenuItem
              className={menuItemClass}
              onSelect={() => openSettings()}
            >
              {t('preferences')}
            </DropdownMenuItem>
            <DropdownMenuSeparator className={menuSeparatorClass} />
            {authenticated ? (
              <DropdownMenuItem className={menuItemClass}>
                {t('account')}
              </DropdownMenuItem>
            ) : (
              <DropdownMenuItem
                className={`${menuItemClass} opacity-60 cursor-default`}
                onSelect={(e) => e.preventDefault()}
              >
                {t('signIn')}
                <span className="ml-auto pl-3 px-1.5 py-0.5 text-[9px] font-medium rounded bg-brand/15 text-brand">
                  {t('comingSoon', { defaultValue: 'Soon' })}
                </span>
              </DropdownMenuItem>
            )}
          </DropdownMenuContent>
        </DropdownMenu>

        {/* Help */}
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <button className="text-xs px-2 h-full text-foreground-muted hover:bg-background-secondary transition-colors outline-none select-none">
              {t('help')}
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" sideOffset={0} className={menuContentClass}>
            <DropdownMenuItem className={menuItemClass}>{t('documentation')}</DropdownMenuItem>
            <DropdownMenuSeparator className={menuSeparatorClass} />
            <DropdownMenuItem className={menuItemClass}>{t('aboutVenore')}</DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>

      {/* Modals */}
      <AuthModal open={showAuthModal} onOpenChange={setShowAuthModal} />
      {projectPath && (
        <RAGIndexModal
          open={showRAGModal}
          onOpenChange={setShowRAGModal}
          projectPath={projectPath}
          projectId={projectId}
        />
      )}
      {/* Run Wizard against the active project. `initialPath` skips the
          folder picker so the wizard starts at Step 1 with the current
          path already loaded. */}
      {projectPath && (
        <OnboardingWizardModal
          open={showWizard}
          onOpenChange={handleWizardClose}
          initialPath={projectPath}
          onComplete={handleWizardComplete}
        />
      )}

      {/* Quick AI-config dialog (Settings → AI Configuration). Same modal
          the launcher pops on first run. Preferences below gives the full
          tabbed surface; this is the shortcut. */}
      <AIConfigModal open={showAIConfigModal} onOpenChange={setShowAIConfigModal} />

      {/* Tabbed Preferences (Settings → Preferences). Driven by
          `useSettingsStore` so any caller can open a specific section. */}
      <SettingsModal />
    </>
  )
}
