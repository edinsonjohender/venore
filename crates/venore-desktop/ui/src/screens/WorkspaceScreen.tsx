// =============================================================================
// WorkspaceScreen - Active project workspace
// =============================================================================
// The main screen after opening a project.
// Renders TitleBar + WorkspaceLayout with canvas and toolbar.
// Includes context auto-updater polling and dialogs.

import { useState, useEffect } from 'react'

import { TitleBar } from '@/components/TitleBar'
import { TitleBarMenus } from '@/components/TitleBarMenus'
import { WorkspaceLayout } from '@/components/workspace'
import { StatusBar } from '@/components/workspace/StatusBar'
import { UpdateReportDialog } from '@/components/workspace/panels/updater/UpdateReportDialog'
import { UpdateResultDialog } from '@/components/workspace/panels/updater/UpdateResultDialog'
import { useUpdateChecker } from '@/hooks/useUpdateChecker'
import { useMeshInit } from '@/hooks/useMeshInit'
import { useUpdaterStore } from '@/stores/updaterStore'
import { useAIConfigStore } from '@/stores/aiConfigStore'
import { useWorkspaceFeatureStore, useFeatureEnabled } from '@/stores/workspaceFeatureStore'
import { useCanvasTabStore } from '@/stores/canvasTabStore'
import { useAutoDisconnectAiOnContextChange } from '@/hooks/useAutoDisconnectAiOnContextChange'
import type { ProjectType } from '@/stores/workspaceFeatureStore'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface WorkspaceScreenProps {
  projectPath: string
  projectId?: string
  projectType?: ProjectType
}

// -----------------------------------------------------------------------------
// Main Component
// -----------------------------------------------------------------------------

export function WorkspaceScreen({ projectPath, projectId, projectType = 'code' }: WorkspaceScreenProps) {
  const [showReport, setShowReport] = useState(false)
  const [showResult, setShowResult] = useState(false)

  const updaterEnabled = useFeatureEnabled('updater')

  // Initialize project type in stores on mount / project switch
  useEffect(() => {
    useWorkspaceFeatureStore.getState().setProjectType(projectType)
    useCanvasTabStore.getState().resetForProjectType(projectType)
  }, [projectType])

  // AI attachments are conversation-scoped; drop them when the user
  // switches projects or chat sessions so stale entries don't bleed
  // across contexts.
  useAutoDisconnectAiOnContextChange(projectPath)

  // Start polling for updates (only for code projects)
  useUpdateChecker(updaterEnabled ? projectPath : undefined)

  // Auto-register in mesh for multi-instance discovery
  useMeshInit(projectPath, projectId)

  const report = useUpdaterStore((s) => s.updateReport)
  const runUpdate = useUpdaterStore((s) => s.runUpdate)
  const completeUpdate = useUpdaterStore((s) => s.completeUpdate)
  const taskSettings = useAIConfigStore((s) => s.taskSettings)

  const handleRegenerate = (moduleNames: string[]) => {
    const settings = taskSettings?.onboarding
    if (!settings?.provider || !settings?.model) {
      console.warn('[WorkspaceScreen] No AI config for onboarding — cannot regenerate')
      return
    }
    setShowReport(false)
    setShowResult(true)
    runUpdate(
      projectPath,
      moduleNames,
      settings.provider,
      settings.model,
      'normal',
      report?.latest_commit ?? '',
    )
  }

  const handleMarkSynced = () => {
    if (report?.latest_commit) {
      completeUpdate(projectPath, report.latest_commit)
    }
    setShowResult(false)
  }

  return (
    <div className="h-full w-full flex flex-col bg-background select-none">
      <TitleBar><TitleBarMenus projectPath={projectPath} projectId={projectId} /></TitleBar>
      <WorkspaceLayout projectPath={projectPath} projectId={projectId} />
      <StatusBar projectPath={projectPath} onShowUpdater={updaterEnabled ? () => setShowReport(true) : undefined} />

      {updaterEnabled && (
        <>
          <UpdateReportDialog
            open={showReport}
            onClose={() => setShowReport(false)}
            onRegenerate={handleRegenerate}
          />
          <UpdateResultDialog
            open={showResult}
            onClose={() => setShowResult(false)}
            onMarkSynced={handleMarkSynced}
          />
        </>
      )}
    </div>
  )
}
