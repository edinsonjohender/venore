// =============================================================================
// Settings store — open state + active section for the Settings modal
// =============================================================================
// Lives in a Zustand store so any component (title-bar menu, command palette,
// deep links) can pop the modal open without prop drilling. The store is
// type-agnostic about the section id — it stays a free-form string so
// `settings.config.ts` can evolve its `SettingsSectionId` union without
// rippling type changes here.

import { create } from 'zustand'

interface SettingsState {
  isOpen: boolean
  /** Currently selected section in the sidebar (matches a section id in
   *  settings.config.ts). Persists across opens of the same session. */
  activeTab: string
  openModal: (section?: string) => void
  closeModal: () => void
  setActiveTab: (section: string) => void
}

const DEFAULT_TAB = 'ai-providers'

export const useSettingsStore = create<SettingsState>((set) => ({
  isOpen: false,
  activeTab: DEFAULT_TAB,
  openModal: (section) =>
    set((s) => ({ isOpen: true, activeTab: section ?? s.activeTab })),
  closeModal: () => set({ isOpen: false }),
  setActiveTab: (section) => set({ activeTab: section }),
}))
