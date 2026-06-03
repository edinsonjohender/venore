//! Helpers shared by Node.js analyzers (monorepo + single-package).
//!
//! These used to live duplicated across `node_monorepo.rs` and
//! `node_single.rs`. The Svelte detection drifted (monorepo had it,
//! single didn't), which is exactly what this extraction prevents.

use std::path::Path;

use super::super::super::traits::DetectedFramework;

/// Does `package.json` declare a workspaces configuration?
///
/// Checks both the npm/yarn top-level `workspaces` field and the
/// `pnpm.workspaces` nested form.
pub fn has_workspaces_config(package_json_path: &Path) -> bool {
    let content = match std::fs::read_to_string(package_json_path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(j) => j,
        Err(_) => return false,
    };

    json.get("workspaces").is_some()
        || json
            .get("pnpm")
            .and_then(|p| p.get("workspaces"))
            .is_some()
}

/// Detect the Node package manager from lockfile presence.
///
/// Order: pnpm → yarn → npm. Returns `None` if no lockfile is present.
pub fn detect_package_manager(project_root: &Path) -> Option<String> {
    if project_root.join("pnpm-lock.yaml").exists() {
        Some("pnpm".to_string())
    } else if project_root.join("yarn.lock").exists() {
        Some("yarn".to_string())
    } else if project_root.join("package-lock.json").exists() {
        Some("npm".to_string())
    } else {
        None
    }
}

/// Detect common frameworks declared in `package.json` dependencies.
///
/// Inspects the `dependencies` key today (not `devDependencies`) and
/// supplements with filesystem-level markers — currently
/// `next.config.{js,ts,mjs}` for Next.js. The fallback catches
/// projects whose `next` dep is hoisted to a monorepo root and not
/// visible in the current package.json.
///
/// Returns an empty vec on parse error or missing file — frameworks
/// are metadata, not a detection signal, so a soft failure is fine.
pub fn detect_frameworks(package_json_path: &Path) -> Vec<DetectedFramework> {
    let content = match std::fs::read_to_string(package_json_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(j) => j,
        Err(_) => return Vec::new(),
    };

    let mut frameworks = Vec::new();

    if let Some(deps) = json.get("dependencies").and_then(|d| d.as_object()) {
        for dep_name in deps.keys() {
            if let Some(fw) = DetectedFramework::from_dep_name(dep_name) {
                frameworks.push(fw);
            }
        }
    }

    if let Some(parent) = package_json_path.parent() {
        if has_next_config(parent) && !frameworks.contains(&DetectedFramework::NextJs) {
            frameworks.push(DetectedFramework::NextJs);
        }
    }

    frameworks
}

/// Does this directory contain a Next.js config file?
///
/// Next.js accepts `next.config.js`, `next.config.ts`, and
/// `next.config.mjs` as valid entry-point names in current templates.
pub fn has_next_config(project_root: &Path) -> bool {
    project_root.join("next.config.js").exists()
        || project_root.join("next.config.ts").exists()
        || project_root.join("next.config.mjs").exists()
}

/// Identify which Next.js router style is in use.
///
/// Next 13+ defaults to the App Router (`app/`); pre-13 used the
/// Pages Router (`pages/`). Both can coexist during migrations.
/// Either layout can live at the repo root or under `src/`.
///
/// Returns `"app"`, `"pages"`, or `"app+pages"`; `None` when neither
/// directory is present.
pub fn detect_next_router(project_root: &Path) -> Option<String> {
    let app_dir = project_root.join("app").is_dir()
        || project_root.join("src").join("app").is_dir();
    let pages_dir = project_root.join("pages").is_dir()
        || project_root.join("src").join("pages").is_dir();

    match (app_dir, pages_dir) {
        (true, true) => Some("app+pages".to_string()),
        (true, false) => Some("app".to_string()),
        (false, true) => Some("pages".to_string()),
        (false, false) => None,
    }
}
