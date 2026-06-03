//! DTOs for wizard commands
//!
//! Request and Response types for the onboarding wizard

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// Step 2: Scan Project Files
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProjectRequest {
    pub project_path: String,
    pub exclusions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProjectResponse {
    pub total_files: usize,
    pub extensions: HashMap<String, usize>, // e.g., { "ts": 45, "rs": 23 }
}

// =============================================================================
// Step 3: Detect Modules
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectModulesRequest {
    pub project_path: String,
    pub depth_level: String, // "quick" | "normal" | "deep"
    pub layers: Vec<String>, // ["context", "status", "connections"]
    pub exclusions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectModulesResponse {
    pub modules: Vec<DetectedModule>,
    pub metrics: ProjectMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedModule {
    pub id: String,
    pub name: String,
    pub path: String,
    pub file_count: usize,
    pub confidence: String, // "high" | "medium" | "low"
    pub has_existing_context: bool,
    pub entry_point: Option<String>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetrics {
    pub total_files: usize,
    pub total_modules: usize,
    pub existing_contexts: usize,
}
/// Basic checkpoint info (exists, progress %)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointInfo {
    pub exists: bool,
    pub completed_count: usize,
    pub total_count: usize,
    pub progress_percent: u8,
}

/// Full checkpoint data with wizard config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub version: String,
    pub project_path: Option<String>,
    pub started_at: String, // ISO 8601
    pub last_updated_at: String, // ISO 8601
    pub wizard_config: WizardConfig,
    pub total_modules: usize,
    pub completed_module_ids: Vec<String>,
}

/// Complete wizard configuration (Steps 1-4)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WizardConfig {
    // Step 1: Project Context
    pub project_name: String,
    pub project_description: String,
    pub project_state: String,
    pub team_size: String,
    pub goals: Vec<String>,

    // Step 2: Analysis Rules
    pub depth_level: String,
    pub layers_to_generate: Vec<String>,
    pub exclusions: Vec<String>,

    // Step 2.5: Project Type Detection
    pub project_type: String,
    pub project_type_confidence: f32,
    pub project_metadata: HashMap<String, String>,

    // Step 3: Analysis Result
    pub total_files_scanned: usize,
    pub total_modules_detected: usize,
    pub module_names: Vec<String>,

    // Step 4: Module Selection + LLM Config
    pub selected_module_names: Vec<String>,
    pub llm_provider: String,
    pub llm_model: Option<String>,
    pub analysis_depth: String,
}

// =============================================================================
// Wizard Index Project (new flow: analysis + RAG indexing)
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WizardIndexResponse {
    pub indexed: u32,
    pub skipped: u32,
    pub removed: u32,
    pub modules_detected: u32,
    pub modules_mapped: u32,
    pub deps_created: u32,
    pub refs_created: u32,
}

// =============================================================================

// =============================================================================
// Wizard Validation (Phase 3)
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "step", content = "data")]
pub enum ValidateWizardStepRequest {
    #[serde(rename = "path")]
    Path { path: String },

    #[serde(rename = "project_context")]
    ProjectContext {
        name: String,
        description: String,
        state: String,
        team_size: String,
        goals: Vec<String>,
    },

    #[serde(rename = "analysis_rules")]
    AnalysisRules {
        depth_level: String,
        layers_to_generate: Vec<String>,
        exclusions: Vec<String>,
    },

    #[serde(rename = "module_selection")]
    ModuleSelection {
        selected_modules: Vec<String>,
    },

    #[serde(rename = "llm_config")]
    LLMConfig {
        provider: String,
        model: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateWizardStepResponse {
    pub is_valid: bool,
    pub errors: Vec<String>,
}

// =============================================================================
// Wizard Session Management
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreWizardSessionResponse {
    pub wizard_config: WizardConfig,
    pub completed_module_names: Vec<String>,
    pub total_modules: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetRecommendedModulesRequest {
    pub project_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetRecommendedModulesResponse {
    pub recommended_modules: Vec<String>,
}

// =============================================================================
// Project Type Detection (Optional)
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTypeResponse {
    pub project_type: String, // "monorepo" | "multi-module" | "single-module"
    pub framework: Option<String>, // "react", "rust", "node", etc.
    pub package_manager: Option<String>, // "npm", "cargo", "pnpm", etc.
}

// =============================================================================
// Module Grouping (UI Helper)
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetModuleGroupsRequest {
    pub modules: Vec<SimpleModule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleModule {
    pub name: String,
    pub path: String,
    pub file_count: usize,
    pub confidence: String, // "high" | "medium" | "low"
    pub has_entry_point: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetModuleGroupsResponse {
    pub high: Vec<SimpleModule>,
    pub medium: Vec<SimpleModule>,
    pub low: Vec<SimpleModule>,
}
