// =============================================================================
// Hex Floating Store — Multi-instance floating panels for knowledge hexagons
// =============================================================================
// Same pattern as nodeFloatingStore. Each click on a hexagon opens its own
// floating panel. Clicking an already-open hexagon brings its panel to front.

import { create } from 'zustand'
import type { Hexagon, Evidence } from '@/components/workspace/canvas/knowledge/mock-data'
import { useAIConnectionStore } from './aiConnectionStore'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

export interface HexPanelData {
  hex: Hexagon
  evidence: Evidence[]
}

interface HexPanelInstance {
  panelId: string // 'hex:{hexId}'
  data: HexPanelData
  zIndex: number
}

interface HexFloatingState {
  panels: HexPanelInstance[]
  _zCounter: number
  openPanel: (data: HexPanelData) => void
  closePanel: (panelId: string) => void
  bringToFront: (panelId: string) => void
  closeAll: () => void
}

// -----------------------------------------------------------------------------
// Store
// -----------------------------------------------------------------------------

export const useHexFloatingStore = create<HexFloatingState>((set, get) => ({
  panels: [],
  _zCounter: 40,

  openPanel: (data) => {
    const panelId = `hex:${data.hex.id}`
    const { panels } = get()
    const existing = panels.find((p) => p.panelId === panelId)

    if (existing) {
      get().bringToFront(panelId)
      return
    }

    const nextZ = get()._zCounter + 1
    set({
      panels: [...panels, { panelId, data, zIndex: nextZ }],
      _zCounter: nextZ,
    })
  },

  closePanel: (panelId) => {
    useAIConnectionStore.getState().unregisterConnection(panelId)
    set((s) => ({ panels: s.panels.filter((p) => p.panelId !== panelId) }))
  },

  bringToFront: (panelId) => {
    const nextZ = get()._zCounter + 1
    set((s) => ({
      _zCounter: nextZ,
      panels: s.panels.map((p) =>
        p.panelId === panelId ? { ...p, zIndex: nextZ } : p,
      ),
    }))
  },

  closeAll: () => {
    const { panels } = get()
    const aiStore = useAIConnectionStore.getState()
    for (const p of panels) aiStore.unregisterConnection(p.panelId)
    set({ panels: [] })
  },
}))
