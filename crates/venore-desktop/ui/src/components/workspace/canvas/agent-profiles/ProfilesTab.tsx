// =============================================================================
// ProfilesTab — Agent profile list + detail editor (CRUD)
// =============================================================================

import { useState, useEffect, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { Bot, Loader2, AlertCircle, Clock, Plus, Trash2, Save } from 'lucide-react'
import { cn } from '@/lib/utils'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { Label } from '@/components/ui/label'
import { Slider } from '@/components/ui/slider'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select'
import { tauriApi } from '@/lib/tauri'
import { STAGE_COLORS, SEVERITY_COLORS } from './types'
import type { AgentProfile, AgentRule, AgentStage, RuleSeverity, ToolDefinition, ToolCategory } from './types'
import { PROVIDERS, type AIProvider } from '@/components/ai-config/AIConfigPanel'

// -----------------------------------------------------------------------------
// ProfileListItem
// -----------------------------------------------------------------------------

function ProfileListItem({
  profile, isSelected, onSelect,
}: {
  profile: AgentProfile
  isSelected: boolean
  onSelect: () => void
}) {
  const { t } = useTranslation('agents')
  const colors = STAGE_COLORS[profile.stage]
  const incomplete = !profile.name.trim() || !profile.provider.trim() || !profile.model.trim()

  return (
    <button
      onClick={onSelect}
      className={cn(
        'w-full text-left px-3 py-2.5 border-b border-border/30 transition-colors',
        isSelected
          ? 'bg-background-tertiary border-l-2 border-l-brand'
          : 'hover:bg-background-tertiary/50 border-l-2 border-l-transparent',
        incomplete && 'opacity-50',
      )}
    >
      <div className="flex items-center gap-2 mb-1">
        <span className={cn('text-xs font-medium truncate flex-1', incomplete ? 'text-foreground-muted italic' : 'text-foreground')}>
          {profile.name || t('profiles.untitled')}
        </span>
        <div className={cn(
          'w-1.5 h-1.5 rounded-full shrink-0',
          profile.isEnabled ? 'bg-green-400' : 'bg-foreground-muted/30',
        )} />
      </div>
      <div className="flex items-center gap-2">
        <span className={cn('text-[10px] px-1.5 py-0.5 rounded-full font-medium', colors.bg, colors.text)}>
          {profile.stage}
        </span>
        <span className="text-[10px] text-foreground-muted/60 truncate">
          {profile.model || t('profiles.noModel')}
        </span>
      </div>
    </button>
  )
}

// -----------------------------------------------------------------------------
// ProfileDetail
// -----------------------------------------------------------------------------

function HistorySection({ className }: { className?: string }) {
  const { t } = useTranslation('agents')
  return (
    <div className={cn('p-4', className)}>
      <div className="flex items-center gap-2 mb-4">
        <Clock className="w-3.5 h-3.5 text-foreground-muted/60" />
        <span className="text-[11px] font-medium uppercase tracking-wider text-foreground-muted">
          {t('profiles.history')}
        </span>
      </div>
      <div className="text-xs text-foreground-muted/40 text-center py-8">
        {t('profiles.noHistory')}
      </div>
    </div>
  )
}

function FieldLabel({ children }: { children: React.ReactNode }) {
  return (
    <Label className="text-[11px] font-medium uppercase tracking-wider text-foreground-muted">
      {children}
    </Label>
  )
}

function ProfileDetail({
  profile, isNew, onUpdate, onDelete, providerStatus, availableModels, allRules, allTools, allCategories,
}: {
  profile: AgentProfile
  isNew: boolean
  onUpdate: (draft: AgentProfile) => void
  onDelete: (id: string) => void
  providerStatus: Record<AIProvider, boolean>
  availableModels: Record<AIProvider, string[]>
  allRules: AgentRule[]
  allTools: ToolDefinition[]
  allCategories: ToolCategory[]
}) {
  const { t } = useTranslation('agents')
  const [draft, setDraft] = useState<AgentProfile>(profile)
  const [saving, setSaving] = useState(false)
  const [confirmDelete, setConfirmDelete] = useState(false)

  // Reset draft when profile.id changes
  useEffect(() => {
    setDraft(profile)
    setSaving(false)
    setConfirmDelete(false)
  }, [profile.id]) // eslint-disable-line react-hooks/exhaustive-deps

  const patch = useCallback(<K extends keyof AgentProfile>(key: K, value: AgentProfile[K]) => {
    setDraft((prev) => ({ ...prev, [key]: value }))
  }, [])

  const isDraft = profile.id.startsWith('draft-')
  const isComplete = !!(draft.name.trim() && draft.provider.trim() && draft.model.trim())
  const isDirty = isDraft || JSON.stringify(draft) !== JSON.stringify(profile)

  const handleSave = async () => {
    if (!isDirty || saving) return
    setSaving(true)
    // Force disabled if required fields are missing
    const toSave = isComplete ? draft : { ...draft, isEnabled: false }
    await onUpdate(toSave)
    if (!isComplete) setDraft(toSave)
    setSaving(false)
  }

  const handleDeleteClick = () => {
    if (profile.isTemplate) return
    if (confirmDelete) {
      setConfirmDelete(false)
      onDelete(profile.id)
    } else {
      setConfirmDelete(true)
      setTimeout(() => setConfirmDelete(false), 2000)
    }
  }

  return (
    <div className="flex-1 flex flex-col min-w-0 min-h-0 overflow-hidden">
      {/* Action bar */}
      <div className="flex items-center justify-between px-4 py-2 border-b border-border/40 shrink-0">
        <span className="text-xs font-medium text-foreground truncate">
          {draft.name || t('profiles.untitled')}
        </span>
        <div className="flex items-center gap-2">
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
            {saving ? t('profiles.saving') : t('profiles.save')}
          </button>
          <button
            onClick={handleDeleteClick}
            disabled={profile.isTemplate}
            className={cn(
              'flex items-center gap-1.5 px-3 py-1 rounded-md text-xs transition-colors disabled:opacity-30 disabled:cursor-not-allowed',
              confirmDelete
                ? 'bg-red-500/15 text-red-400 hover:bg-red-500/25'
                : 'text-foreground-muted/60 hover:text-foreground hover:bg-background-tertiary',
            )}
            title={profile.isTemplate ? t('profiles.cannotDeleteTemplate') : confirmDelete ? t('profiles.clickToConfirm') : t('profiles.deleteAgentTitle')}
          >
            <Trash2 className="w-3 h-3" />
            {confirmDelete ? t('profiles.confirm') : t('profiles.deleteBtn')}
          </button>
        </div>
      </div>

      {/* Content: scrollable form + optional side History */}
      <div className="flex-1 min-h-0 min-w-0 flex">
      <div className="flex-1 min-w-0 min-h-0 overflow-y-auto flex flex-col">
        {/* Columns with fields — wrap when narrow */}
        <div className="flex flex-wrap min-w-0">
          {/* Col 1 — Principal */}
          <div className="flex-1 basis-[260px] p-4 space-y-4 border-r border-border/30">
          {/* Name */}
          <div className="space-y-1.5">
            <FieldLabel>{t('profiles.name')}</FieldLabel>
            <Input
              value={draft.name}
              onChange={(e) => patch('name', e.target.value)}
              className="text-xs"
            />
          </div>

          {/* Description */}
          <div className="space-y-1.5">
            <FieldLabel>{t('profiles.description')}</FieldLabel>
            <Textarea
              value={draft.description}
              onChange={(e) => patch('description', e.target.value)}
              className="min-h-[60px] text-xs resize-none"
            />
          </div>

          {/* Stage */}
          <div className="space-y-1.5">
            <FieldLabel>{t('profiles.stage')}</FieldLabel>
            <Select value={draft.stage} onValueChange={(v) => patch('stage', v as AgentStage)}>
              <SelectTrigger className="text-xs h-9">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="triager">{t('pipeline.triager')}</SelectItem>
                <SelectItem value="specialist">{t('pipeline.specialist')}</SelectItem>
                <SelectItem value="reporter">{t('pipeline.reporter')}</SelectItem>
              </SelectContent>
            </Select>
          </div>

          {/* Provider + Model */}
          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1.5">
              <FieldLabel>{t('profiles.provider')}</FieldLabel>
              <Select
                value={draft.provider}
                onValueChange={(v) => {
                  patch('provider', v)
                  // Auto-select first model of new provider
                  const models = availableModels[v as AIProvider] ?? []
                  if (models.length > 0) patch('model', models[0])
                }}
              >
                <SelectTrigger className="text-xs h-9">
                  <SelectValue placeholder={t('profiles.selectProvider')} />
                </SelectTrigger>
                <SelectContent>
                  {(Object.keys(providerStatus) as AIProvider[])
                    .filter((p) => providerStatus[p])
                    .map((p) => (
                      <SelectItem key={p} value={p}>
                        {PROVIDERS.find((pr) => pr.id === p)?.name ?? p}
                      </SelectItem>
                    ))}
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-1.5">
              <FieldLabel>{t('profiles.model')}</FieldLabel>
              <Select
                value={draft.model}
                onValueChange={(v) => patch('model', v)}
              >
                <SelectTrigger className="text-xs h-9 font-mono">
                  <SelectValue placeholder={t('profiles.selectModel')} />
                </SelectTrigger>
                <SelectContent>
                  {(availableModels[draft.provider as AIProvider] ?? []).map((m) => (
                    <SelectItem key={m} value={m} className="font-mono text-xs">
                      {m}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>

          {/* Temperature */}
          <div className="space-y-1.5">
            <div className="flex items-center justify-between">
              <FieldLabel>{t('profiles.temperature')}</FieldLabel>
              <span className="text-[11px] text-foreground-muted/60 font-mono">
                {draft.temperature.toFixed(1)}
              </span>
            </div>
            <Slider
              value={[draft.temperature]}
              onValueChange={([v]) => patch('temperature', v)}
              min={0}
              max={2}
              step={0.1}
            />
          </div>

          {/* Enabled — toggle */}
          <div className="space-y-1.5">
            <FieldLabel>{t('profiles.enabled')}</FieldLabel>
            <button
              type="button"
              onClick={() => isComplete && patch('isEnabled', !draft.isEnabled)}
              className={cn(
                'flex items-center gap-2 h-9 px-3 w-full rounded-lg border border-border bg-background-secondary text-xs text-left transition-colors',
                isComplete ? 'hover:bg-background-tertiary' : 'opacity-50 cursor-not-allowed',
              )}
              title={isComplete ? undefined : t('profiles.enableHint')}
            >
              <div className={cn(
                'w-2 h-2 rounded-full shrink-0 transition-colors',
                draft.isEnabled ? 'bg-green-400' : 'bg-foreground-muted/30',
              )} />
              <span className="text-foreground-muted">
                {draft.isEnabled ? t('profiles.active') : t('profiles.disabled')}
              </span>
            </button>
          </div>
        </div>

        {/* Col 2 — Advanced (without system prompt) */}
        <div className={cn('flex-1 basis-[260px] p-4 flex flex-col gap-4', !isNew && 'border-r border-border/30')}>
          {/* Max Tokens */}
          <div className="space-y-1.5">
            <FieldLabel>{t('profiles.maxTokens')}</FieldLabel>
            <Input
              type="number"
              value={draft.maxTokensPerRun}
              onChange={(e) => patch('maxTokensPerRun', parseInt(e.target.value) || 0)}
              className="text-xs font-mono"
            />
          </div>

          {/* Rules assignment */}
          {allRules.length > 0 && (
            <div className="flex flex-col gap-1.5 min-h-0">
              <FieldLabel>{t('profiles.rules', { count: draft.ruleIds.length })}</FieldLabel>
              <div className="max-h-[200px] overflow-y-auto border border-border/40 rounded-lg p-2 space-y-0.5">
                {allRules.map((rule) => {
                  const checked = draft.ruleIds.includes(rule.id)
                  const colors = SEVERITY_COLORS[rule.severity]
                  return (
                    <div
                      key={rule.id}
                      className="flex items-center gap-2.5 px-1.5 py-1.5 rounded hover:bg-background-tertiary/50 cursor-pointer"
                      onClick={() => {
                        const next = checked
                          ? draft.ruleIds.filter((id) => id !== rule.id)
                          : [...draft.ruleIds, rule.id]
                        patch('ruleIds', next)
                      }}
                    >
                      <Checkbox
                        checked={checked}
                        onCheckedChange={() => {
                          const next = checked
                            ? draft.ruleIds.filter((id) => id !== rule.id)
                            : [...draft.ruleIds, rule.id]
                          patch('ruleIds', next)
                        }}
                        className="shrink-0"
                      />
                      <span className="text-xs text-foreground truncate flex-1">{rule.name}</span>
                      <span className={cn('text-[9px] px-1.5 py-0.5 rounded-full font-medium shrink-0', colors.bg, colors.text)}>
                        {rule.severity}
                      </span>
                    </div>
                  )
                })}
              </div>
            </div>
          )}

          {/* Tools assignment — grouped by category */}
          {allTools.length > 0 && (() => {
            // Empty toolIds = "all tools" — show all as checked
            const isAllTools = draft.toolIds.length === 0
            const allToolIds = allTools.map((t) => t.id)
            // Group tools by category
            const categoryMap = new Map<string, ToolCategory>()
            for (const cat of allCategories) categoryMap.set(cat.id, cat)
            const grouped = new Map<string, ToolDefinition[]>()
            for (const tool of allTools) {
              const list = grouped.get(tool.categoryId) ?? []
              list.push(tool)
              grouped.set(tool.categoryId, list)
            }
            // Sort categories by displayOrder
            const sortedCatIds = [...grouped.keys()].sort((a, b) => {
              const ca = categoryMap.get(a)
              const cb = categoryMap.get(b)
              return (ca?.displayOrder ?? 99) - (cb?.displayOrder ?? 99)
            })

            const toggleTool = (toolId: string) => {
              if (isAllTools) {
                // Expand to explicit list minus this one
                patch('toolIds', allToolIds.filter((id) => id !== toolId))
              } else {
                const checked = draft.toolIds.includes(toolId)
                const next = checked
                  ? draft.toolIds.filter((id) => id !== toolId)
                  : [...draft.toolIds, toolId]
                // If all are selected, collapse back to empty
                patch('toolIds', next.length === allToolIds.length ? [] : next)
              }
            }

            return (
              <div className="flex-1 flex flex-col gap-1.5 min-h-0">
                <FieldLabel>{t('profiles.tools', { count: isAllTools ? allTools.length : draft.toolIds.length })}</FieldLabel>
                <div className="flex-1 overflow-y-auto border border-border/40 rounded-lg p-2 space-y-2">
                  {sortedCatIds.map((catId) => {
                    const cat = categoryMap.get(catId)
                    const catTools = grouped.get(catId) ?? []
                    return (
                      <div key={catId}>
                        <div className="text-[10px] font-medium uppercase tracking-wider text-foreground-muted/60 px-1.5 mb-1">
                          {cat?.name ?? catId}
                        </div>
                        {catTools.map((tool) => {
                          const checked = isAllTools || draft.toolIds.includes(tool.id)
                          return (
                            <div
                              key={tool.id}
                              className="flex items-center gap-2.5 px-1.5 py-1 rounded hover:bg-background-tertiary/50 cursor-pointer"
                              onClick={() => toggleTool(tool.id)}
                            >
                              <Checkbox checked={checked} onCheckedChange={() => toggleTool(tool.id)} className="shrink-0" />
                              <span className="text-xs text-foreground truncate flex-1">{tool.name}</span>
                              {tool.isReadOnly && (
                                <span className="text-[9px] px-1.5 py-0.5 rounded-full font-medium shrink-0 bg-blue-500/15 text-blue-400">
                                  read-only
                                </span>
                              )}
                            </div>
                          )
                        })}
                      </div>
                    )
                  })}
                </div>
              </div>
            )
          })()}
        </div>
        </div>

        {/* System Prompt (full width, fills remaining height) */}
        <div className="flex-1 min-h-[180px] flex flex-col border-t border-border/30 p-4">
          <div className="flex items-center justify-between mb-2">
            <FieldLabel>{t('profiles.systemPrompt')}</FieldLabel>
            <span className="text-[10px] text-foreground-muted/40 font-mono">
              {t('profiles.chars', { count: draft.systemPrompt.length })}
            </span>
          </div>
          <Textarea
            value={draft.systemPrompt}
            onChange={(e) => patch('systemPrompt', e.target.value)}
            className="flex-1 min-h-[120px] text-xs font-mono resize-none"
          />
        </div>

        {/* History — below form on narrow screens (< xl) */}
        {!isNew && <HistorySection className="border-t border-border/30 xl:hidden" />}
      </div>

      {/* History — side column on wide screens (>= xl) */}
      {!isNew && <HistorySection className="hidden xl:block w-[250px] shrink-0 border-l border-border/30 overflow-y-auto" />}
      </div>
    </div>
  )
}

// -----------------------------------------------------------------------------
// ProfilesTab
// -----------------------------------------------------------------------------

function mapDtoToProfile(d: Awaited<ReturnType<typeof tauriApi.listAgentProfiles>>[number]): AgentProfile {
  let ruleIds: string[] = []
  try { ruleIds = JSON.parse(d.rulesJson || '[]') } catch { /* keep empty */ }
  let toolIds: string[] = []
  try { toolIds = JSON.parse(d.toolsJson || '[]') } catch { /* keep empty */ }

  return {
    id: d.id,
    name: d.name,
    description: d.description,
    stage: d.stage as AgentStage,
    provider: d.provider,
    model: d.model,
    temperature: d.temperature,
    systemPrompt: d.systemPrompt,
    maxTokensPerRun: d.maxTokensPerRun,
    isTemplate: d.isTemplate,
    isEnabled: d.isEnabled,
    ruleIds,
    toolIds,
  }
}

export function ProfilesTab() {
  const { t } = useTranslation('agents')
  const [profiles, setProfiles] = useState<AgentProfile[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [newIds, setNewIds] = useState<Set<string>>(new Set())
  const [providerStatus, setProviderStatus] = useState<Record<AIProvider, boolean>>({
    openai: false, anthropic: false, gemini: false, ollama: false,
  })
  const [availableModels, setAvailableModels] = useState<Record<AIProvider, string[]>>({
    openai: [], anthropic: [], gemini: [], ollama: [],
  })
  const [allRules, setAllRules] = useState<AgentRule[]>([])
  const [allTools, setAllTools] = useState<ToolDefinition[]>([])
  const [allCategories, setAllCategories] = useState<ToolCategory[]>([])

  useEffect(() => {
    // Load tools and categories in parallel (non-blocking)
    tauriApi.listToolDefinitions()
      .then((data) => setAllTools(data.map((d) => ({
        id: d.id,
        name: d.name,
        description: d.description,
        categoryId: d.categoryId,
        parametersJson: d.parametersJson,
        isReadOnly: d.isReadOnly,
        isEnabled: d.isEnabled,
        isTemplate: d.isTemplate,
      }))))
      .catch(() => {})

    tauriApi.listToolCategories()
      .then((data) => setAllCategories(data.map((d) => ({
        id: d.id,
        name: d.name,
        description: d.description,
        icon: d.icon,
        color: d.color,
        displayOrder: d.displayOrder,
        isTemplate: d.isTemplate,
      }))))
      .catch(() => {})

    // Load rules in parallel (non-blocking)
    tauriApi.listAgentRules()
      .then((data) => setAllRules(data.map((d) => ({
        id: d.id,
        name: d.name,
        description: d.description,
        scope: d.scope,
        severity: d.severity as RuleSeverity,
        isActive: d.isActive,
        isTemplate: d.isTemplate,
      }))))
      .catch(() => {})

    // Load profiles first — show UI immediately
    tauriApi.listAgentProfiles()
      .then((data) => setProfiles(data.map(mapDtoToProfile)))
      .catch((err) => setError(err.message ?? 'Failed to load profiles'))
      .finally(() => setLoading(false))

    // Load provider/model data in background (non-blocking)
    ;(async () => {
      try {
        const configuredRes = await tauriApi.getConfiguredProviders()
        const configured = new Set(configuredRes.providers)

        const status: Record<AIProvider, boolean> = {
          openai: configured.has('openai'),
          anthropic: configured.has('anthropic'),
          gemini: configured.has('gemini'),
          ollama: false,
        }

        // Test ollama without blocking — fire and forget update
        tauriApi.testConnection({ provider: 'ollama' })
          .then((test) => {
            if (test.success) {
              setProviderStatus((prev) => ({ ...prev, ollama: true }))
              tauriApi.getOllamaModels()
                .then((models) => setAvailableModels((prev) => ({ ...prev, ollama: models })))
                .catch(() => {})
            }
          })
          .catch(() => {})

        setProviderStatus(status)

        // Load models for configured providers in parallel
        const apiProviders: AIProvider[] = ['openai', 'anthropic', 'gemini']
        const results = await Promise.allSettled(
          apiProviders
            .filter((p) => status[p])
            .map(async (p) => {
              const res = await tauriApi.getAvailableModels(p)
              return { provider: p, models: res.models }
            })
        )

        const modelsMap: Partial<Record<AIProvider, string[]>> = {}
        for (const r of results) {
          if (r.status === 'fulfilled') modelsMap[r.value.provider] = r.value.models
        }
        setAvailableModels((prev) => ({ ...prev, ...modelsMap }))
      } catch { /* non-critical */ }
    })()
  }, [])

  const handleCreate = () => {
    // Don't allow creating another draft while one exists unsaved
    if (newIds.size > 0) {
      // Select the existing draft instead
      const existingDraft = [...newIds][0]
      setSelectedId(existingDraft)
      return
    }

    const tempId = `draft-${crypto.randomUUID()}`
    const draft: AgentProfile = {
      id: tempId,
      name: '',
      description: '',
      stage: 'specialist',
      provider: '',
      model: '',
      temperature: 0.7,
      systemPrompt: '',
      maxTokensPerRun: 30000,
      isTemplate: false,
      isEnabled: false,
      ruleIds: [],
      toolIds: [],
    }
    setProfiles((prev) => [...prev, draft])
    setNewIds((prev) => new Set(prev).add(tempId))
    setSelectedId(tempId)
  }

  const handleUpdate = useCallback(async (draft: AgentProfile) => {
    const isDraft = draft.id.startsWith('draft-')
    try {
      if (isDraft) {
        // First save — create in backend
        const dto = await tauriApi.createAgentProfile({
          name: draft.name,
          description: draft.description,
          stage: draft.stage,
          provider: draft.provider,
          model: draft.model,
          temperature: draft.temperature,
          systemPrompt: draft.systemPrompt,
          maxTokensPerRun: draft.maxTokensPerRun,
          isEnabled: draft.isEnabled,
          rulesJson: JSON.stringify(draft.ruleIds),
          toolsJson: JSON.stringify(draft.toolIds),
        })
        const created = mapDtoToProfile(dto)
        // Replace the local draft with the persisted profile
        setProfiles((prev) => prev.map((p) => p.id === draft.id ? created : p))
        setNewIds((prev) => {
          const next = new Set(prev)
          next.delete(draft.id)
          return next
        })
        setSelectedId(created.id)
      } else {
        // Normal update
        const dto = await tauriApi.updateAgentProfile({
          id: draft.id,
          name: draft.name,
          description: draft.description,
          stage: draft.stage,
          provider: draft.provider,
          model: draft.model,
          temperature: draft.temperature,
          systemPrompt: draft.systemPrompt,
          maxTokensPerRun: draft.maxTokensPerRun,
          isEnabled: draft.isEnabled,
          rulesJson: JSON.stringify(draft.ruleIds),
          toolsJson: JSON.stringify(draft.toolIds),
        })
        const updated = mapDtoToProfile(dto)
        setProfiles((prev) => prev.map((p) => p.id === updated.id ? updated : p))
      }
    } catch {
      // Silently fail — the user sees "Saving..." stay briefly
    }
  }, [])

  const handleDelete = useCallback(async (id: string) => {
    try {
      // Only call backend if it's a persisted profile
      if (!id.startsWith('draft-')) {
        await tauriApi.deleteAgentProfile(id)
      }
      setProfiles((prev) => {
        const next = prev.filter((p) => p.id !== id)
        // Select next profile or null
        if (selectedId === id) {
          const idx = prev.findIndex((p) => p.id === id)
          const nextProfile = next[Math.min(idx, next.length - 1)]
          setSelectedId(nextProfile?.id ?? null)
        }
        return next
      })
      setNewIds((prev) => {
        const next = new Set(prev)
        next.delete(id)
        return next
      })
    } catch {
      // Silently fail
    }
  }, [selectedId])

  const selectedProfile = profiles.find((p) => p.id === selectedId) ?? null

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center text-foreground-muted/50">
        <Loader2 className="w-5 h-5 animate-spin mr-2" />
        <span className="text-xs">{t('profiles.loading')}</span>
      </div>
    )
  }

  if (error) {
    return (
      <div className="flex-1 flex items-center justify-center text-red-400/80">
        <AlertCircle className="w-5 h-5 mr-2" />
        <span className="text-xs">{error}</span>
      </div>
    )
  }

  return (
    <div className="flex-1 flex min-w-0 overflow-hidden">
      {/* Left — Profile list */}
      <div className="w-[250px] shrink-0 border-r border-border overflow-hidden flex flex-col">
        <div className="px-3 py-2 border-b border-border/40 flex items-center justify-between">
          <div>
            <span className="text-[11px] font-medium uppercase tracking-wider text-foreground-muted">
              {t('innerTabs.profiles')}
            </span>
            <span className="text-[10px] text-foreground-muted/50 ml-1.5">
              ({profiles.length})
            </span>
          </div>
          <button
            onClick={handleCreate}
            className="p-1 rounded hover:bg-background-tertiary text-foreground-muted/60 hover:text-foreground transition-colors"
            title={t('profiles.createAgentTitle')}
          >
            <Plus className="w-3.5 h-3.5" />
          </button>
        </div>
        <div className="flex-1 overflow-y-auto">
          {profiles.map((profile) => (
            <ProfileListItem
              key={profile.id}
              profile={profile}
              isSelected={profile.id === selectedId}
              onSelect={() => setSelectedId(profile.id)}
            />
          ))}
        </div>
      </div>

      {/* Right — Profile detail or empty state */}
      <div className="flex-1 min-w-0 flex flex-col">
        {selectedProfile ? (
          <ProfileDetail
            profile={selectedProfile}
            isNew={newIds.has(selectedProfile.id)}
            onUpdate={handleUpdate}
            onDelete={handleDelete}
            providerStatus={providerStatus}
            availableModels={availableModels}
            allRules={allRules}
            allTools={allTools}
            allCategories={allCategories}
          />
        ) : (
          <div className="flex-1 flex flex-col items-center justify-center text-foreground-muted/40">
            <Bot className="w-10 h-10 mb-3 opacity-20" />
            <span className="text-xs mb-3">{t('profiles.noSelected')}</span>
            <button
              onClick={handleCreate}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs bg-brand/15 text-brand hover:bg-brand/25 transition-colors"
            >
              <Plus className="w-3.5 h-3.5" />
              {t('profiles.createAgent')}
            </button>
          </div>
        )}
      </div>
    </div>
  )
}
