//! # LLM Module
//!
//! Generic, reusable module for interacting with LLMs (Large Language Models).
//!
//! ## Features
//!
//! - **Provider-agnostic**: support for multiple providers (Anthropic, OpenAI, etc.)
//! - **Retry & Fallback**: automatic retry and fallback logic
//! - **Streaming**: support for streaming responses
//! - **Tool Calling**: support for function calling
//! - **Structured Output**: JSON schema for structured responses
//! - **Token Management**: tracking of tokens used
//! - **Secure Storage**: safe handling of API keys
//!
//! ## Arquitectura
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │  Gateway (public API)                   │
//! │  complete(), stream()                   │
//! └─────────────────┬───────────────────────┘
//!                   │
//!                   ▼
//! ┌─────────────────────────────────────────┐
//! │  Router                                 │
//! │  Retry logic, Fallbacks, Timeouts       │
//! └─────────────────┬───────────────────────┘
//!                   │
//!                   ▼
//! ┌─────────────────────────────────────────┐
//! │  Providers                              │
//! │  Anthropic, OpenAI, etc.                │
//! └─────────────────────────────────────────┘
//! ```
//!
//! ## Basic usage
//!
//! ```no_run
//! use venore_core::llm::prelude::*;
//!
//! # async fn example() -> venore_core::Result<()> {
//! // TODO: usage example once implemented
//! # Ok(())
//! # }
//! ```

// Core types and traits
pub mod types;
pub mod error;

// Configuration
pub mod config;
pub mod registry;

// Gateway and routing
pub mod gateway;
pub mod router;

// Provider implementations
pub mod providers;

// Utilities
pub mod utils;

// Session logging
pub mod session_logger;

// Re-export public API
pub use types::*;
pub use gateway::{LlmGateway, GatewayOptions};
pub use config::TaskConfig;
pub use session_logger::{SessionLogger, SessionEvent};

/// Prelude para imports convenientes
pub mod prelude {
    pub use super::{
        LlmGateway,
        GatewayOptions,
        LlmRequest,
        LlmResponse,
        LlmMessage,
        MessageRole,
        LlmStreamChunk,
        TokenUsage,
        TaskConfig,
        SessionLogger,
        SessionEvent,
    };

    pub use crate::traits::{
        LlmProviderType,
        LlmTask,
        ApiKeyStore,
        TaskConfigStore,
        ConfigStore,
        LlmProvider,
    };
}
