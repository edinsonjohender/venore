// =============================================================================
// Dashboard Store — single source of truth for the project dashboard fetch
// =============================================================================
// Multiple components (ProjectPanel, ProjectCollapsedPill, …) render in the
// same workspace and previously each ran its own `getProjectDashboard` call,
// doubling backend work and SQLite queries per project open. This store
// dedupes by `project_path`: concurrent calls for the same path share one
// in-flight promise, and the resolved value lives in the store so every
// consumer subscribes to the same data.

import { create } from 'zustand'
import { tauriApi, type ProjectDashboardResponse } from '@/lib/tauri'

// In-flight requests keyed by project_path. Survives across store updates so
// rapid mount/remount cycles (StrictMode, route changes) collapse into one
// network call.
const inflight = new Map<string, Promise<ProjectDashboardResponse>>()

interface DashboardState {
  /// The path currently held in `dashboard`. Used to ignore late responses
  /// for a project the user has since navigated away from.
  projectPath: string | null
  dashboard: ProjectDashboardResponse | null
  loading: boolean
  error: string | null

  /// Load (or share) the dashboard for `projectPath`. Returns the eventual
  /// dashboard so callers that need imperative access can await; the
  /// preferred pattern is to subscribe to `dashboard`/`loading`/`error`.
  loadDashboard: (projectPath: string) => Promise<ProjectDashboardResponse | null>

  /// Force a fresh fetch (bypasses dedupe). Used after re-snapshot or when
  /// the backend emits `context-update-complete`.
  refreshDashboard: (projectPath: string) => Promise<ProjectDashboardResponse | null>

  /// Clear the store — used when the workspace tears down.
  reset: () => void
}

export const useDashboardStore = create<DashboardState>((set, get) => {
  const run = async (
    projectPath: string,
    force: boolean,
  ): Promise<ProjectDashboardResponse | null> => {
    if (!force) {
      const cached = get()
      if (cached.projectPath === projectPath && cached.dashboard != null) {
        return cached.dashboard
      }
    }

    if (!force) {
      const existing = inflight.get(projectPath)
      if (existing) {
        try {
          return await existing
        } catch {
          return null
        }
      }
    }

    set({ projectPath, loading: true, error: null })
    const promise = tauriApi.getProjectDashboard({ project_path: projectPath })
    inflight.set(projectPath, promise)

    try {
      const data = await promise
      // Only commit if the user hasn't navigated away to another project.
      if (get().projectPath === projectPath) {
        set({ dashboard: data, loading: false })
      }
      return data
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      if (get().projectPath === projectPath) {
        set({ error: message, loading: false })
      }
      return null
    } finally {
      // Only clear if this is still the same promise (a force-refresh may
      // have overwritten the slot with a newer one).
      if (inflight.get(projectPath) === promise) {
        inflight.delete(projectPath)
      }
    }
  }

  return {
    projectPath: null,
    dashboard: null,
    loading: false,
    error: null,

    loadDashboard: (projectPath: string) => run(projectPath, false),
    refreshDashboard: (projectPath: string) => run(projectPath, true),

    reset: () => set({ projectPath: null, dashboard: null, loading: false, error: null }),
  }
})
