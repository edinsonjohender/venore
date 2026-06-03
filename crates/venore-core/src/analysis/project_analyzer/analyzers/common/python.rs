//! Helpers shared by Python analyzers.
//!
//! Today only the Poetry analyzer uses these. When a generic Python
//! analyzer (requirements.txt / pip-based projects) is added these
//! helpers will be reused as-is.

use std::path::Path;

/// Filesystem-level Django check.
///
/// Django apps always ship a `manage.py` at the project root — it's
/// the canonical CLI entry point and the file `django-admin
/// startproject` generates first. Using it as a fallback catches
/// Django projects that declare dependencies via `requirements.txt`
/// (not parsed today) instead of `pyproject.toml`.
pub fn is_django_project(project_root: &Path) -> bool {
    project_root.join("manage.py").exists()
}

/// List Django apps within a project.
///
/// A Django app is a subdirectory containing an `apps.py` file (where
/// `AppConfig` is declared). Walks only top-level children of the
/// project root — Django convention puts apps at the root or under a
/// single project-named folder, both of which we cover by also
/// looking one level deeper.
///
/// Returns app folder names (relative paths). Empty vec on missing
/// dir or read error.
pub fn detect_django_apps(project_root: &Path) -> Vec<String> {
    let mut apps = Vec::new();

    // Root-level apps: `<root>/<app>/apps.py`.
    collect_apps_in_dir(project_root, "", &mut apps);

    // Project-folder apps: `<root>/<project_name>/<app>/apps.py`. We
    // can't know the project name without reading settings, so scan
    // every top-level dir that itself contains either a `settings.py`
    // or `__init__.py` and treat it as the project root candidate.
    if let Ok(entries) = std::fs::read_dir(project_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let looks_like_project = path.join("settings.py").exists()
                || (path.join("__init__.py").exists() && path.join("urls.py").exists());
            if looks_like_project {
                let prefix = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                collect_apps_in_dir(&path, &prefix, &mut apps);
            }
        }
    }

    apps.sort();
    apps.dedup();
    apps
}

fn collect_apps_in_dir(dir: &Path, prefix: &str, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path.join("apps.py").exists() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            if name.is_empty() {
                continue;
            }
            // Skip hidden / virtualenv / build folders defensively.
            if name.starts_with('.') || name == "node_modules" || name == "venv" {
                continue;
            }
            let full = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            out.push(full);
        }
    }
}
