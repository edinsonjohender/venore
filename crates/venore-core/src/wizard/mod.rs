//! # Wizard Module
//!
//! Provides wizard-specific functionality for onboarding flow.
//! This module consolidates ALL wizard logic in one place.
//!
//! ## Architecture Decision:
//!
//! ALL business logic lives in venore-core. Clients (Desktop, API, CLI)
//! are thin wrappers that only handle I/O and UI.
//!
//! ## Modules:
//!
//! - `validator` - All validation logic for wizard steps
//! - `batch_manager` - Batch generation state management
//! - `batch_generation` - Core batch context generation orchestration
//! - `session` - Session-scoped state management (replaces ANALYSIS_CACHE)
//! - `ui_state` - Pure functions for computing UI state on backend
//! - `event_emitter` - Event abstraction for progress/completion notifications

pub mod validator;
pub mod batch_manager;
pub mod batch_generation;
pub mod session;
pub mod session_manager;
pub mod ui_state;
pub mod event_emitter;
pub mod cancellation;

// Re-export commonly used items
pub use validator::{
    validate_project_path, validate_project_context, validate_analysis_rules,
    validate_module_selection, validate_llm_config, validate_batch_generation_request,
    ProjectContextInput, AnalysisRulesInput, LLMConfigInput, BatchGenerationRequestInput,
};
pub use batch_manager::{BatchManager, BatchState, BatchStatus, BatchError};
pub use batch_generation::{
    BatchGenerationConfig, ProgressEvent, CompleteEvent,
    BatchGenerationResult, generate_batch_contexts, generate_batch_contexts_with_emitter,
};
pub use session::{WizardSession, WizardConfigInput, RestoredWizardState};
pub use ui_state::{
    compute_progress_ui_state, group_modules_by_confidence, get_module_status,
    ProgressUIState, GroupedModules, ModuleGroup,
};
pub use event_emitter::{
    WizardEventEmitter, NullEventEmitter, CallbackEventEmitter,
    AnalysisProgressEvent, AnalysisCompleteEvent,
};
pub use session_manager::WizardSessionManager;
