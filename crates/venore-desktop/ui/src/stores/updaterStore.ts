// =============================================================================
// Updater Store - Zustand store for context auto-updater
// =============================================================================

import { create } from 'zustand'
import { listen } from '@tauri-apps/api/event'
import { tauriApi } from '@/lib/tauri'
import type {
  UpdateReportResponse,
  ContextUpdateProgressPayload,
  ContextUpdateCompletePayload,
} from '@/lib/tauri'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface RegenerationProgress {
  current: number
  total: number
  moduleId: string
  status: string
}

interface UpdaterState {
  updateReport: UpdateReportResponse | null
  isChecking: boolean
  isRegenerating: boolean
  regenerationProgress: RegenerationProgress | null
  selectedModules: Set<string>

  // Actions
  checkUpdates: (projectPath: string) => Promise<void>
  runUpdate: (
    projectPath: string,
    moduleNames: string[],
    provider: string,
    model: string,
    depthLevel: string,
    latestCommit: string,
  ) => Promise<void>
  completeUpdate: (projectPath: string, latestCommit: string) => Promise<void>
  clearReport: () => void
  toggleModule: (name: string) => void
  selectAllModules: () => void
  deselectAllModules: () => void
  initListeners: () => Promise<() => void>
}

// -----------------------------------------------------------------------------
// Store
// -----------------------------------------------------------------------------

export const useUpdaterStore = create<UpdaterState>((set, get) => ({
  updateReport: null,
  isChecking: false,
  isRegenerating: false,
  regenerationProgress: null,
  selectedModules: new Set(),

  checkUpdates: async (projectPath: string) => {
    set({ isChecking: true })
    try {
      const report = await tauriApi.checkForUpdates(projectPath)
      const selected = new Set(
        report?.affected_modules.map((m) => m.name) ?? [],
      )
      set({ updateReport: report ?? null, selectedModules: selected })
    } catch (e) {
      console.error('Failed to check for updates:', e)
    } finally {
      set({ isChecking: false })
    }
  },

  runUpdate: async (projectPath, moduleNames, provider, model, depthLevel, latestCommit) => {
    set({ isRegenerating: true, regenerationProgress: null })
    try {
      await tauriApi.runContextUpdate({
        project_path: projectPath,
        module_names: moduleNames,
        provider,
        model,
        depth_level: depthLevel,
        latest_commit: latestCommit,
      })
    } catch (e) {
      console.error('Failed to run context update:', e)
      set({ isRegenerating: false })
    }
  },

  completeUpdate: async (projectPath: string, latestCommit: string) => {
    try {
      await tauriApi.completeContextUpdate(projectPath, latestCommit)
      set({ updateReport: null, regenerationProgress: null })
    } catch (e) {
      console.error('Failed to complete update:', e)
    }
  },

  clearReport: () => set({ updateReport: null, regenerationProgress: null }),

  toggleModule: (name: string) => {
    const current = get().selectedModules
    const next = new Set(current)
    if (next.has(name)) {
      next.delete(name)
    } else {
      next.add(name)
    }
    set({ selectedModules: next })
  },

  selectAllModules: () => {
    const report = get().updateReport
    if (!report) return
    set({ selectedModules: new Set(report.affected_modules.map((m) => m.name)) })
  },

  deselectAllModules: () => set({ selectedModules: new Set() }),

  initListeners: async () => {
    const unlistenProgress = await listen<ContextUpdateProgressPayload>(
      'context-update-progress',
      (event) => {
        set({
          regenerationProgress: {
            current: event.payload.current,
            total: event.payload.total,
            moduleId: event.payload.module_id,
            status: event.payload.status,
          },
        })
      },
    )

    const unlistenComplete = await listen<ContextUpdateCompletePayload>(
      'context-update-complete',
      () => {
        set({ isRegenerating: false })
      },
    )

    return () => {
      unlistenProgress()
      unlistenComplete()
    }
  },
}))
