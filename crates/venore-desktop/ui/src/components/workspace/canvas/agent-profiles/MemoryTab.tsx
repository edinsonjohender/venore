// =============================================================================
// MemoryTab — Project Memory editor (single memory per project)
// =============================================================================

import { useState, useEffect, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { Brain, Loader2, Save, RefreshCw, Sparkles } from 'lucide-react'
import { cn } from '@/lib/utils'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { Label } from '@/components/ui/label'
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select'
import { Checkbox } from '@/components/ui/checkbox'
import { tauriApi } from '@/lib/tauri'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface MemoryDraft {
  name: string
  description: string
  state: string
  teamSize: string
  goals: string[]
  architecture: string
  techDebt: string
  responseLanguage: string
  projectSummary: string
}

const GOAL_OPTIONS = ['onboarding', 'understand', 'document', 'refactor', 'audit', 'maintain'] as const

/** Map legacy state values to the unified enum. Old rows might have `state='new'` from
 *  the previous wizard enum; surface them as `planning` (the equivalent in the new set). */
function normalizeState(state: string): string {
  if (state === 'new') return 'planning'
  return state
}

const EMPTY_DRAFT: MemoryDraft = {
  name: '',
  description: '',
  state: 'active',
  teamSize: 'solo',
  goals: [],
  architecture: '',
  techDebt: '',
  responseLanguage: 'en',
  projectSummary: '',
}

// -----------------------------------------------------------------------------
// FieldLabel
// -----------------------------------------------------------------------------

function FieldLabel({ children }: { children: React.ReactNode }) {
  return (
    <Label className="text-[11px] font-medium uppercase tracking-wider text-foreground-muted">
      {children}
    </Label>
  )
}

// -----------------------------------------------------------------------------
// MemoryTab
// -----------------------------------------------------------------------------

interface MemoryTabProps {
  projectPath?: string
  projectId?: string
}

export function MemoryTab({ projectPath, projectId }: MemoryTabProps) {
  const { t } = useTranslation('agents')

  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [regenerating, setRegenerating] = useState(false)
  const [generating, setGenerating] = useState(false)
  const [hasMemory, setHasMemory] = useState(false)
  const [existingId, setExistingId] = useState<string | null>(null)
  const [draft, setDraft] = useState<MemoryDraft>({ ...EMPTY_DRAFT })
  const [savedSnapshot, setSavedSnapshot] = useState<string>('')

  const isDirty = JSON.stringify(draft) !== savedSnapshot

  // Load existing memory
  useEffect(() => {
    if (!projectId) {
      setLoading(false)
      return
    }
    tauriApi.getProjectMemory(projectId)
      .then((data) => {
        if (data) {
          const d: MemoryDraft = {
            name: data.name,
            description: data.description,
            state: normalizeState(data.state),
            teamSize: data.teamSize,
            goals: data.goals,
            architecture: data.architecture,
            techDebt: data.techDebt ?? '',
            responseLanguage: data.responseLanguage,
            projectSummary: data.projectSummary,
          }
          setDraft(d)
          setSavedSnapshot(JSON.stringify(d))
          setExistingId(data.id)
          setHasMemory(true)
        }
      })
      .catch(() => {})
      .finally(() => setLoading(false))
  }, [projectId])

  const patch = useCallback(<K extends keyof MemoryDraft>(key: K, value: MemoryDraft[K]) => {
    setDraft((prev) => ({ ...prev, [key]: value }))
  }, [])

  const toggleGoal = useCallback((goal: string) => {
    setDraft((prev) => {
      const has = prev.goals.includes(goal)
      return {
        ...prev,
        goals: has ? prev.goals.filter((g) => g !== goal) : [...prev.goals, goal],
      }
    })
  }, [])

  const handleSave = async () => {
    if (!projectId || saving) return
    setSaving(true)
    try {
      const result = await tauriApi.saveProjectMemory({
        projectId,
        name: draft.name,
        description: draft.description,
        state: draft.state,
        teamSize: draft.teamSize,
        goals: draft.goals,
        architecture: draft.architecture,
        techDebt: draft.techDebt,
        responseLanguage: draft.responseLanguage,
        conventions: [],
        projectSummary: draft.projectSummary,
      })
      setExistingId(result.id)
      setHasMemory(true)
      setSavedSnapshot(JSON.stringify(draft))
    } catch {
      // silently fail
    } finally {
      setSaving(false)
    }
  }

  const handleRegenerate = async () => {
    if (!projectId || !projectPath || regenerating) return
    setRegenerating(true)
    try {
      const summary = await tauriApi.regenerateMemorySummary({
        projectId,
        projectPath,
      })
      patch('projectSummary', summary)
    } catch {
      // silently fail
    } finally {
      setRegenerating(false)
    }
  }

  /** Generate memory via LLM — fills all fields. Auto-saves only on first creation. */
  const handleGenerate = async () => {
    if (!projectId || !projectPath || generating) return
    setGenerating(true)
    try {
      // Derive project name from path
      const segments = projectPath.replace(/\\/g, '/').split('/')
      const projectName = segments[segments.length - 1] || 'Project'
      const locale = localStorage.getItem('venore-language') || 'en'

      // Call LLM to generate all fields
      const generated = await tauriApi.generateProjectMemory({ projectPath })

      const newDraft: MemoryDraft = {
        name: draft.name || projectName,
        description: generated.description,
        state: normalizeState(generated.state),
        teamSize: draft.teamSize || 'solo',
        goals: generated.goals,
        architecture: generated.architecture,
        techDebt: generated.techDebt ?? '',
        responseLanguage: draft.responseLanguage || locale,
        projectSummary: generated.projectSummary,
      }

      if (!hasMemory) {
        // First creation — save immediately
        const result = await tauriApi.saveProjectMemory({
          projectId,
          ...newDraft,
          conventions: [],
        })
        setSavedSnapshot(JSON.stringify(newDraft))
        setExistingId(result.id)
        setHasMemory(true)
      }

      // Always update the draft (user can review + Save)
      setDraft(newDraft)
    } catch (err) {
      console.error('[MemoryTab] generate failed:', err)
    } finally {
      setGenerating(false)
    }
  }

  // ---- Render: loading ----
  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center text-foreground-muted/50">
        <Loader2 className="w-5 h-5 animate-spin mr-2" />
        <span className="text-xs">{t('memory.loading')}</span>
      </div>
    )
  }

  // ---- Render: no project ----
  if (!projectId) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center text-foreground-muted/40">
        <Brain className="w-10 h-10 mb-3 opacity-20" />
        <span className="text-xs">{t('memory.noMemory')}</span>
      </div>
    )
  }

  // ---- Render: empty state — no memory yet, show generate button ----
  if (!hasMemory) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center text-foreground-muted/40">
        <Brain className="w-12 h-12 mb-4 opacity-20" />
        <span className="text-sm mb-1 text-foreground-muted/60">{t('memory.noMemory')}</span>
        <span className="text-xs mb-5 text-foreground-muted/40 max-w-md text-center">
          {t('memory.noMemoryHint')}
        </span>
        <button
          onClick={handleGenerate}
          disabled={generating}
          className={cn(
            'flex items-center gap-2 px-4 py-2 rounded-lg text-xs font-medium transition-colors',
            'bg-brand/15 text-brand hover:bg-brand/25',
            generating && 'opacity-60',
          )}
        >
          {generating
            ? <Loader2 className="w-4 h-4 animate-spin" />
            : <Sparkles className="w-4 h-4" />}
          {generating ? t('memory.generating') : t('memory.generate')}
        </button>
      </div>
    )
  }

  // ---- Render: form ----
  return (
    <div className="flex-1 flex flex-col min-w-0 min-h-0">
      {/* Action bar */}
      <div className="flex items-center justify-between px-4 py-2 border-b border-border/40">
        <span className="text-xs font-medium text-foreground truncate">
          {draft.name || t('memory.title')}
        </span>
        <div className="flex items-center gap-2">
          <button
            onClick={handleGenerate}
            disabled={generating || !projectPath}
            className={cn(
              'flex items-center gap-1.5 px-3 py-1 rounded-md text-xs transition-colors',
              'text-brand hover:bg-brand/15',
              generating && 'opacity-60',
            )}
          >
            {generating
              ? <Loader2 className="w-3 h-3 animate-spin" />
              : <Sparkles className="w-3 h-3" />}
            {generating ? t('memory.generating') : t('memory.generate')}
          </button>
          <button
            onClick={handleSave}
            disabled={!isDirty || saving}
            className={cn(
              'flex items-center gap-1.5 px-3 py-1 rounded-md text-xs transition-colors',
              isDirty
                ? 'bg-brand/15 text-brand hover:bg-brand/25'
                : 'bg-background-tertiary text-foreground-muted/40 cursor-default',
              saving && 'opacity-60',
            )}
          >
            {saving
              ? <Loader2 className="w-3 h-3 animate-spin" />
              : <Save className="w-3 h-3" />}
            {saving ? t('memory.saving') : t('memory.save')}
          </button>
        </div>
      </div>

      {/* Form */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {/* Two columns */}
        <div className="flex flex-wrap gap-4">
          {/* Col 1 */}
          <div className="flex-1 basis-[260px] space-y-4">
            {/* Name */}
            <div className="space-y-1.5">
              <FieldLabel>{t('memory.name')}</FieldLabel>
              <Input
                value={draft.name}
                onChange={(e) => patch('name', e.target.value)}
                className="text-xs"
              />
            </div>

            {/* Description */}
            <div className="space-y-1.5">
              <FieldLabel>{t('memory.description')}</FieldLabel>
              <Textarea
                value={draft.description}
                onChange={(e) => patch('description', e.target.value)}
                className="min-h-[72px] text-xs resize-none"
                rows={3}
              />
            </div>

            {/* State */}
            <div className="space-y-1.5">
              <FieldLabel>{t('memory.state')}</FieldLabel>
              <Select value={draft.state} onValueChange={(v) => patch('state', v)}>
                <SelectTrigger className="text-xs h-9">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="planning">{t('memory.statePlanning')}</SelectItem>
                  <SelectItem value="active">{t('memory.stateActive')}</SelectItem>
                  <SelectItem value="maintenance">{t('memory.stateMaintenance')}</SelectItem>
                  <SelectItem value="legacy">{t('memory.stateLegacy')}</SelectItem>
                  <SelectItem value="archived">{t('memory.stateArchived')}</SelectItem>
                </SelectContent>
              </Select>
            </div>

            {/* Team Size */}
            <div className="space-y-1.5">
              <FieldLabel>{t('memory.teamSize')}</FieldLabel>
              <Select value={draft.teamSize} onValueChange={(v) => patch('teamSize', v)}>
                <SelectTrigger className="text-xs h-9">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="solo">{t('memory.teamSolo')}</SelectItem>
                  <SelectItem value="small">{t('memory.teamSmall')}</SelectItem>
                  <SelectItem value="medium">{t('memory.teamMedium')}</SelectItem>
                  <SelectItem value="large">{t('memory.teamLarge')}</SelectItem>
                </SelectContent>
              </Select>
            </div>

            {/* Response Language */}
            <div className="space-y-1.5">
              <FieldLabel>{t('memory.responseLanguage')}</FieldLabel>
              <Select value={draft.responseLanguage} onValueChange={(v) => patch('responseLanguage', v)}>
                <SelectTrigger className="text-xs h-9">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="en">{t('memory.langEn')}</SelectItem>
                  <SelectItem value="es">{t('memory.langEs')}</SelectItem>
                  <SelectItem value="zh">{t('memory.langZh')}</SelectItem>
                  <SelectItem value="pt">{t('memory.langPt')}</SelectItem>
                  <SelectItem value="ja">{t('memory.langJa')}</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>

          {/* Col 2 */}
          <div className="flex-1 basis-[260px] space-y-4">
            {/* Goals */}
            <div className="space-y-1.5">
              <FieldLabel>{t('memory.goals')}</FieldLabel>
              <div className="flex flex-wrap gap-x-4 gap-y-2">
                {GOAL_OPTIONS.map((g) => (
                  <div
                    key={g}
                    className="flex items-center gap-1.5 cursor-pointer"
                    onClick={() => toggleGoal(g)}
                  >
                    <Checkbox
                      checked={draft.goals.includes(g)}
                      onCheckedChange={() => toggleGoal(g)}
                    />
                    <span className="text-xs text-foreground-muted capitalize">
                      {t(`memory.goal${g.charAt(0).toUpperCase() + g.slice(1)}`)}
                    </span>
                  </div>
                ))}
              </div>
            </div>

            {/* Architecture */}
            <div className="space-y-1.5">
              <FieldLabel>{t('memory.architecture')}</FieldLabel>
              <Textarea
                value={draft.architecture}
                onChange={(e) => patch('architecture', e.target.value)}
                className="min-h-[72px] text-xs resize-none"
                rows={3}
              />
            </div>

            {/* Tech debt */}
            <div className="space-y-1.5">
              <FieldLabel>{t('memory.techDebt')}</FieldLabel>
              <Textarea
                value={draft.techDebt}
                onChange={(e) => patch('techDebt', e.target.value)}
                placeholder={t('memory.techDebtPlaceholder')}
                className="min-h-[72px] text-xs resize-none"
                rows={3}
              />
            </div>
          </div>
        </div>

        {/* Full-width: Project Summary */}
        <div className="space-y-1.5">
          <div className="flex items-center justify-between">
            <FieldLabel>{t('memory.projectSummary')}</FieldLabel>
            <button
              onClick={handleRegenerate}
              disabled={regenerating || !projectPath}
              className={cn(
                'flex items-center gap-1 text-[10px] transition-colors',
                regenerating
                  ? 'text-foreground-muted/40'
                  : 'text-brand hover:text-brand/80',
              )}
            >
              <RefreshCw className={cn('w-3 h-3', regenerating && 'animate-spin')} />
              {regenerating ? t('memory.regenerating') : t('memory.regenerate')}
            </button>
          </div>
          <Textarea
            value={draft.projectSummary}
            onChange={(e) => patch('projectSummary', e.target.value)}
            className="min-h-[200px] text-xs resize-none font-mono"
            rows={10}
          />
        </div>
      </div>
    </div>
  )
}
