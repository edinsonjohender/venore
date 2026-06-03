//! # UI State Helpers
//!
//! Pure functions for computing UI state on the backend.
//! Backend calculates what the frontend should display.
//!
//! ## Philosophy:
//! - Backend decides what buttons to enable/disable
//! - Backend computes progress percentages
//! - Backend groups/filters data for display
//! - Frontend is a "dumb renderer"

use crate::analysis::ModuleAnalysis;

// =============================================================================
// Progress UI State
// =============================================================================

/// UI state for progress display
#[derive(Debug, Clone)]
pub struct ProgressUIState {
    /// Current completed count
    pub completed: usize,
    /// Total modules to process
    pub total: usize,
    /// Progress percentage (0-100)
    pub progress_percent: u8,
    /// Status message for display
    pub status_message: String,
}

/// Computes progress UI state
///
/// Pure function that calculates progress display state.
///
/// # Arguments
/// * `completed` - Number of completed modules
/// * `total` - Total number of modules
///
/// # Examples
/// ```
/// use venore_core::wizard::ui_state::compute_progress_ui_state;
///
/// let state = compute_progress_ui_state(5, 10);
/// assert_eq!(state.progress_percent, 50);
/// assert_eq!(state.status_message, "5 of 10 modules completed");
/// ```
pub fn compute_progress_ui_state(completed: usize, total: usize) -> ProgressUIState {
    let progress_percent = if total > 0 {
        ((completed * 100) / total).min(100) as u8
    } else {
        0
    };

    let status_message = format!("{} of {} modules completed", completed, total);

    ProgressUIState {
        completed,
        total,
        progress_percent,
        status_message,
    }
}

// =============================================================================
// Module Grouping by Confidence
// =============================================================================

/// Grouped modules by confidence level
#[derive(Debug, Clone)]
pub struct GroupedModules {
    pub high_confidence: Vec<ModuleGroup>,
    pub medium_confidence: Vec<ModuleGroup>,
    pub low_confidence: Vec<ModuleGroup>,
}

/// Module group with metadata
#[derive(Debug, Clone)]
pub struct ModuleGroup {
    pub name: String,
    pub path: String,
    pub file_count: usize,
    pub has_entry_point: bool,
    pub confidence: String,
}

/// Groups modules by confidence level
///
/// Pure function that groups modules based on heuristics:
/// - **High**: Has entry point
/// - **Medium**: 5+ files but no entry point
/// - **Low**: < 5 files and no entry point
///
/// # Arguments
/// * `modules` - List of analyzed modules
///
/// # Examples
/// ```rust,ignore
/// use venore_core::wizard::ui_state::group_modules_by_confidence;
///
/// let grouped = group_modules_by_confidence(&modules);
/// println!("High confidence: {} modules", grouped.high_confidence.len());
/// ```
pub fn group_modules_by_confidence(modules: &[ModuleAnalysis]) -> GroupedModules {
    let mut high = Vec::new();
    let mut medium = Vec::new();
    let mut low = Vec::new();

    for module in modules {
        let has_entry_point = module.entry_point.is_some();
        let file_count = module.file_count;

        // Confidence heuristic
        let confidence = if has_entry_point {
            "high"
        } else if file_count > 5 {
            "medium"
        } else {
            "low"
        };

        let group = ModuleGroup {
            name: module.name.clone(),
            path: module.path.clone(),
            file_count,
            has_entry_point,
            confidence: confidence.to_string(),
        };

        match confidence {
            "high" => high.push(group),
            "medium" => medium.push(group),
            _ => low.push(group),
        }
    }

    GroupedModules {
        high_confidence: high,
        medium_confidence: medium,
        low_confidence: low,
    }
}

// =============================================================================
// Module Status Helpers
// =============================================================================

/// Gets the status of a module (completed, in-progress, pending)
///
/// Pure function that determines module status based on completed IDs
/// and current processing.
///
/// # Arguments
/// * `module_id` - The module identifier
/// * `completed_ids` - List of completed module IDs
/// * `current_module_id` - Optional current module being processed
///
/// # Returns
/// Status string: "completed", "in-progress", or "pending"
///
/// # Examples
/// ```
/// use venore_core::wizard::ui_state::get_module_status;
///
/// let completed = vec!["module1".to_string(), "module2".to_string()];
/// let status = get_module_status("module1", &completed, None);
/// assert_eq!(status, "completed");
/// ```
pub fn get_module_status(
    module_id: &str,
    completed_ids: &[String],
    current_module_id: Option<&str>,
) -> String {
    let module_lower = module_id.to_lowercase();
    if completed_ids.iter().any(|id| id.to_lowercase() == module_lower) {
        "completed".to_string()
    } else if current_module_id == Some(module_id) {
        "in-progress".to_string()
    } else {
        "pending".to_string()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_progress_ui_state() {
        // 50% complete
        let state = compute_progress_ui_state(5, 10);
        assert_eq!(state.completed, 5);
        assert_eq!(state.total, 10);
        assert_eq!(state.progress_percent, 50);
        assert_eq!(state.status_message, "5 of 10 modules completed");

        // 100% complete
        let state = compute_progress_ui_state(10, 10);
        assert_eq!(state.progress_percent, 100);

        // 0% complete
        let state = compute_progress_ui_state(0, 10);
        assert_eq!(state.progress_percent, 0);

        // Edge case: total = 0
        let state = compute_progress_ui_state(0, 0);
        assert_eq!(state.progress_percent, 0);
    }

    #[test]
    fn test_group_modules_by_confidence() {
        use crate::analysis::{ModuleAnalysis, ModuleArchitecture, ModuleSymbols};

        let modules = vec![
            // High confidence (has entry point)
            ModuleAnalysis {
                name: "high1".to_string(),
                path: "/test/high1".to_string(),
                file_count: 10,
                entry_point: Some("/test/high1/index.ts".to_string()),
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
                name: "medium1".to_string(),
                path: "/test/medium1".to_string(),
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
                name: "low1".to_string(),
                path: "/test/low1".to_string(),
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
        ];

        let grouped = group_modules_by_confidence(&modules);

        assert_eq!(grouped.high_confidence.len(), 1);
        assert_eq!(grouped.high_confidence[0].name, "high1");
        assert_eq!(grouped.high_confidence[0].confidence, "high");

        assert_eq!(grouped.medium_confidence.len(), 1);
        assert_eq!(grouped.medium_confidence[0].name, "medium1");
        assert_eq!(grouped.medium_confidence[0].confidence, "medium");

        assert_eq!(grouped.low_confidence.len(), 1);
        assert_eq!(grouped.low_confidence[0].name, "low1");
        assert_eq!(grouped.low_confidence[0].confidence, "low");
    }

    #[test]
    fn test_get_module_status() {
        let completed = vec!["module1".to_string(), "module2".to_string()];

        // Completed module
        assert_eq!(get_module_status("module1", &completed, None), "completed");

        // In-progress module
        assert_eq!(
            get_module_status("module3", &completed, Some("module3")),
            "in-progress"
        );

        // Pending module
        assert_eq!(get_module_status("module4", &completed, None), "pending");

        // Case-insensitive: completed with different case
        assert_eq!(get_module_status("Module1", &completed, None), "completed");
        assert_eq!(get_module_status("MODULE2", &completed, None), "completed");

        let completed_mixed = vec!["MyModule".to_string()];
        assert_eq!(get_module_status("mymodule", &completed_mixed, None), "completed");
    }
}
