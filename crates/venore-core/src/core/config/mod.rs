//! Configuration Management
//!
//! Handles configuration for LLM tasks and providers.
//! Supports multiple configuration sources with precedence:
//!
//! 1. Environment variables (highest priority)
//! 2. SQLite database (persistent user settings)
//! 3. Default values (fallback)

pub mod models;
pub mod defaults;
pub mod loader;

// Re-export main types
pub use models::{TaskSettings, LlmConfig};
pub use defaults::TaskDefaults;
pub use loader::load_task_settings;
