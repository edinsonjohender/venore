// =============================================================================
// Agent Profiles — Types and color helpers
// =============================================================================

// -----------------------------------------------------------------------------
// Domain types
// -----------------------------------------------------------------------------

export type AgentStage = 'triager' | 'specialist' | 'reporter' | 'subagent'
export type PipelineRunStatus = 'running' | 'completed' | 'failed'

export type RuleSeverity = 'critical' | 'warning' | 'info'

export interface AgentProfile {
  id: string
  name: string
  description: string
  stage: AgentStage
  provider: string
  model: string
  temperature: number
  systemPrompt: string
  maxTokensPerRun: number
  isTemplate: boolean
  isEnabled: boolean
  ruleIds: string[]
  toolIds: string[]
}

export interface AgentRule {
  id: string
  name: string
  description: string
  scope: string[]
  severity: RuleSeverity
  isActive: boolean
  isTemplate: boolean
}

export interface AgentTeam {
  id: string
  name: string
  description: string
  profileIds: string[]
  isTemplate: boolean
}

export interface PipelineRun {
  id: string
  teamId: string
  teamName: string
  taskType: string
  title: string
  status: PipelineRunStatus
  prNumber: number | null
  projectPath: string
  startedAt: string
  finishedAt: string | null
  durationMs: number
  totalTokens: number
  createdAt: string
  prAuthor: string | null
  prAuthorAvatar: string | null
  prAdditions: number | null
  prDeletions: number | null
  prChangedFiles: number | null
  depthLevel: string | null
}

export interface PipelineStep {
  id: string
  runId: string
  profileId: string
  profileName: string
  stage: string
  status: string
  inputContext: string
  output: string
  provider: string
  model: string
  promptTokens: number
  completionTokens: number
  totalTokens: number
  durationMs: number
  error: string | null
  stepOrder: number
  startedAt: string
  finishedAt: string | null
}

export interface ConsoleEntry {
  timestamp: string
  agentName: string
  stage: string
  message: string
}

// -----------------------------------------------------------------------------
// Tool Category & Definition
// -----------------------------------------------------------------------------

export interface ToolCategory {
  id: string
  name: string
  description: string
  icon: string
  color: string
  displayOrder: number
  isTemplate: boolean
}

export interface ToolDefinition {
  id: string
  name: string
  description: string
  categoryId: string
  parametersJson: string
  isReadOnly: boolean
  isEnabled: boolean
  isTemplate: boolean
}

export interface ChatMode {
  id: string
  name: string
  description: string
  categoryIds: string[]
  toolIds: string[]
  subAgentIds: string[]
  ruleIds: string[]
  promptId: string | null
  isTemplate: boolean
  /** "code" | "knowledge" | null */
  isDefaultForKind: string | null
}

// -----------------------------------------------------------------------------
// Color helpers
// -----------------------------------------------------------------------------

export const STAGE_COLORS: Record<AgentStage, { bg: string; text: string; border: string; glow: string }> = {
  triager:    { bg: 'bg-blue-500/15',  text: 'text-blue-400',  border: 'border-blue-500/30',  glow: 'bg-blue-500'  },
  specialist: { bg: 'bg-amber-500/15', text: 'text-amber-400', border: 'border-amber-500/30', glow: 'bg-amber-500' },
  reporter:   { bg: 'bg-green-500/15', text: 'text-green-400', border: 'border-green-500/30', glow: 'bg-green-500' },
  subagent:   { bg: 'bg-purple-500/15', text: 'text-purple-400', border: 'border-purple-500/30', glow: 'bg-purple-500' },
}

export const SEVERITY_COLORS: Record<RuleSeverity, { bg: string; text: string }> = {
  critical: { bg: 'bg-red-500/15',   text: 'text-red-400'   },
  warning:  { bg: 'bg-amber-500/15', text: 'text-amber-400' },
  info:     { bg: 'bg-blue-500/15',  text: 'text-blue-400'  },
}

export const RUN_STATUS_COLORS: Record<PipelineRunStatus, { bg: string; text: string }> = {
  running:   { bg: 'bg-brand/15',     text: 'text-brand'     },
  completed: { bg: 'bg-green-500/15', text: 'text-green-400' },
  failed:    { bg: 'bg-red-500/15',   text: 'text-red-400'   },
}

// -----------------------------------------------------------------------------
// Pipeline report types
// -----------------------------------------------------------------------------

export type FindingSeverity = 'critical' | 'warning' | 'info' | 'good'
export type CategoryStatus = 'good' | 'warning' | 'critical'

export interface ReportCategory {
  name: string
  score: number           // 0-100
  status: CategoryStatus
  findings_count: number
}

export interface ReportFinding {
  title: string
  category: string
  severity: FindingSeverity
  description: string
}

export interface PipelineReport {
  overall_score: number   // 0-100
  summary: string
  categories: ReportCategory[]
  findings: ReportFinding[]
}

export const FINDING_SEVERITY_COLORS: Record<FindingSeverity, { bg: string; text: string }> = {
  critical: { bg: 'bg-red-500/15',   text: 'text-red-400' },
  warning:  { bg: 'bg-amber-500/15', text: 'text-amber-400' },
  info:     { bg: 'bg-blue-500/15',  text: 'text-blue-400' },
  good:     { bg: 'bg-green-500/15', text: 'text-green-400' },
}

export const CATEGORY_STATUS_COLORS: Record<CategoryStatus, { bg: string; text: string; border: string }> = {
  good:     { bg: 'bg-green-500/10', text: 'text-green-400', border: 'border-green-500/30' },
  warning:  { bg: 'bg-amber-500/10', text: 'text-amber-400', border: 'border-amber-500/30' },
  critical: { bg: 'bg-red-500/10',   text: 'text-red-400',   border: 'border-red-500/30' },
}

