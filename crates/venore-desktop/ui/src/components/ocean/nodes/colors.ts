// =============================================================================
// Ocean Node — Color constants for layers, bodies, and status indicators
// =============================================================================

import type { LayerType, LayerStatus, NodeStatus } from './types'

// Color per layer type (for stripe + top indicator)
export const LAYER_COLORS: Record<LayerType, string> = {
  context: '#8b5cf6',       // Violet
  tests: '#10b981',         // Green
  documentation: '#06b6d4', // Cyan
  connections: '#3b82f6',   // Blue
  status: '#f59e0b',        // Amber
}

// Tint color per layer status (blended into the stripe gradient)
// complete = no tint (use layer type color as-is)
// partial = shift hue toward amber to signal incomplete
export const LAYER_STATUS_TINT: Record<Exclude<LayerStatus, 'missing'>, string | null> = {
  complete: null,              // no tint — pure layer color
  partial: '#fbbf24',          // amber
}

// Body colors (migrated from ocean-config.ts)
export const NODE_COLORS = {
  body: '#3d3d52',
  bodyLight: '#4d4d65',
  edge: '#6b6b8a',
} as const

// Node status colors — glow indicates context freshness
export const STATUS_COLORS: Record<NodeStatus, string> = {
  fresh: '#4ade80',     // Green
  stale: '#fbbf24',     // Amber
  missing: '#f87171',   // Red
  loading: '#6b7280',   // Gray (neutral, avoids green flash on load)
}
