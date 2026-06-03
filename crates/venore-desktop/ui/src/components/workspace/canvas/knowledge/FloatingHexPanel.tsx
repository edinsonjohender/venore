// =============================================================================
// FloatingHexPanel — One floating panel instance for a single hexagon
// =============================================================================
// Same pattern as FloatingNodePanel — uses useFloatingPanel + FloatingPanelWrapper.

import { useMemo } from 'react'
import { Hexagon } from 'lucide-react'
import { useFloatingPanel } from '@/hooks/useFloatingPanel'
import { useAIConnection } from '@/hooks/useAIConnection'
import { useHexFloatingStore, type HexPanelData } from '@/stores/hexFloatingStore'
import { FloatingPanelWrapper } from '@/components/workspace/FloatingPanelWrapper'
import { HexPanelContent } from './HexPanelContent'
import { HexHeaderActions } from './HexHeaderActions'
import type { AiConnectionTarget } from '@/lib/tauri'

interface FloatingHexPanelProps {
  panelId: string
  data: HexPanelData
  zIndex: number
  boundsRef: React.RefObject<HTMLDivElement | null>
  initialPosition: { x: number; y: number }
  projectPath: string
}

const INITIAL_SIZE = { width: 280, height: 360 }

export function FloatingHexPanel({
  panelId,
  data,
  zIndex,
  boundsRef,
  initialPosition,
  projectPath,
}: FloatingHexPanelProps) {
  const target = useMemo<AiConnectionTarget>(
    () => ({
      kind: 'hexagon',
      project_path: projectPath,
      feature_id: data.hex.featureId,
      hexagon_id: data.hex.id,
      display_name: data.hex.title,
    }),
    [projectPath, data.hex.featureId, data.hex.id, data.hex.title],
  )
  useAIConnection(panelId, target)

  const { position, size, handleDragStart, handleResizeStart } = useFloatingPanel({
    initialSize: INITIAL_SIZE,
    initialPosition,
    boundsRef,
  })

  const closePanel = useHexFloatingStore((s) => s.closePanel)
  const bringToFront = useHexFloatingStore((s) => s.bringToFront)

  return (
    <FloatingPanelWrapper
      title={data.hex.title}
      icon={<Hexagon className="w-3.5 h-3.5" />}
      headerActions={<HexHeaderActions panelId={panelId} />}
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
      <HexPanelContent data={data} />
    </FloatingPanelWrapper>
  )
}
