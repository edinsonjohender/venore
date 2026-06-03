// =============================================================================
// RulesTab — Global rules library (CRUD)
// =============================================================================

import { useState, useEffect, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { ShieldCheck, Loader2, AlertCircle, Plus, Trash2, Save } from 'lucide-react'
import { cn } from '@/lib/utils'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { Label } from '@/components/ui/label'
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select'
import { Checkbox } from '@/components/ui/checkbox'
import { tauriApi } from '@/lib/tauri'
import { SEVERITY_COLORS } from './types'
import type { AgentRule, RuleSeverity } from './types'

// -----------------------------------------------------------------------------
// RuleListItem
// -----------------------------------------------------------------------------

function RuleListItem({
  rule, isSelected, onSelect,
}: {
  rule: AgentRule
  isSelected: boolean
  onSelect: () => void
}) {
  const colors = SEVERITY_COLORS[rule.severity]

  return (
    <button
      onClick={onSelect}
      className={cn(
        'w-full text-left px-3 py-2.5 border-b border-border/30 transition-colors',
        isSelected
          ? 'bg-background-tertiary border-l-2 border-l-brand'
          : 'hover:bg-background-tertiary/50 border-l-2 border-l-transparent',
      )}
    >
      <div className="flex items-center gap-2 mb-1">
        <span className="text-xs font-medium text-foreground truncate flex-1">
          {rule.name}
        </span>
        <div className={cn(
          'w-1.5 h-1.5 rounded-full shrink-0',
          rule.isActive ? 'bg-green-400' : 'bg-foreground-muted/30',
        )} />
      </div>
      <div className="flex items-center gap-2">
        <span className={cn('text-[10px] px-1.5 py-0.5 rounded-full font-medium', colors.bg, colors.text)}>
          {rule.severity}
        </span>
        <span className="text-[10px] text-foreground-muted/60 truncate">
          {rule.scope.join(', ')}
        </span>
      </div>
    </button>
  )
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
// RuleDetail
// -----------------------------------------------------------------------------

const SCOPE_OPTIONS = ['file', 'module', 'project'] as const

function RuleDetail({
  rule, onUpdate, onDelete,
}: {
  rule: AgentRule
  onUpdate: (draft: AgentRule) => void
  onDelete: (id: string) => void
}) {
  const { t } = useTranslation('agents')
  const [draft, setDraft] = useState<AgentRule>(rule)
  const [saving, setSaving] = useState(false)
  const [confirmDelete, setConfirmDelete] = useState(false)

  useEffect(() => {
    setDraft(rule)
    setSaving(false)
    setConfirmDelete(false)
  }, [rule.id]) // eslint-disable-line react-hooks/exhaustive-deps

  const patch = useCallback(<K extends keyof AgentRule>(key: K, value: AgentRule[K]) => {
    setDraft((prev) => ({ ...prev, [key]: value }))
  }, [])

  const isDraft = rule.id.startsWith('draft-')
  const isDirty = isDraft || JSON.stringify(draft) !== JSON.stringify(rule)

  const handleSave = async () => {
    if (!isDirty || saving) return
    setSaving(true)
    await onUpdate(draft)
    setSaving(false)
  }

  const handleDeleteClick = () => {
    if (rule.isTemplate) return
    if (confirmDelete) {
      setConfirmDelete(false)
      onDelete(rule.id)
    } else {
      setConfirmDelete(true)
      setTimeout(() => setConfirmDelete(false), 2000)
    }
  }

  const toggleScope = (s: string) => {
    setDraft((prev) => {
      const has = prev.scope.includes(s)
      return {
        ...prev,
        scope: has ? prev.scope.filter((x) => x !== s) : [...prev.scope, s],
      }
    })
  }

  return (
    <div className="flex-1 flex flex-col min-w-0 min-h-0">
      {/* Action bar */}
      <div className="flex items-center justify-between px-4 py-2 border-b border-border/40">
        <span className="text-xs font-medium text-foreground truncate">
          {draft.name || t('rules.untitled')}
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
            {saving ? t('rules.saving') : t('rules.save')}
          </button>
          <button
            onClick={handleDeleteClick}
            disabled={rule.isTemplate}
            className={cn(
              'flex items-center gap-1.5 px-3 py-1 rounded-md text-xs transition-colors disabled:opacity-30 disabled:cursor-not-allowed',
              confirmDelete
                ? 'bg-red-500/15 text-red-400 hover:bg-red-500/25'
                : 'text-foreground-muted/60 hover:text-foreground hover:bg-background-tertiary',
            )}
            title={rule.isTemplate ? t('rules.cannotDeleteTemplate') : confirmDelete ? t('rules.clickToConfirm') : t('rules.deleteRuleTitle')}
          >
            <Trash2 className="w-3 h-3" />
            {confirmDelete ? t('rules.confirm') : t('rules.deleteBtn')}
          </button>
        </div>
      </div>

      {/* Form */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {/* Name */}
        <div className="space-y-1.5">
          <FieldLabel>{t('rules.name')}</FieldLabel>
          <Input
            value={draft.name}
            onChange={(e) => patch('name', e.target.value)}
            className="text-xs"
          />
        </div>

        {/* Description */}
        <div className="space-y-1.5">
          <FieldLabel>{t('rules.description')}</FieldLabel>
          <Textarea
            value={draft.description}
            onChange={(e) => patch('description', e.target.value)}
            className="min-h-[80px] text-xs resize-none"
          />
        </div>

        {/* Severity */}
        <div className="space-y-1.5">
          <FieldLabel>{t('rules.severity')}</FieldLabel>
          <Select value={draft.severity} onValueChange={(v) => patch('severity', v as RuleSeverity)}>
            <SelectTrigger className="text-xs h-9">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="critical">{t('rules.critical')}</SelectItem>
              <SelectItem value="warning">{t('rules.warning')}</SelectItem>
              <SelectItem value="info">{t('rules.info')}</SelectItem>
            </SelectContent>
          </Select>
        </div>

        {/* Scope checkboxes */}
        <div className="space-y-1.5">
          <FieldLabel>{t('rules.scope')}</FieldLabel>
          <div className="flex gap-4">
            {SCOPE_OPTIONS.map((s) => (
              <div
                key={s}
                className="flex items-center gap-1.5 cursor-pointer"
                onClick={() => toggleScope(s)}
              >
                <Checkbox
                  checked={draft.scope.includes(s)}
                  onCheckedChange={() => toggleScope(s)}
                />
                <span className="text-xs text-foreground-muted capitalize">{t(`rules.scope${s.charAt(0).toUpperCase() + s.slice(1)}`)}</span>
              </div>
            ))}
          </div>
        </div>

        {/* Active toggle */}
        <div className="space-y-1.5">
          <FieldLabel>{t('rules.active')}</FieldLabel>
          <button
            type="button"
            onClick={() => patch('isActive', !draft.isActive)}
            className="flex items-center gap-2 h-9 px-3 w-full rounded-lg border border-border bg-background-secondary text-xs hover:bg-background-tertiary transition-colors text-left"
          >
            <div className={cn(
              'w-2 h-2 rounded-full shrink-0 transition-colors',
              draft.isActive ? 'bg-green-400' : 'bg-foreground-muted/30',
            )} />
            <span className="text-foreground-muted">
              {draft.isActive ? t('rules.enabled') : t('rules.disabled')}
            </span>
          </button>
        </div>
      </div>
    </div>
  )
}

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

function mapDtoToRule(d: Awaited<ReturnType<typeof tauriApi.listAgentRules>>[number]): AgentRule {
  return {
    id: d.id,
    name: d.name,
    description: d.description,
    scope: d.scope,
    severity: d.severity as RuleSeverity,
    isActive: d.isActive,
    isTemplate: d.isTemplate,
  }
}

// -----------------------------------------------------------------------------
// RulesTab
// -----------------------------------------------------------------------------

export function RulesTab() {
  const { t } = useTranslation('agents')
  const [rules, setRules] = useState<AgentRule[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [newIds, setNewIds] = useState<Set<string>>(new Set())

  useEffect(() => {
    tauriApi.listAgentRules()
      .then((data) => {
        const mapped = data.map(mapDtoToRule)
        setRules(mapped)
        if (mapped.length > 0) setSelectedId(mapped[0].id)
      })
      .catch((err) => setError(err.message ?? 'Failed to load rules'))
      .finally(() => setLoading(false))
  }, [])

  const handleCreate = () => {
    // Don't allow creating another draft while one exists unsaved
    if (newIds.size > 0) {
      const existingDraft = [...newIds][0]
      setSelectedId(existingDraft)
      return
    }

    const tempId = `draft-${crypto.randomUUID()}`
    const draft: AgentRule = {
      id: tempId,
      name: '',
      description: '',
      scope: ['file'],
      severity: 'warning',
      isActive: true,
      isTemplate: false,
    }
    setRules((prev) => [...prev, draft])
    setNewIds((prev) => new Set(prev).add(tempId))
    setSelectedId(tempId)
  }

  const handleUpdate = useCallback(async (draft: AgentRule) => {
    const isDraft = draft.id.startsWith('draft-')
    try {
      if (isDraft) {
        const dto = await tauriApi.createAgentRule({
          name: draft.name,
          description: draft.description,
          scope: draft.scope,
          severity: draft.severity,
          isActive: draft.isActive,
        })
        const created = mapDtoToRule(dto)
        setRules((prev) => prev.map((r) => r.id === draft.id ? created : r))
        setNewIds((prev) => {
          const next = new Set(prev)
          next.delete(draft.id)
          return next
        })
        setSelectedId(created.id)
      } else {
        const dto = await tauriApi.updateAgentRule({
          id: draft.id,
          name: draft.name,
          description: draft.description,
          scope: draft.scope,
          severity: draft.severity,
          isActive: draft.isActive,
        })
        const updated = mapDtoToRule(dto)
        setRules((prev) => prev.map((r) => r.id === updated.id ? updated : r))
      }
    } catch {
      // Silently fail
    }
  }, [])

  const handleDelete = useCallback(async (id: string) => {
    try {
      if (!id.startsWith('draft-')) {
        await tauriApi.deleteAgentRule(id)
      }
      setRules((prev) => {
        const next = prev.filter((r) => r.id !== id)
        if (selectedId === id) {
          const idx = prev.findIndex((r) => r.id === id)
          const nextRule = next[Math.min(idx, next.length - 1)]
          setSelectedId(nextRule?.id ?? null)
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

  const selectedRule = rules.find((r) => r.id === selectedId) ?? null

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center text-foreground-muted/50">
        <Loader2 className="w-5 h-5 animate-spin mr-2" />
        <span className="text-xs">{t('rules.loading')}</span>
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
    <div className="flex-1 flex">
      {/* Left — Rule list */}
      <div className="w-[250px] shrink-0 border-r border-border overflow-hidden flex flex-col">
        <div className="px-3 py-2 border-b border-border/40 flex items-center justify-between">
          <div>
            <span className="text-[11px] font-medium uppercase tracking-wider text-foreground-muted">
              {t('innerTabs.rules')}
            </span>
            <span className="text-[10px] text-foreground-muted/50 ml-1.5">
              ({rules.length})
            </span>
          </div>
          <button
            onClick={handleCreate}
            className="p-1 rounded hover:bg-background-tertiary text-foreground-muted/60 hover:text-foreground transition-colors"
            title={t('rules.createRuleTitle')}
          >
            <Plus className="w-3.5 h-3.5" />
          </button>
        </div>
        <div className="flex-1 overflow-y-auto">
          {rules.map((rule) => (
            <RuleListItem
              key={rule.id}
              rule={rule}
              isSelected={rule.id === selectedId}
              onSelect={() => setSelectedId(rule.id)}
            />
          ))}
        </div>
      </div>

      {/* Right — Rule detail or empty state */}
      <div className="flex-1 min-w-0 flex flex-col">
        {selectedRule ? (
          <RuleDetail
            rule={selectedRule}
            onUpdate={handleUpdate}
            onDelete={handleDelete}
          />
        ) : (
          <div className="flex-1 flex flex-col items-center justify-center text-foreground-muted/40">
            <ShieldCheck className="w-10 h-10 mb-3 opacity-20" />
            <span className="text-xs mb-3">{t('rules.noSelected')}</span>
            <button
              onClick={handleCreate}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs bg-brand/15 text-brand hover:bg-brand/25 transition-colors"
            >
              <Plus className="w-3.5 h-3.5" />
              {t('rules.create')}
            </button>
          </div>
        )}
      </div>
    </div>
  )
}
