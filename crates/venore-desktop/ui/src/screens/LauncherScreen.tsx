// =============================================================================
// LauncherScreen - Main screen for opening/creating projects
// =============================================================================
// Features:
// - Two-panel layout (Xcode-style)
// - Left panel: Logo + action buttons
// - Right panel: Recent projects list
// - Drag & drop support
// - Recent projects stored in localStorage

import { useState, useEffect, useCallback } from 'react'
import { FolderOpen, Clock, Trash2, Sparkles, Settings, Github, Hexagon } from 'lucide-react'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import { basename } from '@tauri-apps/api/path'
import { getVersion } from '@tauri-apps/api/app'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { Button } from '../components/ui/button'
import { Input } from '../components/ui/input'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '../components/ui/dialog'
import { DropZone } from '../components/ui/drop-zone'
import { isMacOS } from '../lib/platform'
import { WindowControls } from '../components/WindowControls'
import { AIConfigModal } from '../components/ai-config'
import { OnboardingWizardModal } from '../components/onboarding/OnboardingWizardModal'
import { CloneFromGitHubModal } from '../components/github/CloneFromGitHubModal'
import { tauriApi, type OpenExistingReport } from '../lib/tauri'
import { formatTimeAgo } from '../lib/time'
import venoreLogo from '../assets/venore-logo.svg'
import venoreIcon from '../assets/venore-icon.svg'
import { resetAllWizardStores } from '../lib/wizard/resetAllStores'
import type { WizardResult } from '../lib/wizard/types'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface RecentProject {
  id?: string
  path: string
  name: string
  lastOpened: number
  projectType?: string
}

interface LauncherScreenProps {
  /** Called when a project is opened */
  onProjectOpen?: (projectPath: string, projectType?: string, projectId?: string) => void
}

// -----------------------------------------------------------------------------
// Main Component
// -----------------------------------------------------------------------------

export function LauncherScreen({ onProjectOpen }: LauncherScreenProps) {
  const { t } = useTranslation('screens')
  const [recentProjects, setRecentProjects] = useState<RecentProject[]>([])
  const [appVersion, setAppVersion] = useState('')
  const [isLoading, setIsLoading] = useState(false)
  const [configuredProviders, setConfiguredProviders] = useState<string[]>([])
  const [currentProviderIndex, setCurrentProviderIndex] = useState(0)
  const [showAIConfigModal, setShowAIConfigModal] = useState(false)
  const [showWizard, setShowWizard] = useState(false)
  const [wizardInitialPath, setWizardInitialPath] = useState<string | undefined>()
  const [showCloneModal, setShowCloneModal] = useState(false)
  const [showKnowledgeModal, setShowKnowledgeModal] = useState(false)
  const [knowledgeName, setKnowledgeName] = useState('')

  // Load recent projects and AI config on mount
  useEffect(() => {
    loadRecentProjects()
    loadCurrentProvider()
    getVersion().then(setAppVersion).catch(() => {})
  }, [])

  const loadCurrentProvider = async () => {
    try {
      const providers: string[] = []

      // Get cloud providers with API keys
      const configured = await tauriApi.getConfiguredProviders()
      for (const provider of configured.providers) {
        const providerName = provider.charAt(0).toUpperCase() + provider.slice(1)
        providers.push(providerName)
      }

      // Check if Ollama is running
      try {
        const ollamaTest = await tauriApi.testConnection({ provider: 'ollama' })
        if (ollamaTest.success) {
          providers.push('Ollama')
        }
      } catch (err) {
        // Ollama not running, skip
      }

      setConfiguredProviders(providers)
      setCurrentProviderIndex(0)
    } catch (err) {
      console.error('Failed to load current provider:', err)
      setConfiguredProviders([])
    }
  }

  // Rotate providers every 3 seconds
  useEffect(() => {
    if (configuredProviders.length <= 1) return

    const interval = setInterval(() => {
      setCurrentProviderIndex((prev) => (prev + 1) % configuredProviders.length)
    }, 3000)

    return () => clearInterval(interval)
  }, [configuredProviders])

  const loadRecentProjects = () => {
    try {
      const stored = localStorage.getItem('venore-recent-projects')
      if (stored) {
        const projects = JSON.parse(stored) as RecentProject[]
        setRecentProjects(projects)
      }
    } catch (error) {
      console.error('Failed to load recent projects:', error)
    }
  }

  const saveRecentProject = (path: string, name: string, id?: string, projectType?: string) => {
    const newProject: RecentProject = {
      id,
      path,
      name,
      lastOpened: Date.now(),
      projectType,
    }

    // Add to list, removing duplicates and keeping max 10
    const updated = [
      newProject,
      ...recentProjects.filter(p => p.path !== path),
    ].slice(0, 10)

    localStorage.setItem('venore-recent-projects', JSON.stringify(updated))
    setRecentProjects(updated)
  }

  const removeRecentProject = (path: string, e: React.MouseEvent) => {
    e.stopPropagation()
    const updated = recentProjects.filter(p => p.path !== path)
    localStorage.setItem('venore-recent-projects', JSON.stringify(updated))
    setRecentProjects(updated)
  }

  const clearAllRecent = () => {
    localStorage.removeItem('venore-recent-projects')
    setRecentProjects([])
  }

  // Action handlers
  const handleNewProject = async () => {
    setShowWizard(true)
  }

  const handleNewKnowledge = () => {
    setKnowledgeName('')
    setShowKnowledgeModal(true)
  }

  const handleCreateKnowledge = async () => {
    const name = knowledgeName.trim()
    if (!name) return
    setShowKnowledgeModal(false)
    try {
      setIsLoading(true)
      const project = await tauriApi.createKnowledgeProject(name, '')
      saveRecentProject(project.path, project.name, project.id, 'knowledge')
      onProjectOpen?.(project.path, 'knowledge', project.id)
    } catch (error) {
      console.error('Failed to create knowledge project:', error)
    } finally {
      setIsLoading(false)
    }
  }

  const handleWizardComplete = async (result: WizardResult) => {
    console.log('Wizard completed:', result)
    // Register project to get stable ID, then save to recent
    try {
      const registered = await tauriApi.registerProject(result.projectPath)
      saveRecentProject(result.projectPath, result.projectName, registered.id, 'code')
    } catch {
      saveRecentProject(result.projectPath, result.projectName)
    }
    // Close wizard
    setShowWizard(false)
    // Reset wizard stores so next wizard open starts clean
    resetAllWizardStores()
    // Open the project
    onProjectOpen?.(result.projectPath, 'code')
  }

  // Shared "you just opened a portable workspace" toast. Used by both the
  // folder picker and the clone-from-GitHub success path so the user gets a
  // consistent confirmation of what came across in `.venore/`.
  const showRestorationBanner = (report: OpenExistingReport) => {
    const parts: string[] = []
    if (report.moduleCount > 0) {
      parts.push(
        t('launcher.openModulesPart', { n: report.moduleCount, defaultValue: '{{n}} modules' }),
      )
    }
    if (report.layerCount > 0) {
      parts.push(
        t('launcher.openLayersPart', { n: report.layerCount, defaultValue: '{{n}} layers' }),
      )
    }
    if (report.hasMemory) {
      parts.push(t('launcher.openMemoryPart', { defaultValue: 'project memory' }))
    }
    // Staleness count is no longer computed on open (it froze the app); the
    // Staleness Current fills in drift badges passively after the canvas loads.
    const restoredPrefix = t('launcher.openRestoredPrefix', { defaultValue: 'Restored' })
    const emptyDescription = t('launcher.openEmpty', { defaultValue: 'Project opened' })
    toast.success(report.project.name, {
      description: parts.length > 0 ? `${restoredPrefix} ${parts.join(', ')}` : emptyDescription,
    })
  }

  const handleOpenProject = async () => {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: t('launcher.selectProjectFolder'),
    })

    if (!selected || typeof selected !== 'string') return

    const fallbackName = (await basename(selected)) || t('launcher.unnamed')

    // Strict open: requires a committed `.venore/project.json`. Folders that
    // aren't Venore projects route the user to the wizard instead of getting
    // a silently-half-initialized workspace.
    let report
    try {
      report = await tauriApi.openExistingProject(selected)
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      const isNotVenoreProject =
        message.includes('.venore/project.json') ||
        message.toLowerCase().includes('not a venore project') ||
        message.toLowerCase().includes('not found')
      if (isNotVenoreProject) {
        toast.error(
          t('launcher.openNotVenore', { defaultValue: 'This folder is not a Venore project yet.' }),
          {
            description: t('launcher.openRunWizardHint', {
              defaultValue: 'Run the onboarding wizard to set it up.',
            }),
            action: {
              label: t('launcher.openRunWizardAction', { defaultValue: 'Run wizard' }),
              onClick: () => setShowWizard(true),
            },
          },
        )
      } else {
        toast.error(t('launcher.openFailed', { defaultValue: 'Failed to open project' }), {
          description: message,
        })
      }
      return
    }

    showRestorationBanner(report)
    saveRecentProject(selected, report.project.name || fallbackName, report.project.id, 'code')
    onProjectOpen?.(selected, 'code')
  }

  const handleModelClick = () => {
    setShowAIConfigModal(true)
  }

  const handleAIConfigClose = (open: boolean) => {
    setShowAIConfigModal(open)
    if (!open) {
      // Reload provider when modal closes
      loadCurrentProvider()
    }
  }

  const handleOpenRecent = async (project: RecentProject) => {
    console.log('Opening recent project:', project.path)
    setIsLoading(true)

    if (project.projectType === 'knowledge') {
      // Knowledge projects don't need registerProject (no .venore/project.json)
      saveRecentProject(project.path, project.name, project.id, 'knowledge')
      onProjectOpen?.(project.path, 'knowledge', project.id)
    } else {
      // Strict open: a recent that has lost its `.venore/project.json` (moved,
      // deleted, branch wipe) should error out instead of silently re-creating
      // a half-init project under the same path.
      try {
        const report = await tauriApi.openExistingProject(project.path)
        saveRecentProject(project.path, report.project.name, report.project.id, 'code')
        onProjectOpen?.(project.path, project.projectType, report.project.id)
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err)
        toast.error(
          t('launcher.openRecentFailed', {
            name: project.name,
            defaultValue: 'Cannot open {{name}}',
          }),
          { description: message },
        )
        setIsLoading(false)
        return
      }
    }

    setTimeout(() => setIsLoading(false), 500)
  }

  // Drag & Drop handler (uses Tauri v2 native events via DropZone)
  const handleDrop = useCallback((paths: string[]) => {
    if (paths.length > 0) {
      setWizardInitialPath(paths[0])
      setShowWizard(true)
    }
  }, [])

  const handleWizardClose = useCallback((open: boolean) => {
    setShowWizard(open)
    if (!open) {
      setWizardInitialPath(undefined)
    }
  }, [])

  const handleWizardCancel = useCallback(() => {
    setShowWizard(false)
    setWizardInitialPath(undefined)
  }, [])

  const handleCloneComplete = async (
    path: string,
    name: string,
    _owner: string,
    _repo: string,
    hasVenore: boolean,
  ) => {
    // Git clone already creates remote origin → detect_github_repo finds it automatically.

    // Fresh clone with no committed `.venore/` → onboard from scratch. The
    // wizard will create `.venore/project.json` and the rest of the portable
    // snapshot during indexing.
    if (!hasVenore) {
      setWizardInitialPath(path)
      setShowWizard(true)
      return
    }

    // Committed `.venore/project.json` is present — use the strict open path
    // so the user gets the restoration banner and we don't accidentally
    // auto-create a second identity on top of the cloned one.
    try {
      const report = await tauriApi.openExistingProject(path)
      showRestorationBanner(report)
      saveRecentProject(path, report.project.name || name, report.project.id, 'code')
      onProjectOpen?.(path, 'code')
    } catch (err) {
      // Defensive: the backend flag said `.venore/project.json` exists but
      // the open call still failed (parse error, permissions, etc.). Surface
      // it and route to the wizard so the user isn't stranded.
      const message = err instanceof Error ? err.message : String(err)
      toast.error(t('launcher.openFailed', { defaultValue: 'Failed to open project' }), {
        description: message,
      })
      setWizardInitialPath(path)
      setShowWizard(true)
    }
  }

  return (
    <DropZone onDrop={handleDrop} disabled={showWizard} className="h-full w-full">
    <div className="h-full w-full flex flex-col bg-background select-none">
      {/* Top Bar: macOS always shows full-width, Windows only when no projects */}
      {(isMacOS || recentProjects.length === 0) && (
        <div className="h-8 border-b border-border flex items-center" data-tauri-drag-region>
          {isMacOS && <WindowControls />}
          {!isMacOS && (
            <div className="flex items-center gap-2 px-3 h-full" data-tauri-drag-region>
              <img
                src={venoreIcon}
                alt="Venore"
                className="h-4 w-auto"
                draggable={false}
              />
            </div>
          )}
          <div className="flex-1 h-full" data-tauri-drag-region />
          {!isMacOS && <WindowControls />}
        </div>
      )}

      {/* Main Content */}
      <div className="flex-1 flex">

        {/* Left Panel - Logo and Actions */}
        <div className={`flex-1 flex flex-col items-center justify-center relative ${recentProjects.length > 0 ? 'border-r border-border' : ''}`}>
        <div className="flex flex-col items-center gap-8 max-w-sm px-8">
          {/* Logo */}
          <div className="flex flex-col items-center gap-3">
            <img
              src={venoreLogo}
              alt="Venore"
              className="h-16 w-auto"
            />
            <div className="text-center">
              <p className="text-sm text-foreground-muted">{t('launcher.tagline')}</p>
            </div>
          </div>

          {/* Actions */}
          <div className="w-full space-y-3 mt-4">
            <Button
              onClick={handleNewProject}
              disabled={isLoading}
              className="w-full justify-start gap-3 h-auto py-3 bg-brand/10 hover:bg-brand/20 text-foreground border-brand/20"
              variant="outline"
            >
              <Sparkles className="w-5 h-5 text-brand" />
              <div className="text-left flex-1">
                <div className="font-medium">{t('launcher.newCodebase')}</div>
                <div className="text-xs text-foreground-muted font-normal">
                  {t('launcher.newCodebaseDescription')}
                </div>
              </div>
            </Button>

            <Button
              onClick={handleOpenProject}
              disabled={isLoading}
              className="w-full justify-start gap-3 h-auto py-3"
              variant="outline"
            >
              <FolderOpen className="w-5 h-5 text-foreground-muted" />
              <span>{t('launcher.openExisting')}</span>
            </Button>

            <Button
              onClick={() => setShowCloneModal(true)}
              disabled={isLoading}
              className="w-full justify-start gap-3 h-auto py-3"
              variant="outline"
            >
              <Github className="w-5 h-5 text-foreground-muted" />
              <div className="text-left flex-1">
                <div className="font-medium">{t('launcher.cloneFromGithub')}</div>
                <div className="text-xs text-foreground-muted font-normal">
                  {t('launcher.cloneFromGithubDescription')}
                </div>
              </div>
            </Button>

            <Button
              onClick={handleNewKnowledge}
              disabled={isLoading}
              className="w-full justify-start gap-3 h-auto py-3 bg-purple-500/10 hover:bg-purple-500/20 text-foreground border-purple-500/20"
              variant="outline"
            >
              <Hexagon className="w-5 h-5 text-purple-400" />
              <div className="text-left flex-1">
                <div className="font-medium">{t('launcher.newKnowledge')}</div>
                <div className="text-xs text-foreground-muted font-normal">
                  {t('launcher.newKnowledgeDescription')}
                </div>
              </div>
            </Button>

          </div>

          {/* Hint */}
          <p className="text-xs text-foreground-muted text-center mt-2">
            {t('launcher.dragDropHint')}
          </p>
        </div>

        {/* Model Selector */}
        <div className="absolute bottom-12 left-1/2 -translate-x-1/2">
          <div className="rainbow-border">
            <Button
              onClick={handleModelClick}
              variant="outline"
              className="min-w-[120px] justify-between gap-2 h-auto py-2 px-3 text-xs bg-background"
            >
              <div className="flex items-center gap-2">
                <div className={`w-2 h-2 rounded-full ${configuredProviders.length > 0 ? 'bg-brand' : 'bg-foreground-muted'}`} />
                <span>
                  {configuredProviders.length > 0
                    ? configuredProviders[currentProviderIndex]
                    : t('launcher.selectAI')}
                </span>
              </div>
              <Settings className="w-3 h-3 text-foreground-muted" />
            </Button>
          </div>
        </div>

        {/* Version */}
        <span className="absolute bottom-4 left-4 text-[10px] text-foreground-muted">
          {appVersion && t('launcher.version', { version: appVersion })}
        </span>
        </div>

        {/* Right Panel - Recent Projects (only show if there are projects) */}
        {recentProjects.length > 0 && (
          <div className="w-[220px] flex flex-col bg-background-secondary">
            {/* Window Controls Bar (Windows/Linux only — macOS uses full-width bar above) */}
            {!isMacOS && (
              <div className="h-8 border-b border-border flex items-center" data-tauri-drag-region>
                <div className="flex-1 h-full" data-tauri-drag-region />
                <WindowControls />
              </div>
            )}

            {/* Header */}
            <div className="px-4 py-3 border-b border-border flex items-center justify-between">
              <h2 className="text-xs text-foreground-muted uppercase tracking-wider font-medium">
                {t('launcher.recentProjects')}
              </h2>
              <Button
                onClick={clearAllRecent}
                variant="ghost"
                size="icon"
                className="h-7 w-7"
                title={t('launcher.clearAllRecent')}
              >
                <Trash2 className="w-3.5 h-3.5" />
              </Button>
            </div>

            {/* List */}
            <div className="flex-1 overflow-y-auto">
              <div className="py-1">
                {recentProjects.map((project) => (
                  <div
                    key={project.path}
                    className="group relative"
                  >
                    <button
                      onClick={() => handleOpenRecent(project)}
                      disabled={isLoading}
                      className="w-full flex items-center gap-3 px-4 py-3 hover:bg-background-tertiary transition-colors text-left disabled:opacity-50"
                    >
                      <div className={`w-8 h-8 rounded-md flex items-center justify-center shrink-0 ${project.projectType === 'knowledge' ? 'bg-purple-500/10' : 'bg-brand/10'}`}>
                        {project.projectType === 'knowledge'
                          ? <Hexagon className="w-4 h-4 text-purple-400" />
                          : <FolderOpen className="w-4 h-4 text-brand" />
                        }
                      </div>
                      <div className="flex-1 min-w-0">
                        <div className="text-sm font-medium text-foreground truncate">
                          {project.name}
                        </div>
                        <div className="text-xs text-foreground-muted truncate">
                          {formatTimeAgo(project.lastOpened)}
                        </div>
                      </div>
                    </button>

                    {/* Remove button - visible on hover */}
                    <Button
                      onClick={(e) => removeRecentProject(project.path, e)}
                      variant="ghost"
                      size="icon"
                      className="absolute right-2 top-1/2 -translate-y-1/2 h-7 w-7 opacity-0 group-hover:opacity-100 transition-opacity"
                      title={t('launcher.removeFromRecent')}
                    >
                      <Trash2 className="w-3.5 h-3.5" />
                    </Button>
                  </div>
                ))}
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Onboarding Wizard Modal */}
      <OnboardingWizardModal
        open={showWizard}
        onOpenChange={handleWizardClose}
        initialPath={wizardInitialPath}
        onComplete={handleWizardComplete}
        onCancel={handleWizardCancel}
      />

      {/* AI Config Modal */}
      <AIConfigModal
        open={showAIConfigModal}
        onOpenChange={handleAIConfigClose}
        isRequired={false}
      />

      {/* Clone from GitHub Modal */}
      <CloneFromGitHubModal
        open={showCloneModal}
        onOpenChange={setShowCloneModal}
        onComplete={handleCloneComplete}
      />

      {/* New Knowledge Project Modal */}
      <Dialog open={showKnowledgeModal} onOpenChange={setShowKnowledgeModal}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <div className="flex items-center gap-2">
              <Hexagon className="w-5 h-5 text-purple-400" />
              <DialogTitle>{t('launcher.newKnowledge')}</DialogTitle>
            </div>
            <DialogDescription>
              {t('launcher.newKnowledgeModalDescription')}
            </DialogDescription>
          </DialogHeader>
          <div className="py-2">
            <Input
              placeholder={t('launcher.knowledgeNamePlaceholder', 'e.g. Auth strategy research')}
              value={knowledgeName}
              onChange={(e) => setKnowledgeName(e.target.value)}
              onKeyDown={(e) => { if (e.key === 'Enter') handleCreateKnowledge() }}
              autoFocus
            />
          </div>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setShowKnowledgeModal(false)}>
              {t('common.cancel', 'Cancel')}
            </Button>
            <Button
              onClick={handleCreateKnowledge}
              disabled={!knowledgeName.trim()}
              className="bg-purple-500 hover:bg-purple-600 text-white"
            >
              {t('launcher.createResearch', 'Create')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
    </DropZone>
  )
}

