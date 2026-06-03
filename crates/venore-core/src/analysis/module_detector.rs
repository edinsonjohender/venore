//! Module detector for grouping related files into logical modules
//!
//! Groups files based on:
//! - Directory structure
//! - Entry points (index.ts, mod.rs)
//! - Import/export relationships

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::collections::{HashMap, HashSet};

use super::file_scanner::FileInfo;
use super::ast_parser::{ParseResult, Import};
use super::project_analyzer::traits::ModuleDetectionStrategy;

/// Configuration for module detection
#[derive(Debug, Clone)]
pub struct DetectorConfig {
    /// Files from the scanner
    pub files: Vec<FileInfo>,

    /// Parse results from AST parser
    pub parse_results: Vec<ParseResult>,

    /// Project root directory
    pub project_root: PathBuf,

    /// Optional detection strategy from project analyzer
    /// If provided, uses module_markers to detect modules
    /// If None, falls back to current entry-point algorithm
    pub detection_strategy: Option<ModuleDetectionStrategy>,
}

/// A detected module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    /// Module name (e.g., "auth", "users")
    pub name: String,

    /// Path relative to project root
    pub path: PathBuf,

    /// Files that belong to this module
    pub files: Vec<PathBuf>,

    /// Entry point file (index.ts, mod.rs, etc.)
    pub entry_point: Option<PathBuf>,

    /// Other modules this module depends on
    pub dependencies: Vec<String>,
}

/// Result of module detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDetectionResult {
    /// Detected modules
    pub modules: Vec<Module>,

    /// Files that don't belong to any module
    pub orphan_files: Vec<PathBuf>,

    /// Time taken to detect in milliseconds
    pub detection_duration_ms: u64,
}

/// Detect modules from scanned files and parse results
///
/// # Arguments
///
/// * `config` - Configuration with files, parse results, and project root
///
/// # Returns
///
/// Returns `ModuleDetectionResult` containing detected modules
///
/// # Examples
///
/// ```no_run
/// use std::path::PathBuf;
/// use venore_core::analysis::module_detector::*;
/// use venore_core::analysis::file_scanner::FileInfo;
/// use venore_core::analysis::ast_parser::ParseResult;
///
/// let config = DetectorConfig {
///     files: vec![],
///     parse_results: vec![],
///     project_root: PathBuf::from("./"),
///     detection_strategy: None,
/// };
///
/// let result = detect_modules(config).unwrap();
/// println!("Found {} modules", result.modules.len());
/// ```
pub fn detect_modules(config: DetectorConfig) -> Result<ModuleDetectionResult> {
    let start = std::time::Instant::now();

    // Group files by directory or strategy
    let mut modules_by_dir = if let Some(strategy) = &config.detection_strategy {
        // Use strategy-based detection (module_markers)
        group_files_by_strategy(&config.files, &config.project_root, strategy)
    } else {
        // Fallback to current algorithm (entry points)
        group_files_by_directory(&config.files, &config.project_root)
    };

    // Detect entry points for each module
    for module in modules_by_dir.values_mut() {
        module.entry_point = detect_entry_point(&module.files);
    }

    // Build import map for dependency detection
    let import_map = build_import_map(&config.parse_results);

    // Detect dependencies between modules
    // First collect all module paths and names for lookup
    // Convert relative paths to absolute for comparison
    let module_paths: HashMap<PathBuf, String> = modules_by_dir
        .iter()
        .map(|(path, module)| {
            let abs_path = config.project_root.join(path);
            (abs_path, module.name.clone())
        })
        .collect();

    // Then compute dependencies for each module
    let module_dependencies: HashMap<String, Vec<String>> = modules_by_dir.values().map(|module| {
            let deps = detect_module_dependencies(
                module,
                &module_paths,
                &import_map,
            );
            (module.name.clone(), deps)
        })
        .collect();

    // Finally, update modules with their dependencies
    for module in modules_by_dir.values_mut() {
        if let Some(deps) = module_dependencies.get(&module.name) {
            module.dependencies = deps.clone();
        }
    }

    // Separate orphan files (files in root or single-file "modules")
    let mut modules = Vec::new();
    let mut orphan_files = Vec::new();

    for module in modules_by_dir.into_values() {
        // If module is in root directory, mark all its files as orphans
        // Root modules typically contain single utility files
        if is_root_file(&module.path) {
            orphan_files.extend(module.files);
        } else {
            modules.push(module);
        }
    }

    let detection_duration_ms = start.elapsed().as_millis() as u64;

    Ok(ModuleDetectionResult {
        modules,
        orphan_files,
        detection_duration_ms,
    })
}

/// Group files by their module root (directory containing entry point)
///
/// This algorithm finds the closest entry point (index.ts, mod.rs, etc.) by
/// walking up the directory tree, ensuring that all files in a module's
/// subdirectories are grouped together.
///
/// Example:
/// ```text
/// packages/math/index.ts       → module "math"
/// packages/math/src/angle.ts   → module "math" (walks up to find index.ts)
/// packages/math/src/curve.ts   → module "math" (walks up to find index.ts)
/// ```
fn group_files_by_directory(files: &[FileInfo], project_root: &Path) -> HashMap<PathBuf, Module> {
    // Step 1: Find all module roots (directories containing entry points)
    let module_roots = find_module_roots(files, project_root);

    // Step 2: Initialize modules
    let mut modules: HashMap<PathBuf, Module> = HashMap::new();
    for (module_path, module_name) in &module_roots {
        modules.insert(
            module_path.clone(),
            Module {
                name: module_name.clone(),
                path: module_path.clone(),
                files: Vec::new(),
                entry_point: None,
                dependencies: Vec::new(),
            },
        );
    }

    // Step 3: Assign each file to its closest module root
    for file in files {
        let rel_path = file.path
            .strip_prefix(project_root)
            .unwrap_or(&file.path);

        if let Some(module_root_path) = find_closest_module_root(rel_path, &module_roots) {
            // File belongs to a module
            if let Some(module) = modules.get_mut(&module_root_path) {
                module.files.push(file.path.clone());
            }
        } else {
            // File doesn't belong to any module (will be handled as orphan later)
            // Create a temporary module for files in their own directory
            let parent = rel_path.parent().unwrap_or_else(|| Path::new(""));
            let module_name = get_module_name(parent);
            let module_path = parent.to_path_buf();

            modules
                .entry(module_path.clone())
                .or_insert_with(|| Module {
                    name: module_name,
                    path: module_path.clone(),
                    files: Vec::new(),
                    entry_point: None,
                    dependencies: Vec::new(),
                })
                .files
                .push(file.path.clone());
        }
    }

    modules
}

/// Group files by their module root using a detection strategy
///
/// This algorithm uses module_markers (e.g., package.json, Cargo.toml) instead of
/// hardcoded entry points to detect module boundaries. This solves the problem where
/// packages/math/ with package.json but no index.ts at root would be split into
/// multiple modules.
///
/// Example with module_markers = ["package.json"]:
/// ```text
/// packages/math/package.json       → module "math" root
/// packages/math/global.d.ts        → module "math" (assigned to math)
/// packages/math/src/index.ts       → module "math" (assigned to math)
/// packages/math/src/angle.ts       → module "math" (assigned to math)
/// ```
fn group_files_by_strategy(
    files: &[FileInfo],
    project_root: &Path,
    strategy: &ModuleDetectionStrategy,
) -> HashMap<PathBuf, Module> {
    // Step 1: Find all module roots (directories containing module markers)
    let module_roots = find_module_roots_with_strategy(files, project_root, strategy);

    // Step 2: Initialize modules
    let mut modules: HashMap<PathBuf, Module> = HashMap::new();
    for (module_path, module_name) in &module_roots {
        modules.insert(
            module_path.clone(),
            Module {
                name: module_name.clone(),
                path: module_path.clone(),
                files: Vec::new(),
                entry_point: None,
                dependencies: Vec::new(),
            },
        );
    }

    // Step 3: Assign each file to its closest module root
    for file in files {
        let rel_path = file.path
            .strip_prefix(project_root)
            .unwrap_or(&file.path);

        if let Some(module_root_path) = find_closest_module_root(rel_path, &module_roots) {
            // File belongs to a module
            if let Some(module) = modules.get_mut(&module_root_path) {
                module.files.push(file.path.clone());
            }
        } else {
            // File doesn't belong to any module (will be handled as orphan later)
            // Create a temporary module for files in their own directory
            let parent = rel_path.parent().unwrap_or_else(|| Path::new(""));
            let module_name = get_module_name(parent);
            let module_path = parent.to_path_buf();

            modules
                .entry(module_path.clone())
                .or_insert_with(|| Module {
                    name: module_name,
                    path: module_path.clone(),
                    files: Vec::new(),
                    entry_point: None,
                    dependencies: Vec::new(),
                })
                .files
                .push(file.path.clone());
        }
    }

    modules
}

/// Find all directories that contain module marker files (package.json, Cargo.toml, etc.)
fn find_module_roots_with_strategy(
    files: &[FileInfo],
    project_root: &Path,
    strategy: &ModuleDetectionStrategy,
) -> HashMap<PathBuf, String> {
    let mut module_roots = HashMap::new();

    for file in files {
        if is_module_marker_file(&file.path, &strategy.module_markers) {
            let rel_path = file.path
                .strip_prefix(project_root)
                .unwrap_or(&file.path);

            if let Some(module_dir) = rel_path.parent() {
                let module_name = get_module_name(module_dir);
                module_roots.insert(module_dir.to_path_buf(), module_name);
            }
        }
    }

    module_roots
}

/// Check if a file is a module marker (package.json, Cargo.toml, etc.)
fn is_module_marker_file(path: &Path, module_markers: &[String]) -> bool {
    if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
        module_markers.iter().any(|marker| marker == file_name)
    } else {
        false
    }
}

/// Find all directories that contain entry point files
fn find_module_roots(files: &[FileInfo], project_root: &Path) -> HashMap<PathBuf, String> {
    let mut module_roots = HashMap::new();

    for file in files {
        if is_entry_point_file(&file.path) {
            let rel_path = file.path
                .strip_prefix(project_root)
                .unwrap_or(&file.path);

            if let Some(module_dir) = rel_path.parent() {
                let module_name = get_module_name(module_dir);
                module_roots.insert(module_dir.to_path_buf(), module_name);
            }
        }
    }

    module_roots
}

/// Check if a file is an entry point (index.ts, mod.rs, etc.)
fn is_entry_point_file(path: &Path) -> bool {
    let entry_point_names = [
        "index.ts",
        "index.tsx",
        "index.js",
        "index.jsx",
        "mod.rs",
        "lib.rs",
        "main.rs",
    ];

    if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
        entry_point_names.contains(&file_name)
    } else {
        false
    }
}

/// Find the closest module root by walking up the directory tree
fn find_closest_module_root(
    file_rel_path: &Path,
    module_roots: &HashMap<PathBuf, String>,
) -> Option<PathBuf> {
    let mut current = file_rel_path.parent()?;

    // Walk up the directory tree to find the closest module root
    loop {
        if module_roots.contains_key(current) {
            return Some(current.to_path_buf());
        }

        // Go up one level
        match current.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => {
                current = parent;
            }
            _ => {
                // Reached root or invalid parent
                return None;
            }
        }
    }
}

/// Get module name from directory path
fn get_module_name(path: &Path) -> String {
    // Handle different cases:
    // src/auth → "auth"
    // src/modules/auth → "auth"
    // src/features/auth → "auth"
    // "" (root) → "root"

    if path.as_os_str().is_empty() {
        return "root".to_string();
    }

    // Get the last component of the path
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string()
}

/// Detect entry point for a module
fn detect_entry_point(files: &[PathBuf]) -> Option<PathBuf> {
    // Common entry point names
    let entry_point_names = [
        "index.ts",
        "index.tsx",
        "index.js",
        "index.jsx",
        "mod.rs",
        "lib.rs",
        "main.rs",
    ];

    for file in files {
        if let Some(file_name) = file.file_name().and_then(|n| n.to_str()) {
            if entry_point_names.contains(&file_name) {
                return Some(file.clone());
            }
        }
    }

    // Check if there's a file with the same name as the parent directory
    // e.g., auth/auth.ts
    if let Some(first_file) = files.first() {
        if let Some(parent_name) = first_file
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
        {
            for file in files {
                if let Some(file_stem) = file.file_stem().and_then(|s| s.to_str()) {
                    if file_stem == parent_name {
                        return Some(file.clone());
                    }
                }
            }
        }
    }

    None
}

/// Build a map of file path to its imports
fn build_import_map(parse_results: &[ParseResult]) -> HashMap<PathBuf, Vec<Import>> {
    let mut import_map = HashMap::new();

    for parse_result in parse_results {
        import_map.insert(
            parse_result.file_path.clone(),
            parse_result.imports.clone(),
        );
    }

    import_map
}

/// Detect dependencies between modules based on imports.
///
/// For each relative import, resolves it to an absolute path and picks the
/// **most specific** module that contains it (longest matching path). This
/// matters when modules are nested — e.g. `src/shared/ui/Alert/` lives inside
/// `src/shared/ui/`, so an import to `../Button/` should resolve to the
/// sibling component `Button`, not to the broader parent `ui`. The earlier
/// implementation iterated a `HashMap` (non-deterministic order) and broke on
/// the first match, which let the parent win arbitrarily and produced a
/// hub-and-spoke graph where everything pointed at the parent directory.
///
/// **Barrel-aware**: when an import points to a parent module's barrel file
/// (`index.ts`, `index.tsx`, …) and brings named items, we read the barrel
/// and try to map each named item to its sibling module (`export { Button }
/// from './Button'` → resolve to the `Button` module instead of the parent).
/// Modern UI libraries lean heavily on barrels; without this every import
/// like `import { Button } from '@/shared/ui'` collapsed onto the parent and
/// made the canvas graph look hub-and-spoke.
fn detect_module_dependencies(
    module: &Module,
    module_paths: &HashMap<PathBuf, String>,
    import_map: &HashMap<PathBuf, Vec<Import>>,
) -> Vec<String> {
    let mut dependencies = HashSet::new();
    // Cache barrel parses across this module's imports — a feature-rich
    // design system can have a single barrel re-imported by dozens of files,
    // and parsing it once is much cheaper than once per import line.
    let mut barrel_cache: HashMap<PathBuf, HashMap<String, PathBuf>> = HashMap::new();

    for file_path in &module.files {
        let imports = match import_map.get(file_path) {
            Some(i) => i,
            None => continue,
        };

        for import in imports {
            // Skip external imports (npm packages)
            if !import.module.starts_with('.') && !import.module.starts_with('/') {
                continue;
            }

            let file_dir = match file_path.parent() {
                Some(d) => d,
                None => continue,
            };
            let import_path = resolve_import_path(file_dir, &import.module);

            // Pick the module with the longest matching path (most specific).
            let mut best: Option<(usize, &String, &PathBuf)> = None;
            for (other_module_path, other_module_name) in module_paths {
                if other_module_name == &module.name {
                    continue; // Skip self
                }
                if import_path.starts_with(other_module_path) {
                    let len = other_module_path.as_os_str().len();
                    if best.map(|(l, _, _)| len > l).unwrap_or(true) {
                        best = Some((len, other_module_name, other_module_path));
                    }
                }
            }

            // Barrel resolution: when the import targets a directory (no
            // file extension, no specific file inside) and brings named
            // items, see if there's a barrel file there and route each named
            // item to its sibling module. Two cases worth handling:
            //
            //   (a) the directory IS a detected module — `best` matches it
            //       exactly, and barrel resolution may redirect onto siblings.
            //   (b) the directory is just a container (no source files of
            //       its own, only sub-modules each in their own folder), so
            //       `best` won't match anything. We still try the barrel
            //       there because the named imports likely target siblings.
            //
            // In either case, if barrel resolution finds nothing useful, we
            // fall back to whatever `best` had (preserves the prior behavior
            // for `export * from` barrels we can't statically resolve).
            let mut barrel_resolved = false;
            if !import.items.is_empty() {
                let is_barrel_target = match best {
                    Some((_, _, parent_path)) => &import_path == parent_path,
                    None => import_path.is_dir(),
                };
                if is_barrel_target {
                    let re_exports = barrel_cache
                        .entry(import_path.clone())
                        .or_insert_with(|| parse_barrel_re_exports(&import_path));
                    for item in &import.items {
                        if let Some(target_path) = re_exports.get(item) {
                            let mut sub_best: Option<(usize, &String)> = None;
                            for (omp, omn) in module_paths {
                                if omn == &module.name { continue; }
                                if target_path.starts_with(omp) {
                                    let len = omp.as_os_str().len();
                                    if sub_best.map(|(l, _)| len > l).unwrap_or(true) {
                                        sub_best = Some((len, omn));
                                    }
                                }
                            }
                            if let Some((_, sub_name)) = sub_best {
                                dependencies.insert(sub_name.clone());
                                barrel_resolved = true;
                            }
                        }
                    }
                }
            }

            if !barrel_resolved {
                if let Some((_, name, _)) = best {
                    dependencies.insert(name.clone());
                }
            }
        }
    }

    dependencies.into_iter().collect()
}

/// Parse a module's barrel file (`index.{ts,tsx,js,jsx,mjs}`) and return a
/// map of `re-exported name → target path` (the path the export points at,
/// resolved relative to the barrel's directory).
///
/// Recognized forms (regex-based — good enough for the typical barrel and
/// doesn't pull in an extra AST pass):
///   * `export { A, B as C } from './path'`        → A and C
///   * `export { default as X } from './path'`     → X
///   * `export type { X } from './path'`           → X
///   * `export * from './path'`                    → SKIPPED (no name to
///     attach; leaves the import attributed to the parent, same as before)
///
/// Returns an empty map if no barrel is found or the file can't be read.
fn parse_barrel_re_exports(module_dir: &Path) -> HashMap<String, PathBuf> {
    use regex::Regex;
    let mut map = HashMap::new();

    let barrel_names = ["index.ts", "index.tsx", "index.js", "index.jsx", "index.mjs"];
    let barrel_path = barrel_names
        .iter()
        .map(|n| module_dir.join(n))
        .find(|p| p.is_file());
    let barrel_path = match barrel_path {
        Some(p) => p,
        None => return map,
    };

    let content = match std::fs::read_to_string(&barrel_path) {
        Ok(c) => c,
        Err(_) => return map,
    };

    // `export { A, B as C, default as D } from './path'`
    // Capture the brace list and the path; we parse the list per-name below.
    let re_named = Regex::new(
        r#"(?m)^\s*export\s+(?:type\s+)?\{\s*([^}]+?)\s*\}\s*from\s+['"]([^'"]+)['"]"#,
    )
    .unwrap();

    for cap in re_named.captures_iter(&content) {
        let names = &cap[1];
        let path_str = &cap[2];
        // Only relative paths matter — bare specifiers leave the package.
        if !path_str.starts_with('.') {
            continue;
        }
        let target = resolve_import_path(module_dir, path_str);
        for raw in names.split(',') {
            let raw = raw.trim();
            if raw.is_empty() {
                continue;
            }
            // Forms: `Foo`, `Foo as Bar`, `default as Foo`, `type Foo`.
            // The exposed name is the last identifier-ish token.
            let exposed = raw.rsplit_once(" as ").map(|(_, b)| b.trim()).unwrap_or(raw);
            // Strip a leading `type ` if present (`type Foo`).
            let exposed = exposed.trim_start_matches("type ").trim();
            if !exposed.is_empty() {
                map.insert(exposed.to_string(), target.clone());
            }
        }
    }

    map
}

/// Resolve a relative import path to an absolute path
fn resolve_import_path(from_dir: &Path, import_module: &str) -> PathBuf {
    // Handle different import formats:
    // './auth' → current_dir/auth
    // '../users/user.service' → parent_dir/users/user.service
    // '../../shared/utils' → parent_parent_dir/shared/utils

    // Use PathBuf's join method which handles .. correctly
    let mut resolved = from_dir.to_path_buf();

    // Split import path and process each component
    for component in import_module.split('/') {
        match component {
            "." | "./" => {
                // Current directory, do nothing
            }
            ".." | "../" => {
                // Parent directory, go up one level
                resolved.pop();
            }
            "" => {
                // Empty component, skip
            }
            name => {
                // Normal path component
                resolved.push(name);
            }
        }
    }

    resolved
}

/// Check if a file is in the project root (not in a subdirectory)
fn is_root_file(module_path: &Path) -> bool {
    // If module path is empty or just ".", it's in root
    module_path.as_os_str().is_empty() || module_path == Path::new(".")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    fn create_file_info(path: &str, project_root: &str) -> FileInfo {
        let full_path = PathBuf::from(project_root).join(path);
        FileInfo {
            path: full_path,
            name: path.split('/').next_back().unwrap_or(path).to_string(),
            extension: path.split('.').next_back().unwrap_or("").to_string(),
            size_bytes: 100,
            last_modified: SystemTime::now(),
        }
    }

    fn create_parse_result(file_path: PathBuf, imports: Vec<Import>) -> ParseResult {
        ParseResult {
            file_path,
            language: super::super::ast_parser::Language::TypeScript,
            symbols: vec![],
            imports,
            exports: vec![],
            parse_duration_ms: 0,
        }
    }

    #[test]
    fn test_get_module_name() {
        assert_eq!(get_module_name(Path::new("src/auth")), "auth");
        assert_eq!(get_module_name(Path::new("src/modules/users")), "users");
        assert_eq!(get_module_name(Path::new("")), "root");
        assert_eq!(get_module_name(Path::new("features/products")), "products");
    }

    #[test]
    fn test_detect_modules_by_directory() {
        let project_root = PathBuf::from("/tmp/test-project");

        let files = vec![
            create_file_info("src/auth/auth.service.ts", "/tmp/test-project"),
            create_file_info("src/auth/auth.controller.ts", "/tmp/test-project"),
            create_file_info("src/users/users.service.ts", "/tmp/test-project"),
            create_file_info("src/users/users.controller.ts", "/tmp/test-project"),
        ];

        let config = DetectorConfig {
            files,
            parse_results: vec![],
            project_root: project_root.clone(),
            detection_strategy: None,
        };

        let result = detect_modules(config).unwrap();

        // Should detect 2 modules: auth and users
        assert_eq!(result.modules.len(), 2);

        let module_names: Vec<String> = result.modules.iter().map(|m| m.name.clone()).collect();
        assert!(module_names.contains(&"auth".to_string()));
        assert!(module_names.contains(&"users".to_string()));

        // Each module should have 2 files
        for module in &result.modules {
            assert_eq!(module.files.len(), 2);
        }
    }

    #[test]
    fn test_detect_entry_points() {
        let project_root = PathBuf::from("/tmp/test-project");

        let files = vec![
            create_file_info("src/auth/index.ts", "/tmp/test-project"),
            create_file_info("src/auth/service.ts", "/tmp/test-project"),
        ];

        let config = DetectorConfig {
            files,
            parse_results: vec![],
            project_root: project_root.clone(),
            detection_strategy: None,
        };

        let result = detect_modules(config).unwrap();

        assert_eq!(result.modules.len(), 1);

        let auth_module = &result.modules[0];
        assert_eq!(auth_module.name, "auth");
        assert!(auth_module.entry_point.is_some());

        // Entry point should be index.ts
        let entry_point = auth_module.entry_point.as_ref().unwrap();
        assert!(entry_point.to_string_lossy().contains("index.ts"));
    }

    #[test]
    fn test_detect_module_dependencies() {
        let project_root = PathBuf::from("/tmp/test-project");

        let auth_service = project_root.join("src/auth/auth.service.ts");
        let users_service = project_root.join("src/users/users.service.ts");

        let files = vec![
            create_file_info("src/auth/auth.service.ts", "/tmp/test-project"),
            create_file_info("src/users/users.service.ts", "/tmp/test-project"),
        ];

        // auth.service.ts imports from ../users/
        let parse_results = vec![
            create_parse_result(
                auth_service.clone(),
                vec![Import {
                    module: "../users/users.service".to_string(),
                    items: vec!["UserService".to_string()],
                    line: 1,
                }],
            ),
            create_parse_result(users_service.clone(), vec![]),
        ];

        let config = DetectorConfig {
            files,
            parse_results,
            project_root: project_root.clone(),
            detection_strategy: None,
        };

        let result = detect_modules(config).unwrap();

        // Find auth module
        let auth_module = result
            .modules
            .iter()
            .find(|m| m.name == "auth")
            .expect("auth module should exist");

        // auth should depend on users
        assert!(auth_module.dependencies.contains(&"users".to_string()));
    }

    #[test]
    fn test_barrel_resolves_named_imports_to_siblings() {
        use std::fs;
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let ui_dir = root.join("src/shared/ui");
        let button_dir = ui_dir.join("Button");
        let card_dir = ui_dir.join("Card");
        let alert_dir = ui_dir.join("Alert");
        fs::create_dir_all(&button_dir).unwrap();
        fs::create_dir_all(&card_dir).unwrap();
        fs::create_dir_all(&alert_dir).unwrap();

        // Sibling components — each owns its own file.
        fs::write(button_dir.join("Button.tsx"), "export const Button = () => null;").unwrap();
        fs::write(card_dir.join("Card.tsx"), "export const Card = () => null;").unwrap();
        // Alert imports from the parent barrel (the typical case that was
        // collapsing everything onto the `ui` parent before this fix).
        fs::write(
            alert_dir.join("Alert.tsx"),
            "import { Button, Card } from '../'\nexport const Alert = () => null;",
        )
        .unwrap();

        // Barrel re-exports Button and Card from sibling subdirs.
        fs::write(
            ui_dir.join("index.ts"),
            r#"export { Button } from './Button/Button';
export { Card } from './Card/Card';
"#,
        )
        .unwrap();

        let files = vec![
            create_file_info("src/shared/ui/Button/Button.tsx", root.to_str().unwrap()),
            create_file_info("src/shared/ui/Card/Card.tsx", root.to_str().unwrap()),
            create_file_info("src/shared/ui/Alert/Alert.tsx", root.to_str().unwrap()),
        ];

        let parse_results = vec![
            create_parse_result(button_dir.join("Button.tsx"), vec![]),
            create_parse_result(card_dir.join("Card.tsx"), vec![]),
            create_parse_result(
                alert_dir.join("Alert.tsx"),
                vec![Import {
                    module: "../".to_string(),
                    items: vec!["Button".to_string(), "Card".to_string()],
                    line: 1,
                }],
            ),
        ];

        let config = DetectorConfig {
            files,
            parse_results,
            project_root: root.to_path_buf(),
            detection_strategy: None,
        };

        let result = detect_modules(config).unwrap();

        let alert = result.modules.iter().find(|m| m.name == "Alert")
            .expect("Alert module should exist");

        // Without the barrel fix, Alert's only dependency would be the parent
        // `ui` (or whatever the detector named it). With the fix, the named
        // imports resolve to the sibling modules.
        assert!(
            alert.dependencies.contains(&"Button".to_string()),
            "Alert should depend on Button (resolved via barrel re-export), got: {:?}",
            alert.dependencies
        );
        assert!(
            alert.dependencies.contains(&"Card".to_string()),
            "Alert should depend on Card (resolved via barrel re-export), got: {:?}",
            alert.dependencies
        );
    }

    #[test]
    fn test_handles_orphan_files() {
        let project_root = PathBuf::from("/tmp/test-project");

        let files = vec![
            create_file_info("index.ts", "/tmp/test-project"),  // Root file
            create_file_info("utils.ts", "/tmp/test-project"),  // Root file
            create_file_info("src/auth/auth.service.ts", "/tmp/test-project"),
            create_file_info("src/auth/auth.controller.ts", "/tmp/test-project"),
        ];

        let config = DetectorConfig {
            files,
            parse_results: vec![],
            project_root: project_root.clone(),
            detection_strategy: None,
        };

        let result = detect_modules(config).unwrap();

        // Should have orphan files (index.ts, utils.ts in root)
        assert!(!result.orphan_files.is_empty(), "Should have at least 1 orphan file");

        // Should have auth module
        assert!(result.modules.iter().any(|m| m.name == "auth"));
    }

    #[test]
    fn test_flat_structure() {
        let project_root = PathBuf::from("/tmp/test-project");

        let files = vec![
            create_file_info("main.ts", "/tmp/test-project"),
            create_file_info("config.ts", "/tmp/test-project"),
        ];

        let config = DetectorConfig {
            files,
            parse_results: vec![],
            project_root: project_root.clone(),
            detection_strategy: None,
        };

        let result = detect_modules(config).unwrap();

        // Flat structure: all files should be orphans
        assert_eq!(result.orphan_files.len(), 2);
    }

    #[test]
    fn test_detection_duration_recorded() {
        let project_root = PathBuf::from("/tmp/test-project");

        let files = vec![create_file_info("src/auth/auth.ts", "/tmp/test-project")];

        let config = DetectorConfig {
            files,
            parse_results: vec![],
            project_root,
            detection_strategy: None,
        };

        let result = detect_modules(config).unwrap();

        // Duration should be recorded and reasonable
        assert!(result.detection_duration_ms < 1000); // Less than 1 second
    }

    #[test]
    fn test_rust_entry_points() {
        let project_root = PathBuf::from("/tmp/test-project");

        let files = vec![
            create_file_info("src/auth/mod.rs", "/tmp/test-project"),
            create_file_info("src/auth/service.rs", "/tmp/test-project"),
        ];

        let config = DetectorConfig {
            files,
            parse_results: vec![],
            project_root,
            detection_strategy: None,
        };

        let result = detect_modules(config).unwrap();

        assert_eq!(result.modules.len(), 1);

        let auth_module = &result.modules[0];
        assert!(auth_module.entry_point.is_some());

        // Entry point should be mod.rs
        let entry_point = auth_module.entry_point.as_ref().unwrap();
        assert!(entry_point.to_string_lossy().contains("mod.rs"));
    }

    #[test]
    fn test_monorepo_with_src_subdirectory() {
        // This tests the specific case that was failing:
        // packages/math/index.ts (entry point)
        // packages/math/src/angle.ts (should be in same module)
        // packages/math/src/curve.ts (should be in same module)
        let project_root = PathBuf::from("/tmp/test-project");

        let files = vec![
            create_file_info("packages/math/index.ts", "/tmp/test-project"),
            create_file_info("packages/math/src/angle.ts", "/tmp/test-project"),
            create_file_info("packages/math/src/curve.ts", "/tmp/test-project"),
            create_file_info("packages/math/src/point.ts", "/tmp/test-project"),
        ];

        let config = DetectorConfig {
            files,
            parse_results: vec![],
            project_root,
            detection_strategy: None,
        };

        let result = detect_modules(config).unwrap();

        // Should detect ONE module "math" (not separate "math" and "src")
        assert_eq!(
            result.modules.len(),
            1,
            "Should detect exactly 1 module, got: {:?}",
            result.modules.iter().map(|m| &m.name).collect::<Vec<_>>()
        );

        let math_module = &result.modules[0];
        assert_eq!(math_module.name, "math");

        // Module should contain ALL 4 files
        assert_eq!(
            math_module.files.len(),
            4,
            "Math module should contain 4 files, got: {}",
            math_module.files.len()
        );

        // Entry point should be index.ts
        assert!(math_module.entry_point.is_some());
        let entry_point = math_module.entry_point.as_ref().unwrap();
        assert!(
            entry_point.to_string_lossy().contains("index.ts"),
            "Entry point should be index.ts"
        );

        // Verify all src/ files are included
        let file_names: Vec<String> = math_module
            .files
            .iter()
            .filter_map(|f| f.file_name())
            .filter_map(|n| n.to_str())
            .map(|s| s.to_string())
            .collect();

        assert!(file_names.contains(&"index.ts".to_string()));
        assert!(file_names.contains(&"angle.ts".to_string()));
        assert!(file_names.contains(&"curve.ts".to_string()));
        assert!(file_names.contains(&"point.ts".to_string()));
    }

    #[test]
    fn test_strategy_with_node_monorepo() {
        // Test detection using NodeMonorepo strategy (package.json as module marker)
        let project_root = PathBuf::from("/tmp/test-project");

        let files = vec![
            create_file_info("packages/math/package.json", "/tmp/test-project"),
            create_file_info("packages/math/global.d.ts", "/tmp/test-project"),
            create_file_info("packages/math/src/index.ts", "/tmp/test-project"),
            create_file_info("packages/math/src/angle.ts", "/tmp/test-project"),
            create_file_info("packages/math/src/curve.ts", "/tmp/test-project"),
            create_file_info("packages/utils/package.json", "/tmp/test-project"),
            create_file_info("packages/utils/src/string.ts", "/tmp/test-project"),
        ];

        let strategy = ModuleDetectionStrategy {
            module_markers: vec!["package.json".to_string()],
            entry_point_files: vec!["index.ts".to_string(), "index.js".to_string()],
        };

        let config = DetectorConfig {
            files,
            parse_results: vec![],
            project_root,
            detection_strategy: Some(strategy),
        };

        let result = detect_modules(config).unwrap();

        // Should detect 2 modules: math and utils
        assert_eq!(
            result.modules.len(),
            2,
            "Should detect exactly 2 modules, got: {:?}",
            result.modules.iter().map(|m| &m.name).collect::<Vec<_>>()
        );

        // Find math module
        let math_module = result
            .modules
            .iter()
            .find(|m| m.name == "math")
            .expect("math module should exist");

        // Math module should contain ALL 5 files (package.json + global.d.ts + 3 src files)
        assert_eq!(
            math_module.files.len(),
            5,
            "Math module should contain 5 files, got: {}",
            math_module.files.len()
        );

        // Find utils module
        let utils_module = result
            .modules
            .iter()
            .find(|m| m.name == "utils")
            .expect("utils module should exist");

        // Utils module should contain 2 files
        assert_eq!(utils_module.files.len(), 2);
    }

    #[test]
    fn test_strategy_with_rust_workspace() {
        // Test detection using RustWorkspace strategy (Cargo.toml as module marker)
        let project_root = PathBuf::from("/tmp/test-project");

        let files = vec![
            create_file_info("crates/core/Cargo.toml", "/tmp/test-project"),
            create_file_info("crates/core/src/lib.rs", "/tmp/test-project"),
            create_file_info("crates/core/src/utils.rs", "/tmp/test-project"),
            create_file_info("crates/cli/Cargo.toml", "/tmp/test-project"),
            create_file_info("crates/cli/src/main.rs", "/tmp/test-project"),
        ];

        let strategy = ModuleDetectionStrategy {
            module_markers: vec!["Cargo.toml".to_string()],
            entry_point_files: vec!["lib.rs".to_string(), "main.rs".to_string()],
        };

        let config = DetectorConfig {
            files,
            parse_results: vec![],
            project_root,
            detection_strategy: Some(strategy),
        };

        let result = detect_modules(config).unwrap();

        // Should detect 2 modules: core and cli
        assert_eq!(result.modules.len(), 2);

        let module_names: Vec<String> = result.modules.iter().map(|m| m.name.clone()).collect();
        assert!(module_names.contains(&"core".to_string()));
        assert!(module_names.contains(&"cli".to_string()));

        // Core should have 3 files (Cargo.toml + lib.rs + utils.rs)
        let core_module = result
            .modules
            .iter()
            .find(|m| m.name == "core")
            .expect("core module should exist");
        assert_eq!(core_module.files.len(), 3);
    }

    #[test]
    fn test_strategy_packages_math_without_index() {
        // This is THE KEY TEST that solves the original problem:
        // packages/math/ with package.json but NO index.ts at root
        // Before: Would be split into 2 modules (math with 1 file, src with 15 files)
        // After: Should be 1 module (math with all 16 files)
        let project_root = PathBuf::from("/tmp/test-project");

        let files = vec![
            create_file_info("packages/math/package.json", "/tmp/test-project"),
            create_file_info("packages/math/global.d.ts", "/tmp/test-project"),
            // NO index.ts at packages/math/ level!
            create_file_info("packages/math/src/index.ts", "/tmp/test-project"),
            create_file_info("packages/math/src/angle.ts", "/tmp/test-project"),
            create_file_info("packages/math/src/curve.ts", "/tmp/test-project"),
            create_file_info("packages/math/src/point.ts", "/tmp/test-project"),
            create_file_info("packages/math/src/line.ts", "/tmp/test-project"),
        ];

        let strategy = ModuleDetectionStrategy {
            module_markers: vec!["package.json".to_string()],
            entry_point_files: vec!["index.ts".to_string()],
        };

        let config = DetectorConfig {
            files,
            parse_results: vec![],
            project_root,
            detection_strategy: Some(strategy),
        };

        let result = detect_modules(config).unwrap();

        // CRITICAL: Should detect exactly 1 module (not 2!)
        assert_eq!(
            result.modules.len(),
            1,
            "Should detect exactly 1 module, got: {:?}",
            result.modules.iter().map(|m| &m.name).collect::<Vec<_>>()
        );

        let math_module = &result.modules[0];
        assert_eq!(math_module.name, "math");

        // Module should contain ALL 7 files
        assert_eq!(
            math_module.files.len(),
            7,
            "Math module should contain 7 files (package.json + global.d.ts + 5 src files), got: {}",
            math_module.files.len()
        );

        // Verify specific files are included
        let file_names: Vec<String> = math_module
            .files
            .iter()
            .filter_map(|f| f.file_name())
            .filter_map(|n| n.to_str())
            .map(|s| s.to_string())
            .collect();

        assert!(file_names.contains(&"package.json".to_string()));
        assert!(file_names.contains(&"global.d.ts".to_string()));
        assert!(file_names.contains(&"index.ts".to_string()));
        assert!(file_names.contains(&"angle.ts".to_string()));
        assert!(file_names.contains(&"curve.ts".to_string()));
    }

    #[test]
    fn test_backward_compatibility_without_strategy() {
        // Verify that without a strategy, the algorithm works exactly as before
        let project_root = PathBuf::from("/tmp/test-project");

        let files = vec![
            create_file_info("src/auth/index.ts", "/tmp/test-project"),
            create_file_info("src/auth/service.ts", "/tmp/test-project"),
            create_file_info("src/users/mod.rs", "/tmp/test-project"),
            create_file_info("src/users/model.rs", "/tmp/test-project"),
        ];

        let config = DetectorConfig {
            files,
            parse_results: vec![],
            project_root,
            detection_strategy: None, // No strategy = use old algorithm
        };

        let result = detect_modules(config).unwrap();

        // Should detect 2 modules using entry points
        assert_eq!(result.modules.len(), 2);

        let module_names: Vec<String> = result.modules.iter().map(|m| m.name.clone()).collect();
        assert!(module_names.contains(&"auth".to_string()));
        assert!(module_names.contains(&"users".to_string()));
    }
}
