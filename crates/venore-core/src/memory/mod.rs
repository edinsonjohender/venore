//! Project Memory — compact knowledge block for the LLM system prompt.
//!
//! Stores identity, goals, conventions, response language, and a condensed
//! project summary so the full .context.md doesn't dilute the prompt.

pub mod models;
pub mod repository;
pub mod formatter;
pub mod file_storage;

pub use models::ProjectMemory;
pub use repository::MemoryRepository;
pub use formatter::format_project_memory;
