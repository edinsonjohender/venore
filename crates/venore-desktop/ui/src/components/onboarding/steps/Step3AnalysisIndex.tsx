// =============================================================================
// Step3AnalysisIndex - Analysis + RAG indexing (replaces old Module Selection)
// =============================================================================

import { useEffect, useCallback, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { listen } from '@tauri-apps/api/event'
import { Loader2, AlertCircle, CheckCircle2, Database } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { useWizardDataStore } from '@/stores/wizardDataStore'
import { useWizardCacheStore } from '@/stores/wizardCacheStore'
import { tauriApi, VenoreError } from '@/lib/tauri'
import { adaptModuleFromBackend, adaptMetricsFromBackend } from '@/lib/wizard/adapters'
import { createLogger } from '@/lib/logger'

const log = createLogger('wizard:analysis-index')

export function Step3AnalysisIndex() {
  const { t } = useTranslation('wizard')
  const projectPath = useWizardDataStore((s) => s.step2.projectPath)
  const depthLevel = useWizardDataStore((s) => s.step2.depthLevel)
  const layersToGenerate = useWizardDataStore((s) => s.step2.layersToGenerate)
  const exclusions = useWizardDataStore((s) => s.step2.exclusions)
  const indexResult = useWizardDataStore((s) => s.indexResult)
  const setDetectedModules = useWizardDataStore((s) => s.setDetectedModules)
  const setIndexResult = useWizardDataStore((s) => s.setIndexResult)

  const isAnalyzing = useWizardCacheStore((s) => s.isAnalyzing)
  const analysisProgress = useWizardCacheStore((s) => s.analysisProgress)
  const analysisError = useWizardCacheStore((s) => s.analysisError)
  const isIndexing = useWizardCacheStore((s) => s.isIndexing)
  const indexingError = useWizardCacheStore((s) => s.indexingError)
  const indexProgress = useWizardCacheStore((s) => s.indexProgress)
  const setIsAnalyzing = useWizardCacheStore((s) => s.setIsAnalyzing)
  const setAnalysisProgress = useWizardCacheStore((s) => s.setAnalysisProgress)
  const setAnalysisMetrics = useWizardCacheStore((s) => s.setAnalysisMetrics)
  const setAnalysisError = useWizardCacheStore((s) => s.setAnalysisError)
  const setIsIndexing = useWizardCacheStore((s) => s.setIsIndexing)
  const setIndexingError = useWizardCacheStore((s) => s.setIndexingError)
  const setIndexProgress = useWizardCacheStore((s) => s.setIndexProgress)

  const hasStartedRef = useRef(false)

  const handleAnalyzeAndIndex = useCallback(async () => {
    if (!projectPath) {
      log.error('No project path')
      return
    }

    // Phase 1: Analysis
    setIsAnalyzing(true)
    setAnalysisError(null)
    setIndexingError(null)
    setIndexResult(null)
    setIndexProgress(null)

    // Tracks which phase failed for the catch handler. Using a local variable
    // instead of reading `isAnalyzing` from state — the state setters above
    // don't update the value captured in this function's closure until the
    // next render, so by the time the catch runs the flag is stale.
    let phase: 'analysis' | 'indexing' = 'analysis'

    try {
      setAnalysisProgress({ current: 0, total: 5, currentItem: t('step3.startingAnalysis') })

      // Run module detection (populates WizardSessionManager cache)
      const response = await tauriApi.detectProjectModules({
        project_path: projectPath,
        depth_level: depthLevel,
        layers: ['context'],
        exclusions,
      })

      const adaptedModules = response.modules.map(adaptModuleFromBackend)
      const adaptedMetrics = adaptMetricsFromBackend(response.metrics)

      setDetectedModules(adaptedModules)
      setAnalysisMetrics(adaptedMetrics)
      setAnalysisProgress({ current: 5, total: 5, currentItem: t('step3.analysisComplete') })
      setIsAnalyzing(false)

      // Phase 2: RAG Indexing
      phase = 'indexing'
      setIsIndexing(true)

      const indexResponse = await tauriApi.wizardIndexProject(projectPath, layersToGenerate, exclusions)

      setIndexResult({
        indexed: indexResponse.indexed,
        skipped: indexResponse.skipped,
        removed: indexResponse.removed,
        modulesDetected: indexResponse.modules_detected,
        modulesMapped: indexResponse.modules_mapped,
        depsCreated: indexResponse.deps_created,
        refsCreated: indexResponse.refs_created,
      })

      setIsIndexing(false)
      log.info('Analysis + indexing complete', indexResponse)
    } catch (err) {
      // User-initiated cancellation isn't a failure — the backend bailed at
      // a checkpoint because cancel_wizard_session was called (modal close
      // during analysis/indexing). Log it as info and don't surface a red
      // Alert: the modal is already closing.
      if (err instanceof VenoreError && err.code === 'CANCELLED') {
        log.info(`${phase} cancelled by user`, err.message)
        setIsAnalyzing(false)
        setIsIndexing(false)
        return
      }
      log.error(`${phase} failed`, err)
      const errorMsg = err instanceof Error ? err.message : 'Failed to analyze and index project'
      if (phase === 'analysis') {
        setAnalysisError(errorMsg)
      } else {
        setIndexingError(errorMsg)
      }
      setIsAnalyzing(false)
      setIsIndexing(false)
    }
  }, [projectPath, depthLevel, exclusions, layersToGenerate, t])

  // Listen for real-time analysis progress events.
  //
  // `listen()` is async — the unsubscribe handle isn't available until the
  // promise resolves. If the component unmounts between mount and that
  // resolution, the synchronous cleanup function runs while the handles are
  // still null and the backend registers the listeners anyway, leaking them
  // for the rest of the process lifetime. A `cancelled` flag bridges the gap:
  // when each `listen()` resolves we either store the handle, or unsubscribe
  // immediately if cleanup already ran.
  useEffect(() => {
    if (!projectPath) return

    let cancelled = false
    let unlistenProgress: (() => void) | null = null
    let unlistenComplete: (() => void) | null = null
    let unlistenIndex: (() => void) | null = null

    // Throttle: the indexer emits a wizard-index-progress event per file
    // (can be 30+/s on a large repo). Updating React state at that rate
    // floods the renderer. We coalesce to ~10 updates/second by skipping
    // events that arrive within `THROTTLE_MS` of the last commit, except
    // we always commit phase-boundary events (current === null) so the
    // sub-phase transitions don't get swallowed.
    const THROTTLE_MS = 100
    let lastEmitTs = 0

    const setupListeners = async () => {
      const progressHandle = await listen<{
        session_id: string
        current_step: number
        total_steps: number
        step_description: string
        current_item?: string
      }>('analysis-progress', (event) => {
        const payload = event.payload
        if (payload.session_id === projectPath) {
          setAnalysisProgress({
            current: payload.current_step,
            total: payload.total_steps,
            currentItem: payload.step_description + (payload.current_item ? ` (${payload.current_item})` : ''),
          })
        }
      })
      if (cancelled) { progressHandle(); return }
      unlistenProgress = progressHandle

      const completeHandle = await listen<{
        session_id: string
        success: boolean
        error?: string
      }>('analysis-complete', (event) => {
        const payload = event.payload
        if (payload.session_id === projectPath) {
          if (!payload.success && payload.error) {
            setAnalysisError(payload.error)
            setIsAnalyzing(false)
            setAnalysisProgress(null)
          }
        }
      })
      if (cancelled) { completeHandle(); return }
      unlistenComplete = completeHandle

      const indexHandle = await listen<{
        session_id: string
        current_phase: number
        total_phases: number
        description: string
        current: number | null
        total: number | null
        current_item: string | null
      }>('wizard-index-progress', (event) => {
        const payload = event.payload
        if (payload.session_id !== projectPath) return
        const now = Date.now()
        const isBoundary = payload.current === null
        if (!isBoundary && (now - lastEmitTs) < THROTTLE_MS) return
        lastEmitTs = now
        setIndexProgress({
          currentPhase: payload.current_phase,
          totalPhases: payload.total_phases,
          description: payload.description,
          current: payload.current,
          total: payload.total,
          currentItem: payload.current_item,
        })
      })
      if (cancelled) { indexHandle(); return }
      unlistenIndex = indexHandle
    }

    setupListeners().catch((err) => {
      log.error('Failed to setup event listeners', err)
    })

    return () => {
      cancelled = true
      if (unlistenProgress) unlistenProgress()
      if (unlistenComplete) unlistenComplete()
      if (unlistenIndex) unlistenIndex()
    }
  }, [projectPath])

  // Auto-start on mount
  useEffect(() => {
    if (!hasStartedRef.current && !indexResult && !isAnalyzing && !isIndexing && projectPath) {
      hasStartedRef.current = true
      handleAnalyzeAndIndex()
    }
  }, [indexResult, isAnalyzing, isIndexing, projectPath, handleAnalyzeAndIndex])

  const error = analysisError || indexingError

  // Loading state
  if (isAnalyzing || isIndexing) {
    const phase = isAnalyzing ? t('step3.analyzingProject') : t('step3.indexingProject')
    return (
      <div className="p-6 flex flex-col items-center justify-center min-h-[300px]">
        <Loader2 size={32} className="text-primary animate-spin mb-4" />
        <p className="text-sm font-medium mb-2">{phase}</p>
        {isAnalyzing && analysisProgress && (
          <>
            <div className="w-64 h-1 bg-secondary rounded-full overflow-hidden mb-2">
              <div
                className="h-full bg-primary transition-all duration-300"
                style={{
                  width: `${Math.round((analysisProgress.current / analysisProgress.total) * 100)}%`,
                }}
              />
            </div>
            <p className="text-xs text-muted-foreground">
              {t('step3.stepProgress', { current: analysisProgress.current, total: analysisProgress.total, item: analysisProgress.currentItem })}
            </p>
          </>
        )}
        {isIndexing && (
          <div className="flex flex-col items-center w-full max-w-md gap-2">
            {indexProgress ? (
              <>
                {/* Sub-phase bar: indexing / graph / layers */}
                <div className="flex items-center gap-1 w-64">
                  {[1, 2, 3].map((phaseNum) => (
                    <div
                      key={phaseNum}
                      className={`h-1 flex-1 rounded-full transition-colors duration-300 ${
                        phaseNum < indexProgress.currentPhase
                          ? 'bg-primary'
                          : phaseNum === indexProgress.currentPhase
                            ? 'bg-primary/60'
                            : 'bg-secondary'
                      }`}
                    />
                  ))}
                </div>
                <p className="text-sm font-medium">
                  {indexProgress.description}
                </p>
                {indexProgress.current !== null && indexProgress.total !== null && indexProgress.total > 0 && (
                  <p className="text-xs text-muted-foreground tabular-nums">
                    {indexProgress.current}/{indexProgress.total}
                    {indexProgress.currentItem ? (
                      <span className="ml-2 opacity-70 truncate inline-block max-w-[280px] align-bottom">
                        — {indexProgress.currentItem}
                      </span>
                    ) : null}
                  </p>
                )}
              </>
            ) : (
              <p className="text-xs text-muted-foreground">
                {t('step3.indexingDescription')}
              </p>
            )}
          </div>
        )}
      </div>
    )
  }

  // Error state
  if (error) {
    return (
      <div className="p-6 flex flex-col items-center justify-center min-h-[300px]">
        <AlertCircle size={32} className="text-destructive mb-4" />
        <p className="text-sm mb-2">{t('step3.errorDuringAnalysis')}</p>
        <p className="text-xs text-muted-foreground mb-4">{error}</p>
        <Button onClick={() => {
          hasStartedRef.current = false
          setAnalysisError(null)
          setIndexingError(null)
          handleAnalyzeAndIndex()
        }}>
          {t('step3.retry')}
        </Button>
      </div>
    )
  }

  // Success state
  if (indexResult) {
    return (
      <div className="p-6 flex flex-col items-center justify-center min-h-[300px]">
        <div className="relative mb-6">
          <div className="absolute inset-0 blur-2xl bg-green-500/20 rounded-full" />
          <CheckCircle2 size={48} className="text-green-600 relative" />
        </div>
        <h3 className="text-lg font-semibold mb-2">{t('step3.indexingComplete')}</h3>
        <div className="flex items-center gap-6 text-sm text-muted-foreground mb-4">
          <span>{indexResult.indexed + indexResult.skipped} {t('step3.filesIndexed')}</span>
          <span>{indexResult.modulesMapped} {t('step3.modules')}</span>
          <span>{indexResult.depsCreated} {t('step3.dependencies')}</span>
        </div>
        <p className="text-xs text-muted-foreground">{t('step3.clickNextForResults')}</p>
      </div>
    )
  }

  // Waiting state (shouldn't normally be visible)
  return (
    <div className="p-6 flex flex-col items-center justify-center min-h-[300px]">
      <Database size={40} className="text-muted-foreground mb-3" />
      <p className="text-sm text-muted-foreground">{t('step3.waitingToStart')}</p>
    </div>
  )
}
