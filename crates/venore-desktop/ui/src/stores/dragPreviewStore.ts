// =============================================================================
// dragPreviewStore — Reactive state for the "destination cells" preview
// shown while a group of nodes is being dragged.
// =============================================================================
// Each entry has the cell coordinates plus an `ok` flag (true = free, false =
// blocked by a node outside the dragging group). DragPreviewTiles consumes
// this store and renders a translucent green or red tile per entry.

import { create } from 'zustand'

export interface DragPreviewCell {
  col: number
  row: number
  ok: boolean
}

interface DragPreviewState {
  isActive: boolean
  cells: DragPreviewCell[]
  setActive: (active: boolean) => void
  setCells: (cells: DragPreviewCell[]) => void
  clear: () => void
}

export const useDragPreviewStore = create<DragPreviewState>()((set) => ({
  isActive: false,
  cells: [],
  setActive: (active) => set({ isActive: active }),
  setCells: (cells) => set({ cells }),
  clear: () => set({ isActive: false, cells: [] }),
}))
