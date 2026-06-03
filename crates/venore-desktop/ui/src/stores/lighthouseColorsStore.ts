// =============================================================================
// Lighthouse Colors Store — Per-lighthouse color overrides
// =============================================================================
// Backend persists overrides; this store mirrors them in memory so any 3D
// consumer (IslandTiles, NodeSelectionOutline, future visuals) can resolve
// the effective island color without prop-drilling.
// Source of truth: OceanLayoutResponse.lighthouse_colors. OceanNodes calls
// `setOverrides` after every layout fetch.

import { create } from 'zustand'

interface LighthouseColorsState {
  overrides: Record<string, string>
  setOverrides: (next: Record<string, string>) => void
}

export const useLighthouseColorsStore = create<LighthouseColorsState>((set) => ({
  overrides: {},
  setOverrides: (next) => set({ overrides: next }),
}))
