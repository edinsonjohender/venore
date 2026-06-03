// =============================================================================
// ActivityTab — Timeline of AI research activity
// =============================================================================

import { Clock, Search, CheckCircle2, AlertTriangle, XCircle } from 'lucide-react'
import { cn } from '@/lib/utils'
import { PHASE_COLORS } from './hex-colors'
import type { Feature } from './mock-data'

interface ActivityTabProps {
  feature: Feature
}

// Mock activity entries (will come from backend later)
interface ActivityEntry {
  id: string
  timestamp: string
  type: 'search' | 'advance' | 'dead-end' | 'evidence'
  hexTitle: string
  description: string
  phase?: string
}

function generateMockActivity(feature: Feature): ActivityEntry[] {
  const entries: ActivityEntry[] = []
  let i = 0
  for (const hex of feature.hexagons) {
    if (hex.percentage > 0) {
      entries.push({
        id: `act-${i++}`,
        timestamp: '2 hours ago',
        type: hex.isDeadEnd ? 'dead-end' : hex.percentage >= 80 ? 'advance' : 'search',
        hexTitle: hex.title,
        description: hex.isDeadEnd
          ? `Marked as dead end — ${hex.notes ?? 'no further leads'}`
          : hex.percentage >= 80
            ? `Advanced to ${hex.phase} phase (${hex.percentage}%)`
            : `Searching — ${hex.percentage}% complete`,
        phase: hex.phase,
      })
    }
  }

  for (const ev of feature.evidence) {
    const hex = feature.hexagons.find((h) => h.id === ev.hexagonId)
    entries.push({
      id: `act-${i++}`,
      timestamp: '3 hours ago',
      type: 'evidence',
      hexTitle: hex?.title ?? 'Unknown',
      description: `Found: ${ev.title}`,
      phase: hex?.phase,
    })
  }

  return entries
}

const TYPE_ICON = {
  search: Search,
  advance: CheckCircle2,
  'dead-end': XCircle,
  evidence: AlertTriangle,
}

const TYPE_COLOR = {
  search: 'text-blue-400',
  advance: 'text-emerald-400',
  'dead-end': 'text-red-400',
  evidence: 'text-amber-400',
}

export function ActivityTab({ feature }: ActivityTabProps) {
  const entries = generateMockActivity(feature)

  if (entries.length === 0) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center text-foreground-subtle">
        <Clock className="w-6 h-6 mb-2 opacity-30" />
        <span className="text-xs">No activity yet</span>
      </div>
    )
  }

  return (
    <div className="flex-1 overflow-y-auto p-4">
      <div className="max-w-2xl mx-auto space-y-1">
        {entries.map((entry) => {
          const Icon = TYPE_ICON[entry.type]
          return (
            <div key={entry.id} className="flex items-start gap-3 py-2 px-3 rounded-md hover:bg-background-secondary transition-colors">
              <Icon className={cn('w-3.5 h-3.5 mt-0.5 shrink-0', TYPE_COLOR[entry.type])} />
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <span className="text-xs font-medium text-foreground truncate">{entry.hexTitle}</span>
                  {entry.phase && (
                    <span
                      className="w-1.5 h-1.5 rounded-full shrink-0"
                      style={{ backgroundColor: PHASE_COLORS[entry.phase as keyof typeof PHASE_COLORS] }}
                    />
                  )}
                </div>
                <p className="text-[11px] text-foreground-muted mt-0.5">{entry.description}</p>
              </div>
              <span className="text-[10px] text-foreground-subtle shrink-0 mt-0.5">{entry.timestamp}</span>
            </div>
          )
        })}
      </div>
    </div>
  )
}
