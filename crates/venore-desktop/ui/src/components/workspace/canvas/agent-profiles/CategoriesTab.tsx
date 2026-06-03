// =============================================================================
// CategoriesTab — Tool category library (CRUD with draft pattern)
// =============================================================================

import { useState, useEffect, useCallback } from 'react'
import { FolderTree, Loader2, AlertCircle, Plus, Trash2, Save } from 'lucide-react'
import { cn } from '@/lib/utils'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { Label } from '@/components/ui/label'
import { tauriApi } from '@/lib/tauri'
import type { ToolCategory } from './types'

// -----------------------------------------------------------------------------
// CategoryListItem
// -----------------------------------------------------------------------------

function CategoryListItem({
  category, isSelected, onSelect, toolCount,
}: {
  category: ToolCategory
  isSelected: boolean
  onSelect: () => void
  toolCount: number
}) {
  const incomplete = !category.name.trim()
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
        <div
          className="w-2 h-2 rounded-full shrink-0"
          style={{ backgroundColor: category.color }}
        />
        <span className={cn('text-xs font-medium truncate flex-1',
          incomplete ? 'text-foreground-muted italic' : 'text-foreground')}>
          {category.name || 'Untitled'}
        </span>
      </div>
      <div className="flex items-center gap-2">
        <span className="text-[10px] text-foreground-muted/60">{category.icon}</span>
        <span className="text-[10px] text-foreground-muted/40">
          {toolCount} {toolCount === 1 ? 'tool' : 'tools'}
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
// CategoryDetail
// -----------------------------------------------------------------------------

function CategoryDetail({
  category, isNew, onUpdate, onDelete,
}: {
  category: ToolCategory
  isNew: boolean
  onUpdate: (draft: ToolCategory) => void
  onDelete: (id: string) => void
}) {
  const [draft, setDraft] = useState<ToolCategory>(category)
  const [saving, setSaving] = useState(false)
  const [confirmDelete, setConfirmDelete] = useState(false)

  useEffect(() => {
    setDraft(category)
    setSaving(false)
    setConfirmDelete(false)
  }, [category.id]) // eslint-disable-line react-hooks/exhaustive-deps

  const patch = useCallback(<K extends keyof ToolCategory>(key: K, value: ToolCategory[K]) => {
    setDraft((prev) => ({ ...prev, [key]: value }))
  }, [])

  const isDraft = category.id.startsWith('draft-')
  const isDirty = isDraft || JSON.stringify(draft) !== JSON.stringify(category)

  const handleSave = async () => {
    if (!isDirty || saving) return
    setSaving(true)
    await onUpdate(draft)
    setSaving(false)
  }

  const handleDeleteClick = () => {
    if (category.isTemplate) return
    if (confirmDelete) {
      setConfirmDelete(false)
      onDelete(category.id)
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
          <div className="w-2 h-2 rounded-full shrink-0" style={{ backgroundColor: draft.color }} />
          <span className="text-xs font-medium text-foreground truncate">
            {draft.name || 'Untitled'}
          </span>
          {category.isTemplate && (
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
            disabled={category.isTemplate}
            className={cn(
              'flex items-center gap-1.5 px-3 py-1 rounded-md text-xs transition-colors disabled:opacity-30 disabled:cursor-not-allowed',
              confirmDelete
                ? 'bg-red-500/15 text-red-400 hover:bg-red-500/25'
                : 'text-foreground-muted/60 hover:text-foreground hover:bg-background-tertiary',
            )}
            title={category.isTemplate ? 'Cannot delete template' : confirmDelete ? 'Click to confirm' : 'Delete category'}
          >
            <Trash2 className="w-3 h-3" />
            {confirmDelete ? 'Confirm?' : 'Delete'}
          </button>
        </div>
      </div>

      {/* Form */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        <div className="space-y-1.5">
          <FieldLabel>Name</FieldLabel>
          <Input value={draft.name} onChange={(e) => patch('name', e.target.value)} className="text-xs" />
        </div>

        <div className="space-y-1.5">
          <FieldLabel>Description</FieldLabel>
          <Textarea
            value={draft.description}
            onChange={(e) => patch('description', e.target.value)}
            className="min-h-[80px] text-xs resize-none"
          />
        </div>

        <div className="grid grid-cols-2 gap-4">
          <div className="space-y-1.5">
            <FieldLabel>Icon</FieldLabel>
            <Input
              value={draft.icon}
              onChange={(e) => patch('icon', e.target.value)}
              className="text-xs"
              placeholder="lucide icon name"
            />
          </div>
          <div className="space-y-1.5">
            <FieldLabel>Color</FieldLabel>
            <div className="flex items-center gap-2">
              <input
                type="color"
                value={draft.color}
                onChange={(e) => patch('color', e.target.value)}
                className="w-9 h-9 rounded border border-border cursor-pointer bg-transparent"
              />
              <Input
                value={draft.color}
                onChange={(e) => patch('color', e.target.value)}
                className="text-xs flex-1 font-mono"
                placeholder="#hex"
              />
            </div>
          </div>
        </div>

        <div className="space-y-1.5">
          <FieldLabel>Display Order</FieldLabel>
          <Input
            type="number"
            value={draft.displayOrder}
            onChange={(e) => patch('displayOrder', parseInt(e.target.value) || 0)}
            className="text-xs w-24"
          />
        </div>
      </div>
    </div>
  )
}

// -----------------------------------------------------------------------------
// CategoriesTab
// -----------------------------------------------------------------------------

export function CategoriesTab() {
  const [categories, setCategories] = useState<ToolCategory[]>([])
  const [toolCounts, setToolCounts] = useState<Record<string, number>>({})
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [newIds, setNewIds] = useState<Set<string>>(new Set())

  useEffect(() => {
    Promise.all([tauriApi.listToolCategories(), tauriApi.listToolDefinitions()])
      .then(([cats, tools]) => {
        const mapped: ToolCategory[] = cats.map((c) => ({
          id: c.id, name: c.name, description: c.description,
          icon: c.icon, color: c.color, displayOrder: c.displayOrder,
          isTemplate: c.isTemplate,
        }))
        setCategories(mapped)
        if (mapped.length > 0) setSelectedId(mapped[0].id)

        const counts: Record<string, number> = {}
        for (const t of tools) {
          counts[t.categoryId] = (counts[t.categoryId] || 0) + 1
        }
        setToolCounts(counts)
      })
      .catch((err) => setError(err.message ?? 'Failed to load categories'))
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
    const draft: ToolCategory = {
      id: tempId,
      name: '',
      description: '',
      icon: 'circle',
      color: '#6b7280',
      displayOrder: categories.length,
      isTemplate: false,
    }
    setCategories((prev) => [...prev, draft])
    setNewIds((prev) => new Set(prev).add(tempId))
    setSelectedId(tempId)
  }

  const handleUpdate = useCallback(async (draft: ToolCategory) => {
    const isDraft = draft.id.startsWith('draft-')
    try {
      if (isDraft) {
        const dto = await tauriApi.createToolCategory({
          name: draft.name,
          description: draft.description,
          icon: draft.icon,
          color: draft.color,
          displayOrder: draft.displayOrder,
        })
        const created: ToolCategory = {
          id: dto.id, name: dto.name, description: dto.description,
          icon: dto.icon, color: dto.color, displayOrder: dto.displayOrder,
          isTemplate: dto.isTemplate,
        }
        setCategories((prev) => prev.map((c) => c.id === draft.id ? created : c))
        setNewIds((prev) => {
          const next = new Set(prev)
          next.delete(draft.id)
          return next
        })
        setSelectedId(created.id)
      } else {
        const dto = await tauriApi.updateToolCategory({
          id: draft.id,
          name: draft.name,
          description: draft.description,
          icon: draft.icon,
          color: draft.color,
          displayOrder: draft.displayOrder,
        })
        const updated: ToolCategory = {
          id: dto.id, name: dto.name, description: dto.description,
          icon: dto.icon, color: dto.color, displayOrder: dto.displayOrder,
          isTemplate: dto.isTemplate,
        }
        setCategories((prev) => prev.map((c) => c.id === updated.id ? updated : c))
      }
    } catch {
      // Silently fail
    }
  }, [])

  const handleDelete = useCallback(async (id: string) => {
    try {
      if (!id.startsWith('draft-')) {
        await tauriApi.deleteToolCategory(id)
      }
      setCategories((prev) => {
        const next = prev.filter((c) => c.id !== id)
        if (selectedId === id) {
          const idx = prev.findIndex((c) => c.id === id)
          const nextCat = next[Math.min(idx, next.length - 1)]
          setSelectedId(nextCat?.id ?? null)
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

  const selectedCategory = categories.find((c) => c.id === selectedId) ?? null

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center text-foreground-muted/50">
        <Loader2 className="w-5 h-5 animate-spin mr-2" />
        <span className="text-xs">Loading categories...</span>
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
      {/* Left — Category list */}
      <div className="w-[250px] shrink-0 border-r border-border overflow-hidden flex flex-col">
        <div className="px-3 py-2 border-b border-border/40 flex items-center justify-between">
          <div>
            <span className="text-[11px] font-medium uppercase tracking-wider text-foreground-muted">
              Categories
            </span>
            <span className="text-[10px] text-foreground-muted/50 ml-1.5">
              ({categories.length})
            </span>
          </div>
          <button
            onClick={handleCreate}
            className="p-1 rounded hover:bg-background-tertiary text-foreground-muted/60 hover:text-foreground transition-colors"
            title="Create category"
          >
            <Plus className="w-3.5 h-3.5" />
          </button>
        </div>
        <div className="flex-1 overflow-y-auto">
          {categories.map((cat) => (
            <CategoryListItem
              key={cat.id}
              category={cat}
              toolCount={toolCounts[cat.id] || 0}
              isSelected={cat.id === selectedId}
              onSelect={() => setSelectedId(cat.id)}
            />
          ))}
        </div>
      </div>

      {/* Right — Category detail or empty state */}
      <div className="flex-1 min-w-0 flex flex-col">
        {selectedCategory ? (
          <CategoryDetail
            category={selectedCategory}
            isNew={newIds.has(selectedCategory.id)}
            onUpdate={handleUpdate}
            onDelete={handleDelete}
          />
        ) : (
          <div className="flex-1 flex flex-col items-center justify-center text-foreground-muted/40">
            <FolderTree className="w-10 h-10 mb-3 opacity-20" />
            <span className="text-xs mb-3">Select a category or create a new one</span>
            <button
              onClick={handleCreate}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs bg-brand/15 text-brand hover:bg-brand/25 transition-colors"
            >
              <Plus className="w-3.5 h-3.5" />
              Create Category
            </button>
          </div>
        )}
      </div>
    </div>
  )
}
