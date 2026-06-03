// =============================================================================
// AI Connection Store - Cross-window registry mirror (backend-owned state)
// =============================================================================
// The authoritative state lives in venore-desktop's Rust process. This store
// is a thin local mirror so React components can subscribe synchronously.
//
// Each entry carries `{ active, target }` — the `target` describes WHAT is
// attached (knowledge node / code module / hex) and is what travels to the
// backend resolver every turn. ChatInput uses it to render typed badges
// without going back to the panels list.
//
// Flow:
//   1. Components call register/unregister/toggle/etc. — we apply the change
//      optimistically to the local map and fire-and-forget the Tauri command.
//   2. Backend re-broadcasts the full snapshot via "ai-connection:update".
//   3. Bootstrap hook (`useAiConnectionsBootstrap`) replaces local state
//      with the snapshot — that's the source of truth on conflict.
//
// Connection IDs are queried via [data-connection-id="..."] in the DOM by
// AIConnectionLayer; this store does not hold refs or positions.

import { create } from 'zustand'
import { tauriApi, type AiConnectionDto, type AiConnectionTarget } from '@/lib/tauri'

export interface AiConnectionEntry {
  active: boolean
  target: AiConnectionTarget
}

interface AIConnectionState {
  connections: Record<string, AiConnectionEntry>

  /** Replace local map from a backend snapshot. Called by the event bridge. */
  applySnapshot: (snapshot: AiConnectionDto[]) => void

  registerConnection: (id: string, target: AiConnectionTarget) => void
  unregisterConnection: (id: string) => void
  toggleConnection: (id: string) => void
  disconnectAll: () => void
}

function snapshotToMap(snapshot: AiConnectionDto[]): Record<string, AiConnectionEntry> {
  const out: Record<string, AiConnectionEntry> = {}
  for (const c of snapshot) out[c.id] = { active: c.active, target: c.target }
  return out
}

export const useAIConnectionStore = create<AIConnectionState>()((set, get) => ({
  connections: {},

  applySnapshot: (snapshot) => set({ connections: snapshotToMap(snapshot) }),

  registerConnection: (id, target) => {
    const existing = get().connections[id]
    // Optimistic: refresh target every time, preserve active flag if any.
    set((s) => ({
      connections: {
        ...s.connections,
        [id]: { active: existing?.active ?? false, target },
      },
    }))
    void tauriApi.registerAiConnection(id, target).catch((err) => {
      console.error('registerAiConnection failed', err)
    })
  },

  unregisterConnection: (id) => {
    set((s) => {
      const { [id]: _, ...rest } = s.connections
      return { connections: rest }
    })
    void tauriApi.unregisterAiConnection(id).catch((err) => {
      console.error('unregisterAiConnection failed', err)
    })
  },

  toggleConnection: (id) => {
    set((s) => {
      const existing = s.connections[id]
      if (!existing) return s
      return {
        connections: {
          ...s.connections,
          [id]: { ...existing, active: !existing.active },
        },
      }
    })
    void tauriApi.toggleAiConnection(id).catch((err) => {
      console.error('toggleAiConnection failed', err)
    })
  },

  disconnectAll: () => {
    set((s) => {
      const reset: Record<string, AiConnectionEntry> = {}
      for (const [id, entry] of Object.entries(s.connections)) {
        reset[id] = { ...entry, active: false }
      }
      return { connections: reset }
    })
    void tauriApi.disconnectAllAiConnections().catch((err) => {
      console.error('disconnectAllAiConnections failed', err)
    })
  },
}))
