// =============================================================================
// nodeDragStateStore — flags whether a node drag is currently in progress.
// =============================================================================
// Used by OceanCanvas to disable MapControls during a node drag so the
// camera doesn't pan while the user is moving a node (or a group of nodes).

import { create } from 'zustand'

interface NodeDragState {
  isDragging: boolean
  setDragging: (next: boolean) => void
}

export const useNodeDragStateStore = create<NodeDragState>()((set) => ({
  isDragging: false,
  setDragging: (next) => set({ isDragging: next }),
}))
