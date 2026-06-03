// =============================================================================
// Wizard Data Store - Consolidated data for Steps 1-8
// =============================================================================

import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import type {
  ProjectContext,
  ProjectState,
  TeamSize,
  ProjectGoal,
  DepthLevel,
  Layer,
  AnalysisRules,
  ProjectTypeInfo,
  ModuleInfo,
  GenerationStatus,
  ModuleGenerationStatus,
  SubIsland,
  IslandMetrics,
  IslandDetectionParams,
  LighthouseSummary,
  WizardResult,
  IndexResult,
} from '../lib/wizard/types'

// =============================================================================
// Step 1: Project Context
// =============================================================================

interface Step1State {
  name: string
  description: string
  projectState: ProjectState
  teamSize: TeamSize
  goals: ProjectGoal[]
  architecture: string
  techDebt: string
}

// =============================================================================
// Step 2: Analysis Rules
// =============================================================================

interface Step2State {
  depthLevel: DepthLevel
  layersToGenerate: Layer[]
  exclusions: string[]
  ragEnabled: boolean
  projectType: ProjectTypeInfo | null
  projectPath: string | null
  projectName: string | null
}

// =============================================================================
// Step 3: Module Selection
// =============================================================================

interface Step3State {
  detectedModules: ModuleInfo[]
  selectedModuleIds: string[]
  selectedModuleNames?: string[]  // For checkpoint resume: module names from checkpoint
}

// =============================================================================
// Step 4: LLM Config (DEPRECATED - usando task settings)
// =============================================================================

interface Step4State {
  // Empty - LLM config now comes from task settings
  // Kept for future use if needed
  analysisDepth: DepthLevel
}

// =============================================================================
// Steps 5-8: Generation & Completion
// =============================================================================

interface RootContext {
  content: string
  filePath: string
  generatedAt: string
}

/** AI-generated draft for project memory (Step 5 Complete).
 *  Mirrors `GenerateMemoryResponse` from tauri.ts but kept local to avoid
 *  coupling the store to backend DTOs. */
export interface AiMemoryDraft {
  description: string
  state: string
  goals: string[]
  architecture: string
  techDebt: string
  projectSummary: string
}

interface Steps5to8State {
  // Step 5: Root Context
  rootContext: RootContext | null
  userFeedback: string

  /** AI-generated + user-edited memory draft, ready to be persisted at
   *  "Open Project" time. Null until the user reaches Step 5 and the
   *  generation completes. */
  aiMemoryDraft: AiMemoryDraft | null

  // Step 6: Batch Generation
  status: GenerationStatus
  currentModule: string | null
  currentIndex: number
  totalModules: number
  completedModules: ModuleGenerationStatus[]
  startTime: number | null
  endTime: number | null
  error: string | null
  batchId: string | null
  provider: string | null
  model: string | null

  // Step 7: Sub-Islands
  detectedIslands: SubIsland[]
  selectedIslandIds: string[]
  islandMetrics: IslandMetrics | null
  islandParams: IslandDetectionParams

  // Step 8: Lighthouse & Result
  lighthouse: LighthouseSummary | null
  wizardResult: WizardResult | null
}

// =============================================================================
// Combined State
// =============================================================================

interface WizardDataState {
  step1: Step1State
  step2: Step2State
  step3: Step3State
  step4: Step4State
  step5to8: Steps5to8State
  indexResult: IndexResult | null

  // Step 1 actions
  updateProjectContext: (partial: Partial<ProjectContext>) => void
  setProjectName: (name: string) => void
  setProjectDescription: (description: string) => void
  setProjectState: (state: ProjectState) => void
  setTeamSize: (size: TeamSize) => void
  setProjectGoals: (goals: ProjectGoal[]) => void
  toggleProjectGoal: (goal: ProjectGoal) => void
  setArchitecture: (architecture: string) => void
  setTechDebt: (techDebt: string) => void

  // Step 2 actions
  setDepthLevel: (level: DepthLevel) => void
  setLayersToGenerate: (layers: Layer[]) => void
  toggleLayer: (layer: Layer) => void
  setExclusions: (exclusions: string[]) => void
  addExclusion: (pattern: string) => void
  removeExclusion: (pattern: string) => void
  setRagEnabled: (enabled: boolean) => void
  setProjectType: (info: ProjectTypeInfo | null) => void
  setProjectPath: (path: string, name: string) => void

  // Step 3 actions
  setDetectedModules: (modules: ModuleInfo[]) => void
  setSelectedModuleIds: (ids: string[]) => void
  toggleModule: (id: string) => void
  selectAllModules: () => void
  selectNoModules: () => void
  selectNewModules: () => void

  // Step 4 actions
  setAnalysisDepth: (depth: DepthLevel) => void

  // Step 5 actions (Root Context)
  setRootContext: (context: RootContext) => void
  setUserFeedback: (feedback: string) => void

  // AI memory draft (Step 5 Complete)
  setAiMemoryDraft: (draft: AiMemoryDraft | null) => void
  patchAiMemoryDraft: (partial: Partial<AiMemoryDraft>) => void

  // Step 6 actions (Batch Generation)
  startGeneration: (totalModules: number, batchId: string, provider?: string, model?: string) => void
  updateProgress: (currentIndex: number, currentModule: string, status: GenerationStatus) => void
  addCompletedModule: (moduleStatus: ModuleGenerationStatus) => void
  completeGeneration: () => void
  setBatchError: (error: string) => void
  pauseGeneration: () => void
  resumeGeneration: () => void

  // Step 7 actions (Sub-Islands)
  setDetectedIslands: (islands: SubIsland[], metrics?: IslandMetrics) => void
  setSelectedIslandIds: (ids: string[]) => void
  toggleIsland: (id: string) => void
  setIslandParams: (params: Partial<IslandDetectionParams>) => void

  // Step 8 actions (Lighthouse)
  setLighthouse: (lighthouse: LighthouseSummary) => void
  setWizardResult: (result: WizardResult) => void
  setIndexResult: (result: IndexResult | null) => void

  // Reset
  reset: () => void
}

// =============================================================================
// Initial State
// =============================================================================

const initialState = {
  step1: {
    name: '',
    description: '',
    projectState: 'active' as ProjectState,
    teamSize: 'small' as TeamSize,
    goals: [] as ProjectGoal[],
    architecture: '',
    techDebt: '',
  },
  step2: {
    depthLevel: 'normal' as DepthLevel,
    layersToGenerate: ['status', 'connections'] as Layer[],
    exclusions: ['node_modules', 'dist', 'build', '.next', 'coverage', 'target'],
    ragEnabled: true,
    projectType: null,
    projectPath: null,
    projectName: null,
  },
  step3: {
    detectedModules: [],
    selectedModuleIds: [],
  },
  step4: {
    analysisDepth: 'normal' as DepthLevel,
  },
  step5to8: {
    rootContext: null,
    userFeedback: '',
    aiMemoryDraft: null,
    status: 'idle' as GenerationStatus,
    currentModule: null,
    currentIndex: 0,
    totalModules: 0,
    completedModules: [],
    startTime: null,
    endTime: null,
    error: null,
    batchId: null,
    provider: null,
    model: null,
    detectedIslands: [],
    selectedIslandIds: [],
    islandMetrics: null,
    islandParams: {
      min_modules: 2,
      max_depth: 2,
      cohesion_threshold: 0.05,
      weight_threshold: 3,
      dependency_score: 3,
    },
    lighthouse: null,
    wizardResult: null,
  },
  indexResult: null,
}

// =============================================================================
// Store
// =============================================================================

export const useWizardDataStore = create<WizardDataState>()(
  persist(
    (set, get) => ({
      ...initialState,

      // -----------------------------------------------------------------------
      // Step 1 actions
      // -----------------------------------------------------------------------

      updateProjectContext: (partial) => {
        set((state) => ({
          step1: { ...state.step1, ...partial },
        }))
      },

      setProjectName: (name) => {
        set((state) => ({
          step1: { ...state.step1, name },
        }))
      },

      setProjectDescription: (description) => {
        set((state) => ({
          step1: { ...state.step1, description },
        }))
      },

      setProjectState: (projectState) => {
        set((state) => ({
          step1: { ...state.step1, projectState },
        }))
      },

      setTeamSize: (teamSize) => {
        set((state) => ({
          step1: { ...state.step1, teamSize },
        }))
      },

      setProjectGoals: (goals) => {
        set((state) => ({
          step1: { ...state.step1, goals },
        }))
      },

      toggleProjectGoal: (goal) => {
        const { step1 } = get()
        const newGoals = step1.goals.includes(goal)
          ? step1.goals.filter((g) => g !== goal)
          : [...step1.goals, goal]
        set((state) => ({
          step1: { ...state.step1, goals: newGoals },
        }))
      },

      setArchitecture: (architecture) => {
        set((state) => ({
          step1: { ...state.step1, architecture },
        }))
      },

      setTechDebt: (techDebt) => {
        set((state) => ({
          step1: { ...state.step1, techDebt },
        }))
      },

      // -----------------------------------------------------------------------
      // Step 2 actions
      // -----------------------------------------------------------------------

      setDepthLevel: (depthLevel) => {
        set((state) => ({
          step2: { ...state.step2, depthLevel },
        }))
      },

      setLayersToGenerate: (layersToGenerate) => {
        set((state) => ({
          step2: { ...state.step2, layersToGenerate },
        }))
      },

      toggleLayer: (layer) => {
        const { step2 } = get()
        const newLayers = step2.layersToGenerate.includes(layer)
          ? step2.layersToGenerate.filter((l) => l !== layer)
          : [...step2.layersToGenerate, layer]

        // 'context' layer is required
        if (newLayers.includes('context')) {
          set((state) => ({
            step2: { ...state.step2, layersToGenerate: newLayers },
          }))
        }
      },

      setExclusions: (exclusions) => {
        set((state) => ({
          step2: { ...state.step2, exclusions },
        }))
      },

      addExclusion: (pattern) => {
        const { step2 } = get()
        if (!step2.exclusions.includes(pattern) && pattern.trim()) {
          set((state) => ({
            step2: { ...state.step2, exclusions: [...state.step2.exclusions, pattern.trim()] },
          }))
        }
      },

      removeExclusion: (pattern) => {
        const { step2 } = get()
        set((state) => ({
          step2: { ...state.step2, exclusions: state.step2.exclusions.filter((p) => p !== pattern) },
        }))
      },

      setRagEnabled: (ragEnabled) => {
        set((state) => ({
          step2: { ...state.step2, ragEnabled },
        }))
      },

      setProjectType: (projectType) => {
        set((state) => ({
          step2: { ...state.step2, projectType },
        }))
      },

      setProjectPath: (projectPath, projectName) => {
        set((state) => ({
          step2: { ...state.step2, projectPath, projectName },
        }))
      },

      // -----------------------------------------------------------------------
      // Step 3 actions
      // -----------------------------------------------------------------------

      setDetectedModules: (detectedModules) => {
        set((state) => ({
          step3: { ...state.step3, detectedModules },
        }))
      },

      setSelectedModuleIds: (selectedModuleIds) => {
        set((state) => ({
          step3: { ...state.step3, selectedModuleIds },
        }))
      },

      toggleModule: (id) => {
        const { step3 } = get()
        const newSelection = step3.selectedModuleIds.includes(id)
          ? step3.selectedModuleIds.filter((moduleId) => moduleId !== id)
          : [...step3.selectedModuleIds, id]
        set((state) => ({
          step3: { ...state.step3, selectedModuleIds: newSelection },
        }))
      },

      selectAllModules: () => {
        const { step3 } = get()
        set((state) => ({
          step3: { ...state.step3, selectedModuleIds: state.step3.detectedModules.map((m) => m.id) },
        }))
      },

      selectNoModules: () => {
        set((state) => ({
          step3: { ...state.step3, selectedModuleIds: [] },
        }))
      },

      selectNewModules: () => {
        const { step3 } = get()
        const newModules = step3.detectedModules
          .filter((m) => !m.hasExistingContext)
          .map((m) => m.id)
        set((state) => ({
          step3: { ...state.step3, selectedModuleIds: newModules },
        }))
      },

      // -----------------------------------------------------------------------
      // Step 4 actions
      // -----------------------------------------------------------------------

      setAnalysisDepth: (analysisDepth) => {
        set((state) => ({
          step4: { ...state.step4, analysisDepth },
        }))
      },

      // -----------------------------------------------------------------------
      // Step 5 actions (Root Context)
      // -----------------------------------------------------------------------

      setRootContext: (rootContext) => {
        set((state) => ({
          step5to8: { ...state.step5to8, rootContext },
        }))
      },

      setUserFeedback: (userFeedback) => {
        set((state) => ({
          step5to8: { ...state.step5to8, userFeedback },
        }))
      },

      setAiMemoryDraft: (aiMemoryDraft) => {
        set((state) => ({
          step5to8: { ...state.step5to8, aiMemoryDraft },
        }))
      },

      patchAiMemoryDraft: (partial) => {
        const { step5to8 } = get()
        if (!step5to8.aiMemoryDraft) return
        set((state) => ({
          step5to8: {
            ...state.step5to8,
            aiMemoryDraft: { ...state.step5to8.aiMemoryDraft!, ...partial },
          },
        }))
      },

      // -----------------------------------------------------------------------
      // Step 6 actions (Batch Generation)
      // -----------------------------------------------------------------------

      startGeneration: (totalModules, batchId, provider, model) => {
        const { step5to8 } = get()
        const isResumingFromCheckpoint = step5to8.completedModules.length > 0

        set((state) => ({
          step5to8: {
            ...state.step5to8,
            status: 'running',
            totalModules,
            batchId,
            provider: provider || null,
            model: model || null,
            currentIndex: isResumingFromCheckpoint ? state.step5to8.completedModules.length : 0,
            currentModule: null,
            completedModules: isResumingFromCheckpoint ? state.step5to8.completedModules : [],
            startTime: Date.now(),
            endTime: null,
            error: null,
          },
        }))
      },

      updateProgress: (currentIndex, currentModule, status) => {
        set((state) => ({
          step5to8: {
            ...state.step5to8,
            currentIndex,
            currentModule,
            // Don't overwrite 'paused' status with late progress events
            status: state.step5to8.status === 'paused' ? 'paused' : status,
          },
        }))
      },

      addCompletedModule: (moduleStatus) => {
        set((state) => ({
          step5to8: {
            ...state.step5to8,
            completedModules: [...state.step5to8.completedModules, moduleStatus],
          },
        }))
      },

      completeGeneration: () => {
        set((state) => ({
          step5to8: {
            ...state.step5to8,
            status: 'completed',
            endTime: Date.now(),
            currentModule: null,
          },
        }))
      },

      setBatchError: (error) => {
        set((state) => ({
          step5to8: {
            ...state.step5to8,
            status: 'error',
            error,
            endTime: Date.now(),
          },
        }))
      },

      pauseGeneration: () => {
        set((state) => ({
          step5to8: {
            ...state.step5to8,
            status: 'paused',
          },
        }))
      },

      resumeGeneration: () => {
        set((state) => ({
          step5to8: {
            ...state.step5to8,
            status: 'running',
          },
        }))
      },

      // -----------------------------------------------------------------------
      // Step 7 actions (Sub-Islands)
      // -----------------------------------------------------------------------

      setDetectedIslands: (detectedIslands, metrics) => {
        set((state) => ({
          step5to8: {
            ...state.step5to8,
            detectedIslands,
            islandMetrics: metrics || null,
          },
        }))
      },

      setSelectedIslandIds: (selectedIslandIds) => {
        set((state) => ({
          step5to8: {
            ...state.step5to8,
            selectedIslandIds,
          },
        }))
      },

      toggleIsland: (id) => {
        const { step5to8 } = get()
        const newSelection = step5to8.selectedIslandIds.includes(id)
          ? step5to8.selectedIslandIds.filter((islandId) => islandId !== id)
          : [...step5to8.selectedIslandIds, id]
        set((state) => ({
          step5to8: {
            ...state.step5to8,
            selectedIslandIds: newSelection,
          },
        }))
      },

      setIslandParams: (params) => {
        set((state) => ({
          step5to8: {
            ...state.step5to8,
            islandParams: { ...state.step5to8.islandParams, ...params },
          },
        }))
      },

      // -----------------------------------------------------------------------
      // Step 8 actions (Lighthouse)
      // -----------------------------------------------------------------------

      setLighthouse: (lighthouse) => {
        set((state) => ({
          step5to8: {
            ...state.step5to8,
            lighthouse,
          },
        }))
      },

      setWizardResult: (wizardResult) => {
        set((state) => ({
          step5to8: {
            ...state.step5to8,
            wizardResult,
          },
        }))
      },

      setIndexResult: (indexResult) => {
        set({ indexResult })
      },

      // -----------------------------------------------------------------------
      // Reset
      // -----------------------------------------------------------------------

      reset: () => {
        set(initialState)
      },
    }),
    {
      name: 'venore-wizard-data',
      partialize: (state) => ({
        // Persist all data (navigation state is in wizardStore)
        step1: state.step1,
        step2: state.step2,
        step3: state.step3,
        step4: state.step4,
        step5to8: {
          // Persist only essential generation state for checkpoint resume
          status: state.step5to8.status,
          currentIndex: state.step5to8.currentIndex,
          totalModules: state.step5to8.totalModules,
          completedModules: state.step5to8.completedModules,
          startTime: state.step5to8.startTime,
          rootContext: state.step5to8.rootContext,
          aiMemoryDraft: state.step5to8.aiMemoryDraft,
          detectedIslands: state.step5to8.detectedIslands,
          selectedIslandIds: state.step5to8.selectedIslandIds,
          islandParams: state.step5to8.islandParams,
          lighthouse: state.step5to8.lighthouse,
        },
        indexResult: state.indexResult,
      }),
    }
  )
)
