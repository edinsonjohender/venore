/**
 * Typed Tauri command wrappers
 *
 * This file exports typed functions that call into the Tauri commands.
 * Centralizes every invoke() call so we get type safety.
 */

import { invoke } from "@tauri-apps/api/core";

// ============================================================================
// COMMAND RESULT - Matches Rust CommandResult<T> serialization
// ============================================================================

interface ErrorResponse {
  code: string;
  message: string;
  details?: unknown;
  timestamp: string;
}

type CommandResponse<T> =
  | { success: "true"; data: T }
  | { success: "false"; error: ErrorResponse };

export class VenoreError extends Error {
  code: string;
  details?: unknown;
  timestamp: string;
  constructor(response: ErrorResponse) {
    super(response.message);
    this.name = "VenoreError";
    this.code = response.code;
    this.details = response.details;
    this.timestamp = response.timestamp;
  }
}

async function cmd<T>(promise: Promise<CommandResponse<T>>): Promise<T> {
  const result = await promise;
  if (result.success === "true") return result.data;
  throw new VenoreError(result.error);
}

// ============================================================================
// TYPES - Deben coincidir con los tipos en Rust
// ============================================================================

// AI connection registry — cross-window chat overlay state.
// Each entry carries a typed `target` so the chat backend can resolve the
// attached entity (knowledge node / code module / hex) into a context block
// every turn.
export type AiConnectionTarget =
  | { kind: 'knowledge_node'; project_path: string; node_id: string; display_name: string }
  | { kind: 'code_module'; project_path: string; module_name: string; module_path: string }
  | { kind: 'hexagon'; project_path: string; feature_id: string; hexagon_id: string; display_name: string };

export interface AiConnectionDto {
  id: string;
  active: boolean;
  windowLabel: string;
  target: AiConnectionTarget;
}

export interface ProjectResponse {
  id: string;
  name: string;
  path: string;
  projectType: string;
}

/** Inventory of what was restored from a committed `.venore/` snapshot when
 *  opening an existing project. Drives the post-open confirmation banner. */
export interface OpenExistingReport {
  project: ProjectResponse;
  hasMemory: boolean;
  hasAnalysis: boolean;
  hasOceanLayout: boolean;
  layerCount: number;
  moduleCount: number;
  hashedModuleCount: number;
}

// Knowledge Island types
export interface FeatureResponse {
  id: string;
  projectId: string;
  name: string;
  description: string;
  status: string;
  priority: string;
  objective: string;
  intensity: string;
  maxHexagonsPerPhase: number;
  autoAdvance: boolean;
  tags: string;
  createdAt: string;
  updatedAt: string;
}

export interface HexagonResponse {
  id: string;
  featureId: string;
  title: string;
  description: string;
  phase: string;
  percentage: number;
  confidence: string;
  risk: string;
  priority: string;
  isDeadEnd: boolean;
  blockedBy: string;
  notesUser: string;
  agentStatus: string;
  createdAt: string;
  updatedAt: string;
}

export interface EvidenceResponse {
  id: string;
  hexagonId: string;
  content: string;
  sourceUrl: string;
  sourceType: string;
  confidence: string;
  createdAt: string;
}

export interface GenerateContextRequest {
  island_id: string;
  provider: string;
}

export interface GenerateContextResponse {
  content: string;
  tokens_used?: number;
}

// LLM - API Key Management
export interface SetApiKeyRequest {
  provider: string;
  api_key: string;
}

export interface ApiKeyStatusResponse {
  has_key: boolean;
}

export interface ConfiguredProvidersResponse {
  providers: string[];
}

// LLM - Task Configuration
export interface SetTaskSettingsRequest {
  task: string;
  provider: string;
  model: string;
  temperature?: number;
  max_tokens?: number;
  timeout_ms?: number;
  streaming?: boolean;
}

export interface TaskSettingsResponse {
  provider: string;
  model: string;
  temperature?: number;
  max_tokens?: number;
  timeout_ms?: number;
  streaming?: boolean;
}

export interface AllTaskSettingsResponse {
  onboarding: TaskSettingsResponse;
  chat: TaskSettingsResponse;
  analysis: TaskSettingsResponse;
  embeddings: TaskSettingsResponse;
}

// LLM - Provider Information
export interface AvailableModelsResponse {
  provider: string;
  models: string[];
  default_model: string;
}

export interface TestConnectionRequest {
  provider: string;
  model?: string;
}

export interface TestConnectionResponse {
  success: boolean;
  latency_ms: number;
  model: string;
  error?: string;
}

// LLM - Boot Data (preloaded at startup)
export interface AIBootDataResponse {
  configured_providers: string[];
  ollama_available: boolean;
  task_settings: AllTaskSettingsResponse;
  available_models: Record<string, string[]>;
}

// LLM - Generation
export interface GenerateTextRequest {
  task: string;
  messages: GenerateMessageRequest[];
  temperature?: number;
  max_tokens?: number;
  provider?: string;
  model?: string;
}

export interface GenerateMessageRequest {
  role: string;
  content: string;
}

export interface GenerateTextResponse {
  content: string;
  provider: string;
  model: string;
  prompt_tokens?: number;
  completion_tokens?: number;
  total_tokens?: number;
}

// ============================================================================
// WIZARD - Onboarding Flow Types
// ============================================================================

// Step 2: Scan Project
export interface ScanProjectRequest {
  project_path: string;
  exclusions: string[];
}

export interface ScanProjectResponse {
  total_files: number;
  extensions: Record<string, number>;
}

// Step 3: Detect Modules
export interface DetectModulesRequest {
  project_path: string;
  depth_level: string;
  layers: string[];
  exclusions: string[];
}

export interface DetectModulesResponse {
  modules: DetectedModule[];
  metrics: ProjectMetrics;
}

export interface DetectedModule {
  id: string;
  name: string;
  path: string;
  file_count: number;
  confidence: 'high' | 'medium' | 'low';
  has_existing_context: boolean;
  entry_point: string | null;
  description: string;
}

export interface ProjectMetrics {
  total_files: number;
  total_modules: number;
  existing_contexts: number;
}


// Checkpoint System (NEW - matches backend DTOs)
export interface CheckpointInfo {
  exists: boolean;
  completed_count: number;
  total_count: number;
  progress_percent: number;
}

export interface Checkpoint {
  version: string;
  project_path?: string;
  started_at: string; // ISO 8601
  last_updated_at: string; // ISO 8601
  wizard_config: WizardConfig;
  total_modules: number;
  completed_module_ids: string[];
}

export interface WizardConfig {
  // Step 1: Project Context
  project_name: string;
  project_description: string;
  project_state: string;
  team_size: string;
  goals: string[];

  // Step 2: Analysis Rules
  depth_level: string;
  layers_to_generate: string[];
  exclusions: string[];

  // Step 2.5: Project Type Detection
  project_type: string;
  project_type_confidence: number;
  project_metadata: Record<string, string>;

  // Step 3: Analysis Result
  total_files_scanned: number;
  total_modules_detected: number;
  module_names: string[];

  // Step 4: Module Selection + LLM Config
  selected_module_names: string[];
  llm_provider: string;
  llm_model: string | null;
  analysis_depth: string;
}

// Project Type Detection
export interface ProjectTypeResponse {
  project_type: 'monorepo' | 'multi-module' | 'single-module';
  framework?: string;
  package_manager?: string;
}

export interface WizardIndexResponse {
  indexed: number
  skipped: number
  removed: number
  modules_detected: number
  modules_mapped: number
  deps_created: number
  refs_created: number
}

// ============================================================================
// Phase 2: Session Management (NEW - 2026-02-06)
// ============================================================================

export interface RestoreWizardSessionResponse {
  wizard_config: WizardConfig;
  completed_module_names: string[];
  total_modules: number;
}

// ============================================================================
// Phase 3: Validation & Recommendations (NEW - 2026-02-06)
// ============================================================================

export type ValidateWizardStepRequest =
  | { step: 'path'; data: { path: string } }
  | { step: 'project_context'; data: {
      name: string;
      description: string;
      state: string;
      team_size: string;
      goals: string[];
    }}
  | { step: 'analysis_rules'; data: {
      depth_level: string;
      layers_to_generate: string[];
      exclusions: string[];
    }}
  | { step: 'module_selection'; data: {
      selected_modules: string[];
    }}
  | { step: 'llm_config'; data: {
      provider: string;
      model: string;
    }};

export interface ValidateWizardStepResponse {
  is_valid: boolean;
  errors: string[];
}

export interface GetRecommendedModulesRequest {
  project_path: string;
}

export interface GetRecommendedModulesResponse {
  recommended_modules: string[];
}

export interface GetModuleGroupsRequest {
  modules: SimpleModule[];
}

export interface SimpleModule {
  name: string;
  path: string;
  file_count: number;
  confidence: string;
  has_entry_point: boolean;
}

export interface GetModuleGroupsResponse {
  high: SimpleModule[];
  medium: SimpleModule[];
  low: SimpleModule[];
}

// ============================================================================
// DASHBOARD - Project overview (matches Rust DTOs in commands/dto/dashboard.rs)
// ============================================================================

export interface GetProjectDashboardRequest {
  project_path: string;
}

export interface ProjectDashboardResponse {
  stats: ProjectStatsDto;
  modules: ModuleSummaryDto[];
  orphan_files: string[];
}

export interface ProjectStatsDto {
  total_modules: number;
  total_connections: number;
  fresh_count: number;
  stale_count: number;
  missing_count: number;
}

export interface ModuleSummaryDto {
  name: string;
  path: string;
  file_count: number;
  dependency_count: number;
  dependent_count: number;
  context_status: "fresh" | "stale" | "missing";
  generated_at: string | null;
  model: string | null;
  provider: string | null;
  context_path: string | null;
  files: string[];
}

// ============================================================================
// OCEAN CANVAS - Layout Types (matches Rust DTOs in commands/dto/ocean.rs)
// ============================================================================

export interface InitializeOceanLayoutRequest {
  project_path: string;
}

export interface MoveOceanNodeRequest {
  project_path: string;
  node_id: string;
  target_col: number;
  target_row: number;
}

export interface SaveOceanCameraRequest {
  project_path: string;
  x: number;
  z: number;
  zoom: number;
}

export interface CreateKnowledgeNodeRequest {
  project_path: string;
  name: string;
  col: number;
  row: number;
}

export interface CreateKnowledgeNodeResponse {
  accepted: boolean;
  node_id: string;
  col: number;
  row: number;
  reason: string | null;
}

export interface CreateLighthouseRequest {
  project_path: string;
  name: string;
  col: number;
  row: number;
}

export type CreateLighthouseResponse = CreateKnowledgeNodeResponse;

export interface DeleteOceanNodeRequest {
  project_path: string;
  node_id: string;
}

export interface RenameOceanNodeRequest {
  project_path: string;
  node_id: string;
  new_name: string;
}

export interface OceanNodeMutationResponse {
  ok: boolean;
  node_id: string;
}

export interface SetNodeLighthouseRequest {
  project_path: string;
  node_id: string;
  /** `null` detaches the node from its current lighthouse cluster. */
  lighthouse_id: string | null;
}

export interface SetNodeLighthouseResponse {
  accepted: boolean;
  node_id: string;
  reason: string | null;
}

export interface LighthouseClusterRequest {
  project_path: string;
  lighthouse_id: string;
}

export interface LighthouseClusterResponse {
  ok: boolean;
  lighthouse_id: string;
  affected_nodes: number;
}

// Knowledge node content layer
export type KnowledgeNodeSubtype = "concept" | "feature" | "decision" | "finding" | "question";

export type SourceAttribution =
  | { kind: "user" }
  | { kind: "ai"; model: string; timestamp: number };

export interface NodeSectionDto {
  id: string;
  name: string;
  content_markdown: string;
  source: SourceAttribution;
  created_at: number;
  updated_at: number;
}

export interface KnowledgeNodeDataResponse {
  node_id: string;
  subtype: KnowledgeNodeSubtype;
  sections: NodeSectionDto[];
  created_at: number;
  updated_at: number;
}

export interface GetKnowledgeNodeRequest {
  project_path: string;
  node_id: string;
}

export interface UpdateNodeSubtypeRequest {
  project_path: string;
  node_id: string;
  subtype: KnowledgeNodeSubtype;
}

export interface AddNodeSectionRequest {
  project_path: string;
  node_id: string;
  name: string;
  content_markdown: string;
  source: SourceAttribution;
}

export interface AddNodeSectionResponse {
  ok: boolean;
  section: NodeSectionDto | null;
}

export interface UpdateNodeSectionRequest {
  project_path: string;
  node_id: string;
  section_id: string;
  name: string | null;
  content_markdown: string | null;
}

export interface DeleteNodeSectionRequest {
  project_path: string;
  node_id: string;
  section_id: string;
}

export interface ReorderNodeSectionsRequest {
  project_path: string;
  node_id: string;
  ordered_section_ids: string[];
}

export interface PromoteToLighthouseRequest {
  project_path: string;
  node_id: string;
}

export interface PromoteToLighthouseResponse {
  accepted: boolean;
  node_id: string;
  reason: string | null;
}

export interface ExtractSectionToNodeRequest {
  project_path: string;
  source_node_id: string;
  section_id: string;
}

export interface ExtractSectionToNodeResponse {
  accepted: boolean;
  new_node_id: string;
  col: number;
  row: number;
  name: string;
  reason: string | null;
}

export interface KnowledgeFieldMutationResponse {
  ok: boolean;
}

export type OceanConnectionKind = "dependency" | "manual"

export interface OceanConnectionDto {
  id: string;
  from_id: string;
  to_id: string;
  kind: OceanConnectionKind;
}

export interface SetLighthouseColorRequest {
  project_path: string;
  lighthouse_id: string;
  /** "#RRGGBB" string. Pass null to clear and revert to derived palette. */
  color: string | null;
}

export interface SetLighthouseColorResponse {
  accepted: boolean;
  reason: string | null;
}

export interface CreateOceanConnectionRequest {
  project_path: string;
  from_id: string;
  to_id: string;
}

export interface CreateOceanConnectionResponse {
  accepted: boolean;
  connection: OceanConnectionDto | null;
  reason: string | null;
}

export interface DeleteOceanConnectionRequest {
  project_path: string;
  connection_id: string;
}

export interface OceanLayoutResponse {
  nodes: OceanNodePosition[];
  connections: OceanConnectionDto[];
  camera: OceanCameraState | null;
  /** Per-lighthouse color overrides ("#RRGGBB"). Lighthouses without an entry
   *  fall back to the deterministic palette. */
  lighthouse_colors: Record<string, string>;
}

export type OceanNodeVariant = "module" | "knowledge_node" | "lighthouse" | "buoy" | "cylinder";

export interface OceanNodePosition {
  module_id: string;
  module_name: string;
  module_path: string;
  col: number;
  row: number;
  user_placed: boolean;
  layers: NodeLayerDto[];
  node_status: string;
  node_variant: OceanNodeVariant;
  lighthouse_id: string | null;
  /** Number of logbook sections — drives stack height for knowledge nodes / lighthouses. */
  section_count: number;
  /** Subtype string for knowledge nodes / lighthouses — null for module nodes. */
  subtype: KnowledgeNodeSubtype | null;
  /** Active runtime states emitted by registered scanners (e.g. overflow).
   *  Always present; may be empty when the rover hasn't visited yet.
   *  Live updates arrive via the `ocean-state-changed` event. */
  states: NodeStateDto[];
}

/** Stable kind id for a node state. New kinds are appended here in lockstep
 *  with the Rust `NodeStateKind` enum and the frontend decorator registry.
 *
 *  `stale` is synthesized client-side from `getStaleModules` and merged into
 *  a node's `states` before the decorator registry resolves them. The Rust
 *  side doesn't ship it via the live `ocean-state-changed` event today. */
export type NodeStateKind = "overflow" | "pending_writes" | "stale";

/** How loud the state should render. Drives decorator intensity / color. */
export type NodeStateSeverity = "info" | "warning" | "severe";

export interface NodeStateDto {
  kind: NodeStateKind;
  severity: NodeStateSeverity;
  computed_at: number;
  /** Free-form bag the producing scanner shipped (e.g. for overflow:
   *  `{ score, sections, total_chars, max_section_chars }`). */
  payload: Record<string, unknown>;
}

// =============================================================================
// Node states + rover status
// =============================================================================

export interface GetNodeStatesRequest {
  project_path: string;
  node_id: string;
}

export interface GetNodeStatesResponse {
  node_id: string;
  states: NodeStateDto[];
}

export interface DismissNodeStateRequest {
  project_path: string;
  node_id: string;
  kind: NodeStateKind;
  /** Unix-epoch seconds. `0` (or any non-positive) clears the dismissal. */
  until_ts: number;
}

export interface GridCellDto {
  col: number;
  row: number;
}

export interface GetRoverStatusRequest {
  project_path: string;
}

export interface RoverStatusResponse {
  current_cell: GridCellDto | null;
  target_cell: GridCellDto | null;
  queue_depth: number;
  idle: boolean;
  last_step_at: number;
  /** False when no rover has been spawned for this project yet. */
  running: boolean;
}

// =============================================================================
// Current events
// =============================================================================

/** Payload of the `ocean-current-progress` Tauri event. Emitted once per tick
 *  per running Current (passive Ocean worker). `current_id` distinguishes
 *  independent currents (e.g. "index_current") so the frontend can render a
 *  separate cursor for each. */
export interface OceanCurrentProgressEvent {
  project_path: string;
  current_id: string;
  current_cell: GridCellDto | null;
  target_cell: GridCellDto | null;
  queue_depth: number;
  idle: boolean;
}

/** Payload of the `ocean-stale-module` Tauri event. Emitted once per drifted
 *  module as the Staleness Current sweeps the ocean — incremental badge fill-in.
 *  (Replaces the old synchronous `getStaleModules` call on project open.) */
export interface OceanStaleModuleEvent {
  project_path: string;
  module_name: string;
  missing_on_disk: boolean;
}

/** Payload of the `ocean-stale-modules-reconciled` Tauri event. Emitted when a
 *  staleness sweep completes: the authoritative, ancestor-collapsed set of
 *  drifted modules. The frontend REPLACES its badge map with this. */
export interface OceanStaleReconcileEvent {
  project_path: string;
  modules: Array<{ module_name: string; missing_on_disk: boolean }>;
}

/** Payload of the `ocean-state-changed` Tauri event. Emitted only when the
 *  rover's evaluation actually changes a node's state vector. */
export interface OceanStateChangedEvent {
  project_path: string;
  node_id: string;
  states: NodeStateDto[];
}

export interface NodeLayerDto {
  type: string;
  status: string;
  details?: Record<string, unknown>;
}

export interface OceanCameraState {
  x: number;
  z: number;
  zoom: number;
}

export interface NodeLayerUpdate {
  module_id: string;
  layers: NodeLayerDto[];
  node_status: string;
}

export interface MoveOceanNodeResponse {
  accepted: boolean;
  node_id: string;
  col: number;
  row: number;
  reason: string | null;
}

export interface MoveOceanNodesEntry {
  node_id: string;
  target_col: number;
  target_row: number;
}

export interface MoveOceanNodesRequest {
  project_path: string;
  moves: MoveOceanNodesEntry[];
}

export interface MoveOceanNodesResponse {
  all_accepted: boolean;
  results: MoveOceanNodeResponse[];
}

export interface GetModuleDetailsRequest {
  project_path: string;
  module_name: string;
}

export interface ModuleDetailsResponse {
  name: string;
  path: string;
  file_count: number;
  entry_point: string | null;
  files: string[];
  dependencies: string[];
  dependents: string[];
  external_deps: string[];
  exports: SymbolInfoDto[];
  layers: NodeLayerDto[];
}

export interface SymbolInfoDto {
  name: string;
  kind: string;
  file: string;
}

// ============================================================================
// MESH - Peer Discovery & Transport
// ============================================================================

export interface MeshProjectProfile {
  language: string | null;
  technologies: string[];
  module_names: string[];
  total_files: number;
  total_modules: number;
  description: string | null;
}

export interface MeshPeerInfo {
  project_id: string;
  project_name: string;
  project_path: string;
  port: number;
  is_alive: boolean;
  profile: MeshProjectProfile | null;
}

export interface MeshTransportStatus {
  running: boolean;
  port: number;
  connected_peers: string[];
}

// ============================================================================
// CHAT - Streaming & Sessions
// ============================================================================

export interface ChatMessageInput {
  role: string;
  content: string;
}

export interface ContextModuleInput {
  name: string;
  path: string;
}

export interface AttachmentInput {
  name: string;
  mime_type: string;
  data_base64: string;
}

export interface FileAttachmentData {
  name: string;
  mime_type: string;
  size: number;
  data_base64: string;
  is_image: boolean;
}

export interface SendChatMessageRequest {
  messages: ChatMessageInput[];
  stream_id: string;
  session_id?: string;
  project_path?: string;
  context_modules?: ContextModuleInput[];
  dev_session_id?: string;
  knowledge_feature_id?: string;
  attachments?: AttachmentInput[];
}

export interface SendChatMessageResponse {
  stream_id: string;
}

export interface ChatStreamDeltaPayload {
  stream_id: string;
  session_id?: string;
  content: string;
}

export interface ChatStreamDonePayload {
  stream_id: string;
  session_id?: string;
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
  provider: string;
  model: string;
}

export interface ChatStreamErrorPayload {
  stream_id: string;
  session_id?: string;
  message: string;
  code: string;
}

// Tool call event payloads (emitted during agentic loop)
export interface ChatToolCallPayload {
  stream_id: string;
  session_id?: string;
  tool_call_id: string;
  tool_name: string;
  arguments: Record<string, unknown>;
}

export interface ChatToolResultPayload {
  stream_id: string;
  session_id?: string;
  tool_call_id: string;
  success: boolean;
  output: string;
}

export interface ChatToolConfirmPayload {
  stream_id: string;
  session_id?: string;
  tool_call_id: string;
  tool_name: string;
  arguments: Record<string, unknown>;
  resource?: string;
}

// Snapshot event payload (auto-commit after file edits)
export interface ChatSnapshotPayload {
  stream_id: string;
  session_id?: string;
  tool_call_id: string;
  commit_hash: string;
}

export interface ChatCompactedPayload {
  stream_id: string;
  session_id?: string;
  action: "pruned" | "compacted";
  tokens_saved: number;
}

// Agent interaction event payloads (ask_user, tasks, plan, sub-agents)

export interface ChatAskUserEventPayload {
  stream_id: string;
  session_id?: string;
  tool_call_id: string;
  question: string;
  options: { label: string; description: string | null }[];
}

export interface ChatTaskUpdateEventPayload {
  stream_id: string;
  session_id?: string;
  tasks: { id: string; subject: string; status: string; description: string }[];
}

export interface ChatPlanReadyEventPayload {
  stream_id: string;
  session_id?: string;
  tool_call_id: string;
  summary: string;
  steps: string[];
}

export interface ChatSubAgentEventPayload {
  stream_id: string;
  session_id?: string;
  agent_id: string;
  agent_type: string;
  task: string;
  status: string;
  result: string | null;
}

// Skills
export interface SkillDto {
  name: string;
  description: string;
  prompt: string;
}

export interface CreateChatSessionRequest {
  name?: string;
  project_id?: string;
}

export interface ChatSessionDto {
  id: string;
  name: string;
  project_id: string | null;
  dev_session_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface GetOrCreateDevSessionChatRequest {
  dev_session_id: string;
  session_name: string;
  project_id?: string;
}

export interface ChatMessageDto {
  id: string;
  session_id: string;
  role: string;
  content: string;
  provider: string | null;
  model: string | null;
  prompt_tokens: number | null;
  completion_tokens: number | null;
  created_at: string;
  attachments_json: string | null;
}

export interface ChatContextOptionDto {
  name: string;
  path: string;
  has_context: boolean;
}

// Snapshot DTO (persisted tool_call_id → commit_hash, enriched with tool call data)
export interface SnapshotDto {
  tool_call_id: string;
  commit_hash: string;
  created_at: string;
  tool_name?: string;
  file_path?: string;
}

// Tool call record (persisted tool execution)
export interface ToolCallRecordDto {
  id: string;
  tool_name: string;
  arguments: Record<string, unknown>;
  success: boolean | null;
  output: string | null;
  commit_hash: string | null;
  created_at: string;
}

// Aggregated token usage for a session
export interface TokenSummaryDto {
  total_prompt_tokens: number;
  total_completion_tokens: number;
  message_count: number;
}

// Session activity (tool calls + snapshots + token usage)
export interface SessionActivityDto {
  tool_calls: ToolCallRecordDto[];
  snapshots: SnapshotDto[];
  token_summary: TokenSummaryDto;
}

// ============================================================================
// RAG - Code Indexing & Search
// ============================================================================

export interface IndexProjectRequest {
  project_path: string;
}

export interface IndexProjectResponse {
  indexed: number;
  skipped: number;
  removed: number;
  graphPopulated: boolean;
  modulesMapped: number | null;
  depsCreated: number | null;
  refsCreated: number | null;
}

export interface AnalyzeAndIndexRequest {
  projectPath: string;
  depth?: "minimal" | "normal" | "detailed" | "expert";
}

export interface AnalyzeAndIndexResponse {
  modulesDetected: number;
  indexed: number;
  skipped: number;
  removed: number;
  modulesMapped: number;
  depsCreated: number;
  refsCreated: number;
}

export interface SearchCodeRequest {
  project_id: string;
  query: string;
  max_results?: number;
  max_context_chars?: number;
}

export interface SearchCodeResponse {
  results: SearchResultDto[];
}

export interface SearchResultDto {
  name: string;
  chunk_type: string;
  content: string;
  relative_path: string;
  line_start: number;
  line_end: number;
  score: number;
}

export interface IndexStatusDto {
  project_id: string;
  is_indexed: boolean;
  total_files: number;
  total_chunks: number;
  last_indexed_at: string | null;
}

export interface RagIndexProgressPayload {
  project_id: string;
  current: number;
  total: number;
  current_file: string;
  status: string;
}

// ============================================================================
// GITHUB - Auth & Repo Linking
// ============================================================================

export interface GitHubAuthStatusResponse {
  authenticated: boolean;
  login: string | null;
  name: string | null;
  avatar_url: string | null;
  gcm_detected: boolean;
  gcm_login: string | null;
  gcm_name: string | null;
  gcm_avatar_url: string | null;
}

export interface GitHubStorePATRequest {
  token: string;
}

export interface GitHubDetectRepoRequest {
  project_path: string;
}

export interface GitHubDetectRepoResponse {
  detected: boolean;
  owner: string | null;
  repo: string | null;
}

// GitHub Pull Requests
export interface GitHubListPullsRequest {
  project_path: string;
  state?: string;
  page?: number;
  per_page?: number;
}

export interface GitHubPullRequestDto {
  number: number;
  title: string;
  state: string;
  author: string;
  author_avatar: string;
  created_at: string;
  updated_at: string;
  html_url: string;
  body: string | null;
  head_ref: string;
  base_ref: string;
  labels: GitHubLabelDto[];
  draft: boolean;
  comments: number;
  review_comments: number;
}

export interface GitHubGetPrDetailRequest {
  project_path: string;
  pr_number: number;
}

export interface GitHubPrDetailResponse {
  number: number;
  title: string;
  state: string;
  author: string;
  author_avatar: string;
  created_at: string;
  updated_at: string;
  html_url: string;
  body: string | null;
  head_ref: string;
  base_ref: string;
  labels: GitHubLabelDto[];
  draft: boolean;
  comments: number;
  review_comments: number;
  additions: number;
  deletions: number;
  changed_files: number;
}

export interface GitHubLabelDto {
  name: string;
  color: string;
}

export interface GitHubListPullsResponse {
  pulls: GitHubPullRequestDto[];
  has_more: boolean;
  page: number;
  per_page: number;
}

// GitHub Issues
export interface GitHubListIssuesRequest {
  project_path: string;
  state?: string;
  page?: number;
  per_page?: number;
}

export interface GitHubIssueDto {
  number: number;
  title: string;
  state: string;
  author: string;
  author_avatar: string;
  created_at: string;
  updated_at: string;
  html_url: string;
  body: string | null;
  labels: GitHubLabelDto[];
  assignees: string[];
  comments: number;
}

export interface GitHubListIssuesResponse {
  issues: GitHubIssueDto[];
  has_more: boolean;
  page: number;
  per_page: number;
}

// GitHub PR Files
export interface GitHubGetPrFilesRequest {
  project_path: string;
  pr_number: number;
}

export interface GitHubPrFileDto {
  filename: string;
  status: string;
  additions: number;
  deletions: number;
  patch: string | null;
}

// =============================================================================
// Pending logbook writes (AI write preview)
// =============================================================================

export interface PendingWriteDto {
  write_id: string;
  project_path: string;
  node_id: string;
  session_id: string | null;
  /** "create" | "edit" */
  kind: "create" | "edit";
  /** Only set when kind = "edit". */
  section_id: string | null;
  baseline_name: string | null;
  baseline_content: string | null;
  name: string;
  content_markdown: string;
  ai_prompt: string;
  ai_model: string;
  diff_patch: string | null;
  additions: number;
  deletions: number;
  created_at: number;
}

export interface ListPendingWritesRequest {
  project_path: string;
  node_id: string;
}

export interface ListPendingWritesResponse {
  writes: PendingWriteDto[];
}

export interface ListSessionPendingWritesRequest {
  session_id: string;
}

export interface AcceptPendingWriteRequest {
  write_id: string;
}

export interface AcceptPendingWriteResponse {
  ok: boolean;
  section_id: string | null;
}

export interface DiscardPendingWriteRequest {
  write_id: string;
}

export interface DiscardPendingWriteResponse {
  ok: boolean;
}

export interface RegeneratePendingWriteRequest {
  write_id: string;
}

export interface RegeneratePendingWriteResponse {
  ok: boolean;
  write: PendingWriteDto | null;
}

export interface AiWriteProposedEvent {
  project_path: string;
  node_id: string;
  write_id: string;
  /** "create" | "edit" */
  kind: "create" | "edit";
  /** Node name + variant — only set on initial proposal (from chat tool
   *  dispatch). Re-emits on discard/regenerate omit them since the panel
   *  must already be open when the user triggers those. */
  node_name?: string;
  node_variant?: OceanNodeVariant;
  module_path?: string;
}

export interface GitHubPrFilesResponse {
  files: GitHubPrFileDto[];
}

// GitHub Comments
export interface GitHubGetCommentsRequest {
  project_path: string;
  number: number;
  is_pull_request: boolean;
}

export interface GitHubCommentDto {
  id: number;
  author: string;
  author_avatar: string;
  body: string;
  created_at: string;
  html_url: string;
}

export interface GitHubReviewCommentDto {
  id: number;
  author: string;
  author_avatar: string;
  body: string;
  path: string;
  line: number | null;
  diff_hunk: string | null;
  created_at: string;
}

export interface GitHubCommentsResponse {
  comments: GitHubCommentDto[];
  review_comments: GitHubReviewCommentDto[];
}

// GitHub User Repos & Clone
export interface GitHubListUserReposRequest {
  page?: number;
  per_page?: number;
}

export interface GitHubUserRepoDto {
  id: number;
  name: string;
  full_name: string;
  owner: string;
  description: string | null;
  html_url: string;
  clone_url: string;
  is_private: boolean;
  language: string | null;
  stargazers_count: number;
  updated_at: string;
  default_branch: string;
}

export interface GitHubListUserReposResponse {
  repos: GitHubUserRepoDto[];
  has_more: boolean;
  page: number;
  per_page: number;
}

export interface GitHubCloneRepoRequest {
  clone_id: string;
  clone_url: string;
  owner: string;
  repo: string;
  dest_dir: string;
}

export interface GitHubCloneRepoResponse {
  clone_id: string;
}

export interface GitHubInspectDestinationRequest {
  dest_dir: string;
  repo: string;
}

export interface GitHubInspectDestinationResponse {
  exists: boolean;
  path: string;
  is_venore: boolean;
  suggested_name: string;
}

// Clone event payloads (via Tauri listen)
export interface GitHubCloneProgressPayload {
  clone_id: string;
  percent: number | null;
  phase: string;
}

export interface GitHubCloneDonePayload {
  clone_id: string;
  path: string;
  owner: string;
  repo: string;
  /** True when the cloned repo carries a committed `.venore/project.json`,
   *  i.e. the workspace can be opened directly without running the wizard. */
  has_venore: boolean;
}

export interface GitHubCloneErrorPayload {
  clone_id: string;
  message: string;
}

// PR Analysis event payloads (via Tauri listen)
export interface PrAnalysisDeltaPayload {
  stream_id: string;
  content: string;
}

export interface PrAnalysisDonePayload {
  stream_id: string;
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
  provider: string;
  model: string;
}

export interface PrAnalysisErrorPayload {
  stream_id: string;
  message: string;
}

// GitHub Device Flow event payloads (via Tauri listen)
export interface GitHubAuthSuccessPayload {
  login: string;
  name: string | null;
  avatar_url: string;
}

export interface GitHubAuthErrorPayload {
  reason: string;
}

// ============================================================================
// CLOUD - Auth (Optional)
// ============================================================================

export interface CloudAuthStatusResponse {
  authenticated: boolean;
  user_id: string | null;
  email: string | null;
  display_name: string | null;
  avatar_url: string | null;
}

// Cloud auth event payloads (via Tauri listen)
export interface CloudAuthSuccessPayload {
  user_id: string;
  email: string;
  display_name: string;
  avatar_url: string | null;
}

export interface CloudAuthErrorPayload {
  reason: string;
}

export interface CloudSignInWithEmailRequest {
  email: string;
  password: string;
}

export interface CloudSignUpWithEmailRequest {
  email: string;
  password: string;
  display_name: string;
}

export interface CloudSignUpResponse {
  needs_confirmation: boolean;
}

// ============================================================================
// TERMINAL - Embedded PTY
// ============================================================================

export interface SpawnTerminalRequest {
  cwd?: string;
  cols?: number;
  rows?: number;
  label?: string;
}

export interface SpawnTerminalResponse {
  terminal_id: string;
}

export interface WriteTerminalRequest {
  terminal_id: string;
  data: string;
}

export interface ResizeTerminalRequest {
  terminal_id: string;
  cols: number;
  rows: number;
}

export interface KillTerminalRequest {
  terminal_id: string;
}

export interface TerminalOutputPayload {
  terminal_id: string;
  data: string;
}

export interface TerminalAiSpawnedPayload {
  terminal_id: string;
}

export interface TerminalDeadPayload {
  terminal_id: string;
}

export interface TerminalSessionSpawnedPayload {
  terminal_id: string;
  dev_session_id: string;
  label: string;
}

export interface ListTerminalsResponse {
  terminal_ids: string[];
}

// ============================================================================
// EDITOR - File Read/Write
// ============================================================================

export interface ReadFileRequest {
  project_path: string;
  relative_path: string;
}

export interface ReadFileResponse {
  content: string;
  size: number;
}

export interface WriteFileRequest {
  project_path: string;
  relative_path: string;
  content: string;
}

// ============================================================================
// PROMPTS - Centralized LLM Prompt Registry
// ============================================================================

export interface PromptDto {
  id: string;
  name: string;
  category: string;
  provider: string;
  content: string;
  variables: string[];
  isTemplate: boolean;
  isEnabled: boolean;
  version: number;
  createdAt: string;
  updatedAt: string;
}

export interface SetPromptEnabledRequest {
  id: string;
  enabled: boolean;
}

export interface PromptVersionDto {
  id: string;
  promptId: string;
  version: number;
  content: string;
  createdAt: string;
}

export interface UpdatePromptRequest {
  id: string;
  content: string;
}

export interface SaveTaskPromptRequest {
  category: string;
  provider: string;
  content: string;
}

// ============================================================================
// PROJECT MEMORY
// ============================================================================

export interface ProjectMemoryDto {
  id: string;
  projectId: string;
  name: string;
  description: string;
  state: string;
  teamSize: string;
  goals: string[];
  architecture: string;
  techDebt: string;
  responseLanguage: string;
  conventions: string[];
  projectSummary: string;
  createdAt: string;
  updatedAt: string;
}

export interface SaveProjectMemoryRequest {
  projectId: string;
  name: string;
  description: string;
  state: string;
  teamSize: string;
  goals: string[];
  architecture: string;
  techDebt: string;
  responseLanguage: string;
  conventions: string[];
  projectSummary: string;
}

export interface RegenerateSummaryRequest {
  projectId: string;
  projectPath: string;
}

export interface GenerateMemoryRequest {
  projectPath: string;
  userDescription?: string;
  userArchitecture?: string;
  userTechDebt?: string;
  detectedModules?: string[];
  /** "minimal" | "normal" | "detailed" | "expert" — controls analysis depth. */
  depthLevel?: string;
  /** Free-form note from the user about what to change in `previousDraft`. */
  userFeedback?: string;
  /** The draft the LLM produced last run. Sent back so the model can refine
   *  it instead of rewriting from scratch. */
  previousDraft?: GenerateMemoryResponse;
}

export interface GenerateMemoryResponse {
  description: string;
  state: string;
  goals: string[];
  architecture: string;
  techDebt: string;
  projectSummary: string;
}

// ============================================================================
// STALE-MODULE DETECTION
// ============================================================================

/** Summary returned by `resnapshot_project`. Drives the "Snapshot refreshed"
 *  toast in the workspace UI. Empty/zero counts are normal for projects
 *  with no source files; failures surface as a thrown error, not a zeroed
 *  report. */
export interface ResnapshotReport {
  modules: number;
  indexed: number;
  skipped: number;
  removed: number;
  modulesMapped: number;
  depsCreated: number;
  refsCreated: number;
  layersWritten: number;
  hashesWritten: number;
}

export interface StaleModuleDto {
  moduleName: string;
  modulePath: string;
  storedHash: string;
  /** "sha256-MISSING" when the module directory no longer exists on disk. */
  currentHash: string;
  missingOnDisk: boolean;
}

// ============================================================================
// AGENTS - Profiles & Teams
// ============================================================================

export interface AgentProfileDto {
  id: string;
  name: string;
  description: string;
  stage: string;
  systemPrompt: string;
  provider: string;
  model: string;
  temperature: number;
  isTemplate: boolean;
  isEnabled: boolean;
  rulesJson: string;
  criteriaJson: string;
  toolsJson: string;
  maxTokensPerRun: number;
  createdAt: string;
  updatedAt: string;
}

export interface CreateAgentProfileRequest {
  name: string;
  description: string;
  stage: string;
  systemPrompt: string;
  provider: string;
  model: string;
  temperature: number;
  isEnabled?: boolean;
  rulesJson?: string;
  criteriaJson?: string;
  toolsJson?: string;
  maxTokensPerRun?: number;
}

export interface UpdateAgentProfileRequest {
  id: string;
  name?: string;
  description?: string;
  stage?: string;
  systemPrompt?: string;
  provider?: string;
  model?: string;
  temperature?: number;
  isEnabled?: boolean;
  rulesJson?: string;
  criteriaJson?: string;
  toolsJson?: string;
  maxTokensPerRun?: number;
}

export interface AgentRuleDto {
  id: string;
  name: string;
  description: string;
  scope: string[];
  severity: string;
  isActive: boolean;
  isTemplate: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface CreateAgentRuleRequest {
  name: string;
  description: string;
  scope: string[];
  severity: string;
  isActive?: boolean;
}

export interface UpdateAgentRuleRequest {
  id: string;
  name?: string;
  description?: string;
  scope?: string[];
  severity?: string;
  isActive?: boolean;
}

export interface AgentTeamDto {
  id: string;
  name: string;
  description: string;
  profileIds: string[];
  isTemplate: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface CreateAgentTeamRequest {
  name: string;
  description: string;
  profileIds: string[];
}

export interface UpdateAgentTeamRequest {
  id: string;
  name?: string;
  description?: string;
  profileIds?: string[];
}

// ============================================================================
// TOOL CATEGORIES & DEFINITIONS
// ============================================================================

export interface ToolCategoryDto {
  id: string;
  name: string;
  description: string;
  icon: string;
  color: string;
  displayOrder: number;
  isTemplate: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface CreateToolCategoryRequest {
  name: string;
  description: string;
  icon: string;
  color: string;
  displayOrder?: number;
}

export interface UpdateToolCategoryRequest {
  id: string;
  name?: string;
  description?: string;
  icon?: string;
  color?: string;
  displayOrder?: number;
}

export interface ToolDefinitionDto {
  id: string;
  name: string;
  description: string;
  categoryId: string;
  parametersJson: string;
  isReadOnly: boolean;
  isEnabled: boolean;
  isTemplate: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface CreateToolDefinitionRequest {
  name: string;
  description: string;
  categoryId: string;
  parametersJson?: string;
  isReadOnly?: boolean;
  isEnabled?: boolean;
}

export interface UpdateToolDefinitionRequest {
  id: string;
  name?: string;
  description?: string;
  categoryId?: string;
  parametersJson?: string;
  isReadOnly?: boolean;
  isEnabled?: boolean;
}

// ----------------------------------------------------------------------------
// Chat Mode (named bundle of tools/sub-agents/rules/prompt by project kind)
// ----------------------------------------------------------------------------

export interface ChatModeDto {
  id: string;
  name: string;
  description: string;
  categoryIds: string[];
  toolIds: string[];
  subAgentIds: string[];
  ruleIds: string[];
  promptId: string | null;
  isTemplate: boolean;
  /** "code" | "knowledge" | null. Picks the default for that project kind. */
  isDefaultForKind: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface CreateChatModeRequest {
  name: string;
  description?: string;
  categoryIds?: string[];
  toolIds?: string[];
  subAgentIds?: string[];
  ruleIds?: string[];
  promptId?: string;
  isDefaultForKind?: string;
}

export interface UpdateChatModeRequest {
  id: string;
  name?: string;
  description?: string;
  categoryIds?: string[];
  toolIds?: string[];
  subAgentIds?: string[];
  ruleIds?: string[];
  promptId?: string;
  /** Pass empty string to clear, a kind ("code"|"knowledge") to set. */
  isDefaultForKind?: string;
}

// ============================================================================
// PIPELINE - Execution & History
// ============================================================================

export interface StartPipelineRequest {
  projectPath: string;
  prNumber: number;
  prTitle: string;
  teamId?: string;
}

export interface StartPipelineResponse {
  runId: string;
}

export type AnalysisDepthLevel = 'minimal' | 'normal' | 'detailed' | 'expert';

export interface PipelineRunDto {
  id: string;
  teamId: string;
  teamName: string;
  taskType: string;
  title: string;
  status: string;
  prNumber: number | null;
  projectPath: string;
  startedAt: string;
  finishedAt: string | null;
  durationMs: number;
  totalTokens: number;
  createdAt: string;
  prAuthor: string | null;
  prAuthorAvatar: string | null;
  prAdditions: number | null;
  prDeletions: number | null;
  prChangedFiles: number | null;
  depthLevel: string | null;
}

export interface AuthorStatsDto {
  login: string;
  avatarUrl: string;
  totalRuns: number;
  avgOverallScore: number;
  lastOverallScore: number;
  lastRunAt: string;
}

export interface CategoryAverageDto {
  categoryName: string;
  avgScore: number;
  runCount: number;
}

export interface RunAnalysisContextDto {
  run: PipelineRunDto;
  authorStats: AuthorStatsDto | null;
  authorCategoryAverages: CategoryAverageDto[];
  projectCategoryAverages: CategoryAverageDto[];
}

export interface PipelineStepDto {
  id: string;
  runId: string;
  profileId: string;
  profileName: string;
  stage: string;
  status: string;
  inputContext: string;
  output: string;
  provider: string;
  model: string;
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
  durationMs: number;
  error: string | null;
  stepOrder: number;
  startedAt: string;
  finishedAt: string | null;
}

// Pipeline event payloads (from Tauri events)
export interface PipelineRunStartedPayload {
  runId: string;
  title: string;
  teamName: string;
}

export interface PipelineStepStartedPayload {
  runId: string;
  stepId: string;
  agentName: string;
  stage: string;
}

export interface PipelineStepCompletedPayload {
  runId: string;
  stepId: string;
  agentName: string;
  stage: string;
  durationMs: number;
  tokens: number;
}

export interface PipelineStepFailedPayload {
  runId: string;
  stepId: string;
  agentName: string;
  stage: string;
  error: string;
}

export interface PipelineConsolePayload {
  runId: string;
  agentName: string;
  stage: string;
  message: string;
}

export interface PipelineRunCompletedPayload {
  runId: string;
  durationMs: number;
  totalTokens: number;
}

export interface PipelineRunFailedPayload {
  runId: string;
  error: string;
}

// ============================================================================
// CONTEXT UPDATER - Auto-detect and regenerate stale .context.md
// ============================================================================

export interface CommitSummaryDto {
  hash: string;
  short_hash: string;
  message: string;
}

export interface AffectedModuleDto {
  name: string;
  path: string;
  changed_files: string[];
}

export interface UpdateReportResponse {
  commits: CommitSummaryDto[];
  affected_modules: AffectedModuleDto[];
  latest_commit: string;
}

export interface RunUpdateRequest {
  project_path: string;
  module_names: string[];
  provider: string;
  model: string;
  depth_level: string;
  latest_commit: string;
}

export interface UpdaterStateResponse {
  selected_branch: string;
  last_sync_commit: string | null;
  last_sync_at: string | null;
  auto_update_enabled: boolean;
  check_interval_minutes: number;
}

export interface UpdateUpdaterStateRequest {
  project_path: string;
  selected_branch: string;
  auto_update_enabled: boolean;
  check_interval_minutes: number;
}

export interface ContextUpdateProgressPayload {
  current: number;
  total: number;
  module_id: string;
  status: string;
  tokens_used: number;
  error: string | null;
}

export interface ContextUpdateCompletePayload {
  total_completed: number;
  total_failed: number;
  duration_ms: number;
}

// ============================================================================
// SESSIONS - Branch-per-session workflow
// ============================================================================

export interface CreateSessionRequest {
  name: string;
  objective: string;
  project_path: string;
  project_id: string;
  base_branch: string;
  branch_name: string;
}

export interface SessionDto {
  id: string;
  name: string;
  objective: string;
  project_id: string;
  base_branch: string;
  session_branch: string;
  worktree_path: string;
  status: string;
  files_changed: number;
  additions: number;
  deletions: number;
  created_at: string;
  updated_at: string;
}

export interface SessionDiffFileDto {
  filename: string;
  status: string;
  additions: number;
  deletions: number;
  patch: string | null;
}

export interface SessionFileChangedPayload {
  dev_session_id: string;
  filename: string;
  status: string;
  additions: number;
  deletions: number;
  patch: string | null;
}

export interface SessionCommitDto {
  hash: string;
  short_hash: string;
  message: string;
  author: string;
  date: string;
}

export interface SessionDiffRequest {
  session_id: string;
  project_path: string;
}

export interface ListBranchesRequest {
  project_path: string;
}

export interface ListBranchesResponse {
  branches: string[];
  is_local_git: boolean;
}

// ============================================================================
// SYSTEM - Health Checks
// ============================================================================

export interface SystemCheckResponse {
  success: boolean;
  message: string;
}

// ============================================================================
// COMMANDS
// ============================================================================

export const tauriApi = {
  // System
  initializeBackend: () =>
    cmd(invoke<CommandResponse<SystemCheckResponse>>("initialize_backend")),

  checkBackend: () =>
    cmd(invoke<CommandResponse<SystemCheckResponse>>("check_backend")),

  checkDatabase: () =>
    cmd(invoke<CommandResponse<SystemCheckResponse>>("check_database")),

  checkLlmGateway: () =>
    cmd(invoke<CommandResponse<SystemCheckResponse>>("check_llm_gateway")),

  resizeWindow: (width: number, height: number) =>
    cmd(invoke<CommandResponse<void>>("resize_window", { width, height })),

  /**
   * Open a fresh Venore main window in the SAME process. Boots into the
   * launcher; the user picks a project from there. Each opened project
   * registers as its own mesh peer (keyed by project_id), so two windows
   * with different projects can talk to each other without spawning
   * separate OS processes.
   */
  openMainWindow: () =>
    cmd(invoke<CommandResponse<void>>("open_main_window")),

  openChatWindow: (sessionId: string, projectPath: string, sessionName: string, projectId?: string) =>
    cmd(invoke<CommandResponse<void>>("open_chat_window", { sessionId, projectPath, sessionName, projectId })),

  openNodeWindow: (projectPath: string, moduleId: string, moduleName: string, nodeVariant: string) =>
    cmd(invoke<CommandResponse<void>>("open_node_window", { projectPath, moduleId, moduleName, nodeVariant })),

  readFileForAttachment: (path: string) =>
    cmd(invoke<CommandResponse<FileAttachmentData>>("read_file_for_attachment", { path })),

  // AI connection registry
  listAiConnections: () =>
    cmd(invoke<CommandResponse<AiConnectionDto[]>>("list_ai_connections")),

  registerAiConnection: (id: string, target: AiConnectionTarget, windowLabel?: string) =>
    cmd(invoke<CommandResponse<AiConnectionDto[]>>("register_ai_connection", { id, target, windowLabel })),

  unregisterAiConnection: (id: string) =>
    cmd(invoke<CommandResponse<AiConnectionDto[]>>("unregister_ai_connection", { id })),

  toggleAiConnection: (id: string) =>
    cmd(invoke<CommandResponse<AiConnectionDto[]>>("toggle_ai_connection", { id })),

  disconnectAllAiConnections: () =>
    cmd(invoke<CommandResponse<AiConnectionDto[]>>("disconnect_all_ai_connections")),

  // Health check
  health: () => cmd(invoke<CommandResponse<string>>("health")),

  // Projects
  registerProject: (projectPath: string) =>
    cmd(invoke<CommandResponse<ProjectResponse>>("register_project", { projectPath })),

  /**
   * Open an already-Venorized project. Strict: errors with NotFound when the
   * folder has no `.venore/project.json` (i.e. it's not a Venore project yet).
   * Returns a restoration report the UI can show as a banner.
   */
  openExistingProject: (projectPath: string) =>
    cmd(invoke<CommandResponse<OpenExistingReport>>("open_existing_project", { projectPath })),

  getProject: (id: string) =>
    cmd(invoke<CommandResponse<ProjectResponse>>("get_project", { id })),

  listProjects: () =>
    cmd(invoke<CommandResponse<ProjectResponse[]>>("list_projects")),

  createKnowledgeProject: (name: string, description: string) =>
    cmd(invoke<CommandResponse<ProjectResponse>>("create_knowledge_project", { name, description })),

  // Knowledge — Features
  createKnowledgeFeature: (request: { projectId: string; name: string; description: string; objective?: string; intensity?: string; maxHexagonsPerPhase?: number; autoAdvance?: boolean; tags?: string }) =>
    cmd(invoke<CommandResponse<FeatureResponse>>("create_knowledge_feature", { request })),

  getKnowledgeFeature: (id: string) =>
    cmd(invoke<CommandResponse<FeatureResponse>>("get_knowledge_feature", { id })),

  listKnowledgeFeatures: (projectId: string) =>
    cmd(invoke<CommandResponse<FeatureResponse[]>>("list_knowledge_features", { projectId })),

  updateKnowledgeFeature: (request: { id: string; name: string; description: string; status: string; priority: string; objective?: string; intensity?: string; maxHexagonsPerPhase?: number; autoAdvance?: boolean; tags?: string }) =>
    cmd(invoke<CommandResponse<FeatureResponse>>("update_knowledge_feature", { request })),

  deleteKnowledgeFeature: (id: string) =>
    cmd(invoke<CommandResponse<void>>("delete_knowledge_feature", { id })),

  // Knowledge — Hexagons
  createKnowledgeHexagon: (featureId: string, title: string, description: string) =>
    cmd(invoke<CommandResponse<HexagonResponse>>("create_knowledge_hexagon", { request: { featureId, title, description } })),

  getKnowledgeHexagon: (id: string) =>
    cmd(invoke<CommandResponse<HexagonResponse>>("get_knowledge_hexagon", { id })),

  listKnowledgeHexagons: (featureId: string) =>
    cmd(invoke<CommandResponse<HexagonResponse[]>>("list_knowledge_hexagons", { featureId })),

  updateKnowledgeHexagon: (request: { id: string; title: string; description: string; phase: string; percentage: number; confidence: string; risk: string; priority: string; isDeadEnd: boolean; blockedBy: string; notesUser: string; agentStatus?: string }) =>
    cmd(invoke<CommandResponse<HexagonResponse>>("update_knowledge_hexagon", { request })),

  deleteKnowledgeHexagon: (id: string) =>
    cmd(invoke<CommandResponse<void>>("delete_knowledge_hexagon", { id })),

  // Knowledge — Evidence
  createKnowledgeEvidence: (request: { hexagonId: string; content: string; sourceUrl: string; sourceType: string; confidence: string }) =>
    cmd(invoke<CommandResponse<EvidenceResponse>>("create_knowledge_evidence", { request })),

  listKnowledgeEvidence: (hexagonId: string) =>
    cmd(invoke<CommandResponse<EvidenceResponse[]>>("list_knowledge_evidence", { hexagonId })),

  deleteKnowledgeEvidence: (id: string) =>
    cmd(invoke<CommandResponse<void>>("delete_knowledge_evidence", { id })),

  // Research Engine
  startResearch: (request: { featureId: string }) =>
    cmd(invoke<CommandResponse<{ runId: string }>>("start_research", { request })),

  pauseResearch: (runId: string) =>
    cmd(invoke<CommandResponse<void>>("pause_research", { runId })),

  stopResearch: (runId: string) =>
    cmd(invoke<CommandResponse<void>>("stop_research", { runId })),

  sendResearchInstruction: (runId: string, instruction: string) =>
    cmd(invoke<CommandResponse<void>>("send_research_instruction", { runId, instruction })),

  getResearchStatus: (featureId: string) =>
    cmd(invoke<CommandResponse<{ runId: string; phase: string; status: string; intensity: string; evaluationRound: number; totalWorkersSpawned: number; totalToolCalls: number; durationMs: number } | null>>("get_research_status", { featureId })),

  // Context
  generateContext: (request: GenerateContextRequest) =>
    cmd(invoke<CommandResponse<GenerateContextResponse>>("generate_context", { request })),

  // LLM - API Key Management
  setApiKey: (request: SetApiKeyRequest) =>
    cmd(invoke<CommandResponse<void>>("set_api_key", { request })),

  getApiKey: (provider: string) =>
    cmd(invoke<CommandResponse<string | null>>("get_api_key", { provider })),

  hasApiKey: (provider: string) =>
    cmd(invoke<CommandResponse<ApiKeyStatusResponse>>("has_api_key", { provider })),

  removeApiKey: (provider: string) =>
    cmd(invoke<CommandResponse<void>>("remove_api_key", { provider })),

  getConfiguredProviders: () =>
    cmd(invoke<CommandResponse<ConfiguredProvidersResponse>>("get_configured_providers")),

  // LLM - Task Configuration
  getTaskSettings: (task: string) =>
    cmd(invoke<CommandResponse<TaskSettingsResponse>>("get_task_settings", { task })),

  setTaskSettings: (request: SetTaskSettingsRequest) =>
    cmd(invoke<CommandResponse<void>>("set_task_settings", { request })),

  getAllTaskSettings: () =>
    cmd(invoke<CommandResponse<AllTaskSettingsResponse>>("get_all_task_settings")),

  resetTaskSettings: (task: string) =>
    cmd(invoke<CommandResponse<void>>("reset_task_settings", { task })),

  // LLM - Provider Information
  listProviders: () =>
    cmd(invoke<CommandResponse<string[]>>("list_providers")),

  getAvailableModels: (provider: string) =>
    cmd(invoke<CommandResponse<AvailableModelsResponse>>("get_available_models", { provider })),

  testConnection: (request: TestConnectionRequest) =>
    cmd(invoke<CommandResponse<TestConnectionResponse>>("test_connection", { request })),

  getOllamaModels: () =>
    cmd(invoke<CommandResponse<string[]>>("get_ollama_models")),

  // LLM - Generation
  generateText: (request: GenerateTextRequest) =>
    cmd(invoke<CommandResponse<GenerateTextResponse>>("generate_text", { request })),

  // LLM - Boot Data (preloaded at startup)
  getAIBootData: () =>
    cmd(invoke<CommandResponse<AIBootDataResponse>>("get_ai_boot_data")),

  // ============================================================================
  // WIZARD COMMANDS
  // ============================================================================

  // Step 2: Scan Project
  scanProjectFiles: (request: ScanProjectRequest) =>
    cmd(invoke<CommandResponse<ScanProjectResponse>>("scan_project_files", { request })),

  // Step 3: Detect Modules
  detectProjectModules: (request: DetectModulesRequest) =>
    cmd(invoke<CommandResponse<DetectModulesResponse>>("detect_project_modules", { request })),

  detectProjectType: (project_path: string) =>
    cmd(invoke<CommandResponse<ProjectTypeResponse>>("detect_project_type", { project_path })),

  /** Cancel an in-flight detect_project_modules / wizard_index_project run.
   *  Flips the registered CancellationToken in the backend; the in-flight task
   *  bails at its next checkpoint with VenoreError::Cancelled. */
  cancelWizardSession: (projectPath: string) =>
    cmd(invoke<CommandResponse<boolean>>("cancel_wizard_session", { projectPath })),

  wizardIndexProject: (projectPath: string, layers?: string[], exclusions?: string[]) =>
    cmd(invoke<CommandResponse<WizardIndexResponse>>("wizard_index_project", { projectPath, layers, exclusions })),

  // Checkpoint System
  checkWizardCheckpoint: (path: string) =>
    cmd(invoke<CommandResponse<CheckpointInfo | null>>("check_wizard_checkpoint", { path })),

  loadFullCheckpoint: (path: string) =>
    cmd(invoke<CommandResponse<Checkpoint>>("load_full_checkpoint", { path })),

  deleteWizardCheckpoint: (path: string) =>
    cmd(invoke<CommandResponse<void>>("delete_wizard_checkpoint", { path })),

  // ============================================================================
  // Phase 2: Session Management
  // ============================================================================

  restoreWizardSession: (projectPath: string) =>
    cmd(invoke<CommandResponse<RestoreWizardSessionResponse>>("restore_wizard_session", { projectPath })),

  // ============================================================================
  // Phase 3: Validation & Recommendations
  // ============================================================================

  validateWizardStep: (request: ValidateWizardStepRequest) =>
    cmd(invoke<CommandResponse<ValidateWizardStepResponse>>("validate_wizard_step", { request })),

  getRecommendedModules: (request: GetRecommendedModulesRequest) =>
    cmd(invoke<CommandResponse<GetRecommendedModulesResponse>>("get_recommended_modules", { request })),

  getModuleGroups: (request: GetModuleGroupsRequest) =>
    cmd(invoke<CommandResponse<GetModuleGroupsResponse>>("get_module_groups", { request })),

  // ============================================================================
  // DASHBOARD COMMANDS
  // ============================================================================

  getProjectDashboard: (request: GetProjectDashboardRequest) =>
    cmd(invoke<CommandResponse<ProjectDashboardResponse>>("get_project_dashboard", { request })),

  // ============================================================================
  // OCEAN CANVAS COMMANDS
  // ============================================================================

  initializeOceanLayout: (request: InitializeOceanLayoutRequest) =>
    cmd(invoke<CommandResponse<OceanLayoutResponse>>("initialize_ocean_layout", { request })),

  computeOceanLayers: (request: InitializeOceanLayoutRequest) =>
    cmd(invoke<CommandResponse<NodeLayerUpdate[]>>("compute_ocean_layers", { request })),

  moveOceanNode: (request: MoveOceanNodeRequest) =>
    cmd(invoke<CommandResponse<MoveOceanNodeResponse>>("move_ocean_node", { request })),

  moveOceanNodes: (request: MoveOceanNodesRequest) =>
    cmd(invoke<CommandResponse<MoveOceanNodesResponse>>("move_ocean_nodes", { request })),

  createKnowledgeNode: (request: CreateKnowledgeNodeRequest) =>
    cmd(invoke<CommandResponse<CreateKnowledgeNodeResponse>>("create_knowledge_node", { request })),

  createLighthouse: (request: CreateLighthouseRequest) =>
    cmd(invoke<CommandResponse<CreateLighthouseResponse>>("create_lighthouse", { request })),

  deleteOceanNode: (request: DeleteOceanNodeRequest) =>
    cmd(invoke<CommandResponse<OceanNodeMutationResponse>>("delete_ocean_node", { request })),

  renameOceanNode: (request: RenameOceanNodeRequest) =>
    cmd(invoke<CommandResponse<OceanNodeMutationResponse>>("rename_ocean_node", { request })),

  setNodeLighthouse: (request: SetNodeLighthouseRequest) =>
    cmd(invoke<CommandResponse<SetNodeLighthouseResponse>>("set_node_lighthouse", { request })),

  dissolveLighthouse: (request: LighthouseClusterRequest) =>
    cmd(invoke<CommandResponse<LighthouseClusterResponse>>("dissolve_lighthouse", { request })),

  deleteLighthouseCluster: (request: LighthouseClusterRequest) =>
    cmd(invoke<CommandResponse<LighthouseClusterResponse>>("delete_lighthouse_cluster", { request })),

  getKnowledgeNode: (request: GetKnowledgeNodeRequest) =>
    cmd(invoke<CommandResponse<KnowledgeNodeDataResponse>>("get_knowledge_node", { request })),

  updateNodeSubtype: (request: UpdateNodeSubtypeRequest) =>
    cmd(invoke<CommandResponse<KnowledgeFieldMutationResponse>>("update_node_subtype", { request })),

  addNodeSection: (request: AddNodeSectionRequest) =>
    cmd(invoke<CommandResponse<AddNodeSectionResponse>>("add_node_section", { request })),

  updateNodeSection: (request: UpdateNodeSectionRequest) =>
    cmd(invoke<CommandResponse<KnowledgeFieldMutationResponse>>("update_node_section", { request })),

  deleteNodeSection: (request: DeleteNodeSectionRequest) =>
    cmd(invoke<CommandResponse<KnowledgeFieldMutationResponse>>("delete_node_section", { request })),

  reorderNodeSections: (request: ReorderNodeSectionsRequest) =>
    cmd(invoke<CommandResponse<KnowledgeFieldMutationResponse>>("reorder_node_sections", { request })),

  extractSectionToNode: (request: ExtractSectionToNodeRequest) =>
    cmd(invoke<CommandResponse<ExtractSectionToNodeResponse>>("extract_section_to_node", { request })),

  promoteToLighthouse: (request: PromoteToLighthouseRequest) =>
    cmd(invoke<CommandResponse<PromoteToLighthouseResponse>>("promote_to_lighthouse", { request })),

  createOceanConnection: (request: CreateOceanConnectionRequest) =>
    cmd(invoke<CommandResponse<CreateOceanConnectionResponse>>("create_ocean_connection", { request })),

  deleteOceanConnection: (request: DeleteOceanConnectionRequest) =>
    cmd(invoke<CommandResponse<KnowledgeFieldMutationResponse>>("delete_ocean_connection", { request })),

  setLighthouseColor: (request: SetLighthouseColorRequest) =>
    cmd(invoke<CommandResponse<SetLighthouseColorResponse>>("set_lighthouse_color", { request })),

  saveOceanCamera: (request: SaveOceanCameraRequest) =>
    cmd(invoke<CommandResponse<void>>("save_ocean_camera", { request })),

  getModuleDetails: (request: GetModuleDetailsRequest) =>
    cmd(invoke<CommandResponse<ModuleDetailsResponse>>("get_module_details", { request })),

  getNodeStates: (request: GetNodeStatesRequest) =>
    cmd(invoke<CommandResponse<GetNodeStatesResponse>>("get_node_states", { request })),

  dismissNodeState: (request: DismissNodeStateRequest) =>
    cmd(invoke<CommandResponse<KnowledgeFieldMutationResponse>>("dismiss_node_state", { request })),

  getRoverStatus: (request: GetRoverStatusRequest) =>
    cmd(invoke<CommandResponse<RoverStatusResponse>>("get_rover_status", { request })),

  // ============================================================================
  // PENDING WRITES (AI write preview)
  // ============================================================================

  listPendingWrites: (request: ListPendingWritesRequest) =>
    cmd(invoke<CommandResponse<ListPendingWritesResponse>>("list_pending_writes", { request })),

  listSessionPendingWrites: (request: ListSessionPendingWritesRequest) =>
    cmd(invoke<CommandResponse<ListPendingWritesResponse>>("list_session_pending_writes", { request })),

  acceptPendingWrite: (request: AcceptPendingWriteRequest) =>
    cmd(invoke<CommandResponse<AcceptPendingWriteResponse>>("accept_pending_write", { request })),

  discardPendingWrite: (request: DiscardPendingWriteRequest) =>
    cmd(invoke<CommandResponse<DiscardPendingWriteResponse>>("discard_pending_write", { request })),

  regeneratePendingWrite: (request: RegeneratePendingWriteRequest) =>
    cmd(invoke<CommandResponse<RegeneratePendingWriteResponse>>("regenerate_pending_write", { request })),

  // ============================================================================
  // RAG COMMANDS
  // ============================================================================

  indexProjectCode: (request: IndexProjectRequest) =>
    cmd(invoke<CommandResponse<IndexProjectResponse>>("index_project_code", { request })),

  searchProjectCode: (request: SearchCodeRequest) =>
    cmd(invoke<CommandResponse<SearchCodeResponse>>("search_project_code", { request })),

  getRagIndexStatus: (projectId: string) =>
    cmd(invoke<CommandResponse<IndexStatusDto>>("get_rag_index_status", { projectId })),

  analyzeAndIndexProject: (request: AnalyzeAndIndexRequest) =>
    cmd(invoke<CommandResponse<AnalyzeAndIndexResponse>>("analyze_and_index_project", { request })),

  // ============================================================================
  // CHAT COMMANDS
  // ============================================================================

  sendChatMessage: (request: SendChatMessageRequest) =>
    cmd(invoke<CommandResponse<SendChatMessageResponse>>("send_chat_message", { request })),

  stopChatStream: (streamId: string) =>
    cmd(invoke<CommandResponse<void>>("stop_chat_stream", { streamId })),

  approveToolCall: (toolCallId: string, approved: boolean, allowSession?: boolean, sessionId?: string, toolName?: string) =>
    cmd(invoke<CommandResponse<void>>("approve_tool_call", { toolCallId, approved, allowSession, sessionId, toolName })),

  clearSessionApprovals: (sessionId: string) =>
    cmd(invoke<CommandResponse<void>>("clear_session_approvals", { sessionId })),

  createChatSession: (request: CreateChatSessionRequest) =>
    cmd(invoke<CommandResponse<ChatSessionDto>>("create_chat_session", { request })),

  listChatSessions: (projectId?: string) =>
    cmd(invoke<CommandResponse<ChatSessionDto[]>>("list_chat_sessions", { projectId })),

  deleteChatSession: (sessionId: string) =>
    cmd(invoke<CommandResponse<void>>("delete_chat_session", { sessionId })),

  renameChatSession: (sessionId: string, name: string) =>
    cmd(invoke<CommandResponse<void>>("rename_chat_session", { sessionId, name })),

  generateChatTitle: (userMessage: string) =>
    cmd(invoke<CommandResponse<string>>("generate_chat_title", { userMessage })),

  getChatMessages: (sessionId: string, limit?: number) =>
    cmd(invoke<CommandResponse<ChatMessageDto[]>>("get_chat_messages", { sessionId, limit })),

  getChatSnapshots: (sessionId: string) =>
    cmd(invoke<CommandResponse<SnapshotDto[]>>("get_chat_snapshots", { sessionId })),

  getChatContextOptions: (projectPath: string) =>
    cmd(invoke<CommandResponse<ChatContextOptionDto[]>>("get_chat_context_options", { projectPath })),

  getOrCreateDevSessionChat: (request: GetOrCreateDevSessionChatRequest) =>
    cmd(invoke<CommandResponse<ChatSessionDto>>("get_or_create_dev_session_chat", {
      devSessionId: request.dev_session_id,
      sessionName: request.session_name,
      projectId: request.project_id,
    })),

  getSessionActivity: (sessionId: string) =>
    cmd(invoke<CommandResponse<SessionActivityDto>>("get_session_activity", { sessionId })),

  getSessionStreamStatus: (sessionId: string) =>
    cmd(invoke<CommandResponse<string | null>>("get_session_stream_status", { sessionId })),

  // ============================================================================
  // GITHUB COMMANDS
  // ============================================================================

  githubAuthStatus: () =>
    cmd(invoke<CommandResponse<GitHubAuthStatusResponse>>("github_auth_status")),

  githubValidateSession: () =>
    cmd(invoke<CommandResponse<GitHubAuthStatusResponse>>("github_validate_session")),

  githubStorePat: (request: GitHubStorePATRequest) =>
    cmd(invoke<CommandResponse<GitHubAuthStatusResponse>>("github_store_pat", { request })),

  githubDisconnect: () =>
    cmd(invoke<CommandResponse<void>>("github_disconnect")),

  githubAcceptGcm: () =>
    cmd(invoke<CommandResponse<GitHubAuthStatusResponse>>("github_accept_gcm")),

  githubDetectRepo: (request: GitHubDetectRepoRequest) =>
    cmd(invoke<CommandResponse<GitHubDetectRepoResponse>>("github_detect_repo", { request })),

  githubListPulls: (request: GitHubListPullsRequest) =>
    cmd(invoke<CommandResponse<GitHubListPullsResponse>>("github_list_pulls", { request })),

  githubListIssues: (request: GitHubListIssuesRequest) =>
    cmd(invoke<CommandResponse<GitHubListIssuesResponse>>("github_list_issues", { request })),

  githubGetPrDetail: (request: GitHubGetPrDetailRequest) =>
    cmd(invoke<CommandResponse<GitHubPrDetailResponse>>("github_get_pr_detail", { request })),

  githubGetPrFiles: (request: GitHubGetPrFilesRequest) =>
    cmd(invoke<CommandResponse<GitHubPrFilesResponse>>("github_get_pr_files", { request })),

  githubGetComments: (request: GitHubGetCommentsRequest) =>
    cmd(invoke<CommandResponse<GitHubCommentsResponse>>("github_get_comments", { request })),

  githubListUserRepos: (request: GitHubListUserReposRequest) =>
    cmd(invoke<CommandResponse<GitHubListUserReposResponse>>("github_list_user_repos", { request })),

  githubCloneRepo: (request: GitHubCloneRepoRequest) =>
    cmd(invoke<CommandResponse<GitHubCloneRepoResponse>>("github_clone_repo", { request })),

  githubInspectCloneDestination: (request: GitHubInspectDestinationRequest) =>
    cmd(invoke<CommandResponse<GitHubInspectDestinationResponse>>("github_inspect_clone_destination", { request })),

  // ============================================================================
  // CLOUD AUTH COMMANDS
  // ============================================================================

  cloudAuthStatus: () =>
    cmd(invoke<CommandResponse<CloudAuthStatusResponse>>("cloud_auth_status")),

  cloudStartSignIn: () =>
    cmd(invoke<CommandResponse<void>>("cloud_start_sign_in")),

  cloudStartOAuth: (provider: string) =>
    cmd(invoke<CommandResponse<void>>("cloud_start_oauth", { provider })),

  cloudSignInWithEmail: (request: CloudSignInWithEmailRequest) =>
    cmd(invoke<CommandResponse<CloudAuthStatusResponse>>("cloud_sign_in_with_email", { request })),

  cloudSignUpWithEmail: (request: CloudSignUpWithEmailRequest) =>
    cmd(invoke<CommandResponse<CloudSignUpResponse>>("cloud_sign_up_with_email", { request })),

  cloudSignOut: () =>
    cmd(invoke<CommandResponse<void>>("cloud_sign_out")),

  cloudGetUser: () =>
    cmd(invoke<CommandResponse<CloudAuthStatusResponse>>("cloud_get_user")),

  // ============================================================================
  // EDITOR COMMANDS
  // ============================================================================

  readFile: (request: ReadFileRequest) =>
    cmd(invoke<CommandResponse<ReadFileResponse>>("read_file", { request })),

  writeFile: (request: WriteFileRequest) =>
    cmd(invoke<CommandResponse<void>>("write_file", { request })),

  // ============================================================================
  // TERMINAL COMMANDS
  // ============================================================================

  spawnTerminal: (request: SpawnTerminalRequest) =>
    cmd(invoke<CommandResponse<SpawnTerminalResponse>>("spawn_terminal", { request })),

  writeTerminal: (request: WriteTerminalRequest) =>
    cmd(invoke<CommandResponse<void>>("write_terminal", { request })),

  resizeTerminal: (request: ResizeTerminalRequest) =>
    cmd(invoke<CommandResponse<void>>("resize_terminal", { request })),

  killTerminal: (request: KillTerminalRequest) =>
    cmd(invoke<CommandResponse<void>>("kill_terminal", { request })),

  listTerminals: () =>
    cmd(invoke<CommandResponse<ListTerminalsResponse>>("list_terminals")),

  // ============================================================================
  // PROMPT REGISTRY COMMANDS
  // ============================================================================

  listPrompts: () =>
    cmd(invoke<CommandResponse<PromptDto[]>>("list_prompts")),

  getPrompt: (id: string) =>
    cmd(invoke<CommandResponse<PromptDto>>("get_prompt", { id })),

  updatePrompt: (request: UpdatePromptRequest) =>
    cmd(invoke<CommandResponse<PromptDto>>("update_prompt", { request })),

  resetPrompt: (id: string) =>
    cmd(invoke<CommandResponse<PromptDto>>("reset_prompt", { id })),

  listPromptVersions: (promptId: string) =>
    cmd(invoke<CommandResponse<PromptVersionDto[]>>("list_prompt_versions", { promptId })),

  listPromptTasks: () =>
    cmd(invoke<CommandResponse<string[]>>("list_prompt_tasks")),

  getTaskPrompts: (category: string) =>
    cmd(invoke<CommandResponse<PromptDto[]>>("get_task_prompts", { category })),

  saveTaskPrompt: (request: SaveTaskPromptRequest) =>
    cmd(invoke<CommandResponse<PromptDto>>("save_task_prompt", { request })),

  // -- Chat fragments (Phase 5) -----------------------------------------------
  listChatFragments: () =>
    cmd(invoke<CommandResponse<PromptDto[]>>("list_chat_fragments")),

  setPromptEnabled: (request: SetPromptEnabledRequest) =>
    cmd(invoke<CommandResponse<PromptDto>>("set_prompt_enabled", { request })),

  // ============================================================================
  // AGENT COMMANDS
  // ============================================================================

  listAgentProfiles: () =>
    cmd(invoke<CommandResponse<AgentProfileDto[]>>("list_agent_profiles")),

  getAgentProfile: (id: string) =>
    cmd(invoke<CommandResponse<AgentProfileDto>>("get_agent_profile", { id })),

  createAgentProfile: (request: CreateAgentProfileRequest) =>
    cmd(invoke<CommandResponse<AgentProfileDto>>("create_agent_profile", { request })),

  updateAgentProfile: (request: UpdateAgentProfileRequest) =>
    cmd(invoke<CommandResponse<AgentProfileDto>>("update_agent_profile", { request })),

  deleteAgentProfile: (id: string) =>
    cmd(invoke<CommandResponse<void>>("delete_agent_profile", { id })),

  listAgentTeams: () =>
    cmd(invoke<CommandResponse<AgentTeamDto[]>>("list_agent_teams")),

  getAgentTeam: (id: string) =>
    cmd(invoke<CommandResponse<AgentTeamDto>>("get_agent_team", { id })),

  createAgentTeam: (request: CreateAgentTeamRequest) =>
    cmd(invoke<CommandResponse<AgentTeamDto>>("create_agent_team", { request })),

  updateAgentTeam: (request: UpdateAgentTeamRequest) =>
    cmd(invoke<CommandResponse<AgentTeamDto>>("update_agent_team", { request })),

  deleteAgentTeam: (id: string) =>
    cmd(invoke<CommandResponse<void>>("delete_agent_team", { id })),

  // ============================================================================
  // AGENT RULE COMMANDS
  // ============================================================================

  listAgentRules: () =>
    cmd(invoke<CommandResponse<AgentRuleDto[]>>("list_agent_rules")),

  getAgentRule: (id: string) =>
    cmd(invoke<CommandResponse<AgentRuleDto>>("get_agent_rule", { id })),

  createAgentRule: (request: CreateAgentRuleRequest) =>
    cmd(invoke<CommandResponse<AgentRuleDto>>("create_agent_rule", { request })),

  updateAgentRule: (request: UpdateAgentRuleRequest) =>
    cmd(invoke<CommandResponse<AgentRuleDto>>("update_agent_rule", { request })),

  deleteAgentRule: (id: string) =>
    cmd(invoke<CommandResponse<void>>("delete_agent_rule", { id })),

  // ============================================================================
  // TOOL CATEGORY COMMANDS
  // ============================================================================

  listToolCategories: () =>
    cmd(invoke<CommandResponse<ToolCategoryDto[]>>("list_tool_categories")),

  getToolCategory: (id: string) =>
    cmd(invoke<CommandResponse<ToolCategoryDto>>("get_tool_category", { id })),

  createToolCategory: (request: CreateToolCategoryRequest) =>
    cmd(invoke<CommandResponse<ToolCategoryDto>>("create_tool_category", { request })),

  updateToolCategory: (request: UpdateToolCategoryRequest) =>
    cmd(invoke<CommandResponse<ToolCategoryDto>>("update_tool_category", { request })),

  deleteToolCategory: (id: string) =>
    cmd(invoke<CommandResponse<void>>("delete_tool_category", { id })),

  // ============================================================================
  // TOOL DEFINITION COMMANDS
  // ============================================================================

  listToolDefinitions: () =>
    cmd(invoke<CommandResponse<ToolDefinitionDto[]>>("list_tool_definitions")),

  getToolDefinition: (id: string) =>
    cmd(invoke<CommandResponse<ToolDefinitionDto>>("get_tool_definition", { id })),

  createToolDefinition: (request: CreateToolDefinitionRequest) =>
    cmd(invoke<CommandResponse<ToolDefinitionDto>>("create_tool_definition", { request })),

  updateToolDefinition: (request: UpdateToolDefinitionRequest) =>
    cmd(invoke<CommandResponse<ToolDefinitionDto>>("update_tool_definition", { request })),

  deleteToolDefinition: (id: string) =>
    cmd(invoke<CommandResponse<void>>("delete_tool_definition", { id })),

  // -- Chat modes ----------------------------------------------------------
  listChatModes: () =>
    cmd(invoke<CommandResponse<ChatModeDto[]>>("list_chat_modes")),

  getChatMode: (id: string) =>
    cmd(invoke<CommandResponse<ChatModeDto>>("get_chat_mode", { id })),

  createChatMode: (request: CreateChatModeRequest) =>
    cmd(invoke<CommandResponse<ChatModeDto>>("create_chat_mode", { request })),

  updateChatMode: (request: UpdateChatModeRequest) =>
    cmd(invoke<CommandResponse<ChatModeDto>>("update_chat_mode", { request })),

  deleteChatMode: (id: string) =>
    cmd(invoke<CommandResponse<void>>("delete_chat_mode", { id })),

  // ============================================================================
  // PIPELINE COMMANDS
  // ============================================================================

  startPipeline: (request: StartPipelineRequest) =>
    cmd(invoke<CommandResponse<StartPipelineResponse>>("start_pipeline", { request })),

  listPipelineRuns: () =>
    cmd(invoke<CommandResponse<PipelineRunDto[]>>("list_pipeline_runs")),

  getPipelineSteps: (runId: string) =>
    cmd(invoke<CommandResponse<PipelineStepDto[]>>("get_pipeline_steps", { runId })),

  getRunAnalysisContext: (runId: string) =>
    cmd(invoke<CommandResponse<RunAnalysisContextDto>>("get_run_analysis_context", { runId })),

  getAnalysisDepth: () =>
    cmd(invoke<CommandResponse<string>>("get_analysis_depth")),

  setAnalysisDepth: (depth: AnalysisDepthLevel) =>
    cmd(invoke<CommandResponse<void>>("set_analysis_depth", { depth })),

  // ============================================================================
  // MEMORY COMMANDS
  // ============================================================================

  getProjectMemory: (projectId: string) =>
    cmd(invoke<CommandResponse<ProjectMemoryDto | null>>("get_project_memory", { projectId })),

  /**
   * Read `<project_path>/.venore/project-memory.json` directly. Used by
   * the wizard's Step 5 to detect existing memory before the project is
   * registered (and therefore before a stable id exists). Returns null
   * when the file is missing or corrupted.
   */
  readProjectMemoryByPath: (projectPath: string) =>
    cmd(invoke<CommandResponse<ProjectMemoryDto | null>>("read_project_memory_by_path", { projectPath })),

  saveProjectMemory: (request: SaveProjectMemoryRequest) =>
    cmd(invoke<CommandResponse<ProjectMemoryDto>>("save_project_memory", { request })),

  deleteProjectMemory: (id: string) =>
    cmd(invoke<CommandResponse<void>>("delete_project_memory", { id })),

  regenerateMemorySummary: (request: RegenerateSummaryRequest) =>
    cmd(invoke<CommandResponse<string>>("regenerate_memory_summary", { request })),

  generateProjectMemory: (request: GenerateMemoryRequest) =>
    cmd(invoke<CommandResponse<GenerateMemoryResponse>>("generate_project_memory", { request })),

  // ============================================================================
  // STALE-MODULE DETECTION (portable code-hashes)
  // ============================================================================

  /**
   * Compares the committed `.venore/code-hashes.json` snapshot against the
   * current source tree and returns modules whose code has drifted.
   * Returns [] when the project has no snapshot yet or nothing is stale.
   */
  getStaleModules: (projectPath: string) =>
    cmd(invoke<CommandResponse<StaleModuleDto[]>>("get_stale_modules", { projectPath })),

  /**
   * Refresh `.venore/` snapshot from the current source tree without
   * re-running the wizard or the LLM. Project memory and ocean layout
   * are preserved. Fails for folders without `.venore/project.json`.
   */
  resnapshotProject: (projectPath: string) =>
    cmd(invoke<CommandResponse<ResnapshotReport>>("resnapshot_project", { projectPath })),

  // ============================================================================
  // CONTEXT UPDATER COMMANDS
  // ============================================================================

  checkForUpdates: (projectPath: string) =>
    cmd(invoke<CommandResponse<UpdateReportResponse | null>>("check_for_updates", { projectPath })),

  runContextUpdate: (request: RunUpdateRequest) =>
    cmd(invoke<CommandResponse<void>>("run_context_update", { request })),

  completeContextUpdate: (projectPath: string, latestCommit: string) =>
    cmd(invoke<CommandResponse<void>>("complete_context_update", { projectPath, latestCommit })),

  getUpdaterState: (projectPath: string) =>
    cmd(invoke<CommandResponse<UpdaterStateResponse>>("get_updater_state", { projectPath })),

  updateUpdaterState: (request: UpdateUpdaterStateRequest) =>
    cmd(invoke<CommandResponse<void>>("update_updater_state", { request })),

  // ============================================================================
  // SESSION COMMANDS
  // ============================================================================

  createSession: (request: CreateSessionRequest) =>
    cmd(invoke<CommandResponse<SessionDto>>("create_session", { request })),

  listSessions: (projectId: string, projectPath: string) =>
    cmd(invoke<CommandResponse<SessionDto[]>>("list_sessions", { projectId, projectPath })),

  getSession: (sessionId: string, projectPath: string) =>
    cmd(invoke<CommandResponse<SessionDto>>("get_session", { sessionId, projectPath })),

  completeSession: (sessionId: string, projectPath: string) =>
    cmd(invoke<CommandResponse<SessionDto>>("complete_session", { sessionId, projectPath })),

  abandonSession: (sessionId: string, projectPath: string) =>
    cmd(invoke<CommandResponse<void>>("abandon_session", { sessionId, projectPath })),

  sessionDiffFiles: (request: SessionDiffRequest) =>
    cmd(invoke<CommandResponse<SessionDiffFileDto[]>>("session_diff_files", { request })),

  sessionCommits: (request: SessionDiffRequest) =>
    cmd(invoke<CommandResponse<SessionCommitDto[]>>("session_commits", { request })),

  listGitBranches: (request: ListBranchesRequest) =>
    cmd(invoke<CommandResponse<ListBranchesResponse>>("list_git_branches", { request })),

  revertToSnapshot: (devSessionId: string, commitHash: string, messageId?: string) =>
    cmd(invoke<CommandResponse<void>>("revert_to_snapshot", { devSessionId, commitHash, messageId: messageId ?? null })),

  // ============================================================================
  // AGENT INTERACTION COMMANDS
  // ============================================================================

  respondToAgent: (toolCallId: string, response: string) =>
    cmd(invoke<CommandResponse<void>>("respond_to_agent", { toolCallId, response })),

  approvePlan: (toolCallId: string, approved: boolean) =>
    cmd(invoke<CommandResponse<void>>("approve_plan", { toolCallId, approved })),

  // ============================================================================
  // SKILLS COMMANDS
  // ============================================================================

  listSkills: () =>
    cmd(invoke<CommandResponse<SkillDto[]>>("list_skills")),

  // ============================================================================
  // MESH COMMANDS (lifecycle-based — 5 commands)
  // ============================================================================

  meshInit: (projectId: string, projectName: string, projectPath: string) =>
    cmd(invoke<CommandResponse<void>>("mesh_init", { projectId, projectName, projectPath })),

  meshGetPeers: () =>
    cmd(invoke<CommandResponse<MeshPeerInfo[]>>("mesh_get_peers")),

  /**
   * Tear down a single project's mesh presence (handler + registration
   * file). Called from useMeshInit cleanup so other peers stop seeing
   * this project the moment its window closes, without waiting on TTL.
   */
  meshUnregisterProject: (projectId: string) =>
    cmd(invoke<CommandResponse<void>>("mesh_unregister_project", { projectId })),

  meshTransportStatus: () =>
    cmd(invoke<CommandResponse<MeshTransportStatus>>("mesh_transport_status")),

  meshConnectPeer: (projectId: string) =>
    cmd(invoke<CommandResponse<void>>("mesh_connect_peer", { projectId })),

  meshDisconnectPeer: (projectId: string) =>
    cmd(invoke<CommandResponse<void>>("mesh_disconnect_peer", { projectId })),
};
