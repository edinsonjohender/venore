// =============================================================================
// highlightStore — Per-node elevation level for the focus/highlight system.
// =============================================================================
// Today only level 0 (ground) and level 1 (elevated for "selected island")
// are populated. The structure leaves room for additional levels later
// (e.g. 2 = direct neighbors, 3 = 2-hop, 4 = focus node) once the graph of
// edges between nodes exists.

import { create } from 'zustand'

/** Y offset in world units for each level. Add more entries as new tiers
 *  are introduced (drag-to-connect, neighbor highlight, etc). */
export const LEVEL_HEIGHTS: Record<number, number> = {
  0: 0,
  1: 1.5,
}

/** Opacity multiplier applied to nodes that are NOT elevated while a
 *  highlight is active. 1.0 = no dimming (default when nothing is selected). */
export const DIM_FACTOR = 0.25

interface HighlightState {
  elevations: Map<string, number>
  setElevations: (next: Map<string, number>) => void
  clear: () => void
}

const EMPTY_MAP: ReadonlyMap<string, number> = new Map()

export const useHighlightStore = create<HighlightState>()((set) => ({
  elevations: EMPTY_MAP as Map<string, number>,
  setElevations: (next) => set({ elevations: next }),
  clear: () =>
    set((s) => (s.elevations.size === 0 ? s : { elevations: new Map() })),
}))
