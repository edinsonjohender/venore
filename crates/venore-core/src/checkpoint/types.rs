use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use crate::analysis::AnalysisDepth;
use crate::analysis::project_analyzer::traits::ProjectType;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub version: String,
    #[serde(default)]
    pub project_path: Option<PathBuf>,
    pub started_at: DateTime<Utc>,
    pub last_updated_at: DateTime<Utc>,

    // Full wizard configuration (Steps 1-4)
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
    pub depth_level: String, // DepthLevel as string for serialization
    pub layers_to_generate: Vec<String>,
    pub exclusions: Vec<String>,

    // Step 2.5: Project Type Detection
    pub project_type: ProjectType,
    pub project_type_confidence: f32,
    pub project_metadata: HashMap<String, String>,

    // Step 3: Analysis Result (metadata only)
    pub total_files_scanned: usize,
    pub total_modules_detected: usize,
    pub module_names: Vec<String>,

    // Step 4: Module Selection + LLM Config
    pub selected_module_names: Vec<String>,
    pub llm_provider: String,
    pub llm_model: Option<String>,
    pub analysis_depth: AnalysisDepth,
}

// Legacy config for backward compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointConfig {
    pub llm_provider: String,
    pub model: Option<String>,
    pub analysis_depth: AnalysisDepth,
    pub project_type: ProjectType,
}

#[derive(Debug)]
pub struct CheckpointInfo {
    pub exists: bool,
    pub completed_count: usize,
    pub total_count: usize,
    pub progress_percent: u8,
}
