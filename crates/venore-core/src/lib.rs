//! # Venore Core
//!
//! Business logic library for Venore.
//! This crate contains ALL business logic and can be consumed by:
//! - venore-api (REST API server)
//! - venore-desktop (Tauri desktop app)
//! - Any future client
//!
//! ## Architecture: Feature Modules
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  Shared Foundations                                         │
//! │  entities, traits, error, utils, core/config, infrastructure│
//! └─────────────────────────────────────────────────────────────┘
//!              ▲
//! ┌────────────┴────────────────────────────────────────────────┐
//! │  Feature Modules (each self-contained)                      │
//! │                                                             │
//! │  Analysis    : scan → parse → detect modules                │
//! │  Context     : prompt builder + writer → .context.md        │
//! │  Wizard      : onboarding session + batch generation        │
//! │  Ocean       : 3D canvas layout                             │
//! │  Chat        : orchestrator + SQLite persistence            │
//! │  RAG         : FTS5 search + tree-sitter chunking           │
//! │  Agents      : profiles + teams + rules + pipelines         │
//! │  Tools       : 20 tool definitions + executor               │
//! │  Skills      : slash command registry                       │
//! │  Permissions  : per-tool rule engine                        │
//! │  LLM         : gateway + router + 4 providers               │
//! │  GitHub      : API client, PRs, issues                      │
//! │  Terminal    : PTY sessions                                 │
//! │  Mesh        : cross-project WebSocket communication        │
//! │  ...                                                        │
//! └─────────────────────────────────────────────────────────────┘
//! ```

// ============================================================================
// Shared Foundations
// ============================================================================

/// Error types y Result alias
pub mod error;

/// Utilities (path, string, validation)
pub mod utils;

/// Core infrastructure (config defaults)
pub mod core;

/// Domain entities (Project, Island, Node, etc.)
pub mod entities;

/// Traits para dependency injection (LlmProvider, ConfigStore, etc.)
pub mod traits;

/// Infrastructure implementations (SQLite config store, keyring)
pub mod infrastructure;

// Re-export common types
pub use error::{VenoreError, ErrorResponse, MapDbErr, Result};

// ============================================================================
// Feature Modules
// ============================================================================

/// Analysis modules (file scanning, AST parsing, module detection)
pub mod analysis;

/// Context generation (.context.md writer, frontmatter, prompts, hash cache)
pub mod context;

/// Context auto-updater — detect branch changes, map to modules, regenerate stale .context.md
pub mod context_updater;

/// Wizard module — onboarding flow (session, batch generation, validation)
pub mod wizard;

/// Checkpoint system for resumable operations
pub mod checkpoint;

/// Ocean Canvas layout system
pub mod ocean;

/// Dashboard module — project overview with context status
pub mod dashboard;

/// Layers module — heuristic code inspection per module
pub mod layers;

/// Snapshot pipeline — produces every `.venore/*.json` portable file
pub mod snapshot;

/// Chat module — orchestrator, context builder, SQLite persistence, compaction
pub mod chat;

/// RAG module — code indexing, FTS5 search, tree-sitter chunking
pub mod rag;

/// LLM Integration — gateway, router, 4 providers (Anthropic, OpenAI, Gemini, DeepSeek)
pub mod llm;

/// AI agent profiles, teams, rules, tool categories, pipeline execution
pub mod agents;

/// AI agent tool definitions (20 tools) and executor
pub mod tools;

/// Slash command registry (/commit, /fix, /test, /review, /explain)
pub mod skills;

/// Centralized LLM prompt registry with versioning
pub mod prompts;

/// Permission rule engine for AI agent tool calls
pub mod permissions;

/// Project identity and registration
pub mod project;

/// Knowledge Island — structured research persistence
pub mod knowledge;

/// Research Engine — multi-agent orchestration for knowledge investigation
pub mod research;

/// Session module — branch-per-session workflow
pub mod session;

/// GitHub integration (API client, auth, PRs, issues, AI PR analysis)
pub mod github;

/// Terminal PTY session manager
pub mod terminal;

/// LSP language server integration — post-edit diagnostics (TS, Rust)
pub mod lsp;

/// Project Memory — compact knowledge block for the LLM system prompt
pub mod memory;

/// Mesh — cross-project peer discovery and WebSocket communication
pub mod mesh;

// ============================================================================
// Prelude
// ============================================================================

/// Prelude para imports convenientes
pub mod prelude {
    pub use crate::error::{VenoreError, ErrorResponse, Result};
    pub use crate::entities::*;
    pub use crate::traits::*;
    pub use crate::utils;
    pub use crate::analysis;
    pub use crate::llm;
    pub use crate::context;
}
