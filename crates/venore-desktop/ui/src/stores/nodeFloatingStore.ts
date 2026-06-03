// =============================================================================
// Node Floating Store — Multi-instance floating panels for ocean nodes
// =============================================================================
// Each click on a node opens its own floating panel. Clicking an already-open
// node brings its panel to front (no duplicates). Same behavior as v1.

import { create } from 'zustand'
import type { OceanNodeVariant } from '@/lib/tauri'
import { useAIConnectionStore } from './aiConnectionStore'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

export interface NodePanelData {
  projectPath: string
  moduleId: string
  moduleName: string
  /** Filesystem path relative to project root — only meaningful for module
   *  nodes. Knowledge nodes / lighthouses leave this as an empty string. */
  modulePath: string
  /** Drives which content panel renders. Defaults to "module" for backwards
   *  compat with callers that haven't been updated yet. */
  nodeVariant?: OceanNodeVariant
}

interface NodePanelInstance {
  panelId: string // 'node:{moduleId}'
  data: NodePanelData
  zIndex: number
}

interface ClosePanelOptions {
  /** Default true. Pop-out flow passes false so the connection survives
   *  the in-app panel unmount and is picked up by the NodeWindow. */
  unregisterAi?: boolean
}

interface NodeFloatingState {
  panels: NodePanelInstance[]
  _zCounter: number
  openPanel: (data: NodePanelData) => void
  closePanel: (panelId: string, opts?: ClosePanelOptions) => void
  bringToFront: (panelId: string) => void
}

// -----------------------------------------------------------------------------
// Store
// -----------------------------------------------------------------------------

export const useNodeFloatingStore = create<NodeFloatingState>((set, get) => ({
  panels: [],
  _zCounter: 40, // start above existing floating panels (z-30+)

  openPanel: (data) => {
    const panelId = `node:${data.moduleId}`
    const { panels } = get()
    const existing = panels.find((p) => p.panelId === panelId)

    if (existing) {
      get().bringToFront(panelId)
      return
    }

    const nextZ = get()._zCounter + 1
    set({
      panels: [...panels, { panelId, data, zIndex: nextZ }],
      _zCounter: nextZ,
    })
  },

  closePanel: (panelId, opts) => {
    if (opts?.unregisterAi !== false) {
      useAIConnectionStore.getState().unregisterConnection(panelId)
    }
    set((s) => ({ panels: s.panels.filter((p) => p.panelId !== panelId) }))
  },

  bringToFront: (panelId) => {
    const nextZ = get()._zCounter + 1
    set((s) => ({
      _zCounter: nextZ,
      panels: s.panels.map((p) =>
        p.panelId === panelId ? { ...p, zIndex: nextZ } : p,
      ),
    }))
  },
}))
