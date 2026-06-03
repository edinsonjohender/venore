//! Helpers shared by PHP analyzers.

use std::path::Path;

use super::super::super::traits::DetectedFramework;

/// Parse `composer.json` into a `serde_json::Value` if it exists and is
/// valid JSON. Returns `None` on missing file or parse error.
pub fn parse_composer_json(composer_json_path: &Path) -> Option<serde_json::Value> {
    let content = std::fs::read_to_string(composer_json_path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Extract the package name from `composer.json` `name` field (e.g.
/// `"vendor/package"`).
pub fn get_package_name(composer: &serde_json::Value) -> Option<String> {
    composer.get("name")?.as_str().map(|s| s.to_string())
}

/// Extract the PHP version constraint from `composer.json` `require.php`.
pub fn get_php_version(composer: &serde_json::Value) -> Option<String> {
    composer
        .get("require")?
        .get("php")?
        .as_str()
        .map(|s| s.to_string())
}

/// Detect common PHP frameworks declared in `composer.json` `require`.
///
/// Inspects only `require` today (not `require-dev`). Returns an empty
/// vec on parse error or missing file — frameworks are metadata, not a
/// detection signal, so a soft failure is fine.
pub fn detect_frameworks(composer: &serde_json::Value) -> Vec<DetectedFramework> {
    let mut frameworks = Vec::new();

    if let Some(deps) = composer.get("require").and_then(|d| d.as_object()) {
        for dep_name in deps.keys() {
            if let Some(fw) = DetectedFramework::from_dep_name(dep_name) {
                frameworks.push(fw);
            }
        }
    }

    frameworks
}

/// PSR-4 autoload roots (e.g. `src/`, `app/`) declared in
/// `composer.json` `autoload.psr-4`. Each value is a relative path
/// without trailing slash.
pub fn get_psr4_roots(composer: &serde_json::Value) -> Vec<String> {
    let mut roots = Vec::new();

    if let Some(psr4) = composer
        .get("autoload")
        .and_then(|a| a.get("psr-4"))
        .and_then(|p| p.as_object())
    {
        for value in psr4.values() {
            // PSR-4 values can be a string or an array of strings.
            match value {
                serde_json::Value::String(s) => roots.push(normalize_path(s)),
                serde_json::Value::Array(arr) => {
                    for item in arr {
                        if let Some(s) = item.as_str() {
                            roots.push(normalize_path(s));
                        }
                    }
                }
                _ => {}
            }
        }
    }

    roots
}

fn normalize_path(p: &str) -> String {
    p.trim_end_matches('/').to_string()
}
