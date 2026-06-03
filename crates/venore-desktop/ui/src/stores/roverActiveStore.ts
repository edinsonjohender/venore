// =============================================================================
// roverActiveStore — drives `frameloop="always"` while decorators animate
// =============================================================================
// Two independent signals any source can claim. The canvas keeps the
// continuous frame loop running while either is true:
//
//   - `isActive`             → at least one moving cursor (the singleton rover
//                              OR any Ocean Current) is hopping between cells.
//                              Tracked as a SET of source ids, not a single
//                              boolean, so several cursors can be active at
//                              once: one going idle (e.g. the index current)
//                              must not stop the frame loop while another (e.g.
//                              the staleness current) is still gliding.
//   - `hasAnimatedDecorators` → at least one node has an active state, so an
//                                animated decorator (OverflowHalo, etc.) is
//                                mounted. Without continuous frames its
//                                useFrame loop freezes between camera moves.
//
// Mirrors the pattern used by nodeDragStateStore / dragPreviewStore.

import { create } from 'zustand'

interface RoverActiveState {
  /** Ids of sources currently claiming continuous frames (rover + currents). */
  activeSources: Set<string>
  /** Derived: `activeSources.size > 0`. Read by the canvas. */
  isActive: boolean
  hasAnimatedDecorators: boolean
  /** Claim/release continuous frames for one source (by stable id). */
  setActiveSource: (id: string, active: boolean) => void
  setHasAnimatedDecorators: (next: boolean) => void
}

export const useRoverActiveStore = create<RoverActiveState>()((set) => ({
  activeSources: new Set<string>(),
  isActive: false,
  hasAnimatedDecorators: false,
  setActiveSource: (id, active) =>
    set((state) => {
      const next = new Set(state.activeSources)
      if (active) next.add(id)
      else next.delete(id)
      return { activeSources: next, isActive: next.size > 0 }
    }),
  setHasAnimatedDecorators: (next) => set({ hasAnimatedDecorators: next }),
}))
