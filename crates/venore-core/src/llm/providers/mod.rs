//! LLM Provider Implementations
//!
//! Concrete implementations for different LLM providers.
//!
//! Each provider implements the `LlmProvider` trait, providing:
//! - Text completion
//! - Streaming
//! - Connection testing

pub mod factory;
pub mod base;

// Provider implementations
pub mod anthropic;
pub mod openai;
pub mod gemini;
pub mod ollama;

// Embedding provider implementations
pub mod embedding_openai;
pub mod embedding_gemini;
pub mod embedding_ollama;
