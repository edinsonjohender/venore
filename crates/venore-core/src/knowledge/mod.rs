//! Knowledge Island Module
//!
//! Structured research persistence for knowledge projects:
//! features, hexagons (research points), and evidence.

pub mod types;
pub mod repository;

pub use types::{AgentStatus, HexagonPhase, KnowledgeFeature, KnowledgeHexagon, KnowledgeEvidence, KnowledgeFile, KnowledgeProjectLink};
pub use repository::KnowledgeRepository;
