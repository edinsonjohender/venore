// =============================================================================
// NodeHeaderActions — AI connection + pop-out actions for the floating detail
// =============================================================================

import { useEffect, useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { ExternalLink, Sparkles } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { tauriApi, type AiConnectionTarget } from '@/lib/tauri'
import { useAIConnectionStore } from '@/stores/aiConnectionStore'
import { useNodeFloatingStore } from '@/stores/nodeFloatingStore'
import { useNodePopoutStore } from '@/stores/nodePopoutStore'
import { usePanelStore } from '@/stores/panelStore'
import type { NodePanelData } from '@/stores/nodeFloatingStore'

interface NodeHeaderActionsProps {
  panelId: string
  data: NodePanelData
}

export function NodeHeaderActions({ panelId, data }: NodeHeaderActionsProps) {
  const { t } = useTranslation('project')
  const isActive = useAIConnectionStore(
    (s) => s.connections[panelId]?.active ?? false,
  )
  const registerConnection = useAIConnectionStore((s) => s.registerConnection)
  const toggleConnection = useAIConnectionStore((s) => s.toggleConnection)
  const setMode = usePanelStore((s) => s.setMode)

  // Build the typed attachment payload. `module` variant points at code on
  // disk (uses .context.md); knowledge_node and lighthouse share the same
  // node-id-based resolution path.
  const target = useMemo<AiConnectionTarget>(() => {
    const variant = data.nodeVariant ?? 'module'
    if (variant === 'module' || variant === 'buoy' || variant === 'cylinder') {
      return {
        kind: 'code_module',
        project_path: data.projectPath,
        module_name: data.moduleName,
        module_path: data.modulePath,
      }
    }
    return {
      kind: 'knowledge_node',
      project_path: data.projectPath,
      node_id: data.moduleId,
      display_name: data.moduleName,
    }
  }, [data.projectPath, data.moduleName, data.modulePath, data.moduleId, data.nodeVariant])

  // Register the connection (idempotent in the store + backend) so the
  // entry exists with its payload as soon as the panel mounts. Re-runs if
  // the panel switches identity (rare).
  useEffect(() => {
    registerConnection(panelId, target)
  }, [panelId, registerConnection, target])

  const handleAIClick = () => {
    toggleConnection(panelId)
    if (!isActive) {
      setMode('chat', 'docked')
    }
  }

  const handlePopout = () => {
    tauriApi
      .openNodeWindow(
        data.projectPath,
        data.moduleId,
        data.moduleName,
        data.nodeVariant ?? 'module',
      )
      .then(() => {
        // The OS window now owns the editor — close the in-app floating
        // panel and register the node in the popout store so the canvas
        // overlay still treats it as "in use". `unregisterAi: false` lets
        // the AI connection survive the in-app unmount so the NodeWindow
        // can pick it up.
        useNodeFloatingStore.getState().closePanel(panelId, { unregisterAi: false })
        useNodePopoutStore.getState().add(data.moduleId)
      })
      .catch((err) => console.error('Failed to open node window:', err))
  }

  return (
    <div className="flex items-center gap-0.5">
      <Button
        variant="ghost"
        size="icon"
        className="h-6 w-6"
        onClick={handlePopout}
        title="Abrir en ventana emergente"
      >
        <ExternalLink className="w-3.5 h-3.5" />
      </Button>
      <div className="flex items-center" data-connection-id={panelId}>
        {isActive ? (
          <div className="rainbow-border" onClick={handleAIClick} title={t('nodeLogbookActions.disconnectAI')}>
            <div className="flex items-center justify-center w-6 h-6 rounded-lg bg-background-secondary cursor-pointer">
              <Sparkles className="w-3.5 h-3.5 text-foreground" />
            </div>
          </div>
        ) : (
          <Button
            variant="ghost"
            size="icon"
            className="h-6 w-6"
            onClick={handleAIClick}
            title={t('nodeLogbookActions.connectToAI')}
          >
            <Sparkles className="w-3.5 h-3.5" />
          </Button>
        )}
      </div>
    </div>
  )
}
