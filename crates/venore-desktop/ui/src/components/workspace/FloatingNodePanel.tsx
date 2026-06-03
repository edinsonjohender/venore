// =============================================================================
// FloatingNodePanel — One floating Logbook instance for an ocean node
// =============================================================================

import { useMemo } from 'react'
import { Box } from 'lucide-react'
import { useFloatingPanel } from '@/hooks/useFloatingPanel'
import { useAIConnection } from '@/hooks/useAIConnection'
import { useNodeFloatingStore, type NodePanelData } from '@/stores/nodeFloatingStore'
import { FloatingPanelWrapper } from './FloatingPanelWrapper'
import { NodeLogbook } from './panels/node/NodeLogbook'
import { NodeHeaderActions } from './panels/node/NodeHeaderActions'
import type { AiConnectionTarget } from '@/lib/tauri'

interface FloatingNodePanelProps {
  panelId: string
  data: NodePanelData
  zIndex: number
  canvasZoneRef: React.RefObject<HTMLDivElement | null>
  initialPosition: { x: number; y: number }
}

// Knowledge logbooks need room for sidebar + Monaco editor; the
// code-representational variants (module / buoy / cylinder) only render the
// compact info panel and look sparse at full size.
const SIZE_BY_VARIANT: Record<NonNullable<NodePanelData['nodeVariant']>, { width: number; height: number }> = {
  module: { width: 340, height: 420 },
  knowledge_node: { width: 800, height: 600 },
  lighthouse: { width: 800, height: 600 },
  buoy: { width: 340, height: 420 },
  cylinder: { width: 340, height: 420 },
}

const GEOM_KEY_PREFIX = 'node-panel-geom:'

interface PanelGeometry {
  x: number
  y: number
  width: number
  height: number
}

function readGeom(moduleId: string): PanelGeometry | null {
  try {
    const raw = localStorage.getItem(GEOM_KEY_PREFIX + moduleId)
    if (!raw) return null
    const parsed = JSON.parse(raw)
    if (
      typeof parsed.x === 'number' &&
      typeof parsed.y === 'number' &&
      typeof parsed.width === 'number' &&
      typeof parsed.height === 'number'
    ) {
      return parsed
    }
  } catch {
    // Ignore corrupt entries — fall back to defaults.
  }
  return null
}

function writeGeom(moduleId: string, geom: PanelGeometry) {
  try {
    localStorage.setItem(GEOM_KEY_PREFIX + moduleId, JSON.stringify(geom))
  } catch {
    // Quota or disabled storage — silently drop.
  }
}

export function FloatingNodePanel({
  panelId,
  data,
  zIndex,
  canvasZoneRef,
  initialPosition,
}: FloatingNodePanelProps) {
  // Connection target depends on the node kind: code-representational
  // variants (module / buoy / cylinder) resolve via .context.md on disk,
  // knowledge variants (faro / knowledge_node) inline their sections.
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
  useAIConnection(panelId, target)

  // Restore previous geometry for this node if any; otherwise fall back to
  // the variant-based default size + cascade position from the parent.
  const stored = readGeom(data.moduleId)
  const initialSize = stored
    ? { width: stored.width, height: stored.height }
    : SIZE_BY_VARIANT[data.nodeVariant ?? 'module']
  const startPos = stored ? { x: stored.x, y: stored.y } : initialPosition

  const persist = (pos: { x: number; y: number }, sz: { width: number; height: number }) => {
    writeGeom(data.moduleId, { x: pos.x, y: pos.y, width: sz.width, height: sz.height })
  }

  const { position, size, handleDragStart, handleResizeStart } = useFloatingPanel({
    initialSize,
    initialPosition: startPos,
    boundsRef: canvasZoneRef,
    onDragEnd: persist,
    onResizeEnd: persist,
  })

  const closePanel = useNodeFloatingStore((s) => s.closePanel)
  const bringToFront = useNodeFloatingStore((s) => s.bringToFront)

  return (
    <FloatingPanelWrapper
      title={data.moduleName}
      icon={<Box className="w-3.5 h-3.5" />}
      headerActions={<NodeHeaderActions panelId={panelId} data={data} />}
      left={position.x}
      top={position.y}
      width={size.width}
      height={size.height}
      zIndex={zIndex}
      hideDock
      onFocus={() => bringToFront(panelId)}
      onDragStart={handleDragStart}
      onResizeStart={handleResizeStart}
      onDock={() => {}}
      onClose={() => closePanel(panelId)}
    >
      <NodeLogbook node={data} />
    </FloatingPanelWrapper>
  )
}
