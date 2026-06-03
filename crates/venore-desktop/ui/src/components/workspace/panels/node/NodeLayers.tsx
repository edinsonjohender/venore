// =============================================================================
// NodeLayers — Layer status display with expandable details for node panel
// =============================================================================

import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { ChevronRight } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { NodeLayerDto } from '@/lib/tauri'

interface NodeLayersProps {
  layers: NodeLayerDto[]
}

const STATUS_ICON: Record<string, { icon: string; color: string }> = {
  complete: { icon: '\u25CF', color: 'text-green-400' },       // filled circle
  partial: { icon: '\u25D2', color: 'text-yellow-400' },       // half circle
  'in-progress': { icon: '\u25CB', color: 'text-blue-400' },   // empty circle
  missing: { icon: '\u25CB', color: 'text-foreground-muted/30' }, // empty circle dim
}

function statusInfo(status: string) {
  return STATUS_ICON[status] ?? STATUS_ICON['missing']
}

/** Format detail values into a human-readable summary per layer type. */
function formatDetails(type: string, details: Record<string, unknown>): string | null {
  switch (type) {
    case 'tests': {
      const tests = details.test_files as number | undefined
      const source = details.source_files as number | undefined
      const ratio = details.coverage_ratio as number | undefined
      if (tests == null || source == null) return null
      const pct = ratio != null ? `${Math.round(ratio * 100)}%` : '?'
      return `${tests} test files / ${source} source (${pct})`
    }
    case 'documentation': {
      const readme = details.has_readme as boolean | undefined
      const ratio = details.doc_ratio as number | undefined
      const parts: string[] = []
      if (readme) parts.push('README')
      if (ratio != null) parts.push(`${Math.round(ratio * 100)}% documented`)
      return parts.length > 0 ? parts.join(' + ') : null
    }
    case 'status': {
      const todo = (details.todo_count as number) ?? 0
      const fixme = (details.fixme_count as number) ?? 0
      const hack = (details.hack_count as number) ?? 0
      const total = (details.total_issues as number) ?? 0
      if (total === 0) return 'Clean'
      const parts: string[] = []
      if (todo > 0) parts.push(`${todo} TODOs`)
      if (fixme > 0) parts.push(`${fixme} FIXMEs`)
      if (hack > 0) parts.push(`${hack} HACKs`)
      return parts.join(', ')
    }
    case 'connections': {
      const deps = (details.dependency_count as number) ?? 0
      const dependents = (details.dependent_count as number) ?? 0
      const circular = details.has_circular as boolean | undefined
      const orphan = details.is_orphan as boolean | undefined
      if (orphan) return 'Orphan (no connections)'
      const parts = [`${deps} deps`, `${dependents} dependents`]
      if (circular) parts.push('circular!')
      return parts.join(', ')
    }
    case 'context': {
      const freshness = details.freshness as string | undefined
      return freshness ?? null
    }
    default:
      return null
  }
}

export function NodeLayers({ layers }: NodeLayersProps) {
  const { t } = useTranslation('project')
  const [open, setOpen] = useState(true)
  const [expandedLayer, setExpandedLayer] = useState<string | null>(null)

  if (layers.length === 0) return null

  const completedCount = layers.filter((l) => l.status === 'complete').length

  return (
    <div className="border-b border-border">
      <button
        onClick={() => setOpen(!open)}
        className="flex items-center gap-1 w-full px-3 py-1.5 text-left hover:bg-foreground/5 transition-colors"
      >
        <ChevronRight
          className={cn(
            'w-3 h-3 text-foreground-muted/50 transition-transform',
            open && 'rotate-90',
          )}
        />
        <span className="text-[10px] font-semibold text-foreground-muted uppercase tracking-wider">
          {t('nodeLayers.layersHeader')}
        </span>
        <span className="text-[10px] text-foreground-muted/50">
          ({completedCount}/{layers.length})
        </span>
      </button>

      {open && (
        <ul className="px-3 pb-2 space-y-0.5">
          {layers.map((layer) => {
            const { icon, color } = statusInfo(layer.status)
            const hasDetails = layer.details && Object.keys(layer.details).length > 0
            const isExpanded = expandedLayer === layer.type
            const detailText = hasDetails
              ? formatDetails(layer.type, layer.details!)
              : null

            return (
              <li key={layer.type}>
                <button
                  onClick={() => hasDetails && setExpandedLayer(isExpanded ? null : layer.type)}
                  className={cn(
                    'flex items-center gap-1.5 text-xs w-full text-left',
                    hasDetails && 'cursor-pointer hover:bg-foreground/5 rounded px-0.5 -mx-0.5',
                    !hasDetails && 'cursor-default',
                  )}
                >
                  <span className={cn('text-[10px] w-3 text-center shrink-0', color)}>
                    {icon}
                  </span>
                  <span className="text-foreground">{layer.type}</span>
                  <span className="text-foreground-muted/40 text-[10px] ml-auto">
                    {layer.status}
                  </span>
                </button>

                {isExpanded && detailText && (
                  <div className="ml-[18px] mt-0.5 mb-1 text-[10px] text-foreground-muted/70 leading-snug">
                    {detailText}
                  </div>
                )}
              </li>
            )
          })}
        </ul>
      )}
    </div>
  )
}
