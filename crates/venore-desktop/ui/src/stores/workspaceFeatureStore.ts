// =============================================================================
// Workspace Feature Store — Feature matrix per project type
// =============================================================================
// Single source of truth for which features are enabled per project type.
// Components subscribe to projectType and filter UI accordingly.

import { create } from 'zustand'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

export type ProjectType = 'code' | 'knowledge'

/** Feature IDs that map to UI capabilities */
export type FeatureId =
  // Panels
  | 'project'    // Project panel (left)
  | 'github'     // GitHub panel (left)
  | 'sessions'   // Sessions panel (left)
  | 'chat'       // Chat panel (right)
  // Canvas tabs
  | 'ocean'      // Ocean 3D canvas
  | 'knowledge'  // Knowledge board canvas
  | 'pr'         // PR detail tabs
  | 'issue'      // Issue detail tabs
  | 'file'       // File editor tabs
  | 'session'    // Session detail tabs
  // Activity bar items (non-panel)
  | 'terminal'   // Terminal toggle
  | 'ai'         // AI consolidated tab (profiles + prompts + memory + tools + rules + categories)
  // Background features
  | 'updater'    // Context auto-updater
  | 'mesh'       // Agent mesh

// -----------------------------------------------------------------------------
// Feature Matrix
// -----------------------------------------------------------------------------

export const FEATURE_MATRIX: Record<ProjectType, Set<FeatureId>> = {
  code: new Set([
    'project', 'github', 'sessions', 'chat',
    'ocean', 'pr', 'issue', 'file', 'session',
    'terminal', 'ai',
    'updater', 'mesh',
  ]),
  knowledge: new Set([
    'github', 'chat',
    'ocean', 'knowledge', 'pr', 'issue',
    'terminal', 'ai',
    'mesh',
  ]),
}

// -----------------------------------------------------------------------------
// Store
// -----------------------------------------------------------------------------

interface WorkspaceFeatureStoreState {
  projectType: ProjectType
  setProjectType: (type: ProjectType) => void
}

export const useWorkspaceFeatureStore = create<WorkspaceFeatureStoreState>()((set) => ({
  projectType: 'code',
  setProjectType: (type) => set({ projectType: type }),
}))

// -----------------------------------------------------------------------------
// Hooks
// -----------------------------------------------------------------------------

/** Check if a feature is enabled for the current project type */
export function useFeatureEnabled(id: FeatureId): boolean {
  return useWorkspaceFeatureStore((s) => FEATURE_MATRIX[s.projectType].has(id))
}
