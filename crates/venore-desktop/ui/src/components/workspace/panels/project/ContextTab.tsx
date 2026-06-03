// =============================================================================
// ContextTab - Compact module list with context status (single-line rows)
// =============================================================================

import { useState, useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { Search, SlidersHorizontal, Inbox, Loader2, AlertTriangle } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { ModuleSummaryDto } from '@/lib/tauri'
import { useNodeFloatingStore } from '@/stores/nodeFloatingStore'

type StatusFilter = 'all' | 'fresh' | 'stale' | 'missing'

interface ContextTabProps {
  modules: ModuleSummaryDto[]
  error: string | null
  projectPath: string
  loading: boolean
}

const FILTERS: { value: StatusFilter; labelKey: string }[] = [
  { value: 'all', labelKey: 'context.all' },
  { value: 'fresh', labelKey: 'context.fresh' },
  { value: 'stale', labelKey: 'context.stale' },
  { value: 'missing', labelKey: 'context.missing' },
]

export function ContextTab({ modules, error, projectPath, loading }: ContextTabProps) {
  const { t } = useTranslation('project')
  const [search, setSearch] = useState('')
  const [filter, setFilter] = useState<StatusFilter>('all')
  const [showFilter, setShowFilter] = useState(false)

  // Detect duplicate names and build a display-name map: path → label
  const displayNames = useMemo(() => {
    const counts = new Map<string, number>()
    for (const m of modules) {
      counts.set(m.name, (counts.get(m.name) ?? 0) + 1)
    }
    const map = new Map<string, string>()
    for (const m of modules) {
      if ((counts.get(m.name) ?? 0) > 1) {
        // Disambiguate with parent directory: "scripts (src)" vs "scripts (tools)"
        const parts = m.path.replace(/\\/g, '/').split('/')
        const parent = parts.length >= 2 ? parts[parts.length - 2] : ''
        map.set(m.path, parent ? `${m.name} (${parent})` : m.name)
      } else {
        map.set(m.path, m.name)
      }
    }
    return map
  }, [modules])

  const filtered = useMemo(() => {
    let result = modules
    if (filter !== 'all') {
      result = result.filter((m) => m.context_status === filter)
    }
    if (search.trim()) {
      const q = search.toLowerCase()
      result = result.filter((m) => m.name.toLowerCase().includes(q))
    }
    return result
  }, [modules, filter, search])

  const openNodePanel = useNodeFloatingStore((s) => s.openPanel)

  const handleClick = (m: ModuleSummaryDto) => {
    openNodePanel({
      projectPath,
      moduleId: m.name,
      moduleName: m.name,
      modulePath: m.path,
    })
  }

  if (error) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-2 px-4 h-full">
        <AlertTriangle className="w-8 h-8 text-foreground-muted/30" />
        <span className="text-xs font-medium text-foreground-muted">{t('context.noAnalysisAvailable')}</span>
        <span className="text-[10px] text-foreground-muted/60 text-center leading-relaxed">
          {t('context.runWizardHint')}
        </span>
      </div>
    )
  }

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center h-full">
        <Loader2 className="w-5 h-5 text-foreground-muted/40 animate-spin" />
      </div>
    )
  }

  return (
    <div className="flex flex-col h-full">
      {/* Search toolbar */}
      <div className="flex items-center gap-1.5 px-2 py-1 border-b border-border shrink-0">
        <Search className="w-3 h-3 text-foreground-muted shrink-0" />
        <input
          type="text"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder={t('context.searchModules', { count: modules.length })}
          className="flex-1 h-6 bg-transparent text-[11px] text-foreground placeholder:text-foreground-muted/50 outline-none"
        />
        <button
          onClick={() => setShowFilter(!showFilter)}
          className={cn(
            'shrink-0 p-0.5 rounded hover:bg-background-tertiary text-foreground-muted',
            showFilter && 'bg-background-tertiary text-brand',
          )}
          title={t('context.filterByStatus')}
        >
          <SlidersHorizontal className="w-3 h-3" />
        </button>
      </div>

      {/* Filter chips */}
      {showFilter && (
        <div className="flex items-center gap-1 px-2 py-1 border-b border-border shrink-0">
          {FILTERS.map((f) => (
            <button
              key={f.value}
              onClick={() => setFilter(f.value)}
              className={cn(
                'px-1.5 py-0 rounded text-[10px] font-medium transition-colors',
                filter === f.value
                  ? 'bg-brand/20 text-brand'
                  : 'text-foreground-muted hover:bg-background-tertiary',
              )}
            >
              {t(f.labelKey)}
            </button>
          ))}
        </div>
      )}

      {/* Module list — single-line compact rows */}
      {filtered.length === 0 ? (
        <div className="flex-1 flex flex-col items-center justify-center gap-2 px-4">
          <Inbox className="w-8 h-8 text-foreground-muted/30" />
          <span className="text-[11px] text-foreground-muted">{t('context.noModulesFound')}</span>
        </div>
      ) : (
        <div className="flex-1 overflow-y-auto">
          {filtered.map((m) => (
            <button
              key={m.path}
              onClick={() => handleClick(m)}
              className="w-full flex items-center gap-1.5 h-[22px] px-2 hover:bg-background-tertiary transition-colors text-left group"
            >
              {/* Status dot */}
              <span
                className={cn(
                  'w-[6px] h-[6px] rounded-full shrink-0',
                  m.context_status === 'fresh' && 'bg-emerald-400',
                  m.context_status === 'stale' && 'bg-amber-400',
                  m.context_status === 'missing' && 'bg-foreground-muted/25',
                )}
              />

              {/* Name */}
              <span className="flex-1 text-[11px] text-foreground truncate">{displayNames.get(m.path) ?? m.name}</span>

              {/* File count (subtle) */}
              <span className="text-[10px] text-foreground-muted/30 shrink-0 tabular-nums">
                {m.file_count}
              </span>
            </button>
          ))}
        </div>
      )}

      {/* Summary bar */}
      <div className="flex items-center justify-between px-2 h-5 border-t border-border shrink-0">
        <span className="text-[10px] text-foreground-muted/50">
          {t('context.filteredCount', { filtered: filtered.length, total: modules.length })}
        </span>
        {filter !== 'all' && (
          <span className="text-[10px] text-brand/50">{filter}</span>
        )}
      </div>
    </div>
  )
}
