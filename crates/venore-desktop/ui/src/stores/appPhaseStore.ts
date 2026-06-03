// =============================================================================
// App phase store — top-level navigation between boot / launcher / workspace
// =============================================================================
// Single source of truth for which screen the app is on and which project is
// active. Replaces the prop-drilled `onProjectOpen` / `onBack` chain so any
// component (title-bar menu, command palette, deep-linked notification) can
// navigate without threading callbacks through the tree.

import { create } from 'zustand'
import { tauriApi } from '@/lib/tauri'

export type AppPhase = 'boot' | 'launcher' | 'workspace' | 'ocean-catalog'

interface AppPhaseState {
  phase: AppPhase
  /** Path of the currently-opened project. `null` outside the workspace phase. */
  currentProjectPath: string | null
  /** Backend-stable id for the open project (from `.venore/project.json`). */
  currentProjectId: string | null
  /** 'code' | 'knowledge'. Drives feature flags inside the workspace. */
  currentProjectType: string

  setPhase: (phase: AppPhase) => void
  /** Convenience: drop back to the launcher screen. Clears nothing — the
   *  previously-opened project info stays in the store so a "reopen" path
   *  can pick it back up without re-resizing the window. */
  goToLauncher: () => void
  /** Side-effecting open: resizes the window, registers the project in
   *  SQLite (for code projects), updates current* fields, and flips phase
   *  to `workspace`. Failures in `registerProject` are non-fatal — the
   *  wizard creates `.venore/project.json` on its own if needed. */
  openProject: (path: string, projectType?: string, projectId?: string) => Promise<void>
}

export const useAppPhaseStore = create<AppPhaseState>((set) => ({
  phase: typeof window !== 'undefined' && window.location.hash === '#ocean-catalog'
    ? 'ocean-catalog'
    : 'boot',
  currentProjectPath: null,
  currentProjectId: null,
  currentProjectType: 'code',

  setPhase: (phase) => set({ phase }),

  goToLauncher: () => set({ phase: 'launcher' }),

  openProject: async (path, projectType, projectId) => {
    console.log('[appPhase] openProject:', path, 'type:', projectType || 'code')

    // Resize to workspace dimensions before the screen mounts so the user
    // doesn't see a flash of small-window content.
    try {
      await tauriApi.resizeWindow(1400, 900)
    } catch (e) {
      console.error('[appPhase] Failed to resize window:', e)
    }

    const type = projectType || 'code'
    let resolvedId: string | null = projectId ?? null

    if (type !== 'knowledge') {
      // Best-effort: project may not have `.venore/project.json` yet (the
      // wizard creates it). Non-fatal — workspace screen tolerates a null id.
      try {
        const project = await tauriApi.registerProject(path)
        resolvedId = project.id
      } catch (e) {
        console.warn('[appPhase] registerProject failed (wizard may need to run):', e)
      }
    }

    set({
      phase: 'workspace',
      currentProjectPath: path,
      currentProjectId: resolvedId,
      currentProjectType: type,
    })
  },
}))
