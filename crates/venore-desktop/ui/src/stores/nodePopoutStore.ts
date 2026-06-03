// =============================================================================
// Node Popout Store — tracks which nodes are open as OS pop-out windows
// =============================================================================
// Pop-out panels close the in-app floating panel (see NodeHeaderActions),
// so the floating store no longer "knows" about them. This tiny parallel
// store keeps the truth: a node is "in use" if its floating panel is open
// OR it has been popped out into its own OS window.
//
// Membership is keyed by moduleId. Cross-project collisions are extremely
// unlikely in dev (one project at a time) and in prod (each project owns
// its own windows). Adding `projectPath` to the key would defend against
// it but bloat the store API for no real-world gain.

import { create } from 'zustand'

interface NodePopoutState {
  ids: Set<string>
  add: (moduleId: string) => void
  remove: (moduleId: string) => void
}

export const useNodePopoutStore = create<NodePopoutState>((set) => ({
  ids: new Set<string>(),
  add: (moduleId) =>
    set((s) => {
      if (s.ids.has(moduleId)) return s
      const next = new Set(s.ids)
      next.add(moduleId)
      return { ids: next }
    }),
  remove: (moduleId) =>
    set((s) => {
      if (!s.ids.has(moduleId)) return s
      const next = new Set(s.ids)
      next.delete(moduleId)
      return { ids: next }
    }),
}))
