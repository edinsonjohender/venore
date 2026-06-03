// =============================================================================
// KnowledgePanel — Feature list panel (left sidebar)
// =============================================================================
// Lists research features from the backend. Click opens the research graph.

import { useState } from 'react'
import { Hexagon, Loader2, Plus, X } from 'lucide-react'
import { cn } from '@/lib/utils'
import { toast } from 'sonner'
import { tauriApi } from '@/lib/tauri'
import type { PanelContentProps } from './registry'
import type { Feature, HexPhase } from '@/components/workspace/canvas/knowledge/mock-data'
import { PHASE_COLORS } from '@/components/workspace/canvas/knowledge/hex-colors'
import { useCanvasTabStore } from '@/stores/canvasTabStore'
import { useKnowledgeFeatures } from '@/components/workspace/canvas/knowledge/useKnowledgeData'

// -----------------------------------------------------------------------------
// Feature list item (compact for sidebar)
// -----------------------------------------------------------------------------

const STATUS_DOT: Record<Feature['status'], string> = {
  active: 'bg-emerald-400',
  paused: 'bg-amber-400',
  completed: 'bg-blue-400',
}

function FeatureItem({ feature }: { feature: Feature }) {
  const openKnowledge = useCanvasTabStore((s) => s.openKnowledge)
  const activeTabId = useCanvasTabStore((s) => s.activeTabId)
  const isActive = activeTabId === `knowledge-${feature.id}`

  const { hexagons } = feature
  const deadEnds = hexagons.filter((h) => h.isDeadEnd).length
  const avgProgress = hexagons.length > 0
    ? Math.round(hexagons.reduce((sum, h) => sum + h.percentage, 0) / hexagons.length)
    : 0
  const phases = [...new Set(hexagons.map((h) => h.phase))] as HexPhase[]

  return (
    <button
      onClick={() => openKnowledge(feature.id, feature.name)}
      className={cn(
        'w-full text-left px-3 py-2.5 transition-colors',
        isActive
          ? 'bg-background-tertiary'
          : 'hover:bg-white/[0.03]',
      )}
    >
      {/* Title + status */}
      <div className="flex items-center gap-2 mb-1">
        <span className={cn('w-1.5 h-1.5 rounded-full shrink-0', STATUS_DOT[feature.status] ?? 'bg-zinc-400')} />
        <span className="text-xs font-medium text-foreground truncate">{feature.name}</span>
      </div>

      {/* Stats */}
      <div className="flex items-center gap-2 text-[10px] text-foreground-subtle ml-3.5 mb-1.5">
        <span>{hexagons.length} nodes</span>
        {deadEnds > 0 && <span className="text-red-400">{deadEnds} dead</span>}
        <span>{avgProgress}%</span>
      </div>

      {/* Progress bar */}
      <div className="h-0.5 rounded-full bg-white/5 ml-3.5 mb-1.5 overflow-hidden">
        <div
          className="h-full rounded-full bg-brand-teal/60"
          style={{ width: `${avgProgress}%` }}
        />
      </div>

      {/* Phase dots */}
      <div className="flex gap-1 ml-3.5">
        {phases.map((phase) => (
          <span
            key={phase}
            className="w-2 h-2 rounded-full"
            style={{ backgroundColor: PHASE_COLORS[phase], opacity: 0.7 }}
            title={phase}
          />
        ))}
      </div>
    </button>
  )
}

// -----------------------------------------------------------------------------
// Inline create form
// -----------------------------------------------------------------------------

function CreateFeatureForm({ projectId, onCreated, onCancel }: {
  projectId: string
  onCreated: (id: string, name: string) => void
  onCancel: () => void
}) {
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [submitting, setSubmitting] = useState(false)

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!name.trim() || submitting) return
    setSubmitting(true)
    try {
      const result = await tauriApi.createKnowledgeFeature({
        projectId,
        name: name.trim(),
        description: description.trim(),
      })
      if (result) onCreated(result.id, result.name)
    } catch (err) {
      toast.error('Failed to create feature')
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <form onSubmit={handleSubmit} className="px-3 py-2 border-b border-border/50 space-y-2">
      <input
        type="text"
        value={name}
        onChange={(e) => setName(e.target.value)}
        placeholder="Feature name"
        autoFocus
        className="w-full rounded-md border border-border/50 bg-background-secondary px-2 py-1 text-xs text-foreground outline-none focus:border-brand transition-colors"
      />
      <textarea
        value={description}
        onChange={(e) => setDescription(e.target.value)}
        placeholder="Brief description…"
        rows={2}
        className="w-full rounded-md border border-border/50 bg-background-secondary px-2 py-1 text-xs text-foreground outline-none focus:border-brand transition-colors resize-none"
      />
      <div className="flex gap-1.5">
        <button
          type="submit"
          disabled={!name.trim() || submitting}
          className="px-2.5 py-1 rounded-md text-[10px] font-medium bg-brand/15 text-brand hover:bg-brand/25 disabled:opacity-40 transition-colors"
        >
          {submitting ? 'Creating…' : 'Create'}
        </button>
        <button
          type="button"
          onClick={onCancel}
          className="px-2.5 py-1 rounded-md text-[10px] font-medium text-foreground-muted hover:text-foreground transition-colors"
        >
          Cancel
        </button>
      </div>
    </form>
  )
}

// -----------------------------------------------------------------------------
// Main Panel
// -----------------------------------------------------------------------------

export function KnowledgePanel(props: PanelContentProps) {
  const { features, loading, reload } = useKnowledgeFeatures(props.projectId ?? null)
  const [showForm, setShowForm] = useState(false)
  const openKnowledge = useCanvasTabStore((s) => s.openKnowledge)

  const handleCreated = (id: string, name: string) => {
    setShowForm(false)
    reload()
    openKnowledge(id, name)
  }

  return (
    <div className="flex flex-col h-full">
      {/* Header with add button */}
      <div className="flex items-center justify-between px-3 py-2">
        <span className="text-[10px] text-foreground-subtle uppercase tracking-wider">
          Features ({loading ? '…' : features.length})
        </span>
        {props.projectId && (
          <button
            onClick={() => setShowForm(!showForm)}
            className="p-0.5 rounded hover:bg-white/[0.06] text-foreground-subtle hover:text-foreground transition-colors"
            title="New feature"
          >
            {showForm ? <X className="w-3.5 h-3.5" /> : <Plus className="w-3.5 h-3.5" />}
          </button>
        )}
      </div>

      {/* Inline create form */}
      {showForm && props.projectId && (
        <CreateFeatureForm
          projectId={props.projectId}
          onCreated={handleCreated}
          onCancel={() => setShowForm(false)}
        />
      )}

      {/* Feature list */}
      <div className="flex-1 overflow-y-auto">
        {loading ? (
          <div className="flex flex-col items-center justify-center py-16 text-foreground-subtle">
            <Loader2 className="w-5 h-5 mb-2 opacity-30 animate-spin" />
            <span className="text-xs">Loading…</span>
          </div>
        ) : (
          <>
            {features.map((feature) => (
              <FeatureItem key={feature.id} feature={feature} />
            ))}

            {features.length === 0 && (
              <div className="flex flex-col items-center justify-center py-16 text-foreground-subtle">
                <Hexagon className="w-8 h-8 mb-3 opacity-30" />
                <span className="text-xs">No features yet</span>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  )
}
