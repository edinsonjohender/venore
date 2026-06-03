// =============================================================================
// ProjectPanel - Project overview with Context/Files tabs + real dashboard data
// =============================================================================

import { useState, useEffect, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { RefreshCw } from 'lucide-react'
import { toast } from 'sonner'
import { listen } from '@tauri-apps/api/event'
import type { PanelContentProps } from './registry'
import { ProjectTabs } from './project/ProjectTabs'
import { ContextTab } from './project/ContextTab'
import { FilesTab } from './project/FilesTab'
import { tauriApi } from '@/lib/tauri'
import type { ModuleSummaryDto } from '@/lib/tauri'
import { useDashboardStore } from '@/stores/dashboardStore'

type TabId = 'context' | 'files'

export function ProjectPanel({ projectPath }: PanelContentProps) {
  const { t } = useTranslation('project')
  const [activeTab, setActiveTab] = useState<TabId>('context')
  const [refreshing, setRefreshing] = useState(false)

  // Dashboard data flows through the store, deduped across all consumers
  // (this panel and the collapsed pill render simultaneously and used to
  // double-fetch).
  const dashboard = useDashboardStore((s) => s.dashboard)
  const loading = useDashboardStore((s) => s.loading)
  const error = useDashboardStore((s) => s.error)
  const loadDashboard = useDashboardStore((s) => s.loadDashboard)
  const refreshDashboard = useDashboardStore((s) => s.refreshDashboard)

  useEffect(() => {
    if (!projectPath) return
    void loadDashboard(projectPath)
  }, [projectPath, loadDashboard])

  // Auto-refresh stats whenever the workspace produces a fresh snapshot
  // (wizard, re-snapshot, context updater). The Rust side emits the event
  // after writing `.venore/*.json`, so a forced fetch always reads
  // consistent data.
  useEffect(() => {
    if (!projectPath) return
    let cancelled = false
    const unlistenPromise = listen('context-update-complete', () => {
      if (!cancelled) void refreshDashboard(projectPath)
    })
    return () => {
      cancelled = true
      unlistenPromise.then((fn) => fn())
    }
  }, [projectPath, refreshDashboard])

  const handleResnapshot = useCallback(async () => {
    if (!projectPath || refreshing) return
    setRefreshing(true)
    try {
      const report = await tauriApi.resnapshotProject(projectPath)
      // Backend already emitted `context-update-complete` so the canvas
      // refreshes its layers + stale badges on its own — we just need to
      // refresh the panel's dashboard counts.
      void refreshDashboard(projectPath)
      const parts = [
        t('panel.resnapshotParts.modules', {
          n: report.modules,
          defaultValue: '{{n}} modules',
        }),
        t('panel.resnapshotParts.layers', {
          n: report.layersWritten,
          defaultValue: '{{n}} layers',
        }),
        t('panel.resnapshotParts.hashes', {
          n: report.hashesWritten,
          defaultValue: '{{n}} hashes',
        }),
      ]
      toast.success(t('panel.resnapshotSuccess', { defaultValue: 'Snapshot refreshed' }), {
        description: parts.join(', '),
      })
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      toast.error(t('panel.resnapshotFailed', { defaultValue: 'Snapshot refresh failed' }), {
        description: message,
      })
    } finally {
      setRefreshing(false)
    }
  }, [projectPath, refreshing, refreshDashboard, t])

  const stats = dashboard?.stats
  const modules: ModuleSummaryDto[] = dashboard?.modules ?? []
  const orphanFiles: string[] = dashboard?.orphan_files ?? []

  return (
    <div className="flex flex-col h-full">
      {/* Stats bar */}
      <div className="flex items-center gap-3 px-3 h-7 border-b border-border shrink-0">
        {loading ? (
          <span className="text-[10px] text-foreground-muted/50">{t('panel.loading')}</span>
        ) : error ? (
          <span className="text-[10px] text-red-400">{t('panel.noAnalysis')}</span>
        ) : stats ? (
          <>
            <span className="text-[10px] text-foreground-muted">
              {t('panel.modulesCount', { count: stats.total_modules })}
            </span>
            <span className="text-[10px] text-foreground-muted">
              {t('panel.connectionsCount', { count: stats.total_connections })}
            </span>
            <span className="text-[10px] text-foreground-muted/60">|</span>
            {stats.fresh_count > 0 && (
              <span className="text-[10px] text-emerald-400">{t('panel.freshCount', { count: stats.fresh_count })}</span>
            )}
            {stats.stale_count > 0 && (
              <span className="text-[10px] text-amber-400">{t('panel.staleCount', { count: stats.stale_count })}</span>
            )}
            {stats.missing_count > 0 && (
              <span className="text-[10px] text-foreground-muted/50">{t('panel.missingCount', { count: stats.missing_count })}</span>
            )}
          </>
        ) : null}
        <button
          type="button"
          onClick={handleResnapshot}
          disabled={refreshing || !projectPath || loading}
          className="ml-auto inline-flex items-center justify-center h-5 w-5 rounded text-foreground-muted hover:text-foreground hover:bg-background-tertiary disabled:opacity-40 disabled:cursor-not-allowed"
          title={t('panel.resnapshotTooltip', { defaultValue: 'Refresh .venore/ snapshot from current code' })}
          aria-label={t('panel.resnapshotTooltip', { defaultValue: 'Refresh .venore/ snapshot from current code' })}
        >
          <RefreshCw className={`w-3 h-3 ${refreshing ? 'animate-spin' : ''}`} />
        </button>
      </div>

      {/* Tabs */}
      <ProjectTabs active={activeTab} onChange={setActiveTab} />

      {/* Tab content */}
      <div className="flex-1 overflow-hidden">
        {activeTab === 'context' ? (
          <ContextTab modules={modules} error={error} projectPath={projectPath} loading={loading} />
        ) : (
          <FilesTab modules={modules} orphanFiles={orphanFiles} loading={loading} error={error} />
        )}
      </div>
    </div>
  )
}
