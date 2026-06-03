// =============================================================================
// Knowledge Board — Hex Colors
// =============================================================================
// Phase → SVG color mapping. Uses hex literals (not Tailwind) because SVG
// fill/stroke attributes need raw color values.

import type { HexPhase } from './mock-data'

// -----------------------------------------------------------------------------
// Phase colors
// -----------------------------------------------------------------------------

export const PHASE_COLORS: Record<HexPhase | 'dead-end', string> = {
  discover: '#01e8a2',  // brand teal — exploring
  define:   '#3b82f6',  // blue — defining scope
  validate: '#f59e0b',  // amber — validating hypothesis
  conclude: '#22c55e',  // green — decided/proven
  'dead-end': '#52525b', // gray — discarded
}

export const PHASE_LABELS: Record<HexPhase | 'dead-end', string> = {
  discover: 'Discover',
  define:   'Define',
  validate: 'Validate',
  conclude: 'Conclude',
  'dead-end': 'Dead End',
}

// -----------------------------------------------------------------------------
// Opacity helpers
// -----------------------------------------------------------------------------

/** Fill opacity: more progress → more opaque. Range [0.10, 0.50] */
export function fillOpacityForPercentage(pct: number): number {
  const clamped = Math.max(0, Math.min(100, pct))
  return 0.10 + (clamped / 100) * 0.40
}

/** Stroke opacity: more progress → more opaque. Range [0.40, 0.90] */
export function strokeOpacityForPercentage(pct: number): number {
  const clamped = Math.max(0, Math.min(100, pct))
  return 0.40 + (clamped / 100) * 0.50
}
