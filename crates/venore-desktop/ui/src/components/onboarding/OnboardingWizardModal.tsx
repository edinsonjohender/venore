// =============================================================================
// OnboardingWizardModal - Main wizard modal component (REUSABLE)
// =============================================================================

import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { WizardHeader } from './WizardHeader'
import { WizardFooter } from './WizardFooter'
import { Step1ProjectContext } from './steps/Step1ProjectContext'
import { Step2AnalysisRules } from './steps/Step2AnalysisRules'
import { Step3AnalysisIndex } from './steps/Step3AnalysisIndex'
import { Step4IndexResults } from './steps/Step4IndexResults'
import { Step5Complete } from './steps/Step5Complete'
import { CheckpointDialog } from './CheckpointDialog'
import { RestoringSessionDialog } from './RestoringSessionDialog'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import { basename, dirname } from '@tauri-apps/api/path'
import { useWizardStore } from '@/stores/wizardStore'
import { useWizardDataStore } from '@/stores/wizardDataStore'
import { useWizardCacheStore } from '@/stores/wizardCacheStore'
import { resetAllWizardStores } from '@/lib/wizard/resetAllStores'
import { tauriApi, type CheckpointInfo } from '@/lib/tauri'
import type { OnboardingWizardModalProps } from '@/lib/wizard/types'
import { createLogger } from '@/lib/logger'
import { Button } from '@/components/ui/button'
import { AlertCircle, ArrowRight } from 'lucide-react'

const log = createLogger('wizard:modal')

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

export function OnboardingWizardModal({
  open,
  onOpenChange,
  initialPath,
  onComplete,
  onCancel,
}: OnboardingWizardModalProps) {
  const { t } = useTranslation('wizard')
  const [isSelectingPath, setIsSelectingPath] = useState(false)
  const [showCheckpointDialog, setShowCheckpointDialog] = useState(false)
  const [showCloseConfirm, setShowCloseConfirm] = useState(false)
  const [isRestoringSession, setIsRestoringSession] = useState(false)
  const [checkpointInfo, setCheckpointInfo] = useState<CheckpointInfo | null>(null)
  const [checkpointLastUpdated, setCheckpointLastUpdated] = useState<string | undefined>()
  const [selectedProjectPath, setSelectedProjectPath] = useState<string>('')
  const [openError, setOpenError] = useState<string | null>(null)
  const [isOpening, setIsOpening] = useState(false)
  // 'disk' = legacy checkpoint file from batch flow; 'local' = in-progress
  // wizard data persisted in localStorage by zustand (the new flow's resume).
  const [restoreSource, setRestoreSource] = useState<'disk' | 'local' | null>(null)

  const currentStep = useWizardStore((state) => state.currentStep)
  const setStep = useWizardStore((state) => state.setStep)
  const hasResetStores = useWizardStore((state) => state.hasResetStores)
  const setHasResetStores = useWizardStore((state) => state.setHasResetStores)
  const setProjectPath = useWizardDataStore((state) => state.setProjectPath)
  const setProjectContextName = useWizardDataStore((state) => state.setProjectName)

  /** True while either analysis (Phase 1) or indexing (Phase 2) is running.
   *  Used to decide whether closing the wizard needs a confirmation dialog
   *  and whether a cancellation signal must be sent to the backend. */
  const isGenerationActive = () => {
    const cache = useWizardCacheStore.getState()
    return cache.isAnalyzing || cache.isIndexing
  }

  /** Close the modal (shared by all close paths) */
  const closeModal = () => {
    onCancel?.()
    onOpenChange(false)
  }

  /**
   * Complete wizard: register project, save memory, then open it.
   *
   * Critical operations (registerProject + saveProjectMemory) are NOT silenced.
   * If either fails, the wizard stays open with an error Alert and a Retry
   * button so the user is never dropped into a workspace with missing memory.
   * Checkpoint deletion is best-effort and remains non-fatal.
   */
  const handleOpenProject = async () => {
    const { step1, step2, indexResult } = useWizardDataStore.getState()

    if (!step2.projectPath) {
      setOpenError('Project path missing — cannot open project')
      return
    }

    setOpenError(null)
    setIsOpening(true)

    // Best-effort: delete checkpoint (non-fatal)
    try {
      await tauriApi.deleteWizardCheckpoint(step2.projectPath)
    } catch (err) {
      log.warn('Failed to delete checkpoint', err)
    }

    // Critical: register project to obtain project_id
    let registered
    try {
      registered = await tauriApi.registerProject(step2.projectPath)
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      log.error('Failed to register project', err)
      setOpenError(`Failed to register project: ${msg}`)
      setIsOpening(false)
      return
    }

    // Critical: persist project memory with wizard inputs merged with AI draft.
    // User-supplied fields prevail; the AI draft fills in what the user left empty
    // and contributes the project summary.
    try {
      const locale = localStorage.getItem('venore-language') || 'en'
      const aiDraft = useWizardDataStore.getState().step5to8.aiMemoryDraft
      const pickUser = (user: string | undefined, ai: string | undefined): string =>
        (user && user.trim().length > 0) ? user : (ai ?? '')

      await tauriApi.saveProjectMemory({
        projectId: registered.id,
        name: step1.name,
        description: pickUser(step1.description, aiDraft?.description),
        state: step1.projectState || 'active',
        teamSize: step1.teamSize || 'solo',
        goals: (step1.goals && step1.goals.length > 0) ? step1.goals : (aiDraft?.goals ?? []),
        architecture: pickUser(step1.architecture, aiDraft?.architecture),
        techDebt: pickUser(step1.techDebt, aiDraft?.techDebt),
        responseLanguage: locale,
        conventions: [],
        projectSummary: aiDraft?.projectSummary ?? '',
      })
      log.info('Project memory saved from wizard')
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      log.error('Failed to save project memory', err)
      setOpenError(`Failed to save project memory: ${msg}`)
      setIsOpening(false)
      return
    }

    setIsOpening(false)
    // Project is now registered and memory persisted to DB — the wizard
    // draft in localStorage has served its purpose. Mark for cleanup so the
    // useEffect on modal close wipes it (otherwise reopening the wizard on
    // this same path would offer a "resume" dialog with stale data the user
    // has already committed).
    setHasResetStores(true)
    onComplete?.({
      projectPath: step2.projectPath,
      projectName: step1.name,
      contextsGenerated: indexResult?.indexed ?? 0,
    })
  }

  // Reset stores and flags when modal CLOSES (not when it opens)
  // We don't reset on open because we might be loading from checkpoint
  // If generation was active, checkpoint on disk already has the progress saved
  useEffect(() => {
    if (!open && hasResetStores) {
      resetAllWizardStores()
      setHasResetStores(false)
    }
  }, [open, hasResetStores, setHasResetStores])

  // Auto-select path when modal opens (if no initialPath provided)
  // Always opens folder picker - checkpoint dialog handles resume if needed
  useEffect(() => {
    if (open && !initialPath && !isSelectingPath) {
      selectProjectPath()
    }
  }, [open, initialPath])

  // Initialize with path if provided (e.g. from drag-and-drop)
  useEffect(() => {
    if (open && initialPath) {
      initializeWithPath(initialPath)
    }
  }, [open, initialPath])

  /**
   * Initialize the wizard with a given path, checking for existing checkpoints.
   * Used by both drag-and-drop (initialPath) and folder picker (selectProjectPath).
   *
   * Defensively normalises the path: if the user selected the project's
   * `.venore` config folder (easy mistake via drag-and-drop or folder
   * picker), we climb to the parent. `.venore` is never a project root.
   */
  const initializeWithPath = async (path: string) => {
    try {
      let normalisedPath = path
      const leaf = await basename(path)
      if (leaf === '.venore') {
        normalisedPath = await dirname(path)
        log.info('Normalised .venore selection to parent', { from: path, to: normalisedPath })
      }

      const name = await basename(normalisedPath) || t('modal.unnamedProject')
      setSelectedProjectPath(normalisedPath)

      // 1. Legacy: checkpoint file on disk (old batch_generation flow). Still
      //    relevant for projects that started under the 9-step wizard.
      const info = await tauriApi.checkWizardCheckpoint(normalisedPath)

      if (info && info.exists) {
        setCheckpointInfo(info)
        setRestoreSource('disk')

        try {
          const fullCheckpoint = await tauriApi.loadFullCheckpoint(normalisedPath)
          setCheckpointLastUpdated(fullCheckpoint.last_updated_at)
        } catch (err) {
          log.warn('Could not load full checkpoint for timestamp', err)
        }

        setShowCheckpointDialog(true)
        return
      }

      // 2. New flow: zustand persists Step 1-4 data in localStorage. If the
      //    user is reopening the wizard for the SAME project AND there's
      //    meaningful content already entered, offer to resume that draft
      //    instead of silently wiping it (the previous behavior, which lost
      //    description / state / techDebt / layer selection on every reopen).
      const data = useWizardDataStore.getState()
      const navState = useWizardStore.getState()
      const isSameProject = data.step2.projectPath === normalisedPath
      const hasContent =
        data.step1.description.trim().length > 0 ||
        data.step1.architecture.trim().length > 0 ||
        data.step1.techDebt.trim().length > 0 ||
        (data.step1.goals && data.step1.goals.length > 0)

      if (isSameProject && hasContent) {
        // Synthesize a checkpoint-shaped info so we can reuse CheckpointDialog.
        // completed/total/percent reflect wizard step progress, not module
        // batches — semantics differ from the legacy disk checkpoint but the
        // dialog's progress bar still reads sensibly.
        const total = 5
        const completed = Math.min(navState.currentStep, total)
        setCheckpointInfo({
          exists: true,
          completed_count: completed,
          total_count: total,
          progress_percent: Math.round((completed / total) * 100),
        })
        setRestoreSource('local')
        setCheckpointLastUpdated(undefined)
        setShowCheckpointDialog(true)
        return
      }

      // 3. No draft anywhere — clean slate. We reset here, then set the path
      //    so the user starts on Step 1 with empty fields. We do NOT set
      //    hasResetStores=true here: that flag triggers another reset on
      //    modal close, which would wipe anything the user types during
      //    this session before they get a chance to come back to it. The
      //    next reopen for a different project will reset via this same
      //    branch; the next reopen for the same project will hit branch 2.
      resetAllWizardStores()

      setProjectPath(normalisedPath, name)
      setProjectContextName(name)
    } catch (err) {
      log.error('Failed to initialize with path', err)
      closeModal()
    }
  }

  const selectProjectPath = async () => {
    setIsSelectingPath(true)
    try {
      const selected = await openDialog({
        directory: true,
        multiple: false,
        title: t('modal.selectProjectFolder'),
      })

      if (selected && typeof selected === 'string') {
        await initializeWithPath(selected)
      } else {
        // User cancelled - close modal
        closeModal()
      }
    } catch (err) {
      log.error('Failed to select folder', err)
      closeModal()
    } finally {
      setIsSelectingPath(false)
    }
  }

  const handleContinueCheckpoint = async () => {
    // Local resume: zustand `persist` already hydrated the stores. Just close
    // the dialog. We deliberately do NOT set hasResetStores — that flag
    // triggers a wipe on modal close, which would defeat the point of
    // resuming. The draft stays in localStorage until the user either
    // completes the wizard (handleOpenProject sets the flag) or explicitly
    // picks "Start new".
    if (restoreSource === 'local') {
      setShowCheckpointDialog(false)
      return
    }

    try {
      // CRITICAL SAFETY CHECK: Verify we have a valid project path
      if (!selectedProjectPath || selectedProjectPath.length === 0) {
        throw new Error('No project path selected - cannot resume checkpoint')
      }

      // CRITICAL: Verify this is NOT the Venore app directory
      const venoreAppDir = 'venore_v2'
      const venoreDesktopDir = 'venore-desktop'
      if (selectedProjectPath.includes(venoreAppDir) || selectedProjectPath.includes(venoreDesktopDir)) {
        throw new Error(`SAFETY ABORT: Attempting to scan Venore app directory: ${selectedProjectPath}`)
      }

      // Show loading dialog with real-time progress
      setShowCheckpointDialog(false)
      setIsRestoringSession(true)

      // Mark that we have "reset" stores (loading from checkpoint)
      setHasResetStores(true)

      // Restore wizard session from backend (handles checkpoint loading)
      const restored = await tauriApi.restoreWizardSession(selectedProjectPath)
      const config = restored.wizard_config

      // Preserve architecture/techDebt from local store if checkpoint
      // doesn't carry them — they were captured in Step 1 and shouldn't
      // be wiped on resume.
      const existingStep1 = useWizardDataStore.getState().step1

      // Load basic wizard data from checkpoint
      useWizardDataStore.setState({
        step1: {
          name: config.project_name,
          description: config.project_description,
          projectState: config.project_state as any,
          teamSize: config.team_size as any,
          goals: config.goals as any[],
          architecture: (config as any).architecture ?? existingStep1.architecture ?? '',
          techDebt: (config as any).tech_debt ?? existingStep1.techDebt ?? '',
        },
        step2: {
          depthLevel: config.depth_level as any,
          layersToGenerate: config.layers_to_generate as any[],
          exclusions: config.exclusions,
          ragEnabled: true,
          projectType: null,
          projectPath: selectedProjectPath,
          projectName: config.project_name,
        },
      })

      // Jump to Step 2 (analysis + indexing) so user re-runs the pipeline
      setStep(2)

      // Close loading dialog
      setIsRestoringSession(false)
    } catch (err) {
      log.error('Failed to load checkpoint', err)
      setIsRestoringSession(false)
      handleStartNew()
    }
  }

  const handleStartNew = async () => {
    try {
      await tauriApi.deleteWizardCheckpoint(selectedProjectPath)

      // Reset all stores for new project. Same reasoning as branch 3 of
      // initializeWithPath: do NOT set hasResetStores=true — the cleanup
      // useEffect would wipe again on modal close and lose anything the
      // user types during this fresh session.
      resetAllWizardStores()

      const name = await basename(selectedProjectPath) || t('modal.unnamedProject')
      setProjectPath(selectedProjectPath, name)
      setProjectContextName(name)

      setShowCheckpointDialog(false)
    } catch (err) {
      log.error('Failed to delete checkpoint', err)
      // Continue anyway
      resetAllWizardStores()
      setHasResetStores(true)

      const name = await basename(selectedProjectPath) || t('modal.unnamedProject')
      setProjectPath(selectedProjectPath, name)
      setProjectContextName(name)
      setShowCheckpointDialog(false)
    }
  }

  /** Cancel any in-flight wizard pipeline and close the modal.
   *
   *  Sends a cancellation signal to the backend so detect_project_modules
   *  or wizard_index_project bail at their next checkpoint instead of
   *  running to completion in the background — preventing wasted CPU and
   *  the race condition where a subsequent wizard run for the same project
   *  collides with the old one still writing to the same DB rows.
   */
  const disconnectAndClose = () => {
    setShowCloseConfirm(false)
    if (selectedProjectPath) {
      tauriApi.cancelWizardSession(selectedProjectPath).catch(() => {})
    }
    closeModal()
  }

  const handleCancelCheckpoint = () => {
    setShowCheckpointDialog(false)
    closeModal()
  }

  const handleCancel = () => {
    if (isGenerationActive()) {
      setShowCloseConfirm(true)
      return
    }
    closeModal()
  }

  // Render current step
  const renderStep = () => {
    switch (currentStep) {
      case 0:
        return <Step1ProjectContext />
      case 1:
        return <Step2AnalysisRules />
      case 2:
        return <Step3AnalysisIndex />
      case 3:
        return <Step4IndexResults />
      case 4:
        return <Step5Complete />
      default:
        return <div className="p-6 text-center text-foreground-muted">{t('modal.unknownStep')}</div>
    }
  }

  if (!open) return null

  // Show loading state while selecting path
  if (isSelectingPath) {
    return (
      <div className="fixed inset-0 z-[100] flex items-center justify-center">
        <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" />
        <div className="relative z-10 bg-background border border-border rounded-xl shadow-2xl p-12">
          <p className="text-foreground-muted">{t('modal.openingFolderPicker')}</p>
        </div>
      </div>
    )
  }

  return (
    <>
      {/* Restoring Session Dialog (shows while re-scanning project after checkpoint resume) */}
      {isRestoringSession ? (
        <RestoringSessionDialog
          open={isRestoringSession}
          projectPath={selectedProjectPath}
        />
      ) : showCheckpointDialog ? (
        <CheckpointDialog
          open={showCheckpointDialog}
          checkpoint={checkpointInfo}
          lastUpdatedAt={checkpointLastUpdated}
          kind={restoreSource ?? 'disk'}
          onContinue={handleContinueCheckpoint}
          onStartNew={handleStartNew}
          onCancel={handleCancelCheckpoint}
        />
      ) : (
        /* Main Wizard Modal */
        <div className="fixed inset-0 z-[100] flex items-center justify-center">
        {/* Overlay */}
        <div
          className="absolute inset-0 bg-black/60 backdrop-blur-sm"
          onClick={handleCancel}
        />

        {/* Wizard container */}
        <div className="relative z-10 w-full max-w-3xl mx-4 flex flex-col max-h-[90vh]">
          <div className="bg-background border border-border rounded-xl shadow-2xl flex flex-col overflow-hidden">
            {/* Header with Step Indicator */}
            <WizardHeader currentStep={currentStep} onClose={handleCancel} />

            {/* Content - Scrollable with min height */}
            <div className="flex-1 overflow-y-auto min-h-[450px]">
              {renderStep()}
              {currentStep === 4 && openError && (
                <div className="mx-6 mb-4 p-3 rounded-lg border border-destructive/40 bg-destructive/10 flex items-start gap-2">
                  <AlertCircle className="w-4 h-4 text-destructive shrink-0 mt-0.5" />
                  <div className="min-w-0 flex-1">
                    <p className="text-sm font-medium text-destructive">{t('modal.openFailed', 'Could not open project')}</p>
                    <p className="text-xs text-foreground-muted mt-0.5 break-words">{openError}</p>
                  </div>
                </div>
              )}
            </div>

            {/* Footer */}
            <WizardFooter
              onCancel={handleCancel}
              customNextButton={currentStep === 4 ? (
                <Button onClick={handleOpenProject} disabled={isOpening}>
                  {isOpening
                    ? (openError ? t('modal.retry', 'Retry') : t('modal.opening', 'Opening...'))
                    : (openError ? t('modal.retry', 'Retry') : t('modal.openProject'))}
                  <ArrowRight size={16} className="ml-2" />
                </Button>
              ) : undefined}
            />
          </div>
        </div>
        </div>
      )}

      {/* Close Confirmation Dialog - z-[110] to render ABOVE the wizard (z-[100]) */}
      {showCloseConfirm && (
        <div className="fixed inset-0 z-[110] flex items-center justify-center">
          <div
            className="absolute inset-0 bg-black/60 backdrop-blur-sm"
            onClick={() => setShowCloseConfirm(false)}
          />
          <div className="relative z-10 w-full max-w-md mx-4 bg-background border border-border rounded-xl shadow-2xl p-6 space-y-4">
            {/* Header */}
            <div className="flex items-center gap-3">
              <div className="p-2 rounded-lg bg-destructive/10 shrink-0">
                <AlertCircle className="w-5 h-5 text-destructive" />
              </div>
              <div>
                <h3 className="text-lg font-semibold">{t('modal.closeWizard')}</h3>
                <p className="text-sm text-muted-foreground">{t('modal.generationPaused')}</p>
              </div>
            </div>

            {/* Body */}
            <p className="text-sm text-muted-foreground">
              {t('modal.resumeLater')}
            </p>

            {/* Footer */}
            <div className="flex items-center justify-end gap-3 pt-2">
              <Button variant="ghost" onClick={() => setShowCloseConfirm(false)}>
                {t('modal.continueGenerating')}
              </Button>
              <Button variant="destructive" onClick={disconnectAndClose}>
                {t('modal.close')}
              </Button>
            </div>
          </div>
        </div>
      )}
    </>
  )
}
