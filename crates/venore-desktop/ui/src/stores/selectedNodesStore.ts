// =============================================================================
// selectedNodesStore — UI-only set of currently selected ocean nodes.
// =============================================================================
// Purely client-side state. The backend doesn't know which nodes the user
// has highlighted — selection is ephemeral and doesn't persist across reloads.

import { create } from 'zustand'

interface SelectedNodesState {
  ids: Set<string>
  set: (ids: string[]) => void
  add: (ids: string[]) => void
  toggle: (id: string) => void
  remove: (id: string) => void
  clear: () => void
}

export const useSelectedNodesStore = create<SelectedNodesState>()((set) => ({
  ids: new Set<string>(),
  set: (ids) => set({ ids: new Set(ids) }),
  add: (ids) =>
    set((s) => {
      const next = new Set(s.ids)
      for (const id of ids) next.add(id)
      return { ids: next }
    }),
  toggle: (id) =>
    set((s) => {
      const next = new Set(s.ids)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return { ids: next }
    }),
  remove: (id) =>
    set((s) => {
      if (!s.ids.has(id)) return s
      const next = new Set(s.ids)
      next.delete(id)
      return { ids: next }
    }),
  clear: () => set((s) => (s.ids.size === 0 ? s : { ids: new Set<string>() })),
}))

/** Subscribe to whether a specific node is currently selected. */
export function useIsNodeSelected(id: string): boolean {
  return useSelectedNodesStore((s) => s.ids.has(id))
}
