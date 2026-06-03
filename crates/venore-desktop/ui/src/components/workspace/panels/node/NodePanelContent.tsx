// =============================================================================
// NodePanelContent — Module-node content for the floating detail panel
// =============================================================================
// Lazy-fetches module details from backend on mount, then renders
// overview, connections, and exports sections.
// Falls back to basic info from NodePanelData when no wizard session exists.
// Knowledge nodes / lighthouses are handled directly by NodeLogbook.

import { useState, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import type { NodePanelData } from '@/stores/nodeFloatingStore'
import { tauriApi, type ModuleDetailsResponse } from '@/lib/tauri'
import { NodeOverview } from './NodeOverview'
import { NodeLayers } from './NodeLayers'
import { NodeConnections } from './NodeConnections'
import { NodeExports } from './NodeExports'

interface NodePanelContentProps {
  node: NodePanelData
}

export function NodePanelContent({ node }: NodePanelContentProps) {
  return <ModuleContent node={node} />
}

function ModuleContent({ node }: NodePanelContentProps) {
  const { t } = useTranslation('project')
  const [details, setDetails] = useState<ModuleDetailsResponse | null>(null)
  const [loading, setLoading] = useState(true)
  const [noSession, setNoSession] = useState(false)

  useEffect(() => {
    setLoading(true)
    setNoSession(false)

    tauriApi
      .getModuleDetails({
        project_path: node.projectPath,
        module_name: node.moduleName,
      })
      .then(setDetails)
      .catch((err) => {
        console.error('Failed to fetch module details:', err)
        // No wizard session or no cached analysis — show basic fallback
        setNoSession(true)
      })
      .finally(() => setLoading(false))
  }, [node.projectPath, node.moduleName])

  if (loading) return <LoadingSkeleton />

  // No session: show what we have from the ocean layout data
  if (noSession) return <BasicInfo node={node} />

  if (!details) return <EmptyState />

  return (
    <div className="flex-1 overflow-y-auto">
      <NodeOverview details={details} />
      <NodeLayers layers={details.layers} />
      <NodeConnections
        dependencies={details.dependencies}
        dependents={details.dependents}
      />
      <NodeExports exports={details.exports} />
    </div>
  )
}

// -----------------------------------------------------------------------------
// States
// -----------------------------------------------------------------------------

function LoadingSkeleton() {
  return (
    <div className="flex-1 px-3 py-2 space-y-3 animate-pulse">
      <div className="h-3 bg-foreground/10 rounded w-3/4" />
      <div className="h-2.5 bg-foreground/10 rounded w-1/2" />
      <div className="h-px bg-border my-2" />
      <div className="h-2.5 bg-foreground/10 rounded w-1/3" />
      <div className="space-y-1 ml-3">
        <div className="h-2.5 bg-foreground/10 rounded w-2/3" />
        <div className="h-2.5 bg-foreground/10 rounded w-1/2" />
      </div>
      <div className="h-2.5 bg-foreground/10 rounded w-1/3" />
      <div className="space-y-1 ml-3">
        <div className="h-2.5 bg-foreground/10 rounded w-2/3" />
      </div>
    </div>
  )
}

/** Fallback when no wizard session exists — shows path from ocean layout data */
function BasicInfo({ node }: { node: NodePanelData }) {
  const { t } = useTranslation('project')

  return (
    <div className="flex-1 overflow-y-auto">
      <section className="px-3 py-2 border-b border-border">
        <span className="text-xs text-foreground-muted font-mono truncate block">
          {node.modulePath}
        </span>
      </section>
      <div className="px-3 py-4">
        <p className="text-[10px] text-foreground-muted/50 text-center leading-relaxed">
          {t('node.runWizardHint')}
        </p>
      </div>
    </div>
  )
}

function EmptyState() {
  const { t } = useTranslation('project')

  return (
    <div className="flex-1 flex items-center justify-center px-3">
      <p className="text-[10px] text-foreground-muted/60">{t('node.noModuleData')}</p>
    </div>
  )
}
