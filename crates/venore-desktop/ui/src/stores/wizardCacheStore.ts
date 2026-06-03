// =============================================================================
// Wizard Cache Store - Transient data (NOT persisted)
// =============================================================================

import { create } from 'zustand'

// =============================================================================
// Types
// =============================================================================

interface AnalysisProgress {
  current: number
  total: number
  currentItem: string
}

interface AnalysisMetrics {
  totalFiles: number
  detectedModules: number
}

/** Phase 2 (indexing) live progress driven by `wizard-index-progress` events.
 *  Three sub-phases: indexing files, building graph, analyzing layers.
 *  `current/total` are only set during sub-phases that report counts
 *  (1/3 indexing per file, 3/3 layers per module). 2/3 graph is a boundary
 *  marker with no counted iterations. */
interface IndexProgress {
  currentPhase: number
  totalPhases: number
  description: string
  current: number | null
  total: number | null
  currentItem: string | null
}

// =============================================================================
// State
// =============================================================================

interface WizardCacheState {
  // Step 2: Project Type Detection (transient)
  isDetectingProjectType: boolean
  projectTypeDetectionError: string | null

  // Step 3: Module Analysis (transient)
  isAnalyzing: boolean
  analysisProgress: AnalysisProgress | null
  analysisMetrics: AnalysisMetrics | null
  analysisError: string | null

  // Step 3: Search/Filter (UI state)
  searchQuery: string
  filterType: string | null

  // Step 5: Root Context Generation (transient)
  isGeneratingRootContext: boolean
  rootContextError: string | null

  // Step 7: Island Detection (transient)
  isDetectingIslands: boolean
  islandDetectionError: string | null

  // Step 8: Lighthouse Generation (transient)
  isGeneratingLighthouse: boolean
  lighthouseError: string | null

  // Indexing (transient)
  isIndexing: boolean
  indexingError: string | null
  indexProgress: IndexProgress | null

  // Actions: Project Type Detection
  setIsDetectingProjectType: (isDetecting: boolean) => void
  setProjectTypeDetectionError: (error: string | null) => void

  // Actions: Module Analysis
  setIsAnalyzing: (isAnalyzing: boolean) => void
  setAnalysisProgress: (progress: AnalysisProgress | null) => void
  setAnalysisMetrics: (metrics: AnalysisMetrics | null) => void
  setAnalysisError: (error: string | null) => void

  // Actions: Search/Filter
  setSearchQuery: (query: string) => void
  setFilterType: (type: string | null) => void

  // Actions: Root Context
  setIsGeneratingRootContext: (isGenerating: boolean) => void
  setRootContextError: (error: string | null) => void

  // Actions: Island Detection
  setIsDetectingIslands: (isDetecting: boolean) => void
  setIslandDetectionError: (error: string | null) => void

  // Actions: Lighthouse
  setIsGeneratingLighthouse: (isGenerating: boolean) => void
  setLighthouseError: (error: string | null) => void

  // Actions: Indexing
  setIsIndexing: (isIndexing: boolean) => void
  setIndexingError: (error: string | null) => void
  setIndexProgress: (progress: IndexProgress | null) => void

  // Reset
  reset: () => void
}

// =============================================================================
// Initial State
// =============================================================================

const initialState = {
  isDetectingProjectType: false,
  projectTypeDetectionError: null,
  isAnalyzing: false,
  analysisProgress: null,
  analysisMetrics: null,
  analysisError: null,
  searchQuery: '',
  filterType: null,
  isGeneratingRootContext: false,
  rootContextError: null,
  isDetectingIslands: false,
  islandDetectionError: null,
  isGeneratingLighthouse: false,
  lighthouseError: null,
  isIndexing: false,
  indexingError: null,
  indexProgress: null,
}

// =============================================================================
// Store (NO persist middleware)
// =============================================================================

export const useWizardCacheStore = create<WizardCacheState>()((set) => ({
  ...initialState,

  // Project Type Detection
  setIsDetectingProjectType: (isDetectingProjectType) => {
    set({ isDetectingProjectType })
  },

  setProjectTypeDetectionError: (projectTypeDetectionError) => {
    set({ projectTypeDetectionError, isDetectingProjectType: false })
  },

  // Module Analysis
  setIsAnalyzing: (isAnalyzing) => {
    set({ isAnalyzing })
  },

  setAnalysisProgress: (analysisProgress) => {
    set({ analysisProgress })
  },

  setAnalysisMetrics: (analysisMetrics) => {
    set({ analysisMetrics })
  },

  setAnalysisError: (analysisError) => {
    set({ analysisError, isAnalyzing: false })
  },

  // Search/Filter
  setSearchQuery: (searchQuery) => {
    set({ searchQuery })
  },

  setFilterType: (filterType) => {
    set({ filterType })
  },

  // Root Context
  setIsGeneratingRootContext: (isGeneratingRootContext) => {
    set({ isGeneratingRootContext })
  },

  setRootContextError: (rootContextError) => {
    set({ rootContextError, isGeneratingRootContext: false })
  },

  // Island Detection
  setIsDetectingIslands: (isDetectingIslands) => {
    set({ isDetectingIslands })
  },

  setIslandDetectionError: (islandDetectionError) => {
    set({ islandDetectionError, isDetectingIslands: false })
  },

  // Lighthouse
  setIsGeneratingLighthouse: (isGeneratingLighthouse) => {
    set({ isGeneratingLighthouse })
  },

  setLighthouseError: (lighthouseError) => {
    set({ lighthouseError, isGeneratingLighthouse: false })
  },

  // Indexing
  setIsIndexing: (isIndexing) => {
    set({ isIndexing })
  },

  setIndexingError: (indexingError) => {
    set({ indexingError, isIndexing: false })
  },

  setIndexProgress: (indexProgress) => {
    set({ indexProgress })
  },

  // Reset
  reset: () => {
    set(initialState)
  },
}))
