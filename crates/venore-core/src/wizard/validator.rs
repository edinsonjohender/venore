//! # Wizard Validators
//!
//! Centralized validation logic for all wizard steps.
//!
//! ## Design Principle:
//! ALL validations happen in the backend (venore-core).
//! Clients (Desktop, API) MUST call these validators before processing.
//!
//! ## Steps:
//! - Step 0: Path selection
//! - Step 1: Project context
//! - Step 2: Analysis rules
//! - Step 3: Module detection (auto)
//! - Step 4: Module selection
//! - Step 5: LLM configuration
//! - Step 6+: Generation (auto)

use crate::utils::{
    validate_all, validate_project_description, validate_project_name, validate_required,
    ValidationResult,
};
use std::path::Path;

// =============================================================================
// Step 0: Path Validation
// =============================================================================

/// Validates project path
///
/// Rules:
/// - Must not be empty
/// - Must be a valid path format
/// - Must be an absolute path
///
/// # Examples
/// ```
/// use venore_core::wizard::validate_project_path;
///
/// // Empty and relative paths are rejected on every platform.
/// assert!(!validate_project_path("").is_valid());
/// assert!(!validate_project_path("relative/path").is_valid());
///
/// // What counts as "absolute" is platform-specific.
/// #[cfg(unix)]
/// assert!(validate_project_path("/home/user/project").is_valid());
/// #[cfg(windows)]
/// assert!(validate_project_path("C:\\Users\\Project").is_valid());
/// ```
pub fn validate_project_path(path: &str) -> ValidationResult {
    if path.trim().is_empty() {
        return ValidationResult::err("Project path is required");
    }

    let path_obj = Path::new(path);

    // Must be absolute path
    if !path_obj.is_absolute() {
        return ValidationResult::err("Project path must be an absolute path");
    }

    ValidationResult::ok()
}

// =============================================================================
// Step 1: Project Context Validation
// =============================================================================

/// Validates project context (Step 1)
///
/// Rules:
/// - Name: required, 2-100 characters
/// - Description: required, 20-5000 characters
/// - State: required (development, staging, production)
/// - Team size: required
/// - Goals: at least 1 goal
///
/// # Examples
/// ```
/// use venore_core::wizard::{validate_project_context, ProjectContextInput};
///
/// let context = ProjectContextInput {
///     name: "My Project".to_string(),
///     description: "A comprehensive description of the project with enough detail".to_string(),
///     state: "development".to_string(),
///     team_size: "small".to_string(),
///     goals: vec!["Goal 1".to_string()],
/// };
///
/// assert!(validate_project_context(&context).is_valid());
/// ```
pub fn validate_project_context(context: &ProjectContextInput) -> ValidationResult {
    let mut results = vec![];

    // Validate name
    results.push(validate_required(&context.name, "Project name"));
    results.push(validate_project_name(&context.name));

    // Validate description
    results.push(validate_required(&context.description, "Description"));
    results.push(validate_project_description(
        &context.description,
        Some(20),
    ));

    // Max length check
    if context.description.len() > 5000 {
        results.push(ValidationResult::err(
            "Description must be less than 5000 characters",
        ));
    }

    // Validate state
    results.push(validate_required(&context.state, "Project state"));
    let valid_states = ["development", "staging", "production", "maintenance"];
    if !valid_states.contains(&context.state.as_str()) {
        results.push(ValidationResult::err(format!(
            "Project state must be one of: {}",
            valid_states.join(", ")
        )));
    }

    // Validate team size
    results.push(validate_required(&context.team_size, "Team size"));
    let valid_sizes = ["solo", "small", "medium", "large"];
    if !valid_sizes.contains(&context.team_size.as_str()) {
        results.push(ValidationResult::err(format!(
            "Team size must be one of: {}",
            valid_sizes.join(", ")
        )));
    }

    // Validate goals
    if context.goals.is_empty() {
        results.push(ValidationResult::err("At least one goal is required"));
    }

    validate_all(results)
}

/// Project context input (Step 1)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectContextInput {
    pub name: String,
    pub description: String,
    pub state: String,
    pub team_size: String,
    pub goals: Vec<String>,
}

// =============================================================================
// Step 2: Analysis Rules Validation
// =============================================================================

/// Validates analysis rules (Step 2)
///
/// Rules:
/// - Depth level: required
/// - Layers: at least 1, must include 'context'
/// - Exclusions: optional
///
/// # Examples
/// ```
/// use venore_core::wizard::{validate_analysis_rules, AnalysisRulesInput};
///
/// let rules = AnalysisRulesInput {
///     depth_level: "detailed".to_string(),
///     layers_to_generate: vec!["context".to_string(), "summary".to_string()],
///     exclusions: vec![],
/// };
///
/// assert!(validate_analysis_rules(&rules).is_valid());
/// ```
pub fn validate_analysis_rules(rules: &AnalysisRulesInput) -> ValidationResult {
    let mut results = vec![];

    // Validate depth level
    results.push(validate_required(&rules.depth_level, "Depth level"));
    let valid_depths = ["minimal", "normal", "detailed", "expert"];
    if !valid_depths.contains(&rules.depth_level.as_str()) {
        results.push(ValidationResult::err(format!(
            "Depth level must be one of: {}",
            valid_depths.join(", ")
        )));
    }

    // Validate layers
    if rules.layers_to_generate.is_empty() {
        results.push(ValidationResult::err(
            "At least one layer must be selected",
        ));
    }

    // Context layer is mandatory
    if !rules.layers_to_generate.contains(&"context".to_string()) {
        results.push(ValidationResult::err(
            "Context layer is required and cannot be deselected",
        ));
    }

    validate_all(results)
}

/// Analysis rules input (Step 2)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnalysisRulesInput {
    pub depth_level: String,
    pub layers_to_generate: Vec<String>,
    pub exclusions: Vec<String>,
}

// =============================================================================
// Step 4: Module Selection Validation
// =============================================================================

/// Validates module selection (Step 4)
///
/// Rules:
/// - At least 1 module must be selected
///
/// # Examples
/// ```
/// use venore_core::wizard::validate_module_selection;
///
/// assert!(validate_module_selection(&vec!["module1".to_string()]).is_valid());
/// assert!(!validate_module_selection(&vec![]).is_valid());
/// ```
pub fn validate_module_selection(selected_modules: &[String]) -> ValidationResult {
    if selected_modules.is_empty() {
        return ValidationResult::err("At least one module must be selected");
    }

    ValidationResult::ok()
}

// =============================================================================
// Step 5: LLM Configuration Validation
// =============================================================================

/// Validates LLM configuration (Step 5)
///
/// Rules:
/// - Provider: required
/// - Model: required
///
/// # Examples
/// ```
/// use venore_core::wizard::{validate_llm_config, LLMConfigInput};
///
/// let config = LLMConfigInput {
///     provider: "anthropic".to_string(),
///     model: "claude-sonnet-4-5".to_string(),
/// };
///
/// assert!(validate_llm_config(&config).is_valid());
/// ```
pub fn validate_llm_config(config: &LLMConfigInput) -> ValidationResult {
    let mut results = vec![];

    // Validate provider
    results.push(validate_required(&config.provider, "LLM provider"));
    let valid_providers = ["anthropic", "openai", "gemini", "ollama"];
    if !valid_providers.contains(&config.provider.as_str()) {
        results.push(ValidationResult::err(format!(
            "LLM provider must be one of: {}",
            valid_providers.join(", ")
        )));
    }

    // Validate model
    results.push(validate_required(&config.model, "Model"));

    validate_all(results)
}

/// LLM configuration input (Step 5)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LLMConfigInput {
    pub provider: String,
    pub model: String,
}

// =============================================================================
// Batch Generation Validation (Step 6)
// =============================================================================

/// Validates batch generation request
///
/// Rules:
/// - Project path: required, absolute
/// - Module IDs: at least 1
/// - Provider: required, valid
/// - Model: required
///
/// # Examples
/// ```
/// use venore_core::wizard::{validate_batch_generation_request, BatchGenerationRequestInput};
///
/// // An absolute path; the literal form is platform-specific.
/// #[cfg(unix)]
/// let project_path = "/home/user/project".to_string();
/// #[cfg(windows)]
/// let project_path = "C:\\Users\\Project".to_string();
///
/// let request = BatchGenerationRequestInput {
///     project_path,
///     module_ids: vec!["module1".to_string()],
///     provider: "anthropic".to_string(),
///     model: "claude-sonnet-4-5".to_string(),
/// };
///
/// assert!(validate_batch_generation_request(&request).is_valid());
/// ```
pub fn validate_batch_generation_request(
    request: &BatchGenerationRequestInput,
) -> ValidationResult {
    let mut results = vec![];

    // Validate project path
    results.push(validate_project_path(&request.project_path));

    // Validate module IDs
    if request.module_ids.is_empty() {
        results.push(ValidationResult::err(
            "At least one module must be provided for generation",
        ));
    }

    // Validate LLM config
    let llm_config = LLMConfigInput {
        provider: request.provider.clone(),
        model: request.model.clone(),
    };
    results.push(validate_llm_config(&llm_config));

    validate_all(results)
}

/// Batch generation request input (Step 6)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BatchGenerationRequestInput {
    pub project_path: String,
    pub module_ids: Vec<String>,
    pub provider: String,
    pub model: String,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_project_path() {
        // Valid absolute paths (OS-specific)
        #[cfg(unix)]
        assert!(validate_project_path("/home/user/project").is_valid());

        #[cfg(windows)]
        assert!(validate_project_path("C:\\Users\\Project").is_valid());

        // Invalid
        assert!(!validate_project_path("").is_valid());
        assert!(!validate_project_path("relative/path").is_valid());
    }

    #[test]
    fn test_validate_project_context() {
        let valid_context = ProjectContextInput {
            name: "My Project".to_string(),
            description: "A comprehensive description of the project with enough detail to understand its purpose".to_string(),
            state: "development".to_string(),
            team_size: "small".to_string(),
            goals: vec!["Goal 1".to_string()],
        };

        assert!(validate_project_context(&valid_context).is_valid());

        // Invalid: description too short
        let invalid = ProjectContextInput {
            description: "Too short".to_string(),
            ..valid_context.clone()
        };
        assert!(!validate_project_context(&invalid).is_valid());

        // Invalid: no goals
        let invalid = ProjectContextInput {
            goals: vec![],
            ..valid_context.clone()
        };
        assert!(!validate_project_context(&invalid).is_valid());
    }

    #[test]
    fn test_validate_analysis_rules() {
        let valid_rules = AnalysisRulesInput {
            depth_level: "detailed".to_string(),
            layers_to_generate: vec!["context".to_string(), "summary".to_string()],
            exclusions: vec![],
        };

        assert!(validate_analysis_rules(&valid_rules).is_valid());

        // Invalid: no context layer
        let invalid = AnalysisRulesInput {
            layers_to_generate: vec!["summary".to_string()],
            ..valid_rules.clone()
        };
        assert!(!validate_analysis_rules(&invalid).is_valid());

        // Invalid: no layers
        let invalid = AnalysisRulesInput {
            layers_to_generate: vec![],
            ..valid_rules.clone()
        };
        assert!(!validate_analysis_rules(&invalid).is_valid());

        // Valid: all depth levels that frontend sends
        for depth in &["minimal", "normal", "detailed", "expert"] {
            let rules = AnalysisRulesInput {
                depth_level: depth.to_string(),
                ..valid_rules.clone()
            };
            assert!(validate_analysis_rules(&rules).is_valid(), "depth '{}' should be valid", depth);
        }

        // Invalid: old depth names no longer accepted
        for depth in &["standard", "comprehensive"] {
            let rules = AnalysisRulesInput {
                depth_level: depth.to_string(),
                ..valid_rules.clone()
            };
            assert!(!validate_analysis_rules(&rules).is_valid(), "depth '{}' should be invalid", depth);
        }
    }

    #[test]
    fn test_validate_module_selection() {
        assert!(validate_module_selection(&["module1".to_string()]).is_valid());
        assert!(!validate_module_selection(&[]).is_valid());
    }

    #[test]
    fn test_validate_llm_config() {
        let valid_config = LLMConfigInput {
            provider: "anthropic".to_string(),
            model: "claude-sonnet-4-5".to_string(),
        };

        assert!(validate_llm_config(&valid_config).is_valid());

        // Invalid provider
        let invalid = LLMConfigInput {
            provider: "invalid".to_string(),
            ..valid_config.clone()
        };
        assert!(!validate_llm_config(&invalid).is_valid());
    }

    #[test]
    fn test_validate_batch_generation_request() {
        // Use OS-appropriate paths
        #[cfg(unix)]
        let project_path = "/home/user/project".to_string();

        #[cfg(windows)]
        let project_path = "C:\\Users\\Project".to_string();

        let valid_request = BatchGenerationRequestInput {
            project_path,
            module_ids: vec!["module1".to_string()],
            provider: "anthropic".to_string(),
            model: "claude-sonnet-4-5".to_string(),
        };

        assert!(validate_batch_generation_request(&valid_request).is_valid());

        // Invalid: no modules
        let invalid = BatchGenerationRequestInput {
            module_ids: vec![],
            ..valid_request.clone()
        };
        assert!(!validate_batch_generation_request(&invalid).is_valid());
    }
}
