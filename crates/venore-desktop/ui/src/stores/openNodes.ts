// =============================================================================
// Open nodes — unified surface over the floating + popout stores
// =============================================================================
// A node is "open" if it has an in-app floating panel OR a pop-out OS
// window. Both modes track membership in their own store; this module is
// the single helper everyone should reach for when:
//
//   - asking "is node X open anywhere?" → `useIsNodeOpen` (reactive),
//     `isNodeOpen` (static, e.g. inside event listeners),
//   - opening / focusing a node from any code path → `focusNodePanel`,
//     which routes to the right backing store / OS window.
//
// Without this, every call site has to know about both stores AND about
// the duplicate-spawning hazard when a node is already popped out.

import { tauriApi } from '@/lib/tauri'
import {
  useNodeFloatingStore,
  type NodePanelData,
} from './nodeFloatingStore'
import { useNodePopoutStore } from './nodePopoutStore'

/** Reactive — for components that should re-render when a node is opened
 *  or closed in either mode. */
export function useIsNodeOpen(moduleId: string): boolean {
  const floating = useNodeFloatingStore((s) =>
    s.panels.some((p) => p.panelId === `node:${moduleId}`),
  )
  const popout = useNodePopoutStore((s) => s.ids.has(moduleId))
  return floating || popout
}

/** Reactive — true when ANY node has an open panel anywhere (floating or
 *  popped-out). Used at the canvas root to drive `frameloop="always"`: the
 *  open-node `SecurityPerimeter` decorator animates via `useFrame`, so the
 *  render loop has to tick continuously while at least one panel is up. */
export function useHasOpenNodes(): boolean {
  const floatingCount = useNodeFloatingStore((s) =>
    s.panels.filter((p) => p.panelId.startsWith('node:')).length,
  )
  const popoutCount = useNodePopoutStore((s) => s.ids.size)
  return floatingCount + popoutCount > 0
}

/** Static, snapshot-style — for non-reactive code (event listeners,
 *  effects, etc) that just needs to ask "is it open right now?". */
export function isNodeOpen(moduleId: string): boolean {
  return (
    useNodeFloatingStore
      .getState()
      .panels.some((p) => p.panelId === `node:${moduleId}`) ||
    useNodePopoutStore.getState().ids.has(moduleId)
  )
}

/** Open or focus the panel for a node, routing to the right surface so we
 *  never end up with both an in-app floating panel AND a pop-out window
 *  for the same node.
 *
 *  - If the node is already popped out → ask the backend to focus the
 *    existing OS window (it already de-dups by label) and return.
 *  - If the floating panel is open → bring it to front.
 *  - Otherwise → open a new floating panel.
 */
export function focusNodePanel(data: NodePanelData): void {
  if (useNodePopoutStore.getState().ids.has(data.moduleId)) {
    // The Rust `open_node_window` command focuses an existing window
    // by label and skips creation, so it's safe to call again here.
    tauriApi
      .openNodeWindow(
        data.projectPath,
        data.moduleId,
        data.moduleName,
        data.nodeVariant ?? 'module',
      )
      .catch((err) => {
        console.error('Failed to focus existing pop-out window:', err)
      })
    return
  }
  // Idempotent in the floating store: opens new or bumps zIndex.
  useNodeFloatingStore.getState().openPanel(data)
}
