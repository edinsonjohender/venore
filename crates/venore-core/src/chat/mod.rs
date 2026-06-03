//! Chat Module
//!
//! Provides chat orchestration, context building, and session persistence.

pub mod compaction;
pub mod connection_resolver;
pub mod debug_log;
pub mod guardrails;
pub mod orchestrator;
pub mod pending_writes;
pub mod query_router;
pub mod repository;
pub mod context;

pub mod title;

pub use orchestrator::{ChatMessageInput, build_llm_messages, create_chat_stream, create_chat_stream_with_attachments, continue_chat_stream};
pub use repository::{ChatRepository, ChatSession, ChatMessageRecord, ToolCallRecord, TokenSummary};
pub use context::{ChatContextBuilder, ChatContextDeps, ContextModule, KnowledgeResearchContext, KnowledgeHexagonSummary, MeshPeerContext, SessionContext, build_full_chat_context, build_knowledge_context, build_session_context};
pub use query_router::{classify as classify_query, QueryClass, INVESTIGATION_TOOLS};
pub use title::generate_session_title;
pub use debug_log::{log_event as log_chat_event, now_iso as chat_event_now, ChatDebugEvent};
