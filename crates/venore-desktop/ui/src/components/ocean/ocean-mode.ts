// =============================================================================
// Ocean editor mode — Navigate vs Move Node
// =============================================================================
// Simple state via useSyncExternalStore for zero-dependency reactivity.
// Keyboard: H = navigate, N = move-node

export type OceanMode = 'navigate' | 'move-node'

// -----------------------------------------------------------------------------
// Store
// -----------------------------------------------------------------------------

let mode: OceanMode = 'navigate'
const listeners = new Set<() => void>()

function notify() {
  listeners.forEach(fn => fn())
}

export function getOceanMode(): OceanMode {
  return mode
}

export function setOceanMode(m: OceanMode) {
  if (mode === m) return
  mode = m
  notify()
}

export function subscribeOceanMode(listener: () => void) {
  listeners.add(listener)
  return () => { listeners.delete(listener) }
}
