//! Layers module — heuristic code inspection per module
//!
//! Analyzes modules using fast filesystem heuristics (no LLM) to determine
//! per-layer status: context freshness, test coverage, documentation quality,
//! connection health, and code issues (TODO/FIXME).

pub mod types;
pub mod analyzer;

pub use types::*;
pub use analyzer::analyze_module_layers;
