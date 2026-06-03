// =============================================================================
// Mesh Store — Peer state, connection actions, and event-driven updates
// =============================================================================

import { create } from 'zustand'
import { tauriApi, type MeshPeerInfo, type MeshTransportStatus } from '@/lib/tauri'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface MeshState {
  // Data (pushed from backend events)
  peers: MeshPeerInfo[]
  connectedPeerIds: string[]
  transportRunning: boolean
  transportPort: number
  error: string | null
  /// The project_id this window's mesh is scoped to. Set by useMeshInit
  /// on mount and used to filter "self" out of the peer list — without
  /// this, single-process multi-window would show the window its own
  /// project as a peer.
  myProjectId: string | null

  // UI
  panelOpen: boolean
  connectingPeerId: string | null

  // Event setters (called by useMeshInit listeners)
  setPeers: (peers: MeshPeerInfo[]) => void
  setStatus: (status: MeshTransportStatus) => void
  setError: (error: string | null) => void
  setMyProjectId: (id: string | null) => void

  // Actions
  togglePanel: () => void
  refreshPeers: () => Promise<void>
  refreshStatus: () => Promise<void>
  connectPeer: (projectId: string) => Promise<void>
  disconnectPeer: (projectId: string) => Promise<void>
}

// -----------------------------------------------------------------------------
// Store
// -----------------------------------------------------------------------------

export const useMeshStore = create<MeshState>((set, get) => ({
  peers: [],
  connectedPeerIds: [],
  transportRunning: false,
  transportPort: 0,
  error: null,
  myProjectId: null,
  panelOpen: false,
  connectingPeerId: null,

  // Event setters — called from useMeshInit event listeners.
  // `setPeers` filters out our own project_id so the UI never shows
  // "self" as a connectable peer.
  setPeers: (peers) => {
    const mine = get().myProjectId
    const visible = mine ? peers.filter((p) => p.project_id !== mine) : peers
    set({ peers: visible })
  },
  setStatus: (status) =>
    set({
      transportRunning: status.running,
      transportPort: status.port,
      connectedPeerIds: status.connected_peers,
    }),
  setError: (error) => set({ error }),
  setMyProjectId: (id) => set({ myProjectId: id }),

  togglePanel: () => set((s) => ({ panelOpen: !s.panelOpen })),

  refreshPeers: async () => {
    try {
      const peers = await tauriApi.meshGetPeers()
      const mine = get().myProjectId
      const visible = mine ? peers.filter((p) => p.project_id !== mine) : peers
      set({ peers: visible })
    } catch (e) {
      console.warn('[Mesh] Failed to refresh peers:', e)
    }
  },

  refreshStatus: async () => {
    try {
      const status = await tauriApi.meshTransportStatus()
      set({
        transportRunning: status.running,
        transportPort: status.port,
        connectedPeerIds: status.connected_peers,
      })
    } catch (e) {
      console.warn('[Mesh] Failed to refresh status:', e)
    }
  },

  connectPeer: async (projectId: string) => {
    set({ connectingPeerId: projectId })
    try {
      await tauriApi.meshConnectPeer(projectId)
      await get().refreshStatus()
    } catch (e) {
      console.warn('[Mesh] Connect failed:', e)
    } finally {
      set({ connectingPeerId: null })
    }
  },

  disconnectPeer: async (projectId: string) => {
    try {
      await tauriApi.meshDisconnectPeer(projectId)
      await get().refreshStatus()
    } catch (e) {
      console.warn('[Mesh] Disconnect failed:', e)
    }
  },
}))
