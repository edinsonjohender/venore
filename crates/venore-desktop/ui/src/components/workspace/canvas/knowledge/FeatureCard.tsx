// =============================================================================
// FeatureCard — Card component for the feature overview grid
// =============================================================================

import { cn } from '@/lib/utils'
import { PHASE_COLORS, PHASE_LABELS } from './hex-colors'
import type { Feature, HexPhase } from './mock-data'

interface FeatureCardProps {
  feature: Feature
  onClick: () => void
}

const STATUS_STYLES: Record<Feature['status'], string> = {
  active: 'bg-emerald-500/15 text-emerald-400',
  paused: 'bg-amber-500/15 text-amber-400',
  completed: 'bg-blue-500/15 text-blue-400',
}

const PRIORITY_STYLES: Record<Feature['priority'], string> = {
  low: 'bg-zinc-500/15 text-zinc-400',
  medium: 'bg-blue-500/15 text-blue-400',
  high: 'bg-orange-500/15 text-orange-400',
  critical: 'bg-red-500/15 text-red-400',
}

export function FeatureCard({ feature, onClick }: FeatureCardProps) {
  const { hexagons } = feature
  const deadEnds = hexagons.filter((h) => h.isDeadEnd).length
  const avgProgress = hexagons.length > 0
    ? Math.round(hexagons.reduce((sum, h) => sum + h.percentage, 0) / hexagons.length)
    : 0

  // Collect unique phases present
  const phases = [...new Set(hexagons.map((h) => h.phase))] as HexPhase[]

  return (
    <button
      onClick={onClick}
      className={cn(
        'w-full text-left p-4 rounded-lg border border-border/50',
        'bg-background-secondary hover:bg-background-tertiary',
        'transition-colors group',
      )}
    >
      {/* Header */}
      <div className="flex items-start justify-between gap-3 mb-2">
        <h3 className="text-sm font-medium text-foreground group-hover:text-foreground-bright transition-colors">
          {feature.name}
        </h3>
        <div className="flex gap-1.5 shrink-0">
          <span className={cn('px-1.5 py-0.5 rounded text-[10px] font-medium capitalize', STATUS_STYLES[feature.status])}>
            {feature.status}
          </span>
          <span className={cn('px-1.5 py-0.5 rounded text-[10px] font-medium capitalize', PRIORITY_STYLES[feature.priority])}>
            {feature.priority}
          </span>
        </div>
      </div>

      {/* Description */}
      <p className="text-xs text-foreground-muted line-clamp-2 mb-3">
        {feature.description}
      </p>

      {/* Stats row */}
      <div className="flex items-center gap-3 text-[10px] text-foreground-subtle mb-2.5">
        <span>{hexagons.length} hexagons</span>
        {deadEnds > 0 && (
          <span className="text-red-400">{deadEnds} dead end{deadEnds > 1 ? 's' : ''}</span>
        )}
        <span>{feature.evidence.length} evidence</span>
      </div>

      {/* Progress bar */}
      <div className="h-1 rounded-full bg-white/5 mb-2.5 overflow-hidden">
        <div
          className="h-full rounded-full bg-brand-teal/60 transition-all"
          style={{ width: `${avgProgress}%` }}
        />
      </div>

      {/* Phase tags */}
      <div className="flex gap-1.5 flex-wrap">
        {phases.map((phase) => (
          <span
            key={phase}
            className="flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px]"
            style={{
              backgroundColor: PHASE_COLORS[phase] + '15',
              color: PHASE_COLORS[phase],
            }}
          >
            <span
              className="w-1.5 h-1.5 rounded-full"
              style={{ backgroundColor: PHASE_COLORS[phase] }}
            />
            {PHASE_LABELS[phase]}
          </span>
        ))}
      </div>
    </button>
  )
}
