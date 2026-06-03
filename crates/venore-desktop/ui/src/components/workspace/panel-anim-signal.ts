// =============================================================================
// Panel animation signal — tells R3F canvas when panels are animating
// =============================================================================
// Ref-counted: multiple panels can animate simultaneously.
// Uses useSyncExternalStore pattern (same as ocean-mode.ts).

let activeCount = 0
const listeners = new Set<() => void>()

function notify() {
  listeners.forEach(fn => fn())
}

export function startPanelAnim() {
  activeCount++
  if (activeCount === 1) notify() // 0 → 1 transition
}

export function endPanelAnim() {
  activeCount = Math.max(0, activeCount - 1)
  if (activeCount === 0) notify() // 1 → 0 transition
}

export function getIsPanelAnimating(): boolean {
  return activeCount > 0
}

export function subscribePanelAnim(listener: () => void) {
  listeners.add(listener)
  return () => { listeners.delete(listener) }
}
