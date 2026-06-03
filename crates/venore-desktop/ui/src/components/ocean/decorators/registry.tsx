// =============================================================================
// Decorator registry — maps node states to renderable decorators
// =============================================================================
//
// The backend ships a `states[]` per node (live updates via the
// `ocean-state-changed` event). For each state, the registry decides:
//
//   - Which slot it occupies (Halo / Cap / Perimeter / Billboard).
//     Two states fighting for the same slot are resolved by `priority`
//     (higher wins; loser stays latent in the data, just not rendered).
//
//   - How to render it given the host node's content height.
//
// Adding a new state means: extend the Rust `NodeStateKind` enum + register
// a `Scanner` that produces it + add an entry here. No core wiring changes.

import type { ReactNode } from 'react'
import type { NodeStateDto } from '@/lib/tauri'
import { OverflowHalo } from './OverflowHalo'
import { PendingWritesBadge } from './PendingWritesBadge'
import { StaleBadge } from './StaleBadge'

/** The four anchor positions a decorator can claim around a node. Today
 *  the registry only uses `halo`; the others are reserved for incoming
 *  state types so callers can plan ahead. */
export type DecoratorSlot = 'halo' | 'cap' | 'perimeter' | 'billboard'

export interface DecoratorEntry {
  slot: DecoratorSlot
  /** When two states fight for the same slot, higher wins. */
  priority: number
  /** Build the 3D node. Receives the live state + the host's content
   *  height so wraps/halos scale to the actual pillar. */
  render: (state: NodeStateDto, contentHeight: number) => ReactNode
}

/** Registry keyed by `NodeStateDto.kind` (snake_case, mirrors the Rust
 *  enum). Lookup misses are non-fatal — unknown kinds simply don't render
 *  (e.g. an older frontend talking to a newer backend). */
export const DECORATOR_REGISTRY: Record<string, DecoratorEntry> = {
  overflow: {
    slot: 'halo',
    priority: 10,
    render: (state, contentHeight) => {
      // Severity bumps the visual presence: more rings + brighter color
      // for `severe`, calmer pulse for `warning`.
      const isSevere = state.severity === 'severe'
      const ringCount = isSevere ? 5 : 4
      const color = isSevere ? '#ef4444' : '#fbbf24'
      const intensity = isSevere ? 1.0 : 0.85
      // Spread the rings across ~85% of the host's height, anchored a bit
      // above the floor so the bottom one doesn't clip into the grid.
      const baseHeight = Math.max(0.3, contentHeight * 0.2)
      const spacing = Math.max(
        0.4,
        (contentHeight * 0.85) / Math.max(ringCount - 1, 1),
      )
      return (
        <OverflowHalo
          baseHeight={baseHeight}
          spacing={spacing}
          ringCount={ringCount}
          color={color}
          intensity={intensity}
        />
      )
    },
  },
  pending_writes: {
    // `cap` floats above the node — distinct from `halo` so a node that's
    // both overflowing AND has pending writes shows both at once instead
    // of the registry collapsing to one decorator.
    slot: 'cap',
    priority: 10,
    render: (state, contentHeight) => {
      const payload = (state.payload ?? {}) as { count?: number }
      const count = typeof payload.count === 'number' ? payload.count : 1
      // Float a bit above the pillar's tip; pillar height varies with
      // section count so we offset off the host's content height.
      const yOffset = Math.max(1.2, contentHeight + 0.6)
      return <PendingWritesBadge yOffset={yOffset} count={count} />
    },
  },
  stale: {
    // `billboard` floats higher than `cap` so a node that's both stale and
    // has pending writes shows both: the pending sparkle near the tip, the
    // stale ring above it. Priority is lower than pending_writes so if the
    // two ever land on the same slot the active alert wins.
    slot: 'billboard',
    priority: 5,
    render: (state, contentHeight) => {
      const severity = state.severity === 'severe' || state.severity === 'warning'
        ? 'warning'
        : 'info'
      // Sit above where `cap` would sit (cap = contentHeight + 0.6), leaving
      // breathing room when both are present.
      const yOffset = Math.max(1.8, contentHeight + 1.3)
      return <StaleBadge yOffset={yOffset} severity={severity} />
    },
  },
}

/** Resolve a node's state list into the actual decorators that should
 *  render. Implements slot+priority:
 *    1. Drop states whose kind isn't in the registry.
 *    2. Group by slot.
 *    3. Inside each slot, keep the highest-priority state.
 *  Returns the chosen states in slot-stable order so React's reconciler
 *  has a deterministic key set across renders. */
export function resolveDecorators(states: NodeStateDto[]): NodeStateDto[] {
  const bySlot = new Map<DecoratorSlot, NodeStateDto>()
  for (const state of states) {
    const entry = DECORATOR_REGISTRY[state.kind]
    if (!entry) continue
    const incumbent = bySlot.get(entry.slot)
    if (!incumbent) {
      bySlot.set(entry.slot, state)
      continue
    }
    const incumbentPriority = DECORATOR_REGISTRY[incumbent.kind]?.priority ?? -Infinity
    if (entry.priority > incumbentPriority) {
      bySlot.set(entry.slot, state)
    }
  }
  // Stable order: halo → cap → perimeter → billboard. Reserve room for
  // future slots; falling through the order keeps existing keys stable.
  const order: DecoratorSlot[] = ['halo', 'cap', 'perimeter', 'billboard']
  const result: NodeStateDto[] = []
  for (const slot of order) {
    const winner = bySlot.get(slot)
    if (winner) result.push(winner)
  }
  return result
}
