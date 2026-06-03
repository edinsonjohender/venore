// =============================================================================
// Ocean Node — Shared types for layered node architecture
// =============================================================================

export type LayerType = 'context' | 'tests' | 'documentation' | 'connections' | 'status'
export type LayerStatus = 'complete' | 'partial' | 'missing'
export type NodeStatus = 'fresh' | 'stale' | 'missing' | 'loading'

export interface NodeLayer {
  type: LayerType
  status: LayerStatus
  details?: Record<string, unknown>
}

/** Noop raycast — makes a mesh invisible to the R3F raycaster. */
export const noRaycast = () => {}

/** Fallback default when backend sends no layers. */
export const DEFAULT_LAYERS: NodeLayer[] = [
  { type: 'context', status: 'missing' },
]
