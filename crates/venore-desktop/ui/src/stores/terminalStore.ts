// =============================================================================
// Terminal Store — Zustand store for terminal panel state
// =============================================================================
// Tracks open/close, tabs, and active tab.

import { create } from 'zustand'
import i18n from '@/i18n'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

export interface TerminalTab {
  id: string
  name: string
  devSessionId?: string  // undefined = unbound terminal
}

interface TerminalStoreState {
  isOpen: boolean
  tabs: TerminalTab[]
  activeTabId: string | null
  _counter: number

  // Actions
  open: () => void
  close: () => void
  toggle: () => void
  addTab: (terminalId: string) => void
  addSessionTab: (terminalId: string, devSessionId: string, label: string) => void
  activateSessionTerminal: (devSessionId: string) => void
  removeTab: (terminalId: string) => void
  setActiveTab: (terminalId: string) => void
  renameTab: (terminalId: string, name: string) => void
}

// -----------------------------------------------------------------------------
// Store
// -----------------------------------------------------------------------------

export const useTerminalStore = create<TerminalStoreState>()((set) => ({
  isOpen: false,
  tabs: [],
  activeTabId: null,
  _counter: 0,

  open: () => set({ isOpen: true }),

  close: () => set({ isOpen: false }),

  toggle: () => set((s) => ({ isOpen: !s.isOpen })),

  addTab: (terminalId: string) =>
    set((s) => {
      const counter = s._counter + 1
      const tab: TerminalTab = { id: terminalId, name: `${i18n.t('workspace:terminalTabBar.terminal')} ${counter}` }
      return {
        tabs: [...s.tabs, tab],
        activeTabId: terminalId,
        _counter: counter,
      }
    }),

  addSessionTab: (terminalId: string, devSessionId: string, label: string) =>
    set((s) => {
      // Don't add if already tracked
      if (s.tabs.some((t) => t.id === terminalId)) return s
      const tab: TerminalTab = { id: terminalId, name: label, devSessionId }
      return {
        tabs: [...s.tabs, tab],
        activeTabId: terminalId,
        isOpen: true,
      }
    }),

  activateSessionTerminal: (devSessionId: string) =>
    set((s) => {
      const tab = s.tabs.find((t) => t.devSessionId === devSessionId)
      if (!tab) return s
      return { activeTabId: tab.id, isOpen: true }
    }),

  removeTab: (terminalId: string) =>
    set((s) => {
      const tabs = s.tabs.filter((t) => t.id !== terminalId)
      let activeTabId = s.activeTabId
      if (activeTabId === terminalId) {
        activeTabId = tabs.length > 0 ? tabs[tabs.length - 1].id : null
      }
      return { tabs, activeTabId }
    }),

  setActiveTab: (terminalId: string) =>
    set({ activeTabId: terminalId }),

  renameTab: (terminalId: string, name: string) =>
    set((s) => ({
      tabs: s.tabs.map((t) =>
        t.id === terminalId ? { ...t, name: name.trim() || t.name } : t,
      ),
    })),
}))
