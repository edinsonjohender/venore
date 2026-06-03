// =============================================================================
// Step5Complete - Wizard completion screen + AI memory summary
// =============================================================================
// Auto-generates an enriched project memory at mount time: takes the user's
// Step 1 input + Step 2 detected modules and asks the LLM to produce a
// structured analysis (architecture, summary, etc.) the user can edit before
// the wizard persists everything via handleOpenProject.

import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { CheckCircle2, FolderOpen, FileCode, Boxes, GitBranch, Loader2, RefreshCw, AlertCircle, Sparkles } from 'lucide-react'
import { Card, CardContent } from '@/components/ui/card'
import { Textarea } from '@/components/ui/textarea'
import { Button } from '@/components/ui/button'
import { useWizardDataStore } from '@/stores/wizardDataStore'
import { tauriApi, type GenerateMemoryResponse } from '@/lib/tauri'
import { createLogger } from '@/lib/logger'

const log = createLogger('wizard:step5')

export function Step5Complete() {
  const { t } = useTranslation('wizard')
  const projectName = useWizardDataStore((s) => s.step1.name)
  const userDescription = useWizardDataStore((s) => s.step1.description)
  const userArchitecture = useWizardDataStore((s) => s.step1.architecture)
  const userTechDebt = useWizardDataStore((s) => s.step1.techDebt)
  const projectPath = useWizardDataStore((s) => s.step2.projectPath)
  const depthLevel = useWizardDataStore((s) => s.step2.depthLevel)
  const detectedModules = useWizardDataStore((s) => s.step3.detectedModules)
  const indexResult = useWizardDataStore((s) => s.indexResult)
  const aiDraft = useWizardDataStore((s) => s.step5to8.aiMemoryDraft)
  const setAiMemoryDraft = useWizardDataStore((s) => s.setAiMemoryDraft)
  const patchAiMemoryDraft = useWizardDataStore((s) => s.patchAiMemoryDraft)

  const [isGenerating, setIsGenerating] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [feedback, setFeedback] = useState('')
  const [loadedExisting, setLoadedExisting] = useState(false)
  const startedRef = useRef(false)

  // indexResult.indexed counts only files NEW or CHANGED in this run.
  // For the "total files in the index" stat the user expects, we sum
  // indexed + skipped (skipped = unchanged files that stayed in the index).
  const filesIndexed = (indexResult?.indexed ?? 0) + (indexResult?.skipped ?? 0)
  // Use the count of distinct detected modules (what the user sees in the
  // canvas) rather than indexResult.modulesMapped, which counts file→module
  // mappings (a much larger and confusing number).
  const modulesMapped = detectedModules.length
  const depsCreated = indexResult?.depsCreated ?? 0

  /**
   * Run the AI analysis.
   * - When `withFeedback` is false (or omitted), generates fresh — discards
   *   any prior draft and ignores the feedback textarea.
   * - When true, sends the current `aiDraft` + `feedback` so the LLM refines
   *   instead of rewriting from scratch. The feedback box is cleared on
   *   success so the user can iterate.
   */
  const runGenerate = async (withFeedback: boolean = false) => {
    if (!projectPath) {
      setError('Project path missing')
      return
    }
    setIsGenerating(true)
    setError(null)
    try {
      const useRefinement = withFeedback && aiDraft && feedback.trim().length > 0
      const response: GenerateMemoryResponse = await tauriApi.generateProjectMemory({
        projectPath,
        userDescription: userDescription || undefined,
        userArchitecture: userArchitecture || undefined,
        userTechDebt: userTechDebt || undefined,
        detectedModules: detectedModules.map((m) => m.name),
        depthLevel: depthLevel || 'normal',
        userFeedback: useRefinement ? feedback.trim() : undefined,
        previousDraft: useRefinement ? aiDraft ?? undefined : undefined,
      })
      setAiMemoryDraft({
        description: response.description,
        state: response.state,
        goals: response.goals,
        architecture: response.architecture,
        techDebt: response.techDebt,
        projectSummary: response.projectSummary,
      })
      // The draft is now LLM-generated (fresh or refined). Clear the
      // "loaded from disk" hint so the UI stops claiming the draft is
      // the on-disk version.
      setLoadedExisting(false)
      if (useRefinement) {
        setFeedback('')
      }
      log.info(useRefinement ? 'AI memory draft refined' : 'AI memory draft generated')
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      log.error('Failed to generate AI memory', err)
      setError(msg)
    } finally {
      setIsGenerating(false)
    }
  }

  // Auto-start once on mount. If the project already has a curated memory
  // in `.venore/project-memory.json` (re-run from Tools menu on a
  // Venorized project, or a fresh wizard on a freshly-cloned repo with
  // committed memory), preload it as the draft so we don't burn an LLM
  // call regenerating something the user already iterated on. The user
  // can still hit "Regenerate" to force a fresh Gemini draft, or use the
  // feedback box to refine.
  useEffect(() => {
    if (startedRef.current) return
    if (aiDraft) return
    if (!projectPath) return
    startedRef.current = true
    ;(async () => {
      try {
        const existing = await tauriApi.readProjectMemoryByPath(projectPath)
        if (existing) {
          setAiMemoryDraft({
            description: existing.description,
            state: existing.state,
            goals: existing.goals,
            architecture: existing.architecture,
            techDebt: existing.techDebt,
            projectSummary: existing.projectSummary,
          })
          setLoadedExisting(true)
          log.info('Loaded existing project-memory.json (skipped LLM call)')
          return
        }
      } catch (err) {
        // Read failure is non-fatal — fall through to generation.
        log.warn('Existing memory probe failed, falling back to generation', err)
      }
      runGenerate()
    })()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  return (
    <div className="p-6 space-y-5">
      {/* Compact success header */}
      <div className="flex items-center gap-3">
        <CheckCircle2 size={24} className="text-green-600" />
        <div className="flex-1 min-w-0">
          <h2 className="text-base font-semibold">{t('step8.projectIndexed')}</h2>
          <p className="text-xs text-muted-foreground truncate">{t('step8.indexSuccessDescription')}</p>
        </div>
      </div>

      {/* Stats row */}
      <div className="grid grid-cols-3 gap-3">
        <Card>
          <CardContent className="py-3 text-center">
            <FileCode size={16} className="mx-auto mb-1 text-primary" />
            <p className="text-lg font-bold">{filesIndexed}</p>
            <p className="text-[10px] text-muted-foreground uppercase tracking-wide">{t('step8.filesIndexed')}</p>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="py-3 text-center">
            <Boxes size={16} className="mx-auto mb-1 text-primary" />
            <p className="text-lg font-bold">{modulesMapped}</p>
            <p className="text-[10px] text-muted-foreground uppercase tracking-wide">{t('step8.modules')}</p>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="py-3 text-center">
            <GitBranch size={16} className="mx-auto mb-1 text-primary" />
            <p className="text-lg font-bold">{depsCreated}</p>
            <p className="text-[10px] text-muted-foreground uppercase tracking-wide">{t('step8.dependencies')}</p>
          </CardContent>
        </Card>
      </div>

      {/* Project info card */}
      <Card>
        <CardContent className="pt-4 pb-4">
          <div className="flex items-start gap-3">
            <FolderOpen size={18} className="text-primary shrink-0 mt-0.5" />
            <div className="flex-1 min-w-0">
              <p className="text-sm font-medium">{projectName}</p>
              <p className="text-xs text-muted-foreground break-all">{projectPath}</p>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* AI Summary block */}
      <Card>
        <CardContent className="pt-4 pb-4 space-y-3">
          <div className="flex items-center gap-2">
            <Sparkles size={16} className="text-brand" />
            <h3 className="text-sm font-semibold flex-1">{t('step8.aiSummary', 'AI project analysis')}</h3>
            {(aiDraft || error) && (
              <Button
                variant="ghost"
                size="sm"
                onClick={() => runGenerate(false)}
                disabled={isGenerating}
                className="h-7 text-xs"
                title={t('step8.startOverTip', 'Discard the current draft and analyze the project from scratch')}
              >
                <RefreshCw size={12} className={isGenerating ? 'mr-1.5 animate-spin' : 'mr-1.5'} />
                {t('step8.startOver', 'Start over')}
              </Button>
            )}
          </div>

          {/* Loading state */}
          {isGenerating && !aiDraft && (
            <div className="flex items-center gap-2 py-4 text-xs text-muted-foreground">
              <Loader2 size={14} className="animate-spin text-brand" />
              <span>{t('step8.analyzing', 'Analyzing project structure...')}</span>
            </div>
          )}

          {/* Loaded-from-disk hint — only when the draft came from
              `.venore/project-memory.json` (re-run wizard on Venorized
              project). Tells the user nothing burned LLM tokens; the
              "Start over" button above forces a fresh generation. */}
          {loadedExisting && aiDraft && !isGenerating && (
            <div className="p-2.5 rounded-md border border-brand/30 bg-brand/5 flex items-start gap-2">
              <Sparkles size={14} className="text-brand shrink-0 mt-0.5" />
              <p className="text-xs text-foreground-muted">
                {t('step8.loadedExisting', 'Loaded existing memory from .venore/project-memory.json. Edit fields below, use the feedback box to refine with AI, or click "Start over" to regenerate from scratch.')}
              </p>
            </div>
          )}

          {/* Error state */}
          {error && !isGenerating && (
            <div className="p-2.5 rounded-md border border-destructive/40 bg-destructive/10 flex items-start gap-2">
              <AlertCircle size={14} className="text-destructive shrink-0 mt-0.5" />
              <div className="min-w-0 flex-1">
                <p className="text-xs text-destructive font-medium">{t('step8.analysisFailed', 'Analysis failed')}</p>
                <p className="text-[11px] text-foreground-muted mt-0.5 break-words">{error}</p>
              </div>
            </div>
          )}

          {/* Editable preview */}
          {aiDraft && (
            <div className="space-y-3">
              <div className="space-y-1">
                <label className="text-[10px] font-medium uppercase tracking-wider text-foreground-muted">
                  {t('step8.fieldDescription', 'Description')}
                </label>
                <Textarea
                  value={aiDraft.description}
                  onChange={(e) => patchAiMemoryDraft({ description: e.target.value })}
                  rows={2}
                  className="text-xs resize-none"
                />
              </div>
              <div className="space-y-1">
                <label className="text-[10px] font-medium uppercase tracking-wider text-foreground-muted">
                  {t('step8.fieldArchitecture', 'Architecture')}
                </label>
                <Textarea
                  value={aiDraft.architecture}
                  onChange={(e) => patchAiMemoryDraft({ architecture: e.target.value })}
                  rows={3}
                  className="text-xs resize-none"
                />
              </div>
              <div className="space-y-1">
                <label className="text-[10px] font-medium uppercase tracking-wider text-foreground-muted">
                  {t('step8.fieldTechDebt', 'Tech debt')}
                </label>
                <Textarea
                  value={aiDraft.techDebt}
                  onChange={(e) => patchAiMemoryDraft({ techDebt: e.target.value })}
                  rows={2}
                  className="text-xs resize-none"
                />
              </div>
              <div className="space-y-1">
                <label className="text-[10px] font-medium uppercase tracking-wider text-foreground-muted">
                  {t('step8.fieldProjectSummary', 'Project summary')}
                </label>
                <Textarea
                  value={aiDraft.projectSummary}
                  onChange={(e) => patchAiMemoryDraft({ projectSummary: e.target.value })}
                  rows={10}
                  className="text-xs resize-none font-mono"
                />
              </div>

              {/* Iterative refinement — feedback to the AI without losing the
                  current draft. Different from "Start over" (top right) which
                  discards everything and re-analyzes from scratch. */}
              <div className="pt-3 border-t border-border/60 space-y-2">
                <label className="text-[10px] font-medium uppercase tracking-wider text-foreground-muted">
                  {t('step8.refineWithFeedback', 'Refine with feedback')}
                </label>
                <Textarea
                  value={feedback}
                  onChange={(e) => setFeedback(e.target.value)}
                  placeholder={t('step8.feedbackPlaceholder', 'Tell the AI what to fix, add, or remove...')}
                  rows={2}
                  className="text-xs resize-none"
                  disabled={isGenerating}
                />
                <div className="flex justify-end">
                  <Button
                    size="sm"
                    onClick={() => runGenerate(true)}
                    disabled={isGenerating || feedback.trim().length === 0}
                    className="h-7 text-xs"
                  >
                    <RefreshCw size={12} className={isGenerating ? 'mr-1.5 animate-spin' : 'mr-1.5'} />
                    {t('step8.regenerateWithFeedback', 'Regenerate with feedback')}
                  </Button>
                </div>
              </div>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
