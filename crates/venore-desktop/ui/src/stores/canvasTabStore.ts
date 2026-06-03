// =============================================================================
// Canvas Tab Store — Zustand store for canvas tab management
// =============================================================================
// Manages tabs above the canvas: Ocean (always present) + dynamic PR/Issue/File tabs.

import { create } from 'zustand'
import i18n from '@/i18n'
import type { ProjectType } from './workspaceFeatureStore'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

export interface CanvasTab {
  id: string
  type: 'ocean' | 'knowledge' | 'pr' | 'issue' | 'file' | 'ai' | 'session'
  label: string
  data?: {
    number?: number
    title?: string
    relativePath?: string
    isDirty?: boolean
    sessionId?: string
    worktreePath?: string
    featureId?: string
  }
}

interface CanvasTabStoreState {
  tabs: CanvasTab[]
  activeTabId: string

  // Actions
  resetForProjectType: (type: ProjectType) => void
  openKnowledge: (featureId: string, featureName: string) => void
  openAI: () => void
  openPr: (number: number, title: string) => void
  openIssue: (number: number, title: string) => void
  openFile: (relativePath: string) => void
  openSession: (sessionId: string, name: string, worktreePath?: string) => void
  setTabDirty: (tabId: string, isDirty: boolean) => void
  closeTab: (tabId: string) => void
  setActiveTab: (tabId: string) => void
}

// -----------------------------------------------------------------------------
// Constants
// -----------------------------------------------------------------------------

function getOceanTab(): CanvasTab {
  return { id: 'ocean', type: 'ocean', label: i18n.t('workspace:tabs.ocean') }
}

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

/** Extract filename from a relative path */
function fileNameFromPath(relativePath: string): string {
  const normalized = relativePath.replace(/\\/g, '/')
  const parts = normalized.split('/')
  return parts[parts.length - 1] || relativePath
}

/** Disambiguate label if another tab already has the same filename */
function disambiguateLabel(
  fileName: string,
  relativePath: string,
  tabs: CanvasTab[],
): string {
  const hasDuplicate = tabs.some(
    (t) => t.type === 'file' && t.label === fileName,
  )
  if (!hasDuplicate) return fileName

  // Use parent/filename
  const normalized = relativePath.replace(/\\/g, '/')
  const parts = normalized.split('/')
  if (parts.length >= 2) return `${parts[parts.length - 2]}/${fileName}`
  return fileName
}

// -----------------------------------------------------------------------------
// Store
// -----------------------------------------------------------------------------

export const useCanvasTabStore = create<CanvasTabStoreState>()((set) => ({
  tabs: [getOceanTab()],
  activeTabId: 'ocean',

  resetForProjectType: (_type: ProjectType) =>
    set(() => {
      const tab = getOceanTab()
      return { tabs: [tab], activeTabId: tab.id }
    }),

  openKnowledge: (featureId: string, featureName: string) =>
    set((s) => {
      const tabId = `knowledge-${featureId}`
      const existing = s.tabs.find((t) => t.id === tabId)
      if (existing) return { activeTabId: tabId }
      const tab: CanvasTab = {
        id: tabId,
        type: 'knowledge',
        label: featureName,
        data: { featureId },
      }
      return { tabs: [...s.tabs, tab], activeTabId: tabId }
    }),

  openAI: () =>
    set((s) => {
      const tabId = 'ai'
      const existing = s.tabs.find((t) => t.id === tabId)
      if (existing) return { activeTabId: tabId }
      const tab: CanvasTab = { id: tabId, type: 'ai', label: i18n.t('workspace:tabs.ai') }
      return { tabs: [...s.tabs, tab], activeTabId: tabId }
    }),

  openPr: (number: number, title: string) =>
    set((s) => {
      const tabId = `pr-${number}`
      const existing = s.tabs.find((t) => t.id === tabId)
      if (existing) return { activeTabId: tabId }
      const tab: CanvasTab = {
        id: tabId,
        type: 'pr',
        label: `#${number} ${title}`,
        data: { number, title },
      }
      return { tabs: [...s.tabs, tab], activeTabId: tabId }
    }),

  openIssue: (number: number, title: string) =>
    set((s) => {
      const tabId = `issue-${number}`
      const existing = s.tabs.find((t) => t.id === tabId)
      if (existing) return { activeTabId: tabId }
      const tab: CanvasTab = {
        id: tabId,
        type: 'issue',
        label: `#${number} ${title}`,
        data: { number, title },
      }
      return { tabs: [...s.tabs, tab], activeTabId: tabId }
    }),

  openSession: (sessionId: string, name: string, worktreePath?: string) =>
    set((s) => {
      const tabId = `session-${sessionId}`
      const existing = s.tabs.find((t) => t.id === tabId)
      if (existing) return { activeTabId: tabId }
      const tab: CanvasTab = {
        id: tabId,
        type: 'session',
        label: name,
        data: { sessionId, worktreePath },
      }
      return { tabs: [...s.tabs, tab], activeTabId: tabId }
    }),

  openFile: (relativePath: string) =>
    set((s) => {
      const tabId = `file:${relativePath}`
      const existing = s.tabs.find((t) => t.id === tabId)
      if (existing) return { activeTabId: tabId }

      const fileName = fileNameFromPath(relativePath)
      const label = disambiguateLabel(fileName, relativePath, s.tabs)

      const tab: CanvasTab = {
        id: tabId,
        type: 'file',
        label,
        data: { relativePath, isDirty: false },
      }
      return { tabs: [...s.tabs, tab], activeTabId: tabId }
    }),

  setTabDirty: (tabId: string, isDirty: boolean) =>
    set((s) => ({
      tabs: s.tabs.map((t) =>
        t.id === tabId ? { ...t, data: { ...t.data, isDirty } } : t,
      ),
    })),

  closeTab: (tabId: string) =>
    set((s) => {
      // Protect the first tab (Ocean for code, Agents for knowledge)
      if (s.tabs.length > 0 && s.tabs[0].id === tabId) return s
      const tabs = s.tabs.filter((t) => t.id !== tabId)
      let activeTabId = s.activeTabId
      if (activeTabId === tabId) {
        // Activate previous tab or first tab
        const closedIdx = s.tabs.findIndex((t) => t.id === tabId)
        activeTabId = closedIdx > 0 ? s.tabs[closedIdx - 1].id : s.tabs[0].id
      }
      return { tabs, activeTabId }
    }),

  setActiveTab: (tabId: string) =>
    set({ activeTabId: tabId }),
}))
