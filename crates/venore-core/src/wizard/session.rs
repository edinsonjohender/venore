//! # Wizard Session
//!
//! Session-scoped state management for the onboarding wizard.
//! Replaces global ANALYSIS_CACHE with session-based storage.
//!
//! ## Architecture:
//! - Encapsulates ALL wizard state (Steps 1-8)
//! - Integrates with CheckpointManager for persistence
//! - Backend owns all business logic
//! - Frontend is a "dumb renderer"

use std::path::PathBuf;
use std::collections::HashMap;
use crate::error::{VenoreError, Result};
use crate::analysis::AnalysisOutput;
use crate::checkpoint::{CheckpointManager, WizardConfig};

// =============================================================================
// WizardSession - Core session state
// =============================================================================

/// Session-scoped wizard state
///
/// This struct encapsulates ALL state for a wizard session, including:
/// - Cached analysis results (from Step 3)
/// - Wizard configuration (Steps 1-4)
/// - Checkpoint integration
///
/// Replaces global ANALYSIS_CACHE with session-scoped storage.
pub struct WizardSession {
    /// Project path (unique identifier for session)
    pub project_path: PathBuf,

    /// Cached analysis output (from Step 3: detect_project_modules)
    /// This stores the full analysis to avoid re-scanning in Steps 4-7
    cached_analysis: Option<AnalysisOutput>,

    /// Wizard configuration (accumulated from Steps 1-4)
    wizard_config: Option<WizardConfig>,

    /// Checkpoint manager for persistence
    checkpoint_manager: CheckpointManager,
}

impl WizardSession {
    /// Creates a new wizard session
    ///
    /// # Arguments
    /// * `project_path` - Absolute path to the project directory
    ///
    /// # Examples
    /// ```rust,ignore
    /// use venore_core::wizard::WizardSession;
    /// use std::path::PathBuf;
    ///
    /// let session = WizardSession::new(PathBuf::from("/path/to/project"));
    /// ```
    pub fn new(project_path: PathBuf) -> Self {
        let checkpoint_manager = CheckpointManager::new(&project_path);

        Self {
            project_path,
            cached_analysis: None,
            wizard_config: None,
            checkpoint_manager,
        }
    }

    /// Gets the project path for this session
    ///
    /// # Examples
    /// ```rust,ignore
    /// let path = session.project_path();
    /// ```
    pub fn project_path(&self) -> &PathBuf {
        &self.project_path
    }

    // =========================================================================
    // Analysis Cache Management
    // =========================================================================

    /// Caches analysis output for reuse in later steps
    ///
    /// Called by Step 3 (detect_project_modules) to store the full analysis.
    /// This avoids re-scanning the project in Steps 4-7.
    ///
    /// # Arguments
    /// * `analysis` - The full analysis output from Step 3
    ///
    /// # Examples
    /// ```rust,ignore
    /// session.cache_analysis(full_analysis);
    /// ```
    pub fn cache_analysis(&mut self, analysis: AnalysisOutput) {
        tracing::info!(
            "💾 Caching analysis: {} modules, {} orphan files",
            analysis.modules.len(),
            analysis.orphan_files.len()
        );
        self.cached_analysis = Some(analysis);
    }

    /// Retrieves cached analysis
    ///
    /// Returns the cached analysis if available, otherwise returns an error.
    /// Steps 4-7 use this to avoid re-scanning.
    ///
    /// # Errors
    /// Returns error if no analysis has been cached (Step 3 not run)
    ///
    /// # Examples
    /// ```rust,ignore
    /// let analysis = session.get_cached_analysis()?;
    /// ```
    pub fn get_cached_analysis(&self) -> Result<&AnalysisOutput> {
        self.cached_analysis
            .as_ref()
            .ok_or_else(|| VenoreError::NotFound("No cached analysis. Step 3 (module detection) must run first.".into()))
    }

    /// Checks if analysis is cached
    ///
    /// # Examples
    /// ```rust,ignore
    /// if session.has_cached_analysis() {
    ///     // Use cached analysis
    /// }
    /// ```
    pub fn has_cached_analysis(&self) -> bool {
        self.cached_analysis.is_some()
    }

    // =========================================================================
    // Wizard Config Management
    // =========================================================================

    /// Stores wizard configuration
    ///
    /// Accumulates wizard configuration from Steps 1-4.
    ///
    /// # Arguments
    /// * `config` - The wizard configuration
    pub fn set_wizard_config(&mut self, config: WizardConfig) {
        tracing::info!("📝 Storing wizard config: {}", config.project_name);
        self.wizard_config = Some(config);
    }

    /// Retrieves wizard configuration
    ///
    /// # Errors
    /// Returns error if no configuration has been set
    pub fn get_wizard_config(&self) -> Result<&WizardConfig> {
        self.wizard_config
            .as_ref()
            .ok_or_else(|| VenoreError::NotFound("No wizard configuration available".into()))
    }

    // =========================================================================
    // Checkpoint Integration
    // =========================================================================

    /// Gets reference to checkpoint manager
    ///
    /// Allows direct access to checkpoint operations.
    pub fn checkpoint_manager(&self) -> &CheckpointManager {
        &self.checkpoint_manager
    }

    /// Checks if checkpoint exists for this session
    pub fn has_checkpoint(&self) -> bool {
        self.checkpoint_manager.exists()
    }

    // =========================================================================
    // Phase 3: Auto-Selection and Recommendations
    // =========================================================================

    /// Gets recommended modules based on confidence levels (Phase 3)
    ///
    /// Auto-selects modules with high confidence (has entry point).
    /// This helps users quickly select the most important modules.
    ///
    /// # Returns
    /// List of recommended module names (high confidence only)
    ///
    /// # Examples
    /// ```rust,ignore
    /// let recommended = session.get_recommended_modules()?;
    /// ```
    pub fn get_recommended_modules(&self) -> Result<Vec<String>> {
        tracing::info!("🔍 Getting recommended modules");

        // Get cached analysis
        let analysis = self.get_cached_analysis()?;

        // Use ui_state helper to group by confidence
        let grouped = crate::wizard::ui_state::group_modules_by_confidence(&analysis.modules);

        // Return only high confidence modules
        let recommended: Vec<String> = grouped
            .high_confidence
            .iter()
            .map(|m| m.name.clone())
            .collect();

        tracing::info!("✅ Found {} recommended modules (high confidence)", recommended.len());

        Ok(recommended)
    }

    // =========================================================================
    // Phase 2: Business Logic Methods
    // =========================================================================

    /// Builds wizard config from user inputs (Step 6)
    ///
    /// Migrated from Step6Generation.tsx:164-193.
    /// Constructs WizardConfig from wizard state accumulated in Steps 1-4.
    ///
    /// # Arguments
    /// * `input` - Wizard config input data
    ///
    /// # Examples
    /// ```rust,ignore
    /// let wizard_config = session.build_wizard_config(input)?;
    /// ```
    pub fn build_wizard_config(
        &mut self,
        input: WizardConfigInput,
    ) -> Result<WizardConfig> {
        tracing::info!("🔧 Building wizard config for project: {}", input.project_name);

        // Get cached analysis for module information
        let analysis = self.get_cached_analysis()?;

        let config = WizardConfig {
            // Step 1: Project Context
            project_name: input.project_name,
            project_description: input.project_description,
            project_state: input.project_state,
            team_size: input.team_size,
            goals: input.goals,

            // Step 2: Analysis Rules
            depth_level: input.depth_level.clone(),
            layers_to_generate: input.layers_to_generate,
            exclusions: input.exclusions,

            // Step 2.5: Project Type Detection
            project_type: input.project_type,
            project_type_confidence: input.project_type_confidence,
            project_metadata: input.project_metadata,

            // Step 3: Analysis Results
            total_files_scanned: analysis.repository.total_files,
            total_modules_detected: input.all_detected_modules.len(),
            module_names: input.all_detected_modules,

            // Step 4: Module Selection + LLM Config
            selected_module_names: input.selected_module_names,
            llm_provider: input.llm_provider,
            llm_model: input.llm_model,
            analysis_depth: parse_analysis_depth(&input.depth_level),
        };

        tracing::info!("✅ Built wizard config: {} selected modules, {} layers",
            config.selected_module_names.len(),
            config.layers_to_generate.len(),
        );

        // Store in session so other commands (ocean, etc.) can read it
        self.wizard_config = Some(config.clone());

        Ok(config)
    }

    /// Filters out completed modules (Step 6)
    ///
    /// Migrated from Step6Generation.tsx:126-147.
    /// Uses case-insensitive name comparison because module IDs are sequential
    /// and not stable across detections.
    ///
    /// # Arguments
    /// * `selected_modules` - User-selected module names
    /// * `completed_modules` - Module names that are already completed
    ///
    /// # Returns
    /// List of module names that still need context generation
    ///
    /// # Examples
    /// ```rust,ignore
    /// let remaining = session.get_remaining_modules(&selected, &completed)?;
    /// ```
    pub fn get_remaining_modules(
        &self,
        selected_modules: &[String],
        completed_modules: &[String],
    ) -> Result<Vec<String>> {
        tracing::info!(
            "🔍 Filtering modules: {} selected, {} completed",
            selected_modules.len(),
            completed_modules.len()
        );

        // CRITICAL: Case-insensitive comparison because module names may have different capitalization
        let completed_lower: std::collections::HashSet<String> = completed_modules
            .iter()
            .map(|name| name.to_lowercase())
            .collect();

        let remaining: Vec<String> = selected_modules
            .iter()
            .filter(|name| !completed_lower.contains(&name.to_lowercase()))
            .cloned()
            .collect();

        tracing::info!("✅ Found {} remaining modules to generate", remaining.len());

        Ok(remaining)
    }

    /// Restores wizard state from checkpoint
    ///
    /// Migrated from OnboardingWizardModal.tsx:233-300.
    /// Loads checkpoint and returns structured state for frontend.
    ///
    /// # Returns
    /// Restored wizard state ready for frontend display
    ///
    /// # Examples
    /// ```rust,ignore
    /// let restored_state = session.restore_from_checkpoint()?;
    /// ```
    pub fn restore_from_checkpoint(&mut self) -> Result<RestoredWizardState> {
        tracing::info!("📦 Restoring wizard state from checkpoint");

        // Load checkpoint
        let checkpoint = self.checkpoint_manager
            .load()
            .map_err(|e| VenoreError::FileReadError(format!("Failed to load checkpoint: {}", e)))?
            .ok_or_else(|| VenoreError::NotFound("No checkpoint found".into()))?;

        // Store wizard config in session
        self.wizard_config = Some(checkpoint.wizard_config.clone());

        tracing::info!(
            "✅ Restored checkpoint: {}/{} modules completed",
            checkpoint.completed_module_ids.len(),
            checkpoint.total_modules
        );

        Ok(RestoredWizardState {
            wizard_config: checkpoint.wizard_config,
            completed_module_names: checkpoint.completed_module_ids,
            total_modules: checkpoint.total_modules,
        })
    }
}

// =============================================================================
// Phase 2 Types
// =============================================================================

/// Input for building wizard config
#[derive(Debug, Clone)]
pub struct WizardConfigInput {
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

    // Step 2.5: Project Type
    pub project_type: crate::analysis::project_analyzer::traits::ProjectType,
    pub project_type_confidence: f32,
    pub project_metadata: HashMap<String, String>,

    // Step 3: All detected modules
    pub all_detected_modules: Vec<String>,

    // Step 4: Selected modules + LLM
    pub selected_module_names: Vec<String>,
    pub llm_provider: String,
    pub llm_model: Option<String>,
}

/// Restored wizard state (for frontend display)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RestoredWizardState {
    pub wizard_config: WizardConfig,
    pub completed_module_names: Vec<String>,
    pub total_modules: usize,
}

/// Helper: Parse analysis depth from string
fn parse_analysis_depth(depth: &str) -> crate::analysis::AnalysisDepth {
    use crate::analysis::AnalysisDepth;
    match depth.to_lowercase().as_str() {
        "minimal" => AnalysisDepth::Minimal,
        "detailed" => AnalysisDepth::Detailed,
        "expert" => AnalysisDepth::Expert,
        _ => AnalysisDepth::Normal,
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{RepositoryInfo, ModuleAnalysis};

    fn create_test_analysis() -> AnalysisOutput {
        use crate::analysis::{Language, ModuleArchitecture, ModuleSymbols};

        AnalysisOutput {
            repository: RepositoryInfo {
                name: "test-project".to_string(),
                language: Some(Language::TypeScript),
                technologies: vec!["TypeScript".to_string()],
                total_files: 10,
                total_modules: 1,
            },
            modules: vec![
                ModuleAnalysis {
                    name: "module1".to_string(),
                    path: "/test/module1".to_string(),
                    file_count: 5,
                    entry_point: Some("/test/module1/index.ts".to_string()),
                    architecture: ModuleArchitecture {
                        dependencies: vec![],
                        dependents: vec![],
                        external_deps: vec![],
                    },
                    symbols: ModuleSymbols {
                        exports: vec![],
                        all: vec![],
                    },
                    imports: vec![],
                    code_snippets: String::new(),
                    files: vec![],
                }
            ],
            orphan_files: vec![],
        }
    }

    #[test]
    fn test_session_creation() {
        let project_path = PathBuf::from("/tmp/test-project");
        let session = WizardSession::new(project_path.clone());

        assert_eq!(session.project_path, project_path);
        assert!(!session.has_cached_analysis());
    }

    #[test]
    fn test_cache_analysis() {
        let mut session = WizardSession::new(PathBuf::from("/tmp/test-project"));
        let analysis = create_test_analysis();

        assert!(!session.has_cached_analysis());

        session.cache_analysis(analysis);

        assert!(session.has_cached_analysis());
    }

    #[test]
    fn test_get_cached_analysis() {
        let mut session = WizardSession::new(PathBuf::from("/tmp/test-project"));

        // Should fail when no analysis cached
        assert!(session.get_cached_analysis().is_err());

        // Cache analysis
        let analysis = create_test_analysis();
        session.cache_analysis(analysis);

        // Should succeed now
        let cached = session.get_cached_analysis().unwrap();
        assert_eq!(cached.modules.len(), 1);
        assert_eq!(cached.modules[0].name, "module1");
    }

    #[test]
    fn test_wizard_config() {
        let mut session = WizardSession::new(PathBuf::from("/tmp/test-project"));

        // Should fail when no config set
        assert!(session.get_wizard_config().is_err());

        // Set config
        let config = WizardConfig {
            project_name: "Test Project".to_string(),
            project_description: "A test".to_string(),
            project_state: "active".to_string(),
            team_size: "small".to_string(),
            goals: vec!["Test".to_string()],
            depth_level: "normal".to_string(),
            layers_to_generate: vec!["context".to_string()],
            exclusions: vec![],
            project_type: crate::analysis::project_analyzer::traits::ProjectType::Unknown,
            project_type_confidence: 1.0,
            project_metadata: HashMap::new(),
            total_files_scanned: 10,
            total_modules_detected: 1,
            module_names: vec!["module1".to_string()],
            selected_module_names: vec!["module1".to_string()],
            llm_provider: "openai".to_string(),
            llm_model: Some("gpt-4.1".to_string()),
            analysis_depth: crate::analysis::AnalysisDepth::Normal,
        };

        session.set_wizard_config(config);

        // Should succeed now
        let stored = session.get_wizard_config().unwrap();
        assert_eq!(stored.project_name, "Test Project");
    }

    #[test]
    fn test_build_wizard_config() {
        let mut session = WizardSession::new(PathBuf::from("/tmp/test-project"));

        // Cache analysis first (required)
        let analysis = create_test_analysis();
        session.cache_analysis(analysis);

        // Build wizard config
        let input = WizardConfigInput {
            project_name: "Test Project".to_string(),
            project_description: "A test project".to_string(),
            project_state: "active".to_string(),
            team_size: "small".to_string(),
            goals: vec!["Testing".to_string()],
            depth_level: "normal".to_string(),
            layers_to_generate: vec!["context".to_string()],
            exclusions: vec!["node_modules".to_string()],
            project_type: crate::analysis::project_analyzer::traits::ProjectType::NodeMonorepo,
            project_type_confidence: 0.95,
            project_metadata: HashMap::new(),
            all_detected_modules: vec!["module1".to_string(), "module2".to_string()],
            selected_module_names: vec!["module1".to_string()],
            llm_provider: "openai".to_string(),
            llm_model: Some("gpt-4.1".to_string()),
        };

        let config = session.build_wizard_config(input).unwrap();

        assert_eq!(config.project_name, "Test Project");
        assert_eq!(config.selected_module_names, vec!["module1"]);
        assert_eq!(config.total_modules_detected, 2);
        assert_eq!(config.total_files_scanned, 10);
    }

    #[test]
    fn test_get_remaining_modules() {
        let session = WizardSession::new(PathBuf::from("/tmp/test-project"));

        let selected = vec![
            "ModuleA".to_string(),
            "ModuleB".to_string(),
            "ModuleC".to_string(),
        ];

        let completed = vec!["modulea".to_string()]; // Different case!

        let remaining = session.get_remaining_modules(&selected, &completed).unwrap();

        // Should have 2 remaining (case-insensitive match)
        assert_eq!(remaining.len(), 2);
        assert!(remaining.contains(&"ModuleB".to_string()));
        assert!(remaining.contains(&"ModuleC".to_string()));
        assert!(!remaining.contains(&"ModuleA".to_string()));
    }

    #[test]
    fn test_get_remaining_modules_case_insensitive() {
        let session = WizardSession::new(PathBuf::from("/tmp/test-project"));

        let selected = vec![
            "my-Module".to_string(),
            "other-MODULE".to_string(),
        ];

        let completed = vec!["MY-MODULE".to_string(), "Other-Module".to_string()];

        let remaining = session.get_remaining_modules(&selected, &completed).unwrap();

        // Should have 0 remaining (case-insensitive match)
        assert_eq!(remaining.len(), 0);
    }

    #[test]
    fn test_get_recommended_modules() {
        use crate::analysis::{Language, ModuleAnalysis, ModuleArchitecture, ModuleSymbols};

        let mut session = WizardSession::new(PathBuf::from("/tmp/test-project"));

        // Create analysis with mixed confidence modules
        let analysis = AnalysisOutput {
            repository: RepositoryInfo {
                name: "test-project".to_string(),
                language: Some(Language::TypeScript),
                technologies: vec!["TypeScript".to_string()],
                total_files: 20,
                total_modules: 3,
            },
            modules: vec![
                // High confidence (has entry point)
                ModuleAnalysis {
                    name: "high-module".to_string(),
                    path: "/test/high-module".to_string(),
                    file_count: 10,
                    entry_point: Some("/test/high-module/index.ts".to_string()),
                    architecture: ModuleArchitecture {
                        dependencies: vec![],
                        dependents: vec![],
                        external_deps: vec![],
                    },
                    symbols: ModuleSymbols {
                        exports: vec![],
                        all: vec![],
                    },
                    imports: vec![],
                    code_snippets: String::new(),
                    files: vec![],
                },
                // Medium confidence (6 files, no entry point)
                ModuleAnalysis {
                    name: "medium-module".to_string(),
                    path: "/test/medium-module".to_string(),
                    file_count: 6,
                    entry_point: None,
                    architecture: ModuleArchitecture {
                        dependencies: vec![],
                        dependents: vec![],
                        external_deps: vec![],
                    },
                    symbols: ModuleSymbols {
                        exports: vec![],
                        all: vec![],
                    },
                    imports: vec![],
                    code_snippets: String::new(),
                    files: vec![],
                },
                // Low confidence (3 files, no entry point)
                ModuleAnalysis {
                    name: "low-module".to_string(),
                    path: "/test/low-module".to_string(),
                    file_count: 3,
                    entry_point: None,
                    architecture: ModuleArchitecture {
                        dependencies: vec![],
                        dependents: vec![],
                        external_deps: vec![],
                    },
                    symbols: ModuleSymbols {
                        exports: vec![],
                        all: vec![],
                    },
                    imports: vec![],
                    code_snippets: String::new(),
                    files: vec![],
                },
            ],
            orphan_files: vec![],
        };

        session.cache_analysis(analysis);

        let recommended = session.get_recommended_modules().unwrap();

        // Should only recommend high confidence modules
        assert_eq!(recommended.len(), 1);
        assert!(recommended.contains(&"high-module".to_string()));
        assert!(!recommended.contains(&"medium-module".to_string()));
        assert!(!recommended.contains(&"low-module".to_string()));
    }
}
