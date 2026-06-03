// =============================================================================
// ModesTab — Chat modes (named bundles of tools/sub-agents/rules per kind)
// =============================================================================
// Each project kind ("code" | "knowledge") resolves to a default mode at
// chat init. The mode decides which tool categories the LLM sees plus which
// sub-agents are spawnable. Templates (mode-code, mode-knowledge) cannot be
// deleted but every field can be edited; users can also create custom modes.

import { useCallback, useEffect, useMemo, useState } from 'react'
import { Layers, Loader2, AlertCircle, Plus, Save, Trash2, Copy } from 'lucide-react'

import { cn } from '@/lib/utils'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { Label } from '@/components/ui/label'
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select'
import { tauriApi, type AgentProfileDto, type AgentRuleDto } from '@/lib/tauri'
import type { ChatMode, ToolCategory, ToolDefinition } from './types'

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

const KIND_LABEL: Record<string, string> = {
  code: 'Codebase',
  knowledge: 'Knowledge',
}

function ModeRow({
  mode,
  isSelected,
  onSelect,
}: {
  mode: ChatMode
  isSelected: boolean
  onSelect: () => void
}) {
  return (
    <button
      onClick={onSelect}
      className={cn(
        'w-full text-left px-3 py-2 border-b border-border/30 transition-colors',
        isSelected
          ? 'bg-background-tertiary border-l-2 border-l-brand'
          : 'hover:bg-background-tertiary/50 border-l-2 border-l-transparent',
      )}
    >
      <div className="flex items-center gap-2">
        <Layers className="w-3 h-3 text-foreground-muted shrink-0" />
        <span className="text-xs font-medium text-foreground truncate flex-1">{mode.name}</span>
        {mode.isDefaultForKind && (
          <span className="text-[9px] uppercase tracking-wider px-1.5 py-0.5 rounded border border-emerald-500/40 bg-emerald-500/10 text-emerald-300">
            {KIND_LABEL[mode.isDefaultForKind] ?? mode.isDefaultForKind} default
          </span>
        )}
        {mode.isTemplate && (
          <span className="text-[9px] uppercase tracking-wider px-1.5 py-0.5 rounded border border-foreground/20 text-foreground-muted">
            template
          </span>
        )}
      </div>
      {mode.description && (
        <p className="text-[10px] text-foreground-muted/70 mt-1 ml-5 truncate">
          {mode.description}
        </p>
      )}
    </button>
  )
}

// -----------------------------------------------------------------------------
// Multi-checkbox group
// -----------------------------------------------------------------------------

function CheckboxList<T extends { id: string }>({
  items,
  selected,
  onToggle,
  renderLabel,
}: {
  items: T[]
  selected: string[]
  onToggle: (id: string) => void
  renderLabel: (item: T) => React.ReactNode
}) {
  return (
    <div className="border border-border rounded-md max-h-56 overflow-y-auto">
      {items.length === 0 ? (
        <div className="px-3 py-2 text-xs text-foreground-muted/60">Sin items</div>
      ) : (
        items.map((item) => {
          const checked = selected.includes(item.id)
          return (
            <label
              key={item.id}
              className="flex items-center gap-2 px-3 py-1.5 text-xs cursor-pointer hover:bg-foreground/5 border-b border-border/30 last:border-b-0"
            >
              <input
                type="checkbox"
                checked={checked}
                onChange={() => onToggle(item.id)}
                className="w-3 h-3 accent-brand"
              />
              <span className="flex-1 min-w-0 truncate">{renderLabel(item)}</span>
            </label>
          )
        })
      )}
    </div>
  )
}

// -----------------------------------------------------------------------------
// Main
// -----------------------------------------------------------------------------

export function ModesTab() {
  const [modes, setModes] = useState<ChatMode[]>([])
  const [categories, setCategories] = useState<ToolCategory[]>([])
  const [tools, setTools] = useState<ToolDefinition[]>([])
  const [profiles, setProfiles] = useState<AgentProfileDto[]>([])
  const [rules, setRules] = useState<AgentRuleDto[]>([])
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [draft, setDraft] = useState<ChatMode | null>(null)
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [confirmDelete, setConfirmDelete] = useState(false)

  const loadAll = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const [m, c, t, p, r] = await Promise.all([
        tauriApi.listChatModes(),
        tauriApi.listToolCategories(),
        tauriApi.listToolDefinitions(),
        tauriApi.listAgentProfiles(),
        tauriApi.listAgentRules(),
      ])
      setModes(m)
      setCategories(c)
      setTools(t)
      setProfiles(p)
      setRules(r)
      // Auto-select the first mode only on the very first load (when nothing
      // is selected yet). Re-running this effect when selectedId changes
      // would clobber any unsaved draft just added via "New"/"Duplicar".
      setSelectedId((cur) => {
        if (cur) return cur
        const first = m[0]
        if (first) setDraft({ ...first })
        return first?.id ?? null
      })
    } catch (err) {
      console.error('Failed to load modes data:', err)
      setError(typeof err === 'string' ? err : 'No se pudo cargar')
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    loadAll()
  }, [loadAll])

  const handleSelect = (id: string) => {
    const mode = modes.find((m) => m.id === id)
    if (!mode) return
    setSelectedId(id)
    setDraft({ ...mode })
    setConfirmDelete(false)
  }

  const subAgents = useMemo(
    () => profiles.filter((p) => p.stage === 'subagent'),
    [profiles],
  )

  const sortedCategories = useMemo(
    () => [...categories].sort((a, b) => a.displayOrder - b.displayOrder),
    [categories],
  )

  const toolsByCategory = useMemo(() => {
    const map = new Map<string, ToolDefinition[]>()
    for (const t of tools) {
      const list = map.get(t.categoryId) ?? []
      list.push(t)
      map.set(t.categoryId, list)
    }
    return map
  }, [tools])

  const toggleId = (field: keyof Pick<ChatMode, 'categoryIds' | 'toolIds' | 'subAgentIds' | 'ruleIds'>, id: string) => {
    setDraft((d) => {
      if (!d) return d
      const arr = d[field] as string[]
      const next = arr.includes(id) ? arr.filter((x) => x !== id) : [...arr, id]
      return { ...d, [field]: next }
    })
  }

  const handleSave = async () => {
    if (!draft || saving) return
    setSaving(true)
    setError(null)
    try {
      // Existing modes (already in `modes`) → update. New drafts → create.
      const exists = modes.some((m) => m.id === draft.id)
      const saved = exists
        ? await tauriApi.updateChatMode({
            id: draft.id,
            name: draft.name,
            description: draft.description,
            categoryIds: draft.categoryIds,
            toolIds: draft.toolIds,
            subAgentIds: draft.subAgentIds,
            ruleIds: draft.ruleIds,
            promptId: draft.promptId ?? undefined,
            isDefaultForKind: draft.isDefaultForKind ?? '',
          })
        : await tauriApi.createChatMode({
            name: draft.name,
            description: draft.description,
            categoryIds: draft.categoryIds,
            toolIds: draft.toolIds,
            subAgentIds: draft.subAgentIds,
            ruleIds: draft.ruleIds,
            promptId: draft.promptId ?? undefined,
            isDefaultForKind: draft.isDefaultForKind ?? undefined,
          })
      // Refresh list and reselect by the saved id (server is authoritative)
      const refreshed = await tauriApi.listChatModes()
      setModes(refreshed)
      setSelectedId(saved.id)
      setDraft({ ...saved })
    } catch (err) {
      console.error('Save failed:', err)
      setError(typeof err === 'string' ? err : 'Save failed')
    } finally {
      setSaving(false)
    }
  }

  const handleDuplicate = () => {
    if (!draft) return
    const copy: ChatMode = {
      ...draft,
      id: `draft-${Date.now()}`,
      name: `${draft.name} (copia)`,
      isTemplate: false,
      isDefaultForKind: null,
    }
    setModes((arr) => [...arr, copy])
    setSelectedId(copy.id)
    setDraft(copy)
  }

  const handleDelete = async () => {
    if (!draft) return
    if (draft.isTemplate) return
    if (!confirmDelete) {
      setConfirmDelete(true)
      return
    }
    try {
      // If it was just a draft (never saved), drop it from local list.
      const exists = modes.some((m) => m.id === draft.id) && !draft.id.startsWith('draft-')
      if (exists) {
        await tauriApi.deleteChatMode(draft.id)
      }
      const remaining = modes.filter((m) => m.id !== draft.id)
      setModes(remaining)
      const next = remaining[0] ?? null
      setSelectedId(next?.id ?? null)
      setDraft(next ? { ...next } : null)
      setConfirmDelete(false)
    } catch (err) {
      console.error('Delete failed:', err)
      setError(typeof err === 'string' ? err : 'Delete failed')
    }
  }

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <Loader2 className="w-4 h-4 text-foreground-muted/50 animate-spin" />
      </div>
    )
  }

  return (
    <div className="flex-1 flex min-h-0">
      {/* Left list */}
      <aside className="w-72 shrink-0 border-r border-border flex flex-col">
        <header className="flex items-center justify-between px-3 py-2 border-b border-border">
          <span className="text-[10px] uppercase tracking-wider text-foreground-muted">
            Modes ({modes.length})
          </span>
          <button
            type="button"
            onClick={() => {
              const fresh: ChatMode = {
                id: `draft-${Date.now()}`,
                name: 'New mode',
                description: '',
                categoryIds: [],
                toolIds: [],
                subAgentIds: [],
                ruleIds: [],
                promptId: null,
                isTemplate: false,
                isDefaultForKind: null,
              }
              setModes((arr) => [...arr, fresh])
              setSelectedId(fresh.id)
              setDraft(fresh)
            }}
            className="text-xs flex items-center gap-1 px-2 py-1 rounded hover:bg-foreground/5 text-foreground-muted hover:text-foreground"
          >
            <Plus className="w-3 h-3" />
            New
          </button>
        </header>
        <div className="flex-1 overflow-y-auto">
          {modes.map((m) => (
            <ModeRow
              key={m.id}
              mode={m}
              isSelected={m.id === selectedId}
              onSelect={() => handleSelect(m.id)}
            />
          ))}
        </div>
      </aside>

      {/* Right editor */}
      <main className="flex-1 min-w-0 overflow-y-auto">
        {!draft ? (
          <div className="flex-1 flex items-center justify-center text-xs text-foreground-muted/60 h-full">
            Selecciona un modo
          </div>
        ) : (
          <div className="px-6 py-4 space-y-5 max-w-3xl">
            {error && (
              <div className="flex items-center gap-2 px-3 py-2 rounded border border-red-500/40 bg-red-500/10 text-red-300 text-xs">
                <AlertCircle className="w-3 h-3" />
                {error}
              </div>
            )}

            {/* Header: name + actions */}
            <header className="flex items-center justify-between gap-3">
              <Input
                value={draft.name}
                onChange={(e) => setDraft({ ...draft, name: e.target.value })}
                placeholder="Mode name"
                className="text-sm font-medium flex-1"
              />
              <div className="flex items-center gap-1">
                <button
                  type="button"
                  onClick={handleDuplicate}
                  title="Duplicar"
                  className="p-1.5 rounded hover:bg-foreground/5 text-foreground-muted hover:text-foreground"
                >
                  <Copy className="w-3.5 h-3.5" />
                </button>
                <button
                  type="button"
                  onClick={handleDelete}
                  disabled={draft.isTemplate}
                  title={draft.isTemplate ? 'Templates no se borran' : (confirmDelete ? 'Click again to confirm' : 'Borrar')}
                  className={cn(
                    'p-1.5 rounded hover:bg-red-500/10',
                    confirmDelete ? 'text-red-300 bg-red-500/15' : 'text-foreground-muted hover:text-red-400',
                    draft.isTemplate && 'opacity-30 cursor-not-allowed hover:bg-transparent hover:text-foreground-muted',
                  )}
                >
                  <Trash2 className="w-3.5 h-3.5" />
                </button>
                <button
                  type="button"
                  onClick={handleSave}
                  disabled={saving || !draft.name.trim()}
                  className="text-xs px-3 py-1 rounded bg-emerald-500/90 hover:bg-emerald-500 text-white disabled:opacity-50 flex items-center gap-1"
                >
                  <Save className="w-3 h-3" />
                  {saving ? 'Guardando...' : 'Guardar'}
                </button>
              </div>
            </header>

            <div>
              <Label className="text-[10px] uppercase tracking-wider text-foreground-muted">
                Description
              </Label>
              <Textarea
                value={draft.description}
                onChange={(e) => setDraft({ ...draft, description: e.target.value })}
                rows={2}
                placeholder="How this mode is used, what it intends to enable"
                className="mt-1 text-xs"
              />
            </div>

            <div>
              <Label className="text-[10px] uppercase tracking-wider text-foreground-muted">
                Default for kind
              </Label>
              <Select
                value={draft.isDefaultForKind ?? '__none__'}
                onValueChange={(v) =>
                  setDraft({ ...draft, isDefaultForKind: v === '__none__' ? null : v })
                }
              >
                <SelectTrigger className="text-xs h-8 mt-1">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="__none__">None</SelectItem>
                  <SelectItem value="code">Codebase</SelectItem>
                  <SelectItem value="knowledge">Knowledge</SelectItem>
                </SelectContent>
              </Select>
              <p className="text-[10px] text-foreground-muted/60 mt-1">
                Projects of that kind use this mode unless overridden manually.
              </p>
            </div>

            <div>
              <Label className="text-[10px] uppercase tracking-wider text-foreground-muted">
                Tool categories ({draft.categoryIds.length}/{sortedCategories.length})
              </Label>
              <p className="text-[10px] text-foreground-muted/60 mt-1 mb-1">
                Each selected category enables all its tools in this mode.
              </p>
              <CheckboxList
                items={sortedCategories}
                selected={draft.categoryIds}
                onToggle={(id) => toggleId('categoryIds', id)}
                renderLabel={(c) => (
                  <span className="flex items-center gap-2">
                    <span className="w-2 h-2 rounded-sm shrink-0" style={{ backgroundColor: c.color }} />
                    <span className="font-medium">{c.name}</span>
                    <span className="text-foreground-muted/50">
                      ({(toolsByCategory.get(c.id) ?? []).length})
                    </span>
                  </span>
                )}
              />
            </div>

            <div>
              <Label className="text-[10px] uppercase tracking-wider text-foreground-muted">
                Sub-agents ({draft.subAgentIds.length}/{subAgents.length})
              </Label>
              <p className="text-[10px] text-foreground-muted/60 mt-1 mb-1">
                Those the AI can spawn via spawn_agent in this mode.
              </p>
              <CheckboxList
                items={subAgents}
                selected={draft.subAgentIds}
                onToggle={(id) => toggleId('subAgentIds', id)}
                renderLabel={(p) => (
                  <span>
                    <span className="font-medium">{p.name}</span>
                    {p.description && (
                      <span className="text-foreground-muted/60"> — {p.description}</span>
                    )}
                  </span>
                )}
              />
            </div>

            <div>
              <Label className="text-[10px] uppercase tracking-wider text-foreground-muted">
                Rules ({draft.ruleIds.length}/{rules.length})
              </Label>
              <p className="text-[10px] text-amber-400/70 mt-1 mb-1">
                Selection saved — enforcement in the main chat will arrive in a later phase.
              </p>
              <CheckboxList
                items={rules}
                selected={draft.ruleIds}
                onToggle={(id) => toggleId('ruleIds', id)}
                renderLabel={(r) => (
                  <span>
                    <span className="font-medium">{r.name}</span>
                    <span className="text-foreground-muted/60"> · {r.severity}</span>
                  </span>
                )}
              />
            </div>
          </div>
        )}
      </main>
    </div>
  )
}
