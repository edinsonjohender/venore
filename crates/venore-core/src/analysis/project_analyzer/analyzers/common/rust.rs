//! Helpers shared by Rust analyzers (single-crate + workspace).
//!
//! Both analyzers parse `Cargo.toml` and need to surface frameworks
//! (Tauri today, future: Actix, Rocket, Bevy, Leptos). Keeping the
//! detection in one place ensures the lists don't drift.

use std::path::Path;

use super::super::super::traits::DetectedFramework;

/// Detect Rust frameworks declared in `[dependencies]` and
/// `[workspace.dependencies]` sections of a parsed `Cargo.toml`.
///
/// We inspect both `[dependencies]` (single crate / leaf crate) and
/// `[workspace.dependencies]` (workspace root); Tauri apps typically
/// declare the dep in the leaf crate that builds the desktop binary.
pub fn detect_frameworks(toml: &toml::Value) -> Vec<DetectedFramework> {
    let mut frameworks = Vec::new();

    let mut scan_table = |deps: &toml::map::Map<String, toml::Value>| {
        for dep_name in deps.keys() {
            if let Some(fw) = DetectedFramework::from_dep_name(dep_name) {
                if !frameworks.contains(&fw) {
                    frameworks.push(fw);
                }
            }
        }
    };

    if let Some(deps) = toml.get("dependencies").and_then(|d| d.as_table()) {
        scan_table(deps);
    }
    if let Some(deps) = toml
        .get("workspace")
        .and_then(|w| w.get("dependencies"))
        .and_then(|d| d.as_table())
    {
        scan_table(deps);
    }

    frameworks
}

/// Filesystem-level Tauri check.
///
/// Tauri's canonical layout is a `src-tauri/` directory containing
/// `tauri.conf.json`. We check this as a fallback for projects where
/// the root `Cargo.toml` doesn't declare `tauri` because the desktop
/// crate lives one level down (the standard `create-tauri-app`
/// template).
pub fn has_tauri_layout(project_root: &Path) -> bool {
    project_root.join("src-tauri").join("tauri.conf.json").exists()
        || project_root.join("tauri.conf.json").exists()
}
