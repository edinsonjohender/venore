// =============================================================================
// NodeLogbook — Per-node logbook
// =============================================================================
// "Captain's logbook" of a knowledge node: sidebar = dynamic list of sections
// (add/rename/delete), center = Monaco markdown editor for the active section
// with an edit / preview toggle that renders via MarkdownRenderer.
// Lives inside FloatingNodePanel (in-app, multi-instance) and NodeWindow
// (OS pop-out). Module nodes fall back to the read-only module panel.

import { useCallback, useEffect, useRef, useState } from 'react'
import Editor, { type OnMount } from '@monaco-editor/react'
import { listen } from '@tauri-apps/api/event'
import {
  AlertCircle,
  Eye,
  GripVertical,
  Pencil,
  Plus,
  Sparkles,
  Split,
  Trash2,
} from 'lucide-react'

import {
  tauriApi,
  type AiWriteProposedEvent,
  type KnowledgeNodeDataResponse,
  type KnowledgeNodeSubtype,
  type NodeSectionDto,
  type PendingWriteDto,
} from '@/lib/tauri'
import { PendingSectionPreview } from './PendingSectionPreview'
import type { NodePanelData } from '@/stores/nodeFloatingStore'
import { cn } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import { MarkdownRenderer } from '@/components/ui/markdown-renderer'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { NodePanelContent } from './NodePanelContent'

// -----------------------------------------------------------------------------
// Constants
// -----------------------------------------------------------------------------

const SUBTYPE_LABELS: Record<KnowledgeNodeSubtype, string> = {
  concept: 'Concept',
  feature: 'Feature',
  decision: 'Decision',
  finding: 'Finding',
  question: 'Question',
}

const SUBTYPE_STYLES: Record<KnowledgeNodeSubtype, string> = {
  concept: 'bg-sky-500/15 text-sky-300 border-sky-500/40',
  feature: 'bg-violet-500/15 text-violet-300 border-violet-500/40',
  decision: 'bg-emerald-500/15 text-emerald-300 border-emerald-500/40',
  finding: 'bg-amber-500/15 text-amber-300 border-amber-500/40',
  question: 'bg-pink-500/15 text-pink-300 border-pink-500/40',
}

/** Solid color dot used as a hint inside the subtype dropdown. */
const SUBTYPE_DOT: Record<KnowledgeNodeSubtype, string> = {
  concept: 'bg-sky-400',
  feature: 'bg-violet-400',
  decision: 'bg-emerald-400',
  finding: 'bg-amber-400',
  question: 'bg-pink-400',
}

const SUBTYPE_OPTIONS: KnowledgeNodeSubtype[] = [
  'concept',
  'feature',
  'decision',
  'finding',
  'question',
]

// Past this many sections a node feels overloaded — extracting some into
// their own nodes usually reads better.
const OVERSIZED_THRESHOLD = 10

// -----------------------------------------------------------------------------
// Root
// -----------------------------------------------------------------------------

export function NodeLogbook({ node }: { node: NodePanelData }) {
  if (node.nodeVariant !== 'knowledge_node' && node.nodeVariant !== 'lighthouse') {
    return (
      <div className="flex-1 overflow-hidden flex flex-col">
        <NodePanelContent node={node} />
      </div>
    )
  }
  return <KnowledgeLogbook node={node} />
}

// -----------------------------------------------------------------------------
// Knowledge logbook — sidebar + section editor
// -----------------------------------------------------------------------------

function KnowledgeLogbook({ node }: { node: NodePanelData }) {
  const [data, setData] = useState<KnowledgeNodeDataResponse | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [activeSectionId, setActiveSectionId] = useState<string | null>(null)
  // Pending AI-proposed writes for this node. The chat tool dispatch emits
  // `ai-write-proposed` after the executor stashes a proposal; we refetch
  // to surface it. Selecting a pending takes the main pane over the regular
  // SectionEditor.
  const [pendingWrites, setPendingWrites] = useState<PendingWriteDto[]>([])
  const [activePendingId, setActivePendingId] = useState<string | null>(null)

  const fetchData = useCallback(
    async (preferSectionId?: string | null) => {
      try {
        const fresh = await tauriApi.getKnowledgeNode({
          project_path: node.projectPath,
          node_id: node.moduleId,
        })
        setData(fresh)
        setActiveSectionId((current) => {
          if (preferSectionId !== undefined) {
            return preferSectionId ?? fresh.sections[0]?.id ?? null
          }
          if (current && fresh.sections.some((s) => s.id === current)) return current
          return fresh.sections[0]?.id ?? null
        })
        setError(null)
      } catch (err) {
        console.error('Failed to fetch knowledge node:', err)
        setError(typeof err === 'string' ? err : 'No se pudo cargar el nodo')
      } finally {
        setLoading(false)
      }
    },
    [node.projectPath, node.moduleId],
  )

  const fetchPendings = useCallback(
    async (preferPendingId?: string | null) => {
      try {
        const res = await tauriApi.listPendingWrites({
          project_path: node.projectPath,
          node_id: node.moduleId,
        })
        setPendingWrites(res.writes)
        setActivePendingId((current) => {
          if (preferPendingId !== undefined && preferPendingId !== null) {
            return res.writes.some((w) => w.write_id === preferPendingId)
              ? preferPendingId
              : null
          }
          if (current && res.writes.some((w) => w.write_id === current)) return current
          return null
        })
      } catch (err) {
        console.error('Failed to fetch pending writes:', err)
      }
    },
    [node.projectPath, node.moduleId],
  )

  useEffect(() => {
    setLoading(true)
    fetchData(undefined)
    fetchPendings(undefined)
  }, [fetchData, fetchPendings])

  // Sync across windows: when any other logbook (in-app or pop-out) saves a
  // change for this same node, refetch so we don't drift. The mutator's own
  // refetch is harmless — getKnowledgeNode is cheap and idempotent.
  useEffect(() => {
    let cancelled = false
    const unlisten = listen<{ project_path: string; node_id: string }>(
      'ocean-knowledge-changed',
      (event) => {
        if (cancelled) return
        const p = event.payload
        if (p.project_path !== node.projectPath || p.node_id !== node.moduleId) return
        fetchData()
        fetchPendings()
      },
    )
    return () => {
      cancelled = true
      unlisten.then((fn) => fn())
    }
  }, [node.projectPath, node.moduleId, fetchData, fetchPendings])

  // AI-proposed writes for this node — refetch + auto-select the new pending
  // so the user lands directly on the diff. Filtered by project + node.
  useEffect(() => {
    let cancelled = false
    const unlisten = listen<AiWriteProposedEvent>(
      'ai-write-proposed',
      (event) => {
        if (cancelled) return
        const p = event.payload
        if (p.project_path !== node.projectPath || p.node_id !== node.moduleId) return
        fetchPendings(p.write_id)
      },
    )
    return () => {
      cancelled = true
      unlisten.then((fn) => fn())
    }
  }, [node.projectPath, node.moduleId, fetchPendings])

  if (loading) return <LogbookSkeleton />
  if (error || !data) return <LogbookError message={error ?? 'Sin datos'} />

  const activeSection = data.sections.find((s) => s.id === activeSectionId) ?? null
  const activePending = pendingWrites.find((w) => w.write_id === activePendingId) ?? null

  return (
    <div className="flex-1 min-h-0 flex">
      <Sidebar
        data={data}
        node={node}
        activeSectionId={activePending ? null : activeSectionId}
        onSelect={(id) => {
          setActivePendingId(null)
          setActiveSectionId(id)
        }}
        onChanged={(prefer) => fetchData(prefer)}
        pendingWrites={pendingWrites}
        activePendingId={activePendingId}
        onSelectPending={(id) => {
          setActivePendingId(id)
        }}
      />
      <main className="flex-1 min-w-0 flex flex-col overflow-hidden">
        {activePending ? (
          <PendingSectionPreview
            key={activePending.write_id}
            write={activePending}
            onResolved={() => {
              // Backend emits ocean-knowledge-changed on accept and
              // ai-write-proposed on discard — both listeners refetch. We
              // also clear the local selection so the UI snaps back.
              setActivePendingId(null)
              fetchPendings()
            }}
          />
        ) : activeSection ? (
          <SectionEditor
            key={activeSection.id}
            section={activeSection}
            node={node}
            onChanged={() => fetchData(activeSection.id)}
          />
        ) : (
          <EmptyMain />
        )}
      </main>
    </div>
  )
}

// -----------------------------------------------------------------------------
// Sidebar
// -----------------------------------------------------------------------------

function Sidebar({
  data,
  node,
  activeSectionId,
  onSelect,
  onChanged,
  pendingWrites,
  activePendingId,
  onSelectPending,
}: {
  data: KnowledgeNodeDataResponse
  node: NodePanelData
  activeSectionId: string | null
  onSelect: (id: string) => void
  onChanged: (preferSectionId?: string | null) => void
  pendingWrites: PendingWriteDto[]
  activePendingId: string | null
  onSelectPending: (id: string) => void
}) {
  const [adding, setAdding] = useState(false)
  const [newName, setNewName] = useState('')
  // Drag & drop state — id of section being dragged + id under cursor.
  // We render a thin insert line above the hovered target.
  const [draggingId, setDraggingId] = useState<string | null>(null)
  const [dropTargetId, setDropTargetId] = useState<string | null>(null)

  const handleAdd = async () => {
    const name = newName.trim()
    if (!name) return
    try {
      const res = await tauriApi.addNodeSection({
        project_path: node.projectPath,
        node_id: node.moduleId,
        name,
        content_markdown: '',
        source: { kind: 'user' },
      })
      setNewName('')
      setAdding(false)
      onChanged(res.section?.id ?? null)
    } catch (err) {
      console.error('Failed to add section:', err)
    }
  }

  const commitReorder = async (sourceId: string, target: string) => {
    setDraggingId(null)
    setDropTargetId(null)
    const ids = data.sections.map((s) => s.id)
    const fromIdx = ids.indexOf(sourceId)
    if (fromIdx === -1) return

    let next: string[]
    if (target === '__end__') {
      if (fromIdx === ids.length - 1) return
      next = ids.slice()
      next.splice(fromIdx, 1)
      next.push(sourceId)
    } else {
      if (sourceId === target) return
      const toIdx = ids.indexOf(target)
      if (toIdx === -1) return
      next = ids.slice()
      next.splice(fromIdx, 1)
      next.splice(toIdx, 0, sourceId)
    }
    if (next.every((id, i) => id === ids[i])) return

    try {
      await tauriApi.reorderNodeSections({
        project_path: node.projectPath,
        node_id: node.moduleId,
        ordered_section_ids: next,
      })
      onChanged(sourceId)
    } catch (err) {
      console.error('Reorder failed:', err)
    }
  }

  return (
    <aside className="w-56 shrink-0 border-r border-border bg-background-secondary flex flex-col">
      <div className="p-3 border-b border-border">
        <SubtypeBadgeEditor data={data} node={node} onChanged={() => onChanged()} />
      </div>
      {data.sections.length >= OVERSIZED_THRESHOLD && (
        <div className="mx-2 my-2 px-2 py-1.5 rounded border border-amber-500/40 bg-amber-500/10 flex items-start gap-1.5">
          <AlertCircle className="w-3 h-3 text-amber-400 shrink-0 mt-0.5" />
          <span className="text-[10px] text-amber-200 leading-snug">
            Long logbook ({data.sections.length} sections). Consider extracting some into their own nodes.
          </span>
        </div>
      )}
      <div className="flex-1 overflow-y-auto py-1">
        {pendingWrites.length > 0 && (
          <div className="mb-2">
            <div className="px-3 pt-1 pb-1 text-[10px] uppercase tracking-wide text-amber-300/80 flex items-center gap-1">
              <Sparkles className="w-3 h-3" />
              Pending ({pendingWrites.length})
            </div>
            {pendingWrites.map((w) => (
              <button
                key={w.write_id}
                type="button"
                onClick={() => onSelectPending(w.write_id)}
                className={cn(
                  'w-full flex items-center gap-2 px-3 py-1.5 text-xs text-left',
                  'border-l-2',
                  activePendingId === w.write_id
                    ? 'bg-amber-500/10 border-amber-400 text-foreground'
                    : 'border-transparent text-foreground-muted hover:text-foreground hover:bg-foreground/5',
                )}
                title={
                  w.kind === 'edit'
                    ? `Proposed edit · +${w.additions} -${w.deletions}`
                    : 'New section proposed'
                }
              >
                <Sparkles className="w-3 h-3 shrink-0 text-amber-400" />
                <span className="truncate flex-1">{w.name}</span>
                <span className="shrink-0 text-[10px] text-foreground-muted/70">
                  {w.kind === 'edit' ? `+${w.additions}/-${w.deletions}` : 'nuevo'}
                </span>
              </button>
            ))}
          </div>
        )}
        {data.sections.map((section) => (
          <SidebarItem
            key={section.id}
            section={section}
            active={section.id === activeSectionId}
            onSelect={() => onSelect(section.id)}
            node={node}
            onChanged={onChanged}
            allowDelete={data.sections.length > 1}
            isDragging={draggingId === section.id}
            isDropTarget={dropTargetId === section.id && draggingId !== section.id}
            onDragStart={() => setDraggingId(section.id)}
            onDragOver={() => setDropTargetId(section.id)}
            onDragEnd={() => {
              setDraggingId(null)
              setDropTargetId(null)
            }}
            onDrop={(sourceId) => commitReorder(sourceId, section.id)}
          />
        ))}
        {draggingId && (
          <div
            onDragOver={(e) => {
              e.preventDefault()
              e.dataTransfer.dropEffect = 'move'
              setDropTargetId('__end__')
            }}
            onDrop={(e) => {
              e.preventDefault()
              const sourceId = e.dataTransfer.getData('text/x-section-id')
              if (sourceId) commitReorder(sourceId, '__end__')
            }}
            className="relative h-3 mx-2"
          >
            {dropTargetId === '__end__' && (
              <span className="absolute left-0 right-0 top-1 h-0.5 bg-emerald-400 pointer-events-none" />
            )}
          </div>
        )}
        {adding ? (
          <div className="flex items-center gap-1 px-2 py-1.5">
            <input
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') handleAdd()
                if (e.key === 'Escape') {
                  setAdding(false)
                  setNewName('')
                }
              }}
              autoFocus
              placeholder="Nombre"
              className="flex-1 min-w-0 bg-background border border-border rounded px-2 py-1 text-xs"
            />
            <button
              type="button"
              onClick={handleAdd}
              disabled={!newName.trim()}
              className="text-xs px-2 py-1 rounded bg-emerald-500/90 hover:bg-emerald-500 text-white disabled:opacity-50"
            >
              OK
            </button>
          </div>
        ) : (
          <button
            type="button"
            onClick={() => setAdding(true)}
            className="w-full flex items-center gap-2 px-3 py-1.5 text-xs text-foreground-muted hover:text-foreground hover:bg-foreground/5"
          >
            <Plus className="w-3.5 h-3.5" />
            Add section
          </button>
        )}
      </div>
      <MetaBlock data={data} />
    </aside>
  )
}

function SidebarItem({
  section,
  active,
  onSelect,
  node,
  onChanged,
  allowDelete,
  isDragging,
  isDropTarget,
  onDragStart,
  onDragOver,
  onDragEnd,
  onDrop,
}: {
  section: NodeSectionDto
  active: boolean
  onSelect: () => void
  node: NodePanelData
  onChanged: (preferSectionId?: string | null) => void
  allowDelete: boolean
  isDragging: boolean
  isDropTarget: boolean
  onDragStart: () => void
  onDragOver: () => void
  onDragEnd: () => void
  onDrop: (sourceId: string) => void
}) {
  const [renaming, setRenaming] = useState(false)
  const [name, setName] = useState(section.name)

  useEffect(() => {
    setName(section.name)
  }, [section.name])

  const commitRename = async () => {
    const trimmed = name.trim()
    if (!trimmed || trimmed === section.name) {
      setRenaming(false)
      setName(section.name)
      return
    }
    try {
      await tauriApi.updateNodeSection({
        project_path: node.projectPath,
        node_id: node.moduleId,
        section_id: section.id,
        name: trimmed,
        content_markdown: null,
      })
      setRenaming(false)
      onChanged(section.id)
    } catch (err) {
      console.error('Rename failed:', err)
    }
  }

  const handleDelete = async (e: React.MouseEvent) => {
    e.stopPropagation()
    if (!allowDelete) return
    try {
      await tauriApi.deleteNodeSection({
        project_path: node.projectPath,
        node_id: node.moduleId,
        section_id: section.id,
      })
      onChanged(null)
    } catch (err) {
      console.error('Delete failed:', err)
    }
  }

  // Extract this section into its own knowledge node. Backend places the new
  // node in a free cell near this one and emits ocean-knowledge-changed for
  // both, so the logbook refetch + ocean canvas refetch happen automatically.
  const handleExtract = async (e: React.MouseEvent) => {
    e.stopPropagation()
    if (!allowDelete) return
    try {
      await tauriApi.extractSectionToNode({
        project_path: node.projectPath,
        source_node_id: node.moduleId,
        section_id: section.id,
      })
    } catch (err) {
      console.error('Extract failed:', err)
    }
  }

  return (
    <div
      onClick={onSelect}
      draggable
      onDragStart={(e) => {
        e.dataTransfer.setData('text/x-section-id', section.id)
        e.dataTransfer.effectAllowed = 'move'
        onDragStart()
      }}
      onDragOver={(e) => {
        e.preventDefault()
        e.dataTransfer.dropEffect = 'move'
        onDragOver()
      }}
      onDragEnd={onDragEnd}
      onDrop={(e) => {
        e.preventDefault()
        const sourceId = e.dataTransfer.getData('text/x-section-id')
        if (sourceId) onDrop(sourceId)
      }}
      className={cn(
        'group relative flex items-center gap-1.5 px-3 py-1.5 text-sm cursor-pointer select-none',
        'hover:bg-foreground/5',
        active
          ? 'bg-foreground/10 text-foreground border-l-2 border-foreground/60'
          : 'text-foreground-muted',
        isDragging && 'opacity-40',
      )}
    >
      {isDropTarget && (
        <span className="absolute left-0 right-0 -top-px h-0.5 bg-emerald-400 pointer-events-none" />
      )}
      <GripVertical className="w-3 h-3 text-foreground-muted/40 shrink-0 cursor-grab active:cursor-grabbing" />
      {renaming ? (
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          onClick={(e) => e.stopPropagation()}
          onBlur={commitRename}
          onKeyDown={(e) => {
            if (e.key === 'Enter') commitRename()
            if (e.key === 'Escape') {
              setRenaming(false)
              setName(section.name)
            }
          }}
          autoFocus
          className="flex-1 min-w-0 bg-background border border-border rounded px-1.5 py-0.5 text-xs"
        />
      ) : (
        <span className="flex-1 min-w-0 truncate text-xs">{section.name}</span>
      )}
      {section.source.kind === 'ai' && !renaming && (
        <span
          className="shrink-0"
          title={`Generado por ${section.source.model} · ${new Date(section.source.timestamp * 1000).toLocaleDateString()}`}
        >
          <Sparkles className="w-2.5 h-2.5 text-amber-400/80" />
        </span>
      )}
      <div className="flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity">
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation()
            setRenaming(true)
          }}
          className="p-0.5 text-foreground-muted/60 hover:text-foreground"
          title="Renombrar"
        >
          <Pencil className="w-3 h-3" />
        </button>
        <button
          type="button"
          onClick={handleExtract}
          disabled={!allowDelete}
          className="p-0.5 text-foreground-muted/60 hover:text-emerald-400 disabled:opacity-30 disabled:hover:text-foreground-muted/60"
          title={allowDelete ? 'Extract to new node' : 'At least one section must remain'}
        >
          <Split className="w-3 h-3" />
        </button>
        <button
          type="button"
          onClick={handleDelete}
          disabled={!allowDelete}
          className="p-0.5 text-foreground-muted/60 hover:text-red-400 disabled:opacity-30 disabled:hover:text-foreground-muted/60"
          title={allowDelete ? 'Delete' : 'At least one section must remain'}
        >
          <Trash2 className="w-3 h-3" />
        </button>
      </div>
    </div>
  )
}

function SubtypeBadgeEditor({
  data,
  node,
  onChanged,
}: {
  data: KnowledgeNodeDataResponse
  node: NodePanelData
  onChanged: () => void
}) {
  const handleChange = async (next: string) => {
    const subtype = next as KnowledgeNodeSubtype
    if (subtype === data.subtype) return
    try {
      await tauriApi.updateNodeSubtype({
        project_path: node.projectPath,
        node_id: node.moduleId,
        subtype,
      })
      onChanged()
    } catch (err) {
      console.error('Failed to update subtype:', err)
    }
  }

  return (
    <Select value={data.subtype} onValueChange={handleChange}>
      <SelectTrigger className="text-xs h-8" title="Change type">
        <SelectValue>
          <span className="inline-flex items-center gap-1.5">
            <span
              className={cn('w-[5px] h-[5px]', SUBTYPE_DOT[data.subtype])}
              aria-hidden
            />
            {SUBTYPE_LABELS[data.subtype]}
          </span>
        </SelectValue>
      </SelectTrigger>
      <SelectContent>
        {SUBTYPE_OPTIONS.map((s) => (
          <SelectItem key={s} value={s}>
            <span className="inline-flex items-center gap-1.5">
              <span className={cn('w-[5px] h-[5px]', SUBTYPE_DOT[s])} aria-hidden />
              {SUBTYPE_LABELS[s]}
            </span>
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}

function MetaBlock({ data }: { data: KnowledgeNodeDataResponse }) {
  const created = new Date(data.created_at * 1000).toLocaleDateString()
  const updated = new Date(data.updated_at * 1000).toLocaleDateString()
  return (
    <div className="p-3 border-t border-border space-y-1 text-[10px] text-foreground-muted/60">
      <div className="flex justify-between">
        <span>Creado</span>
        <span className="tabular-nums">{created}</span>
      </div>
      <div className="flex justify-between">
        <span>Modificado</span>
        <span className="tabular-nums">{updated}</span>
      </div>
      <div className="flex justify-between">
        <span>Secciones</span>
        <span className="tabular-nums">{data.sections.length}</span>
      </div>
    </div>
  )
}

// -----------------------------------------------------------------------------
// Section editor — Monaco + preview toggle
// -----------------------------------------------------------------------------

type EditorMode = 'edit' | 'preview'

function SectionEditor({
  section,
  node,
  onChanged,
}: {
  section: NodeSectionDto
  node: NodePanelData
  onChanged: () => void
}) {
  const [content, setContent] = useState(section.content_markdown)
  const [mode, setMode] = useState<EditorMode>('edit')
  const [saving, setSaving] = useState(false)
  const timeoutRef = useRef<number | null>(null)
  const onChangedRef = useRef(onChanged)
  onChangedRef.current = onChanged

  useEffect(() => {
    setContent(section.content_markdown)
  }, [section.id, section.content_markdown])

  const persist = useCallback(
    (next: string) => {
      if (timeoutRef.current) window.clearTimeout(timeoutRef.current)
      setSaving(true)
      timeoutRef.current = window.setTimeout(async () => {
        try {
          await tauriApi.updateNodeSection({
            project_path: node.projectPath,
            node_id: node.moduleId,
            section_id: section.id,
            name: null,
            content_markdown: next,
          })
          onChangedRef.current()
        } catch (err) {
          console.error('Save section failed:', err)
        } finally {
          setSaving(false)
        }
      }, 600)
    },
    [node.projectPath, node.moduleId, section.id],
  )

  const handleEditorChange = (value: string | undefined) => {
    const next = value ?? ''
    setContent(next)
    persist(next)
  }

  const handleEditorMount: OnMount = useCallback((editor, monaco) => {
    document.fonts.ready.then(() => monaco.editor.remeasureFonts())
    editor.updateOptions({ automaticLayout: true })
  }, [])

  return (
    <>
      <header className="h-9 flex items-center gap-3 px-4 border-b border-border shrink-0">
        <span className="text-xs font-medium text-foreground truncate flex-1">
          {section.name}
        </span>
        <span className="text-[10px] text-foreground-muted/60 min-w-[60px] text-right">
          {saving ? 'Guardando...' : 'Guardado'}
        </span>
        <div className="flex items-center bg-background-tertiary border border-border rounded-md p-0.5">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setMode('edit')}
            className={cn(
              'h-6 px-2 text-xs gap-1.5',
              mode === 'edit' && 'bg-foreground/10 text-foreground',
            )}
          >
            <Pencil className="w-3 h-3" />
            Edit
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setMode('preview')}
            className={cn(
              'h-6 px-2 text-xs gap-1.5',
              mode === 'preview' && 'bg-foreground/10 text-foreground',
            )}
          >
            <Eye className="w-3 h-3" />
            Preview
          </Button>
        </div>
      </header>
      <div className="flex-1 min-h-0 relative">
        {mode === 'edit' ? (
          <div className="absolute inset-0">
            <Editor
              path={`venore://node/${node.moduleId}/${section.id}.md`}
              defaultLanguage="markdown"
              value={content}
              theme="vs-dark"
              onChange={handleEditorChange}
              onMount={handleEditorMount}
              options={{
                minimap: { enabled: false },
                fontSize: 13,
                fontFamily: "'Geist Mono', monospace",
                lineNumbers: 'off',
                wordWrap: 'on',
                scrollBeyondLastLine: false,
                automaticLayout: true,
                padding: { top: 12, bottom: 12 },
                renderLineHighlight: 'none',
              }}
            />
          </div>
        ) : (
          <div className="absolute inset-0 overflow-y-auto">
            <div className="px-6 py-4 max-w-4xl">
              {content.trim() ? (
                <MarkdownRenderer content={content} />
              ) : (
                <p className="text-xs text-foreground-muted/40 italic">
                  Empty section. Switch to "Edit" to add content.
                </p>
              )}
            </div>
          </div>
        )}
      </div>
    </>
  )
}

// -----------------------------------------------------------------------------
// States
// -----------------------------------------------------------------------------

function EmptyMain() {
  return (
    <div className="flex-1 flex items-center justify-center text-xs text-foreground-muted/50">
      No sections. Add one from the sidebar to get started.
    </div>
  )
}

function LogbookSkeleton() {
  return (
    <div className="flex-1 flex">
      <aside className="w-56 shrink-0 border-r border-border bg-background-secondary p-3 space-y-3 animate-pulse">
        <div className="h-6 bg-foreground/10 rounded" />
        <div className="space-y-2 mt-4">
          <div className="h-7 bg-foreground/5 rounded" />
          <div className="h-7 bg-foreground/5 rounded" />
          <div className="h-7 bg-foreground/5 rounded" />
        </div>
      </aside>
      <div className="flex-1 p-6 space-y-3 animate-pulse">
        <div className="h-4 bg-foreground/10 rounded w-1/4" />
        <div className="h-64 bg-foreground/5 rounded" />
      </div>
    </div>
  )
}

function LogbookError({ message }: { message: string }) {
  return (
    <div className="flex-1 flex items-center justify-center text-sm text-foreground-muted/60">
      {message}
    </div>
  )
}
