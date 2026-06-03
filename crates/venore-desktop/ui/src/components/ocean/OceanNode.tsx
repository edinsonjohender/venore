// =============================================================================
// OceanNode — Entry point composing BaseNode + body variant
// =============================================================================
// Switches the body geometry based on the node variant:
//   - module / knowledge_node → ModuleBody (stack of layers)
//   - lighthouse              → LighthouseBody (tall pillar + lantern)
//   - buoy                    → BuoyNode (3 mini-buildings cluster)
//   - cylinder                → CylinderNode (stacked cylinders)

import { memo } from 'react'
import type { ThreeEvent } from '@react-three/fiber'
import { GRID_CONFIG } from './ocean-config'
import { BaseNode } from './nodes/BaseNode'
import { ModuleBody } from './nodes/ModuleBody'
import { LighthouseBody, LIGHTHOUSE_TOTAL_HEIGHT } from './nodes/LighthouseBody'
import { BuoyNode, calculateBuoyHeight } from './nodes/BuoyNode'
import { CylinderNode, calculateCylinderHeight } from './nodes/CylinderNode'
import type { NodeStatus, NodeLayer } from './nodes/types'
import type { OceanNodeVariant } from '@/lib/tauri'

export type { NodeStatus, NodeLayer }

export interface OceanNodeProps {
  id: string
  position: [number, number, number]
  label: string
  status: NodeStatus
  layers: NodeLayer[]
  variant: OceanNodeVariant
  onMove?: (id: string, newPosition: [number, number, number]) => Promise<boolean>
  onClick?: (id: string) => void
  onDoubleClick?: (id: string) => void
  onContextMenu?: (id: string, event: ThreeEvent<MouseEvent>, label: string) => void
  getAllNodes?: () => Array<{ id: string; col: number; row: number }>
}

function computeModuleHeight(layerCount: number): number {
  const { layerHeight, layerGap } = GRID_CONFIG
  return layerCount * (layerHeight + layerGap) - layerGap
}

/** Total visual height of the node's body for a given variant + layer set.
 *  Exported so siblings outside the BaseNode group (e.g. the decorator
 *  overlay) can scale wraps/halos to exactly the host's height. */
export function computeContentHeight(variant: OceanNodeVariant, layers: NodeLayer[]): number {
  switch (variant) {
    case 'lighthouse':
      return LIGHTHOUSE_TOTAL_HEIGHT
    case 'buoy':
      return calculateBuoyHeight()
    case 'cylinder':
      return calculateCylinderHeight()
    case 'module':
    case 'knowledge_node':
    default:
      return computeModuleHeight(layers.length)
  }
}

// Variants whose own geometry already carries the status meaning — BaseNode
// skips its small status-color square for these.
function carriesStatusInGeometry(variant: OceanNodeVariant): boolean {
  return variant === 'lighthouse' || variant === 'buoy' || variant === 'cylinder'
}

export const OceanNode = memo(function OceanNode({
  id,
  position,
  label,
  status,
  layers,
  variant,
  onMove,
  onClick,
  onDoubleClick,
  onContextMenu,
  getAllNodes,
}: OceanNodeProps) {
  const contentHeight = computeContentHeight(variant, layers)

  return (
    <BaseNode
      id={id}
      position={position}
      label={label}
      status={status}
      contentHeight={contentHeight}
      hideStatusIndicator={carriesStatusInGeometry(variant)}
      onMove={onMove}
      onClick={onClick}
      onDoubleClick={onDoubleClick}
      onContextMenu={onContextMenu}
      getAllNodes={getAllNodes}
    >
      {({ isHovered }) => {
        switch (variant) {
          case 'lighthouse':
            return <LighthouseBody status={status} isHovered={isHovered} />
          case 'buoy':
            return <BuoyNode status={status} isHovered={isHovered} />
          case 'cylinder':
            return <CylinderNode status={status} isHovered={isHovered} />
          case 'module':
          case 'knowledge_node':
          default:
            return <ModuleBody layers={layers} isHovered={isHovered} />
        }
      }}
    </BaseNode>
  )
})
