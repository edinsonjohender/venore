// =============================================================================
// ProjectCollapsedPill - Collapsed pill view for the Project panel
// =============================================================================
// Shown when the project panel is in 'collapsed' mode. Clicking expands to docked.

import { useCallback, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { Layers, ChevronDown } from 'lucide-react'
import { usePanelStore } from '@/stores/panelStore'
import type { PanelContentProps } from '../registry'
import { useDashboardStore } from '@/stores/dashboardStore'

export function ProjectCollapsedPill({ panelId, projectPath }: PanelContentProps) {
  const { t } = useTranslation('project')
  const { setMode } = usePanelStore()
  const expand = useCallback(() => setMode(panelId, 'docked'), [panelId, setMode])
  const projectName = projectPath.split(/[/\\]/).pop() || 'Project'

  // Shared with ProjectPanel via the store — same fetch, same data.
  const moduleCount = useDashboardStore(
    (s) => s.dashboard?.stats?.total_modules ?? null,
  )
  const loadDashboard = useDashboardStore((s) => s.loadDashboard)

  useEffect(() => {
    if (!projectPath) return
    void loadDashboard(projectPath)
  }, [projectPath, loadDashboard])

  return (
    <button
      onClick={expand}
      className="absolute top-4 left-4 z-[15]
                 flex items-center gap-2 px-3 py-1.5
                 rounded-lg border border-border
                 bg-background/80 backdrop-blur-sm shadow-md
                 hover:bg-background transition-colors
                 text-xs select-none cursor-pointer"
    >
      <Layers className="w-3.5 h-3.5 text-brand" />
      <span className="font-medium text-foreground">{projectName}</span>
      <span className="text-foreground-muted/60">
        {moduleCount != null ? t('panel.modulesCount', { count: moduleCount }) : '...'}
      </span>
      <ChevronDown className="w-3 h-3 text-foreground-muted/40" />
    </button>
  )
}
