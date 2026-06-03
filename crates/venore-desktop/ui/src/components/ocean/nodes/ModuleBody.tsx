// =============================================================================
// ModuleBody — Stack of LayerMesh components for module-type nodes
// =============================================================================

import { memo } from 'react'
import { LayerMesh } from './LayerMesh'
import type { NodeLayer } from './types'

interface ModuleBodyProps {
  layers: NodeLayer[]
  isHovered: boolean
}

export const ModuleBody = memo(function ModuleBody({ layers, isHovered }: ModuleBodyProps) {
  return (
    <>
      {layers.map((layer, index) => (
        <LayerMesh
          key={index}
          layer={layer}
          index={index}
          totalLayers={layers.length}
          isHovered={isHovered}
        />
      ))}
    </>
  )
})
