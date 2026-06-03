// =============================================================================
// Knowledge Board — Hex Layout (Honeycomb by Phase)
// =============================================================================
// Flat-top hexagonal geometry + honeycomb grid grouped by phase.
// Each phase has SLOTS_PER_PHASE slots. Filled = data, empty = ghost.
// Designed to fill the canvas width with ~15 columns.

import type { Hexagon, HexPhase } from './mock-data'

// -----------------------------------------------------------------------------
// Constants
// -----------------------------------------------------------------------------

export const HEX_R = 1.8       // circumradius (flat-top)
const HEX_W = HEX_R * 2
const HEX_H = HEX_R * Math.sqrt(3)
const GAP = 0.3                // tiny gap between hexagons

const COL_STEP = HEX_W * 0.75 + GAP
const ROW_STEP = (HEX_H + GAP) * 0.5

const PHASE_GAP = 0.8          // vertical gap between phase bands
const TAB_WIDTH = 3            // phase tab width
const PADDING_LEFT = TAB_WIDTH + 0.5
const MAX_COLS = 20            // fill the width
const SLOTS_PER_PHASE = 100   // 20 cols x 5 rows

// Phase order
export const PHASE_ORDER: HexPhase[] = ['discover', 'define', 'validate', 'conclude']

// Total content width (used for viewBox)
export const CONTENT_WIDTH = PADDING_LEFT + MAX_COLS * COL_STEP + HEX_R + 2

// -----------------------------------------------------------------------------
// Geometry
// -----------------------------------------------------------------------------

export function hexPoints(cx: number, cy: number, r: number): string {
  const pts: string[] = []
  for (let i = 0; i < 6; i++) {
    const angleDeg = 60 * i
    const angleRad = (Math.PI / 180) * angleDeg
    pts.push(`${cx + r * Math.cos(angleRad)},${cy + r * Math.sin(angleRad)}`)
  }
  return pts.join(' ')
}

// -----------------------------------------------------------------------------
// Layout types
// -----------------------------------------------------------------------------

export interface LayoutNode {
  id: string
  cx: number
  cy: number
  ghost: boolean
}

export interface PhaseBand {
  phase: HexPhase
  y: number
  height: number
}

export interface LayoutResult {
  nodes: LayoutNode[]
  bands: PhaseBand[]
  width: number
  height: number
}

// -----------------------------------------------------------------------------
// Slot position
// -----------------------------------------------------------------------------

function slotPosition(index: number, bandY: number): { cx: number; cy: number } {
  const col = index % MAX_COLS
  const row = Math.floor(index / MAX_COLS)
  const isOddCol = col % 2 === 1

  const cx = PADDING_LEFT + col * COL_STEP + HEX_R
  const cy = bandY + HEX_H / 2 + row * (HEX_H + GAP) + (isOddCol ? ROW_STEP : 0)

  return { cx, cy }
}

// -----------------------------------------------------------------------------
// Layout
// -----------------------------------------------------------------------------

export function computeLayout(hexagons: Hexagon[]): LayoutResult {
  const byPhase = new Map<HexPhase, Hexagon[]>()
  for (const phase of PHASE_ORDER) byPhase.set(phase, [])
  for (const hex of hexagons) {
    const list = byPhase.get(hex.phase)
    if (list) list.push(hex)
  }

  const nodes: LayoutNode[] = []
  const bands: PhaseBand[] = []
  let currentY = 1

  const numRows = Math.ceil(SLOTS_PER_PHASE / MAX_COLS)
  const bandHeight = numRows * (HEX_H + GAP) + ROW_STEP

  for (const phase of PHASE_ORDER) {
    const phaseHexes = byPhase.get(phase) ?? []
    bands.push({ phase, y: currentY, height: bandHeight })

    for (let i = 0; i < SLOTS_PER_PHASE; i++) {
      const { cx, cy } = slotPosition(i, currentY)
      if (i < phaseHexes.length) {
        nodes.push({ id: phaseHexes[i].id, cx, cy, ghost: false })
      } else {
        nodes.push({ id: `ghost-${phase}-${i}`, cx, cy, ghost: true })
      }
    }

    currentY += bandHeight + PHASE_GAP
  }

  return {
    nodes,
    bands,
    width: CONTENT_WIDTH,
    height: currentY,
  }
}
