//! Shared test helpers for the ocean module.

use super::types::ModuleInfo;

/// Create a ModuleInfo for testing.
pub fn make_module(id: &str, deps: Vec<&str>, dependents: Vec<&str>) -> ModuleInfo {
    ModuleInfo {
        id: id.to_string(),
        name: id.to_string(),
        path: format!("src/{}", id),
        dependencies: deps.into_iter().map(String::from).collect(),
        dependents: dependents.into_iter().map(String::from).collect(),
    }
}
