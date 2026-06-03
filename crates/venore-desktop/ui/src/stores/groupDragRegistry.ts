// =============================================================================
// groupDragRegistry — Imperative registry of node Group refs.
// =============================================================================
// Non-reactive: writes here do NOT trigger re-renders. Used during a group
// drag so the "anchor" node can imperatively move every other selected
// node's THREE.Group at 60Hz without paying the React reconciliation cost.
//
// Lifecycle: each BaseNode registers its ref on mount (key = node id) and
// removes itself on unmount.

import type { RefObject } from 'react'
import type { Group } from 'three'

const registry = new Map<string, RefObject<Group>>()

export const groupDragRegistry = {
  register(id: string, ref: RefObject<Group>): void {
    registry.set(id, ref)
  },
  unregister(id: string): void {
    registry.delete(id)
  },
  /** Returns the ref for a node, or undefined if not currently mounted. */
  get(id: string): RefObject<Group> | undefined {
    return registry.get(id)
  },
  /** Returns refs for the given ids, skipping any that aren't mounted. */
  getMany(ids: Iterable<string>): RefObject<Group>[] {
    const result: RefObject<Group>[] = []
    for (const id of ids) {
      const ref = registry.get(id)
      if (ref) result.push(ref)
    }
    return result
  },
}
