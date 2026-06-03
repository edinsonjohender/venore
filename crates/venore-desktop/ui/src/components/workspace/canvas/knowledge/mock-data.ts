// =============================================================================
// Knowledge Board — Mock Data (prototype)
// =============================================================================
// Types + hardcoded dummy data for the Knowledge Board prototype.
// Will be replaced with backend data once the UI is validated.

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

export type HexPhase = 'discover' | 'define' | 'validate' | 'conclude'

export interface Evidence {
  id: string
  hexagonId: string
  type: 'link' | 'note' | 'file'
  title: string
  url?: string
  content?: string
}

export interface Hexagon {
  id: string
  featureId: string
  parentId: string | null
  title: string
  description: string
  phase: HexPhase
  percentage: number
  isDeadEnd: boolean
  confidence: 'low' | 'medium' | 'high'
  risk: 'low' | 'medium' | 'high'
  notes: string
  blockedBy: string[]  // cross-connections to hexagons in other branches
}

export type ResearchObjective = 'validate' | 'understand' | 'compare' | 'decide' | 'explore'

export type ResearchIntensity = 'shallow' | 'moderate' | 'deep'

export interface Feature {
  id: string
  name: string
  description: string
  objective: ResearchObjective
  status: 'active' | 'paused' | 'completed'
  priority: 'low' | 'medium' | 'high' | 'critical'
  intensity: ResearchIntensity
  maxHexagonsPerPhase: number
  autoAdvance: boolean
  tags: string[]
  hexagons: Hexagon[]
  evidence: Evidence[]
}

// -----------------------------------------------------------------------------
// Feature 1: Real-time Collaboration (12 hexagons)
// -----------------------------------------------------------------------------

const rtcHexagons: Hexagon[] = [
  // Root
  {
    id: 'rtc-root',
    featureId: 'rtc',
    parentId: null,
    title: 'Real-time Collaboration',
    description: 'Investigate feasibility of real-time multi-user editing in context files.',
    phase: 'discover',
    percentage: 65,
    isDeadEnd: false,
    confidence: 'medium',
    risk: 'high',
    notes: 'Core question: can we keep .context.md files consistent across concurrent edits?',
    blockedBy: [],
  },
  // Level 1 — three branches
  {
    id: 'rtc-crdt',
    featureId: 'rtc',
    parentId: 'rtc-root',
    title: 'CRDT Approach',
    description: 'Use Conflict-free Replicated Data Types for merge-free concurrent editing.',
    phase: 'define',
    percentage: 70,
    isDeadEnd: false,
    confidence: 'high',
    risk: 'medium',
    notes: 'Yjs and Automerge are the two main candidates.',
    blockedBy: [],
  },
  {
    id: 'rtc-ot',
    featureId: 'rtc',
    parentId: 'rtc-root',
    title: 'OT Approach',
    description: 'Operational Transform — classic Google Docs style.',
    phase: 'validate',
    percentage: 40,
    isDeadEnd: false,
    confidence: 'medium',
    risk: 'high',
    notes: 'Requires a central server; may not suit our P2P mesh architecture.',
    blockedBy: [],
  },
  {
    id: 'rtc-lock',
    featureId: 'rtc',
    parentId: 'rtc-root',
    title: 'File Locking',
    description: 'Simple pessimistic locking — one editor at a time per file.',
    phase: 'conclude',
    percentage: 90,
    isDeadEnd: true,
    confidence: 'high',
    risk: 'low',
    notes: 'Too restrictive for real-time collaboration. Discarded.',
    blockedBy: [],
  },
  // Level 2 — CRDT children
  {
    id: 'rtc-yjs',
    featureId: 'rtc',
    parentId: 'rtc-crdt',
    title: 'Yjs Integration',
    description: 'Evaluate Yjs library for CRDT-based text editing.',
    phase: 'validate',
    percentage: 55,
    isDeadEnd: false,
    confidence: 'high',
    risk: 'medium',
    notes: 'Good docs, active community. WebSocket provider fits our mesh.',
    blockedBy: [],
  },
  {
    id: 'rtc-automerge',
    featureId: 'rtc',
    parentId: 'rtc-crdt',
    title: 'Automerge Test',
    description: 'Evaluate Automerge as an alternative CRDT backend.',
    phase: 'discover',
    percentage: 25,
    isDeadEnd: false,
    confidence: 'low',
    risk: 'medium',
    notes: 'Rust-native via automerge-rs. Could integrate better with venore-core.',
    blockedBy: [],
  },
  // Level 2 — OT children
  {
    id: 'rtc-ot-server',
    featureId: 'rtc',
    parentId: 'rtc-ot',
    title: 'Central Server Design',
    description: 'Design a lightweight OT server for context file synchronization.',
    phase: 'define',
    percentage: 30,
    isDeadEnd: false,
    confidence: 'medium',
    risk: 'high',
    notes: 'Would need to run alongside mesh. Complex.',
    blockedBy: ['rtc-yjs'], // blocked by: if Yjs works, skip OT
  },
  {
    id: 'rtc-ot-transform',
    featureId: 'rtc',
    parentId: 'rtc-ot',
    title: 'Transform Functions',
    description: 'Implement transform functions for markdown operations.',
    phase: 'discover',
    percentage: 10,
    isDeadEnd: false,
    confidence: 'low',
    risk: 'high',
    notes: 'Complex for structured markdown (frontmatter, sections, lists).',
    blockedBy: ['rtc-ot-server'],
  },
  // Level 3 — Yjs children
  {
    id: 'rtc-yjs-ws',
    featureId: 'rtc',
    parentId: 'rtc-yjs',
    title: 'WebSocket Provider',
    description: 'Connect Yjs to our existing mesh WebSocket transport.',
    phase: 'discover',
    percentage: 15,
    isDeadEnd: false,
    confidence: 'medium',
    risk: 'medium',
    notes: 'Mesh already has WebSocket infra. Need to add Yjs awareness protocol.',
    blockedBy: [],
  },
  {
    id: 'rtc-yjs-md',
    featureId: 'rtc',
    parentId: 'rtc-yjs',
    title: 'Markdown Binding',
    description: 'Bind Yjs document to markdown AST for structured editing.',
    phase: 'discover',
    percentage: 5,
    isDeadEnd: false,
    confidence: 'low',
    risk: 'high',
    notes: 'No existing Yjs markdown binding. Would need to build custom.',
    blockedBy: ['rtc-yjs-ws'],
  },
  // Level 3 — Automerge children
  {
    id: 'rtc-am-rust',
    featureId: 'rtc',
    parentId: 'rtc-automerge',
    title: 'Rust FFI Bridge',
    description: 'Build FFI bridge between automerge-rs and venore-core.',
    phase: 'discover',
    percentage: 10,
    isDeadEnd: false,
    confidence: 'low',
    risk: 'high',
    notes: 'automerge-rs is in pre-release. API unstable.',
    blockedBy: [],
  },
  {
    id: 'rtc-am-perf',
    featureId: 'rtc',
    parentId: 'rtc-automerge',
    title: 'Performance Benchmark',
    description: 'Benchmark Automerge vs Yjs for typical .context.md file sizes.',
    phase: 'discover',
    percentage: 0,
    isDeadEnd: false,
    confidence: 'low',
    risk: 'low',
    notes: 'Blocked until both have basic integration working.',
    blockedBy: ['rtc-am-rust', 'rtc-yjs-ws'],
  },
]

const rtcEvidence: Evidence[] = [
  {
    id: 'ev-1',
    hexagonId: 'rtc-yjs',
    type: 'link',
    title: 'Yjs documentation — Getting Started',
    url: 'https://docs.yjs.dev/getting-started',
  },
  {
    id: 'ev-2',
    hexagonId: 'rtc-crdt',
    type: 'note',
    title: 'CRDT comparison notes',
    content: 'Yjs: smaller bundle, better perf for text. Automerge: richer data types, Rust-native.',
  },
]

// -----------------------------------------------------------------------------
// Feature 2: AI Context Compression (10 hexagons)
// -----------------------------------------------------------------------------

const accHexagons: Hexagon[] = [
  // Root
  {
    id: 'acc-root',
    featureId: 'acc',
    parentId: null,
    title: 'AI Context Compression',
    description: 'Research methods to compress .context.md files for LLM context windows.',
    phase: 'define',
    percentage: 45,
    isDeadEnd: false,
    confidence: 'medium',
    risk: 'medium',
    notes: 'Context windows are growing but cost per token still matters.',
    blockedBy: [],
  },
  // Level 1 — two branches
  {
    id: 'acc-summary',
    featureId: 'acc',
    parentId: 'acc-root',
    title: 'Hierarchical Summary',
    description: 'Generate multi-level summaries: full → module → one-liner.',
    phase: 'validate',
    percentage: 60,
    isDeadEnd: false,
    confidence: 'high',
    risk: 'low',
    notes: 'Most promising approach. Can use existing LLM to summarize.',
    blockedBy: [],
  },
  {
    id: 'acc-embed',
    featureId: 'acc',
    parentId: 'acc-root',
    title: 'Embedding-based Selection',
    description: 'Use embeddings to select only relevant context sections.',
    phase: 'discover',
    percentage: 30,
    isDeadEnd: false,
    confidence: 'medium',
    risk: 'medium',
    notes: 'Requires embedding model. Could piggyback on RAG infrastructure.',
    blockedBy: [],
  },
  // Level 2 — Summary children
  {
    id: 'acc-sum-prompt',
    featureId: 'acc',
    parentId: 'acc-summary',
    title: 'Summary Prompt Design',
    description: 'Design prompts that produce consistently structured summaries.',
    phase: 'validate',
    percentage: 75,
    isDeadEnd: false,
    confidence: 'high',
    risk: 'low',
    notes: 'Using few-shot examples. Results are promising.',
    blockedBy: [],
  },
  {
    id: 'acc-sum-cache',
    featureId: 'acc',
    parentId: 'acc-summary',
    title: 'Summary Caching',
    description: 'Cache summaries and invalidate when source .context.md changes.',
    phase: 'define',
    percentage: 40,
    isDeadEnd: false,
    confidence: 'high',
    risk: 'low',
    notes: 'Can reuse hash_cache from context module.',
    blockedBy: [],
  },
  {
    id: 'acc-sum-levels',
    featureId: 'acc',
    parentId: 'acc-summary',
    title: 'Compression Levels',
    description: 'Define 3 levels: brief (1 line), standard (paragraph), full (original).',
    phase: 'discover',
    percentage: 20,
    isDeadEnd: false,
    confidence: 'medium',
    risk: 'low',
    notes: 'Need to define when each level is appropriate.',
    blockedBy: ['acc-sum-prompt'],
  },
  // Level 2 — Embedding children
  {
    id: 'acc-emb-model',
    featureId: 'acc',
    parentId: 'acc-embed',
    title: 'Embedding Model Choice',
    description: 'Evaluate local vs API embedding models for code context.',
    phase: 'discover',
    percentage: 35,
    isDeadEnd: false,
    confidence: 'medium',
    risk: 'medium',
    notes: 'nomic-embed-text (local) vs text-embedding-3-small (API).',
    blockedBy: [],
  },
  {
    id: 'acc-emb-chunk',
    featureId: 'acc',
    parentId: 'acc-embed',
    title: 'Chunk Strategy',
    description: 'Define how to chunk .context.md for embedding.',
    phase: 'discover',
    percentage: 15,
    isDeadEnd: false,
    confidence: 'low',
    risk: 'medium',
    notes: 'By section? By paragraph? By semantic block?',
    blockedBy: [],
  },
  // Level 3
  {
    id: 'acc-emb-rag',
    featureId: 'acc',
    parentId: 'acc-emb-model',
    title: 'RAG Integration',
    description: 'Integrate context selection into existing RAG pipeline.',
    phase: 'discover',
    percentage: 5,
    isDeadEnd: false,
    confidence: 'low',
    risk: 'medium',
    notes: 'RAG module already has FTS5. Need to add vector search alongside.',
    blockedBy: ['acc-emb-model', 'acc-emb-chunk'],
  },
  {
    id: 'acc-emb-eval',
    featureId: 'acc',
    parentId: 'acc-emb-model',
    title: 'Quality Evaluation',
    description: 'Build eval set to measure if compression preserves key information.',
    phase: 'discover',
    percentage: 0,
    isDeadEnd: false,
    confidence: 'low',
    risk: 'low',
    notes: 'Need ground truth dataset. Could use existing .context.md files.',
    blockedBy: ['acc-emb-rag'],
  },
]

const accEvidence: Evidence[] = [
  {
    id: 'ev-3',
    hexagonId: 'acc-sum-prompt',
    type: 'file',
    title: 'Summary prompt v2 — few-shot template',
    content: 'prompts/summary-v2.txt',
  },
]

// -----------------------------------------------------------------------------
// Exported mock data
// -----------------------------------------------------------------------------

export const MOCK_FEATURES: Feature[] = [
  {
    id: 'rtc',
    name: 'Real-time Collaboration',
    description: 'Enable multiple users to edit .context.md files simultaneously with conflict resolution.',
    objective: 'compare',
    status: 'active',
    priority: 'high',
    intensity: 'moderate',
    maxHexagonsPerPhase: 100,
    autoAdvance: true,
    tags: ['collaboration', 'crdt', 'mesh'],
    hexagons: rtcHexagons,
    evidence: rtcEvidence,
  },
  {
    id: 'acc',
    name: 'AI Context Compression',
    description: 'Compress context files to fit more information in LLM context windows while preserving key details.',
    objective: 'validate',
    status: 'active',
    priority: 'medium',
    intensity: 'moderate',
    maxHexagonsPerPhase: 100,
    autoAdvance: true,
    tags: ['ai', 'compression', 'context'],
    hexagons: accHexagons,
    evidence: accEvidence,
  },
]
