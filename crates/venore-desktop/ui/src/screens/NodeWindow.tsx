// =============================================================================
// NodeWindow — Pop-out OS window hosting the node Logbook
// =============================================================================
// Same NodeLogbook used by FloatingNodePanel, mounted at full window size.
// Lazy-fetches data from the backend (no localStorage handoff required —
// the logbook reads directly from the persisted layout).
//
// AI connection: this window mirrors the cross-window registry through
// `useAiConnectionsBootstrap` and exposes a Sparkles toggle on the title
// bar. Toggling here updates the same backend record the main window sees,
// so a connection that was active before the pop-out remains active and
// can be turned off from either side.

import { useEffect } from 'react'
import { Box, PictureInPicture2, Sparkles } from 'lucide-react'
import { emitTo } from '@tauri-apps/api/event'
import { Window } from '@tauri-apps/api/window'
import { TitleBar } from '@/components/TitleBar'
import { WindowControlButton } from '@/components/WindowControls'
import { NodeLogbook } from '@/components/workspace/panels/node/NodeLogbook'
import type { NodePanelData } from '@/stores/nodeFloatingStore'
import type { OceanNodeVariant } from '@/lib/tauri'
import { useAIConnection } from '@/hooks/useAIConnection'
import { useAiConnectionsBootstrap } from '@/hooks/useAiConnectionsBootstrap'
import { cn } from '@/lib/utils'
import { useMemo } from 'react'
import type { AiConnectionTarget } from '@/lib/tauri'

interface NodeWindowProps {
  projectPath: string
  moduleId: string
  moduleName: string
  nodeVariant: OceanNodeVariant
}

export function NodeWindow({ projectPath, moduleId, moduleName, nodeVariant }: NodeWindowProps) {
  // Mirror the backend AI-connection registry into this window's local store
  // so toggles and rainbow-border state stay in sync with the main window.
  useAiConnectionsBootstrap()

  // Same connection id the in-app FloatingNodePanel uses, so the entry is
  // shared 1:1 between both surfaces. Variant decides which resolver path
  // the chat backend takes — knowledge nodes inline their sections, code
  // modules pull from .context.md on disk.
  const connectionId = `node:${moduleId}`
  const target = useMemo<AiConnectionTarget>(() => {
    if (nodeVariant === 'module' || nodeVariant === 'buoy' || nodeVariant === 'cylinder') {
      return {
        kind: 'code_module',
        project_path: projectPath,
        module_name: moduleName,
        // Pop-out window doesn't carry modulePath — the resolver will
        // skip with a warning if .context.md isn't found there.
        module_path: '',
      }
    }
    return {
      kind: 'knowledge_node',
      project_path: projectPath,
      node_id: moduleId,
      display_name: moduleName,
    }
  }, [projectPath, moduleId, moduleName, nodeVariant])
  const { isActive, toggle } = useAIConnection(connectionId, target)

  // Build the same NodePanelData shape the in-app panel consumes. The layout
  // service is queried by id, so position fields aren't needed here.
  const node: NodePanelData = {
    projectPath,
    moduleId,
    moduleName,
    modulePath: '',
    nodeVariant,
  }

  // Tell the main window when this pop-out is about to close so it can drop
  // the node from `useNodePopoutStore` and the canvas overlay clears the
  // "EN USO" perimeter. Covers OS X-button, alt-F4, etc. The dock-back path
  // emits the same event to keep state coherent regardless of close source.
  useEffect(() => {
    const win = Window.getCurrent()
    const unlistenPromise = win.onCloseRequested(async () => {
      try {
        await emitTo('main', 'node-popout-closed', { projectPath, moduleId })
      } catch (err) {
        console.error('Failed to emit popout-closed event:', err)
      }
    })
    return () => {
      unlistenPromise.then((fn) => fn()).catch(() => {})
    }
  }, [projectPath, moduleId])

  // Send the node data back to the main window so it can reopen the logbook
  // as a floating panel, then destroy this OS window. Mirrors openNodeWindow
  // in the opposite direction.
  //
  // We use `destroy()` instead of `close()` so the window's `onCloseRequested`
  // handler — which exists for X-close / alt-F4 to emit `node-popout-closed`
  // (= "drop the AI connection") — does NOT fire on dock-back. Otherwise the
  // active connection would be unregistered the moment the user docked back,
  // and the next Sparkles click would flash on then revert because the
  // backend toggle is a no-op on a missing entry.
  const handleDockBack = async () => {
    try {
      await emitTo('main', 'node-popout-dock', node)
    } catch (err) {
      console.error('Failed to emit dock-back event:', err)
    }
    try {
      await Window.getCurrent().destroy()
    } catch (err) {
      console.error('Failed to destroy pop-out window:', err)
    }
  }

  const sparklesButton = isActive ? (
    <div
      data-connection-id={connectionId}
      className="rainbow-border cursor-pointer"
      onClick={toggle}
      title="Desconectar de la IA"
    >
      <div className="flex items-center justify-center w-6 h-6 rounded-lg bg-background-secondary">
        <Sparkles className="w-3.5 h-3.5 text-foreground" />
      </div>
    </div>
  ) : (
    <button
      type="button"
      data-connection-id={connectionId}
      onClick={toggle}
      title="Conectar a la IA"
      className={cn(
        'inline-flex items-center justify-center w-6 h-6 rounded',
        'text-foreground-muted hover:bg-foreground/10 hover:text-foreground transition-colors',
      )}
    >
      <Sparkles className="w-3.5 h-3.5" />
    </button>
  )

  const titleContent = (
    <div className="flex items-center h-full shrink-0 min-w-0 gap-2 pl-2">
      <Box className="w-3.5 h-3.5 text-foreground-muted" />
      <span className="text-xs text-foreground-muted select-none truncate">{moduleName}</span>
    </div>
  )

  const rightActions = (
    <>
      {sparklesButton}
      <WindowControlButton
        onClick={handleDockBack}
        title="Devolver a la app"
        aria-label="Devolver a la app"
        icon={<PictureInPicture2 className="w-3.5 h-3.5" />}
      />
    </>
  )

  return (
    <div className="h-screen w-screen flex flex-col bg-background overflow-hidden">
      <TitleBar rightActions={rightActions}>{titleContent}</TitleBar>
      <div className="flex-1 min-h-0 flex flex-col">
        <NodeLogbook node={node} />
      </div>
    </div>
  )
}
