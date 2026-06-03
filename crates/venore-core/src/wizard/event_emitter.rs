//! # Event Emitter Abstraction
//!
//! Provides a trait-based abstraction for emitting wizard events.
//! This decouples the core business logic from the event delivery mechanism,
//! allowing the same wizard logic to work with:
//! - Desktop apps (Tauri events)
//! - Web APIs (WebSocket/SSE streams)
//! - CLI tools (console output or no output)
//! - Tests (mock emitters)
//!
//! ## Design Pattern
//!
//! The `WizardEventEmitter` trait defines the contract for event emission.
//! Different implementations handle events in their own way:
//!
//! - `NullEventEmitter`: No-op implementation for CLI/tests
//! - (Future) `TauriEventEmitter`: Emits to Tauri frontend
//! - (Future) `WebSocketEventEmitter`: Sends via WebSocket
//! - (Future) `ConsoleEventEmitter`: Prints to stdout
//!
//! ## Usage
//!
//! ```rust,ignore
//! use std::sync::Arc;
//! use venore_core::wizard::event_emitter::{WizardEventEmitter, NullEventEmitter};
//!
//! let emitter: Arc<dyn WizardEventEmitter> = Arc::new(NullEventEmitter);
//!
//! emitter.emit_progress(ProgressEvent {
//!     batch_id: "batch-123".to_string(),
//!     current: 1,
//!     total: 10,
//!     module_id: "module-1".to_string(),
//!     status: "running".to_string(),
//!     tokens_used: 0,
//!     error: None,
//! });
//! ```

use crate::wizard::batch_generation::{ProgressEvent, CompleteEvent};

// =============================================================================
// Analysis Event Types
// =============================================================================

/// Progress event emitted during project analysis
///
/// Tracks progress through the analysis pipeline:
/// 1. Scanning files
/// 2. Parsing AST
/// 3. Detecting project type
/// 4. Detecting modules
/// 5. Building analysis cache
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnalysisProgressEvent {
    /// Unique identifier for this analysis session
    pub session_id: String,

    /// Current step (1-5)
    pub current_step: u8,

    /// Total steps (always 5)
    pub total_steps: u8,

    /// Human-readable description of current step
    pub step_description: String,

    /// Optional: current item being processed (e.g., file name)
    pub current_item: Option<String>,

    /// Optional: progress within current step (0-100)
    pub step_progress: Option<u8>,
}

/// Completion event emitted when analysis finishes
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnalysisCompleteEvent {
    /// Unique identifier for this analysis session
    pub session_id: String,

    /// Total files scanned
    pub total_files: usize,

    /// Total modules detected
    pub total_modules: usize,

    /// Duration in milliseconds
    pub duration_ms: u64,

    /// Whether analysis was successful
    pub success: bool,

    /// Optional error message if failed
    pub error: Option<String>,
}

// =============================================================================
// Trait Definition
// =============================================================================

/// Trait for emitting wizard events
///
/// Implementations of this trait can deliver events to different backends:
/// - Tauri (desktop app)
/// - WebSocket/SSE (web API)
/// - Console (CLI)
/// - Null (tests)
pub trait WizardEventEmitter: Send + Sync {
    /// Emit a progress event for a single module
    ///
    /// Called during batch generation for each module processed.
    ///
    /// # Arguments
    ///
    /// * `event` - Progress event with module status and metadata
    fn emit_progress(&self, event: ProgressEvent);

    /// Emit a completion event for the entire batch
    ///
    /// Called once when batch generation finishes (success or failure).
    ///
    /// # Arguments
    ///
    /// * `event` - Completion event with statistics
    fn emit_complete(&self, event: CompleteEvent);

    /// Emit an error event
    ///
    /// Called when a critical error occurs that doesn't fit into progress/complete flow.
    ///
    /// # Arguments
    ///
    /// * `batch_id` - Batch ID for context
    /// * `error` - Error message
    fn emit_error(&self, batch_id: String, error: String);

    /// Emit a progress event during project analysis
    ///
    /// Called during the analysis pipeline (scanning, parsing, detecting modules).
    ///
    /// # Arguments
    ///
    /// * `event` - Analysis progress event with step information
    fn emit_analysis_progress(&self, event: AnalysisProgressEvent);

    /// Emit a completion event when analysis finishes
    ///
    /// Called once when analysis completes (success or failure).
    ///
    /// # Arguments
    ///
    /// * `event` - Analysis completion event with statistics
    fn emit_analysis_complete(&self, event: AnalysisCompleteEvent);
}

// =============================================================================
// NullEventEmitter - No-op implementation for CLI/tests
// =============================================================================

/// No-op event emitter that discards all events
///
/// Useful for:
/// - CLI tools that don't need event streams
/// - Tests where events are not relevant
/// - Batch processes without user interaction
///
/// # Example
///
/// ```rust
/// use venore_core::wizard::event_emitter::{WizardEventEmitter, NullEventEmitter};
/// use std::sync::Arc;
///
/// let emitter: Arc<dyn WizardEventEmitter> = Arc::new(NullEventEmitter);
/// // Events are silently discarded
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct NullEventEmitter;

impl WizardEventEmitter for NullEventEmitter {
    fn emit_progress(&self, _event: ProgressEvent) {
        // No-op: discard event
    }

    fn emit_complete(&self, _event: CompleteEvent) {
        // No-op: discard event
    }

    fn emit_error(&self, _batch_id: String, _error: String) {
        // No-op: discard error
    }

    fn emit_analysis_progress(&self, _event: AnalysisProgressEvent) {
        // No-op: discard analysis progress
    }

    fn emit_analysis_complete(&self, _event: AnalysisCompleteEvent) {
        // No-op: discard analysis complete
    }
}

// =============================================================================
// CallbackEventEmitter - Adapter for backward compatibility
// =============================================================================

/// Event emitter that wraps callback functions
///
/// This adapter allows using the old callback-based API with the new
/// trait-based system. Useful for backward compatibility.
///
/// # Example
///
/// ```rust,ignore
/// use std::sync::Arc;
/// use venore_core::wizard::event_emitter::{WizardEventEmitter, CallbackEventEmitter};
///
/// let emitter: Arc<dyn WizardEventEmitter> = Arc::new(CallbackEventEmitter::new(
///     |event| println!("Progress: {:?}", event),
///     |event| println!("Complete: {:?}", event),
/// ));
/// ```
pub struct CallbackEventEmitter<F1, F2>
where
    F1: Fn(ProgressEvent) + Send + Sync,
    F2: Fn(CompleteEvent) + Send + Sync,
{
    on_progress: F1,
    on_complete: F2,
}

impl<F1, F2> CallbackEventEmitter<F1, F2>
where
    F1: Fn(ProgressEvent) + Send + Sync,
    F2: Fn(CompleteEvent) + Send + Sync,
{
    /// Creates a new CallbackEventEmitter
    ///
    /// # Arguments
    ///
    /// * `on_progress` - Callback for progress events
    /// * `on_complete` - Callback for completion events
    pub fn new(on_progress: F1, on_complete: F2) -> Self {
        Self {
            on_progress,
            on_complete,
        }
    }
}

impl<F1, F2> WizardEventEmitter for CallbackEventEmitter<F1, F2>
where
    F1: Fn(ProgressEvent) + Send + Sync,
    F2: Fn(CompleteEvent) + Send + Sync,
{
    fn emit_progress(&self, event: ProgressEvent) {
        (self.on_progress)(event);
    }

    fn emit_complete(&self, event: CompleteEvent) {
        (self.on_complete)(event);
    }

    fn emit_error(&self, batch_id: String, error: String) {
        // Emit as a failed progress event for backward compatibility
        (self.on_progress)(ProgressEvent {
            batch_id,
            current: 0,
            total: 0,
            module_id: "error".to_string(),
            status: "failed".to_string(),
            tokens_used: 0,
            error: Some(error),
        });
    }

    fn emit_analysis_progress(&self, _event: AnalysisProgressEvent) {
        // No-op for backward compatibility (old callbacks don't have analysis events)
    }

    fn emit_analysis_complete(&self, _event: AnalysisCompleteEvent) {
        // No-op for backward compatibility (old callbacks don't have analysis events)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_null_emitter_implements_trait() {
        let emitter: Arc<dyn WizardEventEmitter> = Arc::new(NullEventEmitter);

        // Should not panic, just no-op
        emitter.emit_progress(ProgressEvent {
            batch_id: "test-batch".to_string(),
            current: 1,
            total: 10,
            module_id: "test-module".to_string(),
            status: "running".to_string(),
            tokens_used: 0,
            error: None,
        });

        emitter.emit_complete(CompleteEvent {
            batch_id: "test-batch".to_string(),
            total_completed: 10,
            total_failed: 0,
            duration_ms: 1000,
        });

        emitter.emit_error("test-batch".to_string(), "Test error".to_string());

        // If we get here, the test passes (no panics)
    }

    #[test]
    fn test_null_emitter_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<NullEventEmitter>();
    }

    #[test]
    fn test_null_emitter_default() {
        let _emitter = NullEventEmitter;
        // Should compile and construct
    }
}
