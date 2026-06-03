// =============================================================================
// ToolsTab — Tool definitions library (CRUD with draft pattern), grouped by category
// =============================================================================

import { useState, useEffect, useCallback, useMemo } from 'react'
import { Wrench, Loader2, AlertCircle, Plus, Trash2, Save } from 'lucide-react'
import { cn } from '@/lib/utils'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { Label } from '@/components/ui/label'
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select'
import { tauriApi } from '@/lib/tauri'
import type { ToolDefinition, ToolCategory } from './types'

// -----------------------------------------------------------------------------
// ToolListItem
// -----------------------------------------------------------------------------

function ToolListItem({
  tool, isSelected, onSelect, categoryColor,
}: {
  tool: ToolDefinition
  isSelected: boolean
  onSelect: () => void
  categoryColor: string
}) {
  const incomplete = !tool.name.trim()
  return (
    <button
      onClick={onSelect}
      className={cn(
        'w-full text-left px-3 py-2 border-b border-border/30 transition-colors',
        isSelected
          ? 'bg-background-tertiary border-l-2 border-l-brand'
          : 'hover:bg-background-tertiary/50 border-l-2 border-l-transparent',
        incomplete && 'opacity-50',
      )}
    >
      <div className="flex items-center gap-2">
        <div
          className="w-1.5 h-1.5 rounded-full shrink-0"
          style={{ backgroundColor: categoryColor }}
        />
        <span className={cn('text-xs font-medium truncate flex-1',
          incomplete ? 'text-foreground-muted italic' : 'text-foreground')}>
          {tool.name || 'Untitled'}
        </span>
        <div className={cn(
          'w-1.5 h-1.5 rounded-full shrink-0',
          tool.isEnabled ? 'bg-green-400' : 'bg-foreground-muted/30',
        )} />
      </div>
      <div className="flex items-center gap-2 mt-0.5 ml-3.5">
        {tool.isReadOnly && (
          <span className="text-[9px] px-1 py-0.5 rounded bg-blue-500/15 text-blue-400">
            read-only
          </span>
        )}
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
// ToolDetail
// -----------------------------------------------------------------------------

function ToolDetail({
  tool, categories, isNew, onUpdate, onDelete,
}: {
  tool: ToolDefinition
  categories: ToolCategory[]
  isNew: boolean
  onUpdate: (draft: ToolDefinition) => void
  onDelete: (id: string) => void
}) {
  const [draft, setDraft] = useState<ToolDefinition>(tool)
  const [saving, setSaving] = useState(false)
  const [confirmDelete, setConfirmDelete] = useState(false)

  useEffect(() => {
    setDraft(tool)
    setSaving(false)
    setConfirmDelete(false)
  }, [tool.id]) // eslint-disable-line react-hooks/exhaustive-deps

  const patch = useCallback(<K extends keyof ToolDefinition>(key: K, value: ToolDefinition[K]) => {
    setDraft((prev) => ({ ...prev, [key]: value }))
  }, [])

  const isDraft = tool.id.startsWith('draft-')
  const isDirty = isDraft || JSON.stringify(draft) !== JSON.stringify(tool)
  const category = categories.find((c) => c.id === draft.categoryId)

  const handleSave = async () => {
    if (!isDirty || saving) return
    setSaving(true)
    await onUpdate(draft)
    setSaving(false)
  }

  const handleDeleteClick = () => {
    if (tool.isTemplate) return
    if (confirmDelete) {
      setConfirmDelete(false)
      onDelete(tool.id)
    } else {
      setConfirmDelete(true)
      setTimeout(() => setConfirmDelete(false), 2000)
    }
  }

  return (
    <div className="flex-1 flex flex-col min-w-0 min-h-0">
      {/* Action bar */}
      <div className="flex items-center justify-between px-4 py-2 border-b border-border/40">
        <div className="flex items-center gap-2 truncate">
          {category && (
            <div className="w-2 h-2 rounded-full shrink-0" style={{ backgroundColor: category.color }} />
          )}
          <span className="text-xs font-medium text-foreground truncate">
            {draft.name || 'Untitled'}
          </span>
          {tool.isTemplate && (
            <span className="text-[9px] px-1.5 py-0.5 rounded-full bg-foreground-muted/10 text-foreground-muted/60">
              template
            </span>
          )}
        </div>
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
            {saving ? <Loader2 className="w-3 h-3 animate-spin" /> : <Save className="w-3 h-3" />}
            {saving ? 'Saving...' : 'Save'}
          </button>
          <button
            onClick={handleDeleteClick}
            disabled={tool.isTemplate}
            className={cn(
              'flex items-center gap-1.5 px-3 py-1 rounded-md text-xs transition-colors disabled:opacity-30 disabled:cursor-not-allowed',
              confirmDelete
                ? 'bg-red-500/15 text-red-400 hover:bg-red-500/25'
                : 'text-foreground-muted/60 hover:text-foreground hover:bg-background-tertiary',
            )}
            title={tool.isTemplate ? 'Cannot delete template' : confirmDelete ? 'Click to confirm' : 'Delete tool'}
          >
            <Trash2 className="w-3 h-3" />
            {confirmDelete ? 'Confirm?' : 'Delete'}
          </button>
        </div>
      </div>

      {/* Form */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {/* Name */}
        <div className="space-y-1.5">
          <FieldLabel>Name</FieldLabel>
          <Input
            value={draft.name}
            onChange={(e) => patch('name', e.target.value)}
            className="text-xs font-mono"
            readOnly={tool.isTemplate}
          />
        </div>

        {/* Description */}
        <div className="space-y-1.5">
          <FieldLabel>Description</FieldLabel>
          <Textarea
            value={draft.description}
            onChange={(e) => patch('description', e.target.value)}
            className="min-h-[120px] text-xs font-mono resize-none"
          />
        </div>

        {/* Category */}
        <div className="space-y-1.5">
          <FieldLabel>Category</FieldLabel>
          <Select value={draft.categoryId} onValueChange={(v) => patch('categoryId', v)}>
            <SelectTrigger className="text-xs h-9">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {categories.map((c) => (
                <SelectItem key={c.id} value={c.id}>
                  <div className="flex items-center gap-2">
                    <div className="w-2 h-2 rounded-full" style={{ backgroundColor: c.color }} />
                    {c.name}
                  </div>
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        {/* Parameters JSON */}
        <div className="space-y-1.5">
          <FieldLabel>Parameters (JSON Schema)</FieldLabel>
          <Textarea
            value={draft.parametersJson}
            onChange={(e) => patch('parametersJson', e.target.value)}
            className="min-h-[100px] text-xs font-mono resize-none"
            readOnly={tool.isTemplate}
          />
        </div>

        {/* Toggles */}
        <div className="grid grid-cols-2 gap-4">
          <div className="space-y-1.5">
            <FieldLabel>Read-only</FieldLabel>
            <button
              type="button"
              onClick={() => patch('isReadOnly', !draft.isReadOnly)}
              className="flex items-center gap-2 h-9 px-3 w-full rounded-lg border border-border bg-background-secondary text-xs hover:bg-background-tertiary transition-colors text-left"
            >
              <div className={cn(
                'w-2 h-2 rounded-full shrink-0 transition-colors',
                draft.isReadOnly ? 'bg-blue-400' : 'bg-foreground-muted/30',
              )} />
              <span className="text-foreground-muted">
                {draft.isReadOnly ? 'Yes — safe for plan mode' : 'No — can modify'}
              </span>
            </button>
          </div>

          <div className="space-y-1.5">
            <FieldLabel>Enabled</FieldLabel>
            <button
              type="button"
              onClick={() => patch('isEnabled', !draft.isEnabled)}
              className="flex items-center gap-2 h-9 px-3 w-full rounded-lg border border-border bg-background-secondary text-xs hover:bg-background-tertiary transition-colors text-left"
            >
              <div className={cn(
                'w-2 h-2 rounded-full shrink-0 transition-colors',
                draft.isEnabled ? 'bg-green-400' : 'bg-foreground-muted/30',
              )} />
              <span className="text-foreground-muted">
                {draft.isEnabled ? 'Enabled' : 'Disabled'}
              </span>
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}

// -----------------------------------------------------------------------------
// ToolsTab
// -----------------------------------------------------------------------------

export function ToolsTab() {
  const [tools, setTools] = useState<ToolDefinition[]>([])
  const [categories, setCategories] = useState<ToolCategory[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [newIds, setNewIds] = useState<Set<string>>(new Set())

  useEffect(() => {
    Promise.all([tauriApi.listToolDefinitions(), tauriApi.listToolCategories()])
      .then(([toolsData, catsData]) => {
        const mappedTools: ToolDefinition[] = toolsData.map((t) => ({
          id: t.id, name: t.name, description: t.description,
          categoryId: t.categoryId, parametersJson: t.parametersJson,
          isReadOnly: t.isReadOnly, isEnabled: t.isEnabled, isTemplate: t.isTemplate,
        }))
        const mappedCats: ToolCategory[] = catsData.map((c) => ({
          id: c.id, name: c.name, description: c.description,
          icon: c.icon, color: c.color, displayOrder: c.displayOrder,
          isTemplate: c.isTemplate,
        }))
        setTools(mappedTools)
        setCategories(mappedCats)
        if (mappedTools.length > 0) setSelectedId(mappedTools[0].id)
      })
      .catch((err) => setError(err.message ?? 'Failed to load tools'))
      .finally(() => setLoading(false))
  }, [])

  // Group tools by category
  const groupedTools = useMemo(() => {
    const groups: { category: ToolCategory; tools: ToolDefinition[] }[] = []
    const catMap = new Map(categories.map((c) => [c.id, c]))

    for (const cat of categories) {
      const catTools = tools.filter((t) => t.categoryId === cat.id)
      if (catTools.length > 0) {
        groups.push({ category: cat, tools: catTools })
      }
    }

    // Uncategorized tools (including drafts with no valid category)
    const uncategorized = tools.filter((t) => !catMap.has(t.categoryId))
    if (uncategorized.length > 0) {
      groups.push({
        category: { id: '_uncategorized', name: 'Uncategorized', description: '', icon: '', color: '#6b7280', displayOrder: 999, isTemplate: false },
        tools: uncategorized,
      })
    }

    return groups
  }, [tools, categories])

  const categoryColorMap = useMemo(() => {
    const map: Record<string, string> = {}
    for (const c of categories) map[c.id] = c.color
    return map
  }, [categories])

  const handleCreate = () => {
    // Don't allow creating another draft while one exists unsaved
    if (newIds.size > 0) {
      const existingDraft = [...newIds][0]
      setSelectedId(existingDraft)
      return
    }

    const tempId = `draft-${crypto.randomUUID()}`
    const defaultCat = categories[0]?.id || ''
    const draft: ToolDefinition = {
      id: tempId,
      name: '',
      description: '',
      categoryId: defaultCat,
      parametersJson: '{}',
      isReadOnly: false,
      isEnabled: true,
      isTemplate: false,
    }
    setTools((prev) => [...prev, draft])
    setNewIds((prev) => new Set(prev).add(tempId))
    setSelectedId(tempId)
  }

  const handleUpdate = useCallback(async (draft: ToolDefinition) => {
    const isDraft = draft.id.startsWith('draft-')
    try {
      if (isDraft) {
        const dto = await tauriApi.createToolDefinition({
          name: draft.name,
          description: draft.description,
          categoryId: draft.categoryId,
          parametersJson: draft.parametersJson,
          isReadOnly: draft.isReadOnly,
          isEnabled: draft.isEnabled,
        })
        const created: ToolDefinition = {
          id: dto.id, name: dto.name, description: dto.description,
          categoryId: dto.categoryId, parametersJson: dto.parametersJson,
          isReadOnly: dto.isReadOnly, isEnabled: dto.isEnabled, isTemplate: dto.isTemplate,
        }
        setTools((prev) => prev.map((t) => t.id === draft.id ? created : t))
        setNewIds((prev) => {
          const next = new Set(prev)
          next.delete(draft.id)
          return next
        })
        setSelectedId(created.id)
      } else {
        const dto = await tauriApi.updateToolDefinition({
          id: draft.id,
          name: draft.name,
          description: draft.description,
          categoryId: draft.categoryId,
          parametersJson: draft.parametersJson,
          isReadOnly: draft.isReadOnly,
          isEnabled: draft.isEnabled,
        })
        const updated: ToolDefinition = {
          id: dto.id, name: dto.name, description: dto.description,
          categoryId: dto.categoryId, parametersJson: dto.parametersJson,
          isReadOnly: dto.isReadOnly, isEnabled: dto.isEnabled, isTemplate: dto.isTemplate,
        }
        setTools((prev) => prev.map((t) => t.id === updated.id ? updated : t))
      }
    } catch {
      // Silently fail
    }
  }, [])

  const handleDelete = useCallback(async (id: string) => {
    try {
      if (!id.startsWith('draft-')) {
        await tauriApi.deleteToolDefinition(id)
      }
      setTools((prev) => {
        const next = prev.filter((t) => t.id !== id)
        if (selectedId === id) {
          const idx = prev.findIndex((t) => t.id === id)
          const nextTool = next[Math.min(idx, next.length - 1)]
          setSelectedId(nextTool?.id ?? null)
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

  const selectedTool = tools.find((t) => t.id === selectedId) ?? null

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center text-foreground-muted/50">
        <Loader2 className="w-5 h-5 animate-spin mr-2" />
        <span className="text-xs">Loading tools...</span>
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
      {/* Left — Tool list grouped by category */}
      <div className="w-[250px] shrink-0 border-r border-border overflow-hidden flex flex-col">
        <div className="px-3 py-2 border-b border-border/40 flex items-center justify-between">
          <div>
            <span className="text-[11px] font-medium uppercase tracking-wider text-foreground-muted">
              Tools
            </span>
            <span className="text-[10px] text-foreground-muted/50 ml-1.5">
              ({tools.length})
            </span>
          </div>
          <button
            onClick={handleCreate}
            className="p-1 rounded hover:bg-background-tertiary text-foreground-muted/60 hover:text-foreground transition-colors"
            title="Create tool"
          >
            <Plus className="w-3.5 h-3.5" />
          </button>
        </div>
        <div className="flex-1 overflow-y-auto">
          {groupedTools.map(({ category, tools: catTools }) => (
            <div key={category.id}>
              {/* Category header */}
              <div className="px-3 py-1.5 bg-background-secondary/50 border-b border-border/20 flex items-center gap-2">
                <div className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: category.color }} />
                <span className="text-[10px] font-medium uppercase tracking-wider text-foreground-muted/70">
                  {category.name}
                </span>
                <span className="text-[9px] text-foreground-muted/40">({catTools.length})</span>
              </div>
              {catTools.map((tool) => (
                <ToolListItem
                  key={tool.id}
                  tool={tool}
                  categoryColor={categoryColorMap[tool.categoryId] || '#6b7280'}
                  isSelected={tool.id === selectedId}
                  onSelect={() => setSelectedId(tool.id)}
                />
              ))}
            </div>
          ))}
        </div>
      </div>

      {/* Right — Tool detail or empty state */}
      <div className="flex-1 min-w-0 flex flex-col">
        {selectedTool ? (
          <ToolDetail
            tool={selectedTool}
            categories={categories}
            isNew={newIds.has(selectedTool.id)}
            onUpdate={handleUpdate}
            onDelete={handleDelete}
          />
        ) : (
          <div className="flex-1 flex flex-col items-center justify-center text-foreground-muted/40">
            <Wrench className="w-10 h-10 mb-3 opacity-20" />
            <span className="text-xs mb-3">Select a tool or create a new one</span>
            <button
              onClick={handleCreate}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs bg-brand/15 text-brand hover:bg-brand/25 transition-colors"
            >
              <Plus className="w-3.5 h-3.5" />
              Create Tool
            </button>
          </div>
        )}
      </div>
    </div>
  )
}
