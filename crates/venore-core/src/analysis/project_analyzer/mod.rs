//! Pluggable project-analyzer system.
//!
//! This module provides an extensible mechanism to auto-detect the project
//! type (Node monorepo, Rust workspace, Python, etc.) and adjust module
//! detection accordingly.
//!
//! ## Architecture
//!
//! - **Trait `ProjectAnalyzer`**: interface every analyzer implements
//! - **`AnalyzerRegistry`**: global registry with auto-detection
//! - **Factory functions**: helpers to create and use analyzers
//!
//! ## Basic usage
//!
//! ```rust,ignore
//! use venore_core::analysis::project_analyzer;
//!
//! // Auto-detect the project type
//! let detection = project_analyzer::detect_project_type(&project_root).await?;
//!
//! println!("Detected: {:?} (confidence: {}%)",
//!          detection.project_type,
//!          detection.confidence * 100.0);
//!
//! // Fetch the module-detection strategy
//! if let Ok(analyzer) = project_analyzer::get_analyzer(detection.project_type) {
//!     let strategy = analyzer.module_detection_strategy();
//!     // Use the strategy with module_detector
//! }
//! ```

pub mod analyzers;
pub mod factory;
pub mod registry;
pub mod traits;

// Public re-exports
pub use factory::{detect_project_type, get_analyzer};
pub use registry::registry;
pub use traits::{
    ModuleDetectionStrategy, ProjectAnalyzer, ProjectType, ProjectTypeDetection,
};
