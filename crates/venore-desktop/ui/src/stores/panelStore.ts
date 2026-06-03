// =============================================================================
// Panel Store - Zustand store for panel mode and z-index management
// =============================================================================
// Centralized state for all workspace panels. Lazy-initialized: any unknown
// panelId defaults to { mode: 'closed', zIndex: 30 }.

import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import { useShallow } from 'zustand/react/shallow'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

export type PanelMode = 'closed' | 'docked' | 'floating' | 'collapsed'

interface PanelState {
  mode: PanelMode
  zIndex: number
}

interface PanelStoreState {
  panels: Record<string, PanelState>
  _zCounter: number
  activityBarOrder: string[]

  // Actions
  setMode: (panelId: string, mode: PanelMode) => void
  togglePanel: (panelId: string) => void
  bringToFront: (panelId: string) => void
  setActivityBarOrder: (order: string[]) => void
}

// -----------------------------------------------------------------------------
// Defaults
// -----------------------------------------------------------------------------

const DEFAULT_PANEL_STATE: PanelState = { mode: 'closed', zIndex: 30 }

export const DEFAULT_ACTIVITY_BAR_ORDER = ['project', 'github', 'sessions', 'knowledge', 'chat', 'terminal', 'ai']

function getPanel(panels: Record<string, PanelState>, id: string): PanelState {
  return panels[id] ?? DEFAULT_PANEL_STATE
}

// -----------------------------------------------------------------------------
// Store
// -----------------------------------------------------------------------------

export const usePanelStore = create<PanelStoreState>()(
  persist(
    (set) => ({
      panels: {},
      _zCounter: 30,
      activityBarOrder: DEFAULT_ACTIVITY_BAR_ORDER,

      setMode: (panelId, mode) =>
        set((state) => ({
          panels: {
            ...state.panels,
            [panelId]: { ...getPanel(state.panels, panelId), mode },
          },
        })),

      togglePanel: (panelId) =>
        set((state) => {
          const current = getPanel(state.panels, panelId)
          const newMode: PanelMode = current.mode === 'closed' ? 'docked' : 'closed'
          return {
            panels: {
              ...state.panels,
              [panelId]: { ...current, mode: newMode },
            },
          }
        }),

      bringToFront: (panelId) =>
        set((state) => {
          const next = state._zCounter + 1
          return {
            _zCounter: next,
            panels: {
              ...state.panels,
              [panelId]: { ...getPanel(state.panels, panelId), zIndex: next },
            },
          }
        }),

      setActivityBarOrder: (order) => set({ activityBarOrder: order }),
    }),
    {
      name: 'venore-panel-state',
      partialize: (state) => ({ activityBarOrder: state.activityBarOrder }),
    }
  )
)

// -----------------------------------------------------------------------------
// Selector hooks (avoid re-renders for unrelated panels)
// -----------------------------------------------------------------------------

export function usePanelMode(panelId: string): PanelMode {
  return usePanelStore(
    useShallow((s) => getPanel(s.panels, panelId).mode),
  )
}

export function usePanelZ(panelId: string): number {
  return usePanelStore(
    useShallow((s) => getPanel(s.panels, panelId).zIndex),
  )
}
