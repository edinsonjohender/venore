// =============================================================================
// NodeDecoratorOverlay — sibling layer that paints state decorators
// =============================================================================
// Renders the winning decorators for a single node's `states[]`. Lives
// OUTSIDE the OceanNode group on purpose: the BaseNode wrapper traverses
// its children to apply dim/elevation animation. Putting decorators inside
// would dim them with the host node — not what we want for an alert
// (overflow halo should stay loud even when its node is dimmed).

import { memo, useMemo } from 'react'
import { cellToWorld } from './ocean-config'
import { computeContentHeight } from './OceanNode'
import { resolveDecorators, DECORATOR_REGISTRY } from './decorators/registry'
import { SecurityPerimeter } from './decorators/SecurityPerimeter'
import { useIsNodeOpen } from '@/stores/openNodes'
import type { NodeLayer } from './nodes/types'
import type { NodeStateDto, OceanNodePosition } from '@/lib/tauri'

interface NodeDecoratorOverlayProps {
  node: OceanNodePosition
  /** Same layers the OceanNode renders with — needed by computeContentHeight
   *  for `module` / `knowledge_node` variants whose height scales with the
   *  layer count. Other variants ignore this. */
  layers: NodeLayer[]
  /** When set, merges a synthetic `kind: 'stale'` state into the node's
   *  states before the registry resolves them. Drives the StaleBadge
   *  decorator. Computed by the canvas root from `getStaleModules` and
   *  passed in per node. */
  staleSeverity?: 'info' | 'warning'
}

export const NodeDecoratorOverlay = memo(function NodeDecoratorOverlay({
  node,
  layers,
  staleSeverity,
}: NodeDecoratorOverlayProps) {
  // "In use" perimeter — a node is "in use" if its floating panel is open
  // OR it's been popped out into its own OS window. The unified hook hides
  // both stores so other call sites don't have to know about the split.
  // Persists across AI write accept/discard because neither path
  // auto-closes a panel.
  const isInUse = useIsNodeOpen(node.module_id)

  // Combine backend-shipped states with the client-synthesized stale state
  // (if any). The registry handles slot/priority — stale uses `billboard`,
  // which doesn't collide with backend slots (`halo`, `cap`), so both can
  // render at once.
  const effectiveStates: NodeStateDto[] = useMemo(() => {
    const backend = node.states ?? []
    if (!staleSeverity) return backend
    const synthetic: NodeStateDto = {
      kind: 'stale',
      severity: staleSeverity,
      computed_at: Date.now() / 1000,
      payload: {},
    }
    return [...backend, synthetic]
  }, [node.states, staleSeverity])

  const hasStates = effectiveStates.length > 0
  if (!hasStates && !isInUse) return null

  const winners = hasStates ? resolveDecorators(effectiveStates) : []

  const [x, , z] = cellToWorld(node.col, node.row)
  const contentHeight = computeContentHeight(node.node_variant, layers)

  return (
    <group position={[x, 0, z]}>
      {winners.map((state) => {
        const entry = DECORATOR_REGISTRY[state.kind]
        if (!entry) return null
        return (
          <group key={`${node.module_id}-${state.kind}`}>
            {entry.render(state, contentHeight)}
          </group>
        )
      })}
      {isInUse && (
        <SecurityPerimeter
          color="#3b82f6"
          text="EN USO"
          textColor="#ffffff"
          tapeCount={2}
          baseHeight={Math.max(0.3, contentHeight * 0.25)}
          spacing={Math.max(0.4, contentHeight * 0.4)}
          intensity={0.9}
        />
      )}
    </group>
  )
})
