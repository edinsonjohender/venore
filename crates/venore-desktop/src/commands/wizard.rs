//! Wizard Commands - Complete onboarding workflow
//!
//! All commands use venore-core (NO MOCKS).
//! Architecture follows venore-cli patterns.

use std::sync::Arc;

use crate::commands::dto::wizard::*;
use crate::state::LazyAppState;
use crate::utils::{CommandResult, StateCommandResult, IntoStateCommandResult};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use venore_core::error::VenoreError;

// venore-core imports
use venore_core::analysis::{
    file_scanner::{self, ScanConfig},
    ast_parser::{self, ParseConfig, Language},
    module_detector::{self, DetectorConfig},
    project_analyzer,
    AnalysisBuilder, AnalysisConfig, AnalysisDepth,
};
use venore_core::checkpoint::CheckpointManager;
use venore_core::project::ProjectService;
use venore_core::wizard::validator::*;
use venore_core::wizard::WizardSessionManager;

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Convert string depth level to venore-core DepthLevel
fn parse_depth_level(level: &str) -> venore_core::context::DepthLevel {
    match level.to_lowercase().as_str() {
        "minimal" => venore_core::context::DepthLevel::Minimal,
        "detailed" => venore_core::context::DepthLevel::Detailed,
        "expert" => venore_core::context::DepthLevel::Expert,
        _ => venore_core::context::DepthLevel::Normal,
    }
}

/// Convert depth level to AnalysisDepth
fn depth_to_analysis_depth(level: &venore_core::context::DepthLevel) -> AnalysisDepth {
    match level {
        venore_core::context::DepthLevel::Minimal => AnalysisDepth::Minimal,
        venore_core::context::DepthLevel::Normal => AnalysisDepth::Normal,
        venore_core::context::DepthLevel::Detailed => AnalysisDepth::Detailed,
        venore_core::context::DepthLevel::Expert => AnalysisDepth::Expert,
    }
}

// =============================================================================
// Step 1: File Scanning (REAL - uses venore-core)
// =============================================================================

#[tauri::command]
pub async fn scan_project_files(
    request: ScanProjectRequest,
) -> CommandResult<ScanProjectResponse> {
    tracing::info!("scan_project_files: {}", request.project_path);

    let result: Result<ScanProjectResponse, VenoreError> = (|| {
        // VALIDATION: Validate project path
        let validation = validate_project_path(&request.project_path);
        if !validation.is_valid() {
            let error_msg = validation.error.unwrap_or_else(|| "Invalid project path".to_string());
            tracing::warn!("🚫 Path validation failed: {}", error_msg);
            return Err(VenoreError::InvalidPath(error_msg));
        }

        let ignore_patterns = if request.exclusions.is_empty() {
            vec![
                "node_modules".into(),
                "dist".into(),
                "build".into(),
                ".next".into(),
                "coverage".into(),
                "target".into(),
            ]
        } else {
            request.exclusions
        };

        let scan_config = ScanConfig {
            project_path: PathBuf::from(&request.project_path),
            target_extensions: Language::all_extensions()
                .iter()
                .map(|s| s.to_string())
                .collect(),
            ignore_patterns,
            max_file_size_kb: 1024,
        };

        let scan_result = file_scanner::scan_directory(scan_config)
            .map_err(|e| VenoreError::AnalysisError(format!("Failed to scan directory: {}", e)))?;

        tracing::info!("✅ Scanned {} files in {}ms",
            scan_result.files.len(),
            scan_result.scan_duration_ms
        );

        let mut extensions = std::collections::HashMap::new();
        for file in &scan_result.files {
            *extensions.entry(file.extension.clone()).or_insert(0) += 1;
        }

        Ok(ScanProjectResponse {
            total_files: scan_result.files.len(),
            extensions,
        })
    })();

    result.into()
}

// =============================================================================
// Step 3: Module Detection (REAL - uses venore-core)
// =============================================================================

#[tauri::command]
pub async fn detect_project_modules(
    app_handle: AppHandle,
    request: DetectModulesRequest,
) -> CommandResult<DetectModulesResponse> {
    use venore_core::wizard::{AnalysisProgressEvent, AnalysisCompleteEvent};
    use venore_core::wizard::cancellation::CancellationGuard;
    use std::time::Instant;

    tracing::info!("detect_project_modules: {}", request.project_path);

    let start_time = Instant::now();
    let session_id = request.project_path.clone();
    let cancel_guard = CancellationGuard::register(&request.project_path);

    let result: Result<DetectModulesResponse, VenoreError> = async {
        // VALIDATION: Validate project path
        let validation = validate_project_path(&request.project_path);
        if !validation.is_valid() {
            let error_msg = validation.error.unwrap_or_else(|| "Invalid project path".to_string());
            tracing::warn!("🚫 Path validation failed: {}", error_msg);

            let _ = app_handle.emit("analysis-complete", AnalysisCompleteEvent {
                session_id: session_id.clone(),
                total_files: 0,
                total_modules: 0,
                duration_ms: start_time.elapsed().as_millis() as u64,
                success: false,
                error: Some(error_msg.clone()),
            });

            return Err(VenoreError::InvalidPath(error_msg));
        }

        // Step 1/5: Scan files
        let _ = app_handle.emit("analysis-progress", AnalysisProgressEvent {
            session_id: session_id.clone(),
            current_step: 1,
            total_steps: 5,
            step_description: "Scanning files...".to_string(),
            current_item: None,
            step_progress: None,
        });
        let scan_config = ScanConfig {
            project_path: PathBuf::from(&request.project_path),
            target_extensions: Language::all_extensions()
                .iter()
                .map(|s| s.to_string())
                .collect(),
            ignore_patterns: request.exclusions,
            max_file_size_kb: 1024,
        };

        let scan_result = file_scanner::scan_directory(scan_config)
            .map_err(|e| {
                let error_msg = format!("Failed to scan: {}", e);
                let _ = app_handle.emit("analysis-complete", AnalysisCompleteEvent {
                    session_id: session_id.clone(),
                    total_files: 0,
                    total_modules: 0,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                    success: false,
                    error: Some(error_msg.clone()),
                });
                VenoreError::AnalysisError(error_msg)
            })?;

        tracing::info!("📁 Scanned {} files", scan_result.files.len());

        if cancel_guard.is_cancelled() {
            return Err(VenoreError::Cancelled("detect_project_modules cancelled after scan".to_string()));
        }

        // Step 2/5: Parse files for AST analysis
        let _ = app_handle.emit("analysis-progress", AnalysisProgressEvent {
            session_id: session_id.clone(),
            current_step: 2,
            total_steps: 5,
            step_description: "Parsing AST...".to_string(),
            current_item: Some(format!("{} files", scan_result.files.len())),
            step_progress: None,
        });
        let parse_results: Vec<_> = scan_result
            .files
            .iter()
            .filter_map(|file| {
                let content = std::fs::read_to_string(&file.path).ok()?;
                let language = Language::from_extension(&file.extension)?;

                let config = ParseConfig {
                    file_path: file.path.clone(),
                    language,
                    content,
                };

                ast_parser::parse_file(config).ok()
            })
            .collect();

        tracing::info!("🔍 Parsed {} files", parse_results.len());

        if cancel_guard.is_cancelled() {
            return Err(VenoreError::Cancelled("detect_project_modules cancelled after parsing".to_string()));
        }

        // Step 3/5: Detect project type
        let _ = app_handle.emit("analysis-progress", AnalysisProgressEvent {
            session_id: session_id.clone(),
            current_step: 3,
            total_steps: 5,
            step_description: "Detecting project type...".to_string(),
            current_item: None,
            step_progress: None,
        });
        let project_path_buf = PathBuf::from(&request.project_path);
        let detection_strategy = project_analyzer::detect_project_type(&project_path_buf)
            .await
            .ok()
            .and_then(|detection| {
                project_analyzer::get_analyzer(detection.project_type)
                    .ok()
                    .map(|analyzer| analyzer.module_detection_strategy())
            });

        // Step 4/5: Detect modules
        let _ = app_handle.emit("analysis-progress", AnalysisProgressEvent {
            session_id: session_id.clone(),
            current_step: 4,
            total_steps: 5,
            step_description: "Detecting modules...".to_string(),
            current_item: None,
            step_progress: None,
        });
        let detector_config = DetectorConfig {
            files: scan_result.files.clone(),
            parse_results: parse_results.clone(),
            project_root: PathBuf::from(&request.project_path),
            detection_strategy,
        };

        let detection_result = module_detector::detect_modules(detector_config)
            .map_err(|e| VenoreError::AnalysisError(format!("Failed to detect modules: {}", e)))?;

        tracing::info!("✅ Detected {} modules in {}ms",
            detection_result.modules.len(),
            detection_result.detection_duration_ms
        );

        // Step 5/5: Build analysis cache
        let _ = app_handle.emit("analysis-progress", AnalysisProgressEvent {
            session_id: session_id.clone(),
            current_step: 5,
            total_steps: 5,
            step_description: "Building analysis cache...".to_string(),
            current_item: Some(format!("{} modules", detection_result.modules.len())),
            step_progress: None,
        });
        let depth_level = parse_depth_level(&request.depth_level);
        let analysis_depth = depth_to_analysis_depth(&depth_level);

        let analysis_config = AnalysisConfig {
            scan_result: scan_result.clone(),
            parse_results,
            modules: detection_result.clone(),
            project_root: PathBuf::from(&request.project_path),
            depth: analysis_depth,
        };

        let full_analysis = AnalysisBuilder::new(analysis_config).build();

        // Persist analysis to disk so node panels can load it without an active session
        if let Err(e) = full_analysis.save_to_disk(std::path::Path::new(&request.project_path)) {
            tracing::warn!("Failed to persist analysis to disk: {}", e);
        }

        tracing::info!("💾 Caching analysis in session for path: '{}'", request.project_path);
        {
            let manager = WizardSessionManager::global();
            let mut sessions = manager.lock().unwrap();
            let session = sessions.get_or_create(PathBuf::from(&request.project_path));
            session.cache_analysis(full_analysis);
        }

        tracing::info!("💾 Cached analysis for {} modules in session", detection_result.modules.len());

        let modules: Vec<DetectedModule> = detection_result
            .modules
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let confidence = if m.entry_point.is_some() {
                    "high".to_string()
                } else if m.files.len() > 5 {
                    "medium".to_string()
                } else {
                    "low".to_string()
                };

                let has_existing_context = m.path.join(".context.md").exists();

                let absolute_path = if m.path.is_absolute() {
                    m.path.clone()
                } else {
                    project_path_buf.join(&m.path)
                };

                DetectedModule {
                    id: format!("module-{}", i),
                    name: m.name.clone(),
                    path: absolute_path.display().to_string(),
                    file_count: m.files.len(),
                    confidence,
                    has_existing_context,
                    entry_point: m.entry_point.as_ref().map(|p| {
                        p.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown")
                            .to_string()
                    }),
                    description: format!("Module with {} files", m.files.len()),
                }
            })
            .collect();

        let _ = app_handle.emit("analysis-complete", AnalysisCompleteEvent {
            session_id: session_id.clone(),
            total_files: scan_result.files.len(),
            total_modules: detection_result.modules.len(),
            duration_ms: start_time.elapsed().as_millis() as u64,
            success: true,
            error: None,
        });

        Ok(DetectModulesResponse {
            modules,
            metrics: ProjectMetrics {
                total_files: scan_result.files.len(),
                total_modules: detection_result.modules.len(),
                existing_contexts: detection_result.modules.iter()
                    .filter(|m| m.path.join(".context.md").exists())
                    .count(),
            },
        })
    }.await;

    result.into()
}

// =============================================================================
// UI Helper: Group Modules by Confidence
// =============================================================================

#[tauri::command]
pub fn get_module_groups(
    request: GetModuleGroupsRequest,
) -> CommandResult<GetModuleGroupsResponse> {
    tracing::info!("get_module_groups: grouping {} modules", request.modules.len());

    let mut high = Vec::new();
    let mut medium = Vec::new();
    let mut low = Vec::new();

    for module in request.modules {
        match module.confidence.as_str() {
            "high" => high.push(module),
            "medium" => medium.push(module),
            "low" => low.push(module),
            _ => low.push(module),
        }
    }

    CommandResult::ok(GetModuleGroupsResponse { high, medium, low })
}

// =============================================================================
// Step 2.5: Project Type Detection (REAL - uses venore-core)
// =============================================================================

#[tauri::command]
pub async fn detect_project_type(
    project_path: String,
) -> CommandResult<ProjectTypeResponse> {
    tracing::info!("detect_project_type: {}", project_path);

    let result: Result<ProjectTypeResponse, VenoreError> = async {
        let project_path_buf = PathBuf::from(&project_path);
        let detection = project_analyzer::detect_project_type(&project_path_buf)
            .await
            .map_err(|e| VenoreError::AnalysisError(format!("Failed to detect project type: {}", e)))?;

        let project_type = format!("{:?}", detection.project_type);

        let framework = if PathBuf::from(&project_path).join("package.json").exists() {
            Some("typescript".to_string())
        } else if PathBuf::from(&project_path).join("Cargo.toml").exists() {
            Some("rust".to_string())
        } else if PathBuf::from(&project_path).join("go.mod").exists() {
            Some("go".to_string())
        } else {
            None
        };

        let package_manager = if PathBuf::from(&project_path).join("pnpm-lock.yaml").exists() {
            Some("pnpm".to_string())
        } else if PathBuf::from(&project_path).join("yarn.lock").exists() {
            Some("yarn".to_string())
        } else if PathBuf::from(&project_path).join("package-lock.json").exists() {
            Some("npm".to_string())
        } else {
            None
        };

        tracing::info!("✅ Detected project type: {}", project_type);

        Ok(ProjectTypeResponse { project_type, framework, package_manager })
    }.await;

    result.into()
}


/// Cancel an in-flight wizard pipeline for `project_path`.
///
/// Flips the cancellation token registered by `detect_project_modules` /
/// `wizard_index_project`; the in-flight task notices at its next checkpoint
/// and returns `VenoreError::Cancelled`. If no pipeline is registered (e.g.
/// the user closed the wizard outside the analysis/indexing phases), the
/// call is a no-op.
#[tauri::command]
pub async fn cancel_wizard_session(project_path: String) -> CommandResult<bool> {
    use venore_core::wizard::cancellation;
    tracing::info!("🛑 cancel_wizard_session: {}", project_path);
    let cancelled = cancellation::cancel(&project_path);
    if cancelled {
        tracing::info!("✅ Cancellation signal sent");
    } else {
        tracing::info!("ℹ️  No active wizard pipeline for this project (no-op)");
    }
    CommandResult::ok(cancelled)
}



// =============================================================================
// Step 8: Checkpoint Management
// =============================================================================

/// Check if a checkpoint exists for a project (lightweight)
#[tauri::command]
pub async fn check_wizard_checkpoint(path: String) -> CommandResult<Option<CheckpointInfo>> {
    tracing::info!("🔍 Checking checkpoint for: {}", path);

    let result: Result<Option<CheckpointInfo>, VenoreError> = (|| {
        let manager = CheckpointManager::new(Path::new(&path));

        if !manager.exists() {
            tracing::info!("   No checkpoint found");
            return Ok(None);
        }

        match manager.load() {
            Ok(Some(_checkpoint)) => {
                let info = manager.get_info();

                tracing::info!(
                    "   ✓ Checkpoint found: {}/{} modules ({}%)",
                    info.completed_count,
                    info.total_count,
                    info.progress_percent
                );

                Ok(Some(CheckpointInfo {
                    exists: info.exists,
                    completed_count: info.completed_count,
                    total_count: info.total_count,
                    progress_percent: info.progress_percent,
                }))
            }
            Ok(None) => {
                tracing::warn!("   Checkpoint corrupted, auto-backed up");
                Ok(None)
            }
            Err(e) => {
                tracing::error!("   Error loading checkpoint: {}", e);
                Err(VenoreError::FileReadError(format!("Error loading checkpoint: {}", e)))
            }
        }
    })();

    result.into()
}

/// Load full checkpoint data (for resuming wizard)
#[tauri::command]
pub async fn load_full_checkpoint(path: String) -> CommandResult<Checkpoint> {
    tracing::info!("📦 Loading full checkpoint for: {}", path);

    let result: Result<Checkpoint, VenoreError> = (|| {
        let manager = CheckpointManager::new(Path::new(&path));

        let checkpoint = manager
            .load()
            .map_err(|e| VenoreError::FileReadError(format!("Failed to load checkpoint: {}", e)))?
            .ok_or_else(|| VenoreError::NotFound("No checkpoint found".into()))?;

        tracing::info!(
            "   ✓ Loaded checkpoint: {}/{} modules completed",
            checkpoint.completed_module_ids.len(),
            checkpoint.total_modules
        );

        Ok(Checkpoint {
            version: checkpoint.version,
            project_path: checkpoint.project_path.map(|p| p.to_string_lossy().to_string()),
            started_at: checkpoint.started_at.to_rfc3339(),
            last_updated_at: checkpoint.last_updated_at.to_rfc3339(),
            wizard_config: WizardConfig {
                project_name: checkpoint.wizard_config.project_name,
                project_description: checkpoint.wizard_config.project_description,
                project_state: checkpoint.wizard_config.project_state,
                team_size: checkpoint.wizard_config.team_size,
                goals: checkpoint.wizard_config.goals,
                depth_level: checkpoint.wizard_config.depth_level,
                layers_to_generate: checkpoint.wizard_config.layers_to_generate,
                exclusions: checkpoint.wizard_config.exclusions,
                project_type: format!("{:?}", checkpoint.wizard_config.project_type),
                project_type_confidence: checkpoint.wizard_config.project_type_confidence,
                project_metadata: checkpoint.wizard_config.project_metadata,
                total_files_scanned: checkpoint.wizard_config.total_files_scanned,
                total_modules_detected: checkpoint.wizard_config.total_modules_detected,
                module_names: checkpoint.wizard_config.module_names,
                selected_module_names: checkpoint.wizard_config.selected_module_names,
                llm_provider: checkpoint.wizard_config.llm_provider,
                llm_model: checkpoint.wizard_config.llm_model,
                analysis_depth: format!("{:?}", checkpoint.wizard_config.analysis_depth),
            },
            total_modules: checkpoint.total_modules,
            completed_module_ids: checkpoint.completed_module_ids,
        })
    })();

    result.into()
}

/// Delete checkpoint (for "Start New" or on completion)
#[tauri::command]
pub async fn delete_wizard_checkpoint(path: String) -> CommandResult<()> {
    tracing::info!("🗑️  Deleting checkpoint for: {}", path);

    let result: Result<(), VenoreError> = (|| {
        let manager = CheckpointManager::new(Path::new(&path));
        manager
            .delete()
            .map_err(|e| VenoreError::FileWriteError(format!("Failed to delete checkpoint: {}", e)))?;
        tracing::info!("   ✓ Checkpoint deleted");
        Ok(())
    })();

    result.into()
}

/// Restore wizard session from checkpoint (Phase 2)
///
/// Migrated from OnboardingWizardModal.tsx:233-300.
/// Backend now handles checkpoint restore logic.
#[tauri::command]
pub async fn restore_wizard_session(
    project_path: String,
) -> CommandResult<RestoreWizardSessionResponse> {
    tracing::info!("📦 Restoring wizard session for: {}", project_path);

    let result: Result<RestoreWizardSessionResponse, VenoreError> = (|| {
        let manager = WizardSessionManager::global();
        let mut sessions = manager.lock().unwrap();
        let session = sessions.get_or_create(PathBuf::from(&project_path));

        let restored_state = session
            .restore_from_checkpoint()?;

        let wizard_config_dto = WizardConfig {
            project_name: restored_state.wizard_config.project_name,
            project_description: restored_state.wizard_config.project_description,
            project_state: restored_state.wizard_config.project_state,
            team_size: restored_state.wizard_config.team_size,
            goals: restored_state.wizard_config.goals,
            depth_level: restored_state.wizard_config.depth_level,
            layers_to_generate: restored_state.wizard_config.layers_to_generate,
            exclusions: restored_state.wizard_config.exclusions,
            project_type: format!("{:?}", restored_state.wizard_config.project_type),
            project_type_confidence: restored_state.wizard_config.project_type_confidence,
            project_metadata: restored_state.wizard_config.project_metadata,
            total_files_scanned: restored_state.wizard_config.total_files_scanned,
            total_modules_detected: restored_state.wizard_config.total_modules_detected,
            module_names: restored_state.wizard_config.module_names,
            selected_module_names: restored_state.wizard_config.selected_module_names,
            llm_provider: restored_state.wizard_config.llm_provider,
            llm_model: restored_state.wizard_config.llm_model,
            analysis_depth: format!("{:?}", restored_state.wizard_config.analysis_depth),
        };

        tracing::info!(
            "   ✓ Restored: {}/{} modules completed",
            restored_state.completed_module_names.len(),
            restored_state.total_modules
        );

        Ok(RestoreWizardSessionResponse {
            wizard_config: wizard_config_dto,
            completed_module_names: restored_state.completed_module_names,
            total_modules: restored_state.total_modules,
        })
    })();

    result.into()
}


// =============================================================================
// Phase 3: Auto-Selection and Recommendations
// =============================================================================

/// Get recommended modules (Phase 3)
///
/// Returns high-confidence modules that should be auto-selected.
/// Based on heuristics: modules with entry points are high confidence.
#[tauri::command]
pub async fn get_recommended_modules(
    request: GetRecommendedModulesRequest,
) -> CommandResult<GetRecommendedModulesResponse> {
    tracing::info!("🔍 Getting recommended modules for: {}", request.project_path);

    let result: Result<GetRecommendedModulesResponse, VenoreError> = (|| {
        let manager = WizardSessionManager::global();
        let sessions = manager.lock().unwrap();
        let session = sessions
            .get(&request.project_path)
            .ok_or_else(|| VenoreError::NotFound("No wizard session found. Please run module detection first.".into()))?;

        let recommended = session
            .get_recommended_modules()?;

        tracing::info!("   ✓ Found {} recommended modules", recommended.len());

        Ok(GetRecommendedModulesResponse { recommended_modules: recommended })
    })();

    result.into()
}

// =============================================================================
// Phase 3: Unified Validation Command
// =============================================================================

/// Validate wizard step (Phase 3)
///
/// Eliminates validation.ts from frontend (143 lines).
/// All validation logic now lives in backend.
#[tauri::command]
pub async fn validate_wizard_step(
    request: ValidateWizardStepRequest,
) -> CommandResult<ValidateWizardStepResponse> {
    use venore_core::wizard::{
        validate_project_path, validate_project_context, validate_analysis_rules,
        validate_module_selection, validate_llm_config,
        ProjectContextInput, AnalysisRulesInput, LLMConfigInput,
    };

    let validation_result = match request {
        ValidateWizardStepRequest::Path { path } => {
            tracing::info!("🔍 Validating path: {}", path);
            validate_project_path(&path)
        }

        ValidateWizardStepRequest::ProjectContext {
            name,
            description,
            state,
            team_size,
            goals,
        } => {
            tracing::info!("🔍 Validating project context: {}", name);
            let input = ProjectContextInput { name, description, state, team_size, goals };
            validate_project_context(&input)
        }

        ValidateWizardStepRequest::AnalysisRules {
            depth_level,
            layers_to_generate,
            exclusions,
        } => {
            tracing::info!("🔍 Validating analysis rules: depth={}", depth_level);
            let input = AnalysisRulesInput { depth_level, layers_to_generate, exclusions };
            validate_analysis_rules(&input)
        }

        ValidateWizardStepRequest::ModuleSelection { selected_modules } => {
            tracing::info!("🔍 Validating module selection: {} modules", selected_modules.len());
            validate_module_selection(&selected_modules)
        }

        ValidateWizardStepRequest::LLMConfig { provider, model } => {
            tracing::info!("🔍 Validating LLM config: {}/{}", provider, model);
            let input = LLMConfigInput { provider, model };
            validate_llm_config(&input)
        }
    };

    let response = ValidateWizardStepResponse {
        is_valid: validation_result.is_valid(),
        errors: validation_result.error.into_iter().collect(),
    };

    if response.is_valid {
        tracing::info!("   ✅ Validation passed");
    } else {
        tracing::warn!("   ❌ Validation failed: {:?}", response.errors);
    }

    CommandResult::ok(response)
}

// =============================================================================
// WIZARD INDEX PROJECT (new code intelligence flow)
// =============================================================================

/// Wizard step: run analysis + RAG indexing with graph population.
/// Reads cached AnalysisOutput from WizardSessionManager (populated by detect_project_modules),
/// falls back to disk. Emits rag-index-progress events for the frontend.
#[tauri::command]
pub async fn wizard_index_project(
    app: AppHandle,
    lazy_state: tauri::State<'_, LazyAppState>,
    project_path: String,
    layers: Option<Vec<String>>,
    exclusions: Option<Vec<String>>,
) -> StateCommandResult<WizardIndexResponse> {
    use venore_core::analysis::AnalysisOutput;
    use venore_core::rag::{self, IndexConfig};
    use venore_core::wizard::cancellation::CancellationGuard;

    tracing::info!("wizard_index_project: {}", project_path);

    // Register a cancellation token for this run. If the wizard is closed
    // mid-flight, `cancel_wizard_session` flips this token and the indexer
    // will return VenoreError::Cancelled at its next checkpoint. The guard
    // unregisters automatically on Drop.
    let cancel_guard = CancellationGuard::register(&project_path);

    let path = PathBuf::from(&project_path);

    // RAG repository drives the indexing + graph populate inside the
    // snapshot pipeline. ContextRepository is no longer needed here — the
    // snapshot writes layers straight to `.venore/module-layers.json`.
    let repo = {
        let guard = lazy_state.get();
        match guard.as_ref() {
            Some(state) => Ok(Arc::clone(&state.rag_repository)),
            None => Err(VenoreError::NotFound("Backend not initialized".into())),
        }
    };

    let result: Result<WizardIndexResponse, VenoreError> = async {
        let repo = repo?;

        // 1. Get AnalysisOutput from WizardSessionManager cache (set by detect_project_modules)
        let analysis: Option<venore_core::analysis::AnalysisOutput> = {
            let session_manager = WizardSessionManager::global();
            let mgr = session_manager.lock().unwrap();
            mgr.get(&project_path)
                .and_then(|session| session.get_cached_analysis().ok().cloned())
        };

        // 2. Fall back to disk if not cached
        let analysis = match analysis {
            Some(a) => {
                tracing::info!("wizard_index_project: using cached analysis ({} modules)", a.modules.len());
                a
            }
            None => {
                tracing::info!("wizard_index_project: no cached analysis, loading from disk");
                AnalysisOutput::load_from_disk(&path)?
                    .ok_or_else(|| VenoreError::NotFound(
                        "No analysis output found. Run project analysis first.".into()
                    ))?
            }
        };

        let modules_detected = analysis.modules.len() as u32;

        // 3. Resolve project identity
        let identity = ProjectService::read_or_create_identity(&path)?;
        let project_id = identity.id.to_string();

        // 4. Index with graph, emitting progress events.
        //
        // Phase 2 (this command) has three sub-phases the user should see:
        //   1/3 — Indexing files          (per-file progress via callback)
        //   2/3 — Building dependency graph
        //   3/3 — Analyzing module layers (per-module progress in the loop below)
        //
        // We emit a single `wizard-index-progress` event for all three.
        // The frontend uses it to drive a 3-segment progress bar plus a
        // detail line (current file / current module) when applicable.
        let pid = project_id.clone();
        let session_id = project_path.clone();

        // Helper closure to emit a phase boundary or per-item update.
        let emit_progress = |app: &AppHandle, session: &str, current_phase: u32, total_phases: u32,
                             description: &str, current: Option<u32>, total: Option<u32>,
                             current_item: Option<&str>| {
            let _ = app.emit("wizard-index-progress", serde_json::json!({
                "session_id": session,
                "current_phase": current_phase,
                "total_phases": total_phases,
                "description": description,
                "current": current,
                "total": total,
                "current_item": current_item,
            }));
        };

        // Sub-phase 1/3 — Indexing files (boundary marker).
        emit_progress(&app, &session_id, 1, 3, "Indexing files...", None, None, None);

        // Per-file progress callback. Used by the indexer's inner loop.
        // The indexer emits a final event with status="completed" when the
        // file loop ends — we use that as the transition signal into the
        // graph-populate sub-phase (2/3).
        let session_for_cb = session_id.clone();
        let app_for_cb = app.clone();
        let progress_cb = move |event: rag::IndexProgressEvent| {
            // Legacy event — keep emitting for any consumer that depends on it.
            let _ = app_for_cb.emit("rag-index-progress", serde_json::json!({
                "project_id": pid.clone(),
                "current": event.current,
                "total": event.total,
                "current_file": event.current_file.clone(),
                "status": event.status.clone(),
            }));

            if event.status == "completed" {
                // Indexer finished the file loop — transition to graph populate.
                let _ = app_for_cb.emit("wizard-index-progress", serde_json::json!({
                    "session_id": session_for_cb,
                    "current_phase": 2u32,
                    "total_phases": 3u32,
                    "description": "Building dependency graph...",
                    "current": null,
                    "total": null,
                    "current_item": null,
                }));
            } else {
                let _ = app_for_cb.emit("wizard-index-progress", serde_json::json!({
                    "session_id": session_for_cb,
                    "current_phase": 1u32,
                    "total_phases": 3u32,
                    "description": "Indexing files...",
                    "current": event.current,
                    "total": event.total,
                    "current_item": event.current_file,
                }));
            }
        };

        // Merge user-supplied exclusions with the defaults. We never drop the
        // safety-critical defaults (`.git`, `.venore`, `node_modules`, ...) —
        // the user's list is appended so they only ADD ignores, can't remove
        // essentials by accident.
        let mut config = IndexConfig::default();
        if let Some(user_exclusions) = exclusions {
            for pattern in user_exclusions {
                let trimmed = pattern.trim().trim_end_matches('/').to_string();
                if !trimmed.is_empty() && !config.ignore_patterns.contains(&trimmed) {
                    config.ignore_patterns.push(trimmed);
                }
            }
        }
        tracing::info!(
            "wizard_index_project: ignore_patterns ({}) = {:?}",
            config.ignore_patterns.len(),
            config.ignore_patterns,
        );

        // Sub-phase 3/3 boundary — emitted upfront so the UI flips into
        // the layers phase as soon as RAG finishes. The snapshot pipeline
        // doesn't know about Tauri events; we fire this here.
        let total_modules_u32 = analysis.modules.len() as u32;
        let app_for_layers = app.clone();
        let session_for_layers = session_id.clone();
        let on_layer = move |current: u32, _total: u32, module_name: &str| {
            let _ = app_for_layers.emit("wizard-index-progress", serde_json::json!({
                "session_id": session_for_layers,
                "current_phase": 3u32,
                "total_phases": 3u32,
                "description": "Analyzing module layers...",
                "current": current,
                "total": total_modules_u32,
                "current_item": module_name,
            }));
        };

        let layer_types: Vec<String> = layers.unwrap_or_default();
        tracing::info!("wizard_index_project: snapshot starting (layer_types={:?})", layer_types);

        let report = venore_core::snapshot::run_snapshot(
            venore_core::snapshot::SnapshotInputs {
                project_path: &path,
                project_id: &project_id,
                analysis: &analysis,
                rag_config: &config,
                layer_types,
                cancel: Some(cancel_guard.token()),
            },
            &repo,
            Some(&progress_cb),
            Some(&on_layer),
        )
        .await?;

        tracing::info!(
            indexed = report.indexed,
            skipped = report.skipped,
            modules_mapped = report.modules_mapped,
            deps = report.deps_created,
            refs = report.refs_created,
            layers = report.layers_written,
            hashes = report.hashes_written,
            "wizard_index_project: snapshot complete"
        );

        // Reuse the same event `resnapshot_project` fires so the workspace
        // (canvas layers, ProjectPanel dashboard, stale badges) auto-refreshes
        // when the wizard runs from anywhere — launcher or Tools menu. Without
        // this, a re-onboard from the Tools menu would leave the canvas
        // showing pre-wizard state until the user re-opens the project.
        let _ = app.emit("context-update-complete", ());

        Ok(WizardIndexResponse {
            indexed: report.indexed,
            skipped: report.skipped,
            removed: report.removed,
            modules_detected,
            modules_mapped: report.modules_mapped,
            deps_created: report.deps_created,
            refs_created: report.refs_created,
        })
    }.await;

    result.into_state()
}
