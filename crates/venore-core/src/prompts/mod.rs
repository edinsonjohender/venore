//! Prompts module — centralized LLM prompt registry
//!
//! Provides SQLite-backed storage for all LLM prompt templates,
//! auto-versioning on edits, seed defaults, and variable rendering.

pub mod models;
pub mod repository;
pub mod seed;
pub mod fragments;

pub use models::{Prompt, PromptVersion};
pub use repository::PromptRepository;
pub use fragments::{
    build_fragment_map, render_template, ChatFragmentEntry, ChatFragmentId,
    ChatFragmentMap, CATEGORY_CHAT_FRAGMENT,
};
