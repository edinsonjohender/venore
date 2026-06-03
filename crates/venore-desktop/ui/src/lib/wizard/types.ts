// =============================================================================
// Wizard Types - Type definitions for the onboarding wizard
// =============================================================================

// -----------------------------------------------------------------------------
// Enums and Constants
// -----------------------------------------------------------------------------

export type DepthLevel = 'minimal' | 'normal' | 'detailed' | 'expert'

export type ProjectState = 'planning' | 'active' | 'maintenance' | 'legacy' | 'archived'

export type TeamSize = 'solo' | 'small' | 'medium' | 'large'

export type ProjectGoal =
  | 'onboarding'
  | 'understand'
  | 'document'
  | 'refactor'
  | 'audit'
  | 'maintain'

export type Layer =
  | 'context'
  | 'status'
  | 'connections'
  | 'tests'
  | 'documentation'

export type ModuleType =
  | 'package'
  | 'crate'
  | 'component'
  | 'service'
  | 'library'
  | 'other'

export type GenerationStatus =
  | 'idle'
  | 'running'
  | 'paused'
  | 'completed'
  | 'error'

export type ModuleStatus = 'pending' | 'running' | 'completed' | 'failed'

// Aliases for backwards compatibility
export type AnalysisDepth = DepthLevel
export type LayerType = Layer

// -----------------------------------------------------------------------------
// Step 0: Path Selection
// -----------------------------------------------------------------------------

export interface PathSelectionState {
  projectPath: string | null
  projectName: string | null
  isValid: boolean
  error: string | null
}

// -----------------------------------------------------------------------------
// Step 1: Project Context
// -----------------------------------------------------------------------------

export interface ProjectContext {
  name: string
  description: string
  projectState: ProjectState
  teamSize: TeamSize
  goals: ProjectGoal[]
  architecture?: string
  techDebt?: string
}

// -----------------------------------------------------------------------------
// Step 2: Analysis Rules
// -----------------------------------------------------------------------------

export interface AnalysisRules {
  depthLevel: DepthLevel
  layersToGenerate: Layer[]
  exclusions: string[]
  ragEnabled: boolean
}

export interface ProjectTypeInfo {
  detectedType: string
  confidence: number
  evidence: string[]
  metadata: Record<string, string>
}

// -----------------------------------------------------------------------------
// Step 3: Module Selection
// -----------------------------------------------------------------------------

export type ModuleConfidence = 'high' | 'medium' | 'low'

export interface ModuleInfo {
  id: string
  name: string
  path: string
  fileCount: number
  entryPoint?: string
  description?: string
  moduleType: ModuleType
  confidence: ModuleConfidence // Backend returns 'high'|'medium'|'low'
  hasExistingContext: boolean
}

export interface ModuleSelectionState {
  detectedModules: ModuleInfo[]
  selectedModuleIds: string[]
  isAnalyzing: boolean
  analysisError: string | null
}

// -----------------------------------------------------------------------------
// Step 4: LLM Configuration
// -----------------------------------------------------------------------------

export interface LLMConfig {
  provider: string | null
  model: string | null
  analysisDepth: DepthLevel
}

// -----------------------------------------------------------------------------
// Step 5: Generation
// -----------------------------------------------------------------------------

export interface ModuleGenerationStatus {
  moduleId: string
  moduleName: string
  path?: string  // Full path for UI display
  status: ModuleStatus
  progress?: number
  error?: string
  tokensUsed?: number
  duration?: number
}

export interface GenerationProgress {
  status: GenerationStatus
  currentModule: string | null
  currentIndex: number
  totalModules: number
  completedModules: ModuleGenerationStatus[]
  startTime: number | null
  endTime: number | null
  error: string | null
}

// -----------------------------------------------------------------------------
// Step 6: Sub-Islands
// -----------------------------------------------------------------------------

export interface SubIsland {
  id: string
  name: string
  description: string
  modules: string[]

  // Metadata
  cohesion: number         // 0.0-1.0 internal dependency ratio
  weight: number           // Number of sub-modules
  criticality: number      // Number of incoming dependencies
  level: number            // Hierarchy level (0=root, 1=sub-island, etc)
  parent_id: string | null // Parent island if nested
}

export interface IslandMetrics {
  total_modules: number
  islands_detected: number
  avg_cohesion: number
  critical_modules: string[] // Modules with high incoming dependencies
}

export interface IslandDetectionParams {
  min_modules: number         // Minimum modules to form island (default: 2)
  max_depth: number           // Max path depth for grouping (default: 2)
  cohesion_threshold: number  // Min cohesion 0-1 (default: 0.3)
  weight_threshold: number    // Min sub-features to extract (default: 3)
  dependency_score: number    // Min incoming deps for criticality (default: 3)
}

export interface SubIslandSelectionState {
  detectedIslands: SubIsland[]
  selectedIslandIds: string[]
  isDetecting: boolean
  metrics: IslandMetrics | null
  params: IslandDetectionParams
}

// -----------------------------------------------------------------------------
// Step 7: Lighthouse
// -----------------------------------------------------------------------------

export interface LighthouseSummary {
  projectName: string
  projectPath: string
  configuration: {
    depthLevel: DepthLevel
    layers: Layer[]
    provider: string
    model: string
  }
  statistics: {
    totalFiles: number
    totalModules: number
    contextsGenerated: number
    totalTokens: number
    duration: number
  }
  generatedContexts: {
    moduleName: string
    path: string
    tokensUsed: number
  }[]
}

// -----------------------------------------------------------------------------
// Index Result (new code intelligence flow)
// -----------------------------------------------------------------------------

export interface IndexResult {
  indexed: number
  skipped: number
  removed: number
  modulesDetected: number
  modulesMapped: number
  depsCreated: number
  refsCreated: number
}

// -----------------------------------------------------------------------------
// Step 8: Complete
// -----------------------------------------------------------------------------

export interface WizardResult {
  projectPath: string
  projectName: string
  contextsGenerated: number
  lighthousePath?: string
}

// -----------------------------------------------------------------------------
// Checkpoint System
// -----------------------------------------------------------------------------

export interface CheckpointInfo {
  exists: boolean
  timestamp: number
  step: number
  progress: number
}

export interface WizardCheckpoint {
  version: string
  timestamp: number
  projectPath: string
  currentStep: number
  projectContext: ProjectContext
  analysisRules: AnalysisRules
  projectType: ProjectTypeInfo | null
  selectedModules: string[]
  llmConfig: LLMConfig
  generationProgress: GenerationProgress
  completedModules: ModuleGenerationStatus[]
}

// -----------------------------------------------------------------------------
// Wizard State
// -----------------------------------------------------------------------------

export interface WizardState {
  // Navigation
  currentStep: number
  isOpen: boolean

  // Checkpoint
  hasCheckpoint: boolean
  checkpointInfo: CheckpointInfo | null

  // Actions
  setStep: (step: number) => void
  nextStep: () => void
  prevStep: () => void
  openWizard: () => void
  closeWizard: () => void
  resetWizard: () => void
  loadCheckpoint: (checkpoint: WizardCheckpoint) => void
}

// -----------------------------------------------------------------------------
// Validation
// -----------------------------------------------------------------------------

export interface ValidationResult {
  isValid: boolean
  errors: string[]
}

// -----------------------------------------------------------------------------
// Backend Request/Response Types
// -----------------------------------------------------------------------------

export interface ScanProjectRequest {
  path: string
  extensions: string[]
  ignorePatterns: string[]
}

export interface ScanProjectResponse {
  totalFiles: number
  totalSizeBytes: number
  scanDurationMs: number
}

export interface DetectModulesRequest {
  projectPath: string
}

export interface DetectModulesResponse {
  modules: ModuleInfo[]
  metrics: {
    totalFiles: number
    detectedModules: number
  }
  orphanFiles: number
  detectionDurationMs: number
}

export interface DetectProjectTypeRequest {
  projectPath: string
}

export interface DetectProjectTypeResponse {
  projectType: string
  confidence: number
  evidence: string[]
  metadata: Record<string, string>
}

export interface GenerateModuleContextRequest {
  moduleName: string
  modulePath: string
  moduleFiles: string[]
  depthLevel: DepthLevel
  projectContext: ProjectContext
  provider: string
  model: string
}

export interface GenerateModuleContextResponse {
  content: string
  filePath: string
  tokensUsed: number
}

export interface GenerateBatchContextsRequest {
  moduleIds: string[]
  depthLevel: DepthLevel
  projectContext: ProjectContext
  llmConfig: LLMConfig
}

export interface GenerateBatchContextsResponse {
  batchId: string
}

export interface GenerationProgressEvent {
  batchId: string
  current: number
  total: number
  currentModule: string
  status: ModuleStatus
  tokensUsed?: number
}

export interface SaveCheckpointRequest {
  projectPath: string
  checkpoint: WizardCheckpoint
}

export interface LoadCheckpointResponse {
  exists: boolean
  checkpoint: WizardCheckpoint | null
}

// -----------------------------------------------------------------------------
// UI Props
// -----------------------------------------------------------------------------

export interface OnboardingWizardModalProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  initialPath?: string
  onComplete?: (result: WizardResult) => void
  onCancel?: () => void
}
