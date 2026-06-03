//! Change mapper — maps changed file paths to affected modules.
//!
//! Uses `ModuleAnalysis.path` as a prefix to determine which module
//! a changed file belongs to. Files that don't match any module are ignored.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::analysis::AnalysisOutput;

/// A module that has been affected by file changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffectedModule {
    /// Module name (from ModuleAnalysis)
    pub name: String,
    /// Module path relative to project root (e.g. `src/auth`)
    pub path: String,
    /// Changed files within this module
    pub changed_files: Vec<String>,
}

/// Map changed file paths to the modules they belong to.
///
/// A file matches a module if its path starts with `module.path/`.
/// Files that don't belong to any module are silently ignored.
pub fn map_files_to_modules(
    changed_files: &[String],
    analysis: &AnalysisOutput,
) -> Vec<AffectedModule> {
    // Build a map: module_name → (module_path, changed_files)
    let mut module_map: HashMap<String, (String, Vec<String>)> = HashMap::new();

    for file_path in changed_files {
        // Normalize separators (git always uses /)
        let normalized = file_path.replace('\\', "/");

        for module in &analysis.modules {
            let module_prefix = module.path.replace('\\', "/");

            // Check if file is inside this module's directory
            let is_match = normalized.starts_with(&module_prefix)
                && normalized[module_prefix.len()..].starts_with('/');

            // Also match if file IS the module path (single-file module)
            let is_exact = normalized == module_prefix;

            if is_match || is_exact {
                module_map
                    .entry(module.name.clone())
                    .or_insert_with(|| (module.path.clone(), Vec::new()))
                    .1
                    .push(file_path.clone());
                break; // A file belongs to at most one module
            }
        }
    }

    let mut result: Vec<AffectedModule> = module_map
        .into_iter()
        .map(|(name, (path, changed_files))| AffectedModule {
            name,
            path,
            changed_files,
        })
        .collect();

    // Sort by module name for deterministic output
    result.sort_by(|a, b| a.name.cmp(&b.name));
    result
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{
        AnalysisOutput, ModuleAnalysis, ModuleArchitecture, ModuleSymbols, RepositoryInfo,
    };

    fn make_analysis() -> AnalysisOutput {
        AnalysisOutput {
            repository: RepositoryInfo {
                name: "test-repo".to_string(),
                language: None,
                technologies: vec![],
                total_files: 10,
                total_modules: 2,
            },
            modules: vec![
                ModuleAnalysis {
                    name: "auth".to_string(),
                    path: "src/auth".to_string(),
                    file_count: 3,
                    entry_point: Some("index.ts".to_string()),
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
                    files: vec!["login.ts".to_string(), "types.ts".to_string()],
                },
                ModuleAnalysis {
                    name: "api".to_string(),
                    path: "src/api".to_string(),
                    file_count: 2,
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
                    files: vec!["routes.ts".to_string()],
                },
            ],
            orphan_files: vec![],
        }
    }

    #[test]
    fn test_maps_files_to_correct_modules() {
        let analysis = make_analysis();
        let changed = vec![
            "src/auth/login.ts".to_string(),
            "src/api/routes.ts".to_string(),
        ];

        let affected = map_files_to_modules(&changed, &analysis);
        assert_eq!(affected.len(), 2);

        let api = affected.iter().find(|m| m.name == "api").unwrap();
        assert_eq!(api.changed_files, vec!["src/api/routes.ts"]);

        let auth = affected.iter().find(|m| m.name == "auth").unwrap();
        assert_eq!(auth.changed_files, vec!["src/auth/login.ts"]);
    }

    #[test]
    fn test_ignores_orphan_files() {
        let analysis = make_analysis();
        let changed = vec![
            "README.md".to_string(),
            "package.json".to_string(),
        ];

        let affected = map_files_to_modules(&changed, &analysis);
        assert!(affected.is_empty());
    }

    #[test]
    fn test_groups_multiple_files_per_module() {
        let analysis = make_analysis();
        let changed = vec![
            "src/auth/login.ts".to_string(),
            "src/auth/types.ts".to_string(),
            "src/auth/middleware.ts".to_string(),
        ];

        let affected = map_files_to_modules(&changed, &analysis);
        assert_eq!(affected.len(), 1);
        assert_eq!(affected[0].name, "auth");
        assert_eq!(affected[0].changed_files.len(), 3);
    }

    #[test]
    fn test_mixed_files_and_orphans() {
        let analysis = make_analysis();
        let changed = vec![
            "src/auth/login.ts".to_string(),
            "README.md".to_string(),
            "src/api/routes.ts".to_string(),
            ".gitignore".to_string(),
        ];

        let affected = map_files_to_modules(&changed, &analysis);
        assert_eq!(affected.len(), 2);
    }

    #[test]
    fn test_does_not_match_partial_prefix() {
        let analysis = make_analysis();
        // "src/authorization" should NOT match "src/auth"
        let changed = vec!["src/authorization/policy.ts".to_string()];

        let affected = map_files_to_modules(&changed, &analysis);
        assert!(affected.is_empty());
    }
}
