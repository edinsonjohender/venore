//! Layer analyzers — heuristic inspection of module code
//!
//! Each analyzer examines filesystem evidence (no LLM calls) to determine
//! layer status. Returns `None` when the layer is not applicable to the module
//! (e.g. no source files → no tests layer).

use std::collections::HashMap;
use std::path::Path;
use std::time::SystemTime;

use tracing::debug;
use walkdir::WalkDir;

use super::types::*;
use crate::utils::staleness::{SOURCE_EXTENSIONS, SKIP_DIRS};

/// Test file patterns: prefix/suffix patterns that indicate a test file.
const TEST_SUFFIXES: &[&str] = &[
    ".test.", ".spec.", "_test.", ".tests.", ".specs.",
];
const TEST_PREFIXES: &[&str] = &["test_"];
const TEST_DIRS: &[&str] = &["__tests__", "tests", "test", "spec", "specs"];

/// Doc comment markers to search for in source files.
const DOC_MARKERS_JS: &[&str] = &["@param", "@returns", "@return", "@description", "@example"];
const DOC_MARKERS_RUST: &[&str] = &["///", "//!"];
const DOC_MARKERS_DOXYGEN: &[&str] = &["@brief", "\\brief", "@param", "\\param", "//!"];


// =============================================================================
// Public API
// =============================================================================

/// Analyze all requested layers for a module. Only returns layers that are
/// applicable (e.g. tests layer is skipped if the module has no source files).
pub fn analyze_module_layers(
    project_path: &Path,
    module_relative_path: &str,
    connection_info: Option<&ModuleConnectionInfo>,
    layers_config: &[String],
) -> ModuleLayerAnalysis {
    let module_dir = project_path.join(module_relative_path);

    // Single WalkDir pass: collects source files, max mtime, and test dir files
    let file_info = collect_module_files(&module_dir);

    debug!(
        module = module_relative_path,
        source_count = file_info.source_files.len(),
        layers = ?layers_config,
        "Analyzing module layers"
    );

    // Determine which layers need file content analysis
    let needs_docs = layers_config.iter().any(|l| l == "documentation");
    let needs_status = layers_config.iter().any(|l| l == "status");

    // Single file read pass for documentation + status when both are needed
    let file_analysis = if needs_docs || needs_status {
        Some(analyze_file_contents(&file_info.source_files, needs_docs, needs_status))
    } else {
        None
    };

    let mut layers = Vec::new();

    for layer_name in layers_config {
        let layer_type = match LayerType::from_config_name(layer_name) {
            Some(t) => t,
            None => {
                debug!(layer = layer_name, "Unknown layer type, skipping");
                continue;
            }
        };

        let analysis = match layer_type {
            LayerType::Context => Some(analyze_context(
                project_path,
                module_relative_path,
                &module_dir,
                file_info.max_source_mtime,
            )),
            LayerType::Tests => analyze_tests(&file_info.source_files, &file_info.test_dir_files),
            LayerType::Documentation => {
                let fa = file_analysis.as_ref().unwrap();
                analyze_documentation(&module_dir, &file_info.source_files, fa.documented_count)
            }
            LayerType::Connections => Some(analyze_connections(connection_info)),
            LayerType::Status => {
                let fa = file_analysis.as_ref().unwrap();
                analyze_status_from_counts(&file_info.source_files, &fa.status_counts)
            }
        };

        if let Some(a) = analysis {
            layers.push(a);
        }
    }

    ModuleLayerAnalysis {
        module_name: module_relative_path
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(module_relative_path)
            .to_string(),
        module_path: module_relative_path.to_string(),
        layers,
    }
}

// =============================================================================
// Context layer
// =============================================================================

fn analyze_context(
    project_path: &Path,
    module_relative_path: &str,
    module_dir: &Path,
    max_source_mtime: Option<SystemTime>,
) -> LayerAnalysis {
    let context_file = module_dir.join(".context.md");
    let mut details = HashMap::new();

    if !context_file.exists() {
        details.insert("freshness".into(), "missing".into());
        return LayerAnalysis {
            layer_type: LayerType::Context,
            status: LayerStatus::Missing,
            details,
        };
    }

    let context_path = format!(
        "{}/{}/.context.md",
        project_path.display(),
        module_relative_path
    );
    details.insert("context_path".into(), context_path.into());

    // Compare mtimes (max_source_mtime already collected in single WalkDir pass)
    let context_mtime = std::fs::metadata(&context_file)
        .and_then(|m| m.modified())
        .ok();

    let (status, freshness) = match (context_mtime, max_source_mtime) {
        (Some(ctx), Some(src)) if src > ctx => (LayerStatus::Partial, "stale"),
        (Some(_), Some(_)) => (LayerStatus::Complete, "fresh"),
        (Some(_), None) => (LayerStatus::Complete, "fresh"),
        _ => (LayerStatus::Partial, "stale"),
    };

    details.insert("freshness".into(), freshness.into());

    LayerAnalysis {
        layer_type: LayerType::Context,
        status,
        details,
    }
}

// =============================================================================
// Tests layer
// =============================================================================

fn analyze_tests(
    source_files: &[SourceFileInfo],
    test_dir_files: &[String],
) -> Option<LayerAnalysis> {
    if source_files.is_empty() {
        return None;
    }

    let mut test_files = Vec::new();
    let mut non_test_source = Vec::new();
    let mut frameworks_detected: Vec<String> = Vec::new();

    for sf in source_files {
        if is_test_file(&sf.name) {
            test_files.push(sf.name.clone());
        } else {
            non_test_source.push(sf.name.clone());
        }
    }

    // Add files from test directories that weren't already counted
    for tf in test_dir_files {
        if !test_files.contains(tf) {
            test_files.push(tf.clone());
        }
    }

    // Detect frameworks from test file contents (quick heuristic from filenames)
    if test_files.iter().any(|f| f.ends_with(".test.ts") || f.ends_with(".test.tsx") || f.ends_with(".spec.ts")) {
        frameworks_detected.push("jest/vitest".to_string());
    }
    if test_files.iter().any(|f| f.contains("_test.rs")) {
        frameworks_detected.push("rust-test".to_string());
    }
    if test_files.iter().any(|f| f.starts_with("test_") && f.ends_with(".py")) {
        frameworks_detected.push("pytest".to_string());
    }

    let total_source = non_test_source.len();
    let test_count = test_files.len();

    if test_count == 0 && total_source == 0 {
        return None;
    }

    let ratio = if total_source > 0 {
        test_count as f64 / total_source as f64
    } else {
        0.0
    };

    let status = if test_count == 0 {
        LayerStatus::Missing
    } else if ratio > 0.5 {
        LayerStatus::Complete
    } else {
        LayerStatus::Partial
    };

    let mut details = HashMap::new();
    details.insert("test_files".into(), (test_count as u64).into());
    details.insert("source_files".into(), (total_source as u64).into());
    details.insert("coverage_ratio".into(), ((ratio * 100.0).round() / 100.0).into());
    if !frameworks_detected.is_empty() {
        details.insert("frameworks_detected".into(), serde_json::json!(frameworks_detected));
    }

    Some(LayerAnalysis {
        layer_type: LayerType::Tests,
        status,
        details,
    })
}

fn is_test_file(filename: &str) -> bool {
    let lower = filename.to_lowercase();
    for suffix in TEST_SUFFIXES {
        if lower.contains(suffix) {
            return true;
        }
    }
    for prefix in TEST_PREFIXES {
        if lower.starts_with(prefix) {
            return true;
        }
    }
    false
}


// =============================================================================
// Documentation layer
// =============================================================================

fn analyze_documentation(
    module_dir: &Path,
    source_files: &[SourceFileInfo],
    documented_count: u32,
) -> Option<LayerAnalysis> {
    if source_files.is_empty() {
        return None;
    }

    let has_readme = module_dir.join("README.md").exists()
        || module_dir.join("readme.md").exists()
        || module_dir.join("README").exists();

    let total_source = source_files.len() as u32;

    let doc_ratio = if total_source > 0 {
        documented_count as f64 / total_source as f64
    } else {
        0.0
    };

    let status = if has_readme && doc_ratio > 0.5 {
        LayerStatus::Complete
    } else if has_readme || doc_ratio > 0.2 || documented_count > 0 {
        LayerStatus::Partial
    } else {
        LayerStatus::Missing
    };

    let mut details = HashMap::new();
    details.insert("has_readme".into(), has_readme.into());
    details.insert("documented_files".into(), (documented_count as u64).into());
    details.insert("total_source".into(), (total_source as u64).into());
    details.insert("doc_ratio".into(), ((doc_ratio * 100.0).round() / 100.0).into());

    Some(LayerAnalysis {
        layer_type: LayerType::Documentation,
        status,
        details,
    })
}


// =============================================================================
// Connections layer
// =============================================================================

fn analyze_connections(connection_info: Option<&ModuleConnectionInfo>) -> LayerAnalysis {
    let info = connection_info.cloned().unwrap_or_default();
    let dep_count = info.dependencies.len();
    let dependent_count = info.dependents.len();

    // Detect circular: module appears in both its deps and dependents
    let has_circular = info.dependencies.iter().any(|d| info.dependents.contains(d));
    let is_orphan = dep_count == 0 && dependent_count == 0;

    let status = if dep_count > 0 && dependent_count > 0 && !has_circular {
        LayerStatus::Complete
    } else if (dep_count > 0 || dependent_count > 0) && has_circular {
        LayerStatus::Partial
    } else if dep_count > 0 || dependent_count > 0 {
        LayerStatus::Partial
    } else {
        LayerStatus::Missing
    };

    let mut details = HashMap::new();
    details.insert("dependency_count".into(), (dep_count as u64).into());
    details.insert("dependent_count".into(), (dependent_count as u64).into());
    details.insert("has_circular".into(), has_circular.into());
    details.insert("is_orphan".into(), is_orphan.into());

    LayerAnalysis {
        layer_type: LayerType::Connections,
        status,
        details,
    }
}

// =============================================================================
// Status layer (TODO/FIXME/HACK/XXX)
// =============================================================================

fn analyze_status_from_counts(
    source_files: &[SourceFileInfo],
    counts: &StatusCounts,
) -> Option<LayerAnalysis> {
    if source_files.is_empty() {
        return None;
    }

    let total_issues = counts.todo + counts.fixme + counts.hack;

    let status = if total_issues == 0 {
        LayerStatus::Complete // Clean code
    } else if total_issues <= 5 {
        LayerStatus::Partial
    } else {
        LayerStatus::Missing // Many issues
    };

    let mut details = HashMap::new();
    details.insert("todo_count".into(), counts.todo.into());
    details.insert("fixme_count".into(), counts.fixme.into());
    details.insert("hack_count".into(), counts.hack.into());
    details.insert("total_issues".into(), total_issues.into());

    Some(LayerAnalysis {
        layer_type: LayerType::Status,
        status,
        details,
    })
}

// =============================================================================
// Shared helpers
// =============================================================================

struct SourceFileInfo {
    name: String,
    path: std::path::PathBuf,
}

/// Result of a single WalkDir pass over the module directory.
struct ModuleFileInfo {
    source_files: Vec<SourceFileInfo>,
    max_source_mtime: Option<SystemTime>,
    test_dir_files: Vec<String>,
}

/// Single WalkDir pass: collects source files, max mtime, and test-dir files.
/// Replaces the previous 3 separate WalkDir calls per module.
fn collect_module_files(module_dir: &Path) -> ModuleFileInfo {
    if !module_dir.exists() {
        return ModuleFileInfo {
            source_files: Vec::new(),
            max_source_mtime: None,
            test_dir_files: Vec::new(),
        };
    }

    let mut source_files = Vec::new();
    let mut max_mtime: Option<SystemTime> = None;
    let mut test_dir_files = Vec::new();

    for entry in WalkDir::new(module_dir)
        .into_iter()
        .filter_entry(|e| {
            if e.file_type().is_dir() {
                let name = e.file_name().to_string_lossy();
                if name.starts_with('.') || SKIP_DIRS.contains(&name.as_ref()) {
                    return false;
                }
            }
            true
        })
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let ext = match entry.path().extension().and_then(|e| e.to_str()) {
            Some(e) => e,
            None => continue,
        };

        if !SOURCE_EXTENSIONS.contains(&ext) {
            continue;
        }

        let name = entry
            .path()
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        // Track max mtime
        if let Some(mtime) = entry.metadata().ok().and_then(|m| m.modified().ok()) {
            max_mtime = Some(match max_mtime {
                Some(current) if mtime > current => mtime,
                Some(current) => current,
                None => mtime,
            });
        }

        // Check if this file is inside a test directory
        let is_in_test_dir = entry.path().components().any(|c| {
            if let std::path::Component::Normal(name) = c {
                let s = name.to_string_lossy();
                TEST_DIRS.contains(&s.as_ref())
            } else {
                false
            }
        });

        if is_in_test_dir {
            test_dir_files.push(name.clone());
        }

        source_files.push(SourceFileInfo {
            name,
            path: entry.path().to_path_buf(),
        });
    }

    ModuleFileInfo {
        source_files,
        max_source_mtime: max_mtime,
        test_dir_files,
    }
}

/// Aggregated counts from status marker analysis.
struct StatusCounts {
    todo: u32,
    fixme: u32,
    hack: u32,
}

/// Result of single-pass file content analysis (docs + status).
struct FileContentAnalysis {
    documented_count: u32,
    status_counts: StatusCounts,
}

/// Read each source file ONCE, extracting both doc-comment presence and TODO/FIXME counts.
/// Replaces the previous approach of reading files twice (once for docs, once for status).
fn analyze_file_contents(
    source_files: &[SourceFileInfo],
    needs_docs: bool,
    needs_status: bool,
) -> FileContentAnalysis {
    let mut documented_count = 0u32;
    let mut todo_count = 0u32;
    let mut fixme_count = 0u32;
    let mut hack_count = 0u32;

    for sf in source_files {
        if is_test_file(&sf.name) {
            continue;
        }

        let content = match std::fs::read_to_string(&sf.path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Doc comment check (first 8KB)
        if needs_docs {
            let limit = content.floor_char_boundary(8192);
            let scan = &content[..limit];
            let ext = sf.path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let has_docs = match ext {
                "rs" => DOC_MARKERS_RUST.iter().any(|m| scan.contains(m)),
                "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "vue" | "svelte" => {
                    DOC_MARKERS_JS.iter().any(|m| scan.contains(m)) || scan.contains("/**")
                }
                "py" => has_python_docstrings(scan),
                "go" => has_go_doc_comments(scan),
                "c" | "cpp" | "h" | "hpp" => {
                    DOC_MARKERS_DOXYGEN.iter().any(|m| scan.contains(m)) || scan.contains("/**")
                }
                "rb" => scan.contains("# @param") || scan.contains("# @return")
                    || scan.contains("# @!") || scan.contains("##"),
                _ => scan.contains("/**") || scan.contains("///"),
            };
            if has_docs {
                documented_count += 1;
            }
        }

        // Status marker counts (full file)
        if needs_status {
            for line in content.lines() {
                let upper = line.to_uppercase();
                if upper.contains("TODO") {
                    todo_count += 1;
                }
                if upper.contains("FIXME") {
                    fixme_count += 1;
                }
                if upper.contains("HACK") {
                    hack_count += 1;
                }
            }
        }
    }

    FileContentAnalysis {
        documented_count,
        status_counts: StatusCounts {
            todo: todo_count,
            fixme: fixme_count,
            hack: hack_count,
        },
    }
}

// =============================================================================
// Language-specific doc detection
// =============================================================================

/// Detect Python docstrings by checking structural placement, not just triple-quote presence.
///
/// Checks for: (1) module-level docstring (triple-quote as first non-blank/comment line),
/// (2) function/class docstrings (triple-quote on the line after def/class).
fn has_python_docstrings(scan: &str) -> bool {
    let mut prev_is_def = false;
    let mut first_code_line = true;

    for line in scan.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let starts_docstring = trimmed.starts_with("\"\"\"") || trimmed.starts_with("'''")
            || trimmed.starts_with("r\"\"\"") || trimmed.starts_with("r'''");

        // Module-level docstring: first non-blank, non-comment line is triple-quote
        if first_code_line && starts_docstring {
            return true;
        }
        first_code_line = false;

        // Function/class docstring: triple-quote right after def/class
        if prev_is_def && starts_docstring {
            return true;
        }

        prev_is_def = trimmed.starts_with("def ")
            || trimmed.starts_with("async def ")
            || trimmed.starts_with("class ");
    }
    false
}

/// Detect Go doc comments by checking the godoc convention:
/// a `// Comment` line (starting with uppercase) immediately before an exported declaration.
///
/// Also accepts `// Package` for package-level docs.
fn has_go_doc_comments(scan: &str) -> bool {
    let lines: Vec<&str> = scan.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Check for comment starting with uppercase (godoc convention)
        if let Some(comment_text) = trimmed.strip_prefix("// ") {
            if !comment_text.starts_with(char::is_uppercase) {
                continue;
            }

            // Package-level doc is always valid
            if comment_text.starts_with("Package ") {
                return true;
            }

            // Look ahead (skip continuation comment lines) for an export declaration
            for j in (i + 1)..lines.len().min(i + 6) {
                let next = lines[j].trim();
                if next.starts_with("// ") {
                    continue; // multi-line comment block
                }
                if next.starts_with("func ") || next.starts_with("type ")
                    || next.starts_with("var ") || next.starts_with("const ")
                {
                    return true;
                }
                break; // non-comment, non-declaration → stop
            }
        }
    }
    false
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_module_dir(tmp: &TempDir, files: &[(&str, &str)]) -> std::path::PathBuf {
        let dir = tmp.path().join("test_module");
        fs::create_dir_all(&dir).unwrap();
        for (name, content) in files {
            let path = dir.join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&path, content).unwrap();
        }
        dir
    }

    #[test]
    fn test_context_missing() {
        let tmp = TempDir::new().unwrap();
        let _dir = make_module_dir(&tmp, &[("index.ts", "export {}")]);

        let result = analyze_module_layers(
            tmp.path(),
            "test_module",
            None,
            &["context".to_string()],
        );

        assert_eq!(result.layers.len(), 1);
        assert_eq!(result.layers[0].layer_type, LayerType::Context);
        assert_eq!(result.layers[0].status, LayerStatus::Missing);
    }

    #[test]
    fn test_context_fresh() {
        let tmp = TempDir::new().unwrap();
        let dir = make_module_dir(&tmp, &[("index.ts", "export {}")]);

        // Write source first, then context (newer)
        std::thread::sleep(std::time::Duration::from_millis(50));
        fs::write(dir.join(".context.md"), "# Context").unwrap();

        let result = analyze_module_layers(
            tmp.path(),
            "test_module",
            None,
            &["context".to_string()],
        );

        assert_eq!(result.layers[0].status, LayerStatus::Complete);
    }

    #[test]
    fn test_tests_with_files() {
        let tmp = TempDir::new().unwrap();
        let _dir = make_module_dir(&tmp, &[
            ("index.ts", "export {}"),
            ("utils.ts", "export {}"),
            ("index.test.ts", "test('it works', () => {})"),
        ]);

        let result = analyze_module_layers(
            tmp.path(),
            "test_module",
            None,
            &["tests".to_string()],
        );

        assert_eq!(result.layers.len(), 1);
        assert_eq!(result.layers[0].layer_type, LayerType::Tests);
        assert_eq!(result.layers[0].status, LayerStatus::Partial);
    }

    #[test]
    fn test_tests_missing() {
        let tmp = TempDir::new().unwrap();
        let _dir = make_module_dir(&tmp, &[
            ("index.ts", "export {}"),
        ]);

        let result = analyze_module_layers(
            tmp.path(),
            "test_module",
            None,
            &["tests".to_string()],
        );

        assert_eq!(result.layers.len(), 1);
        assert_eq!(result.layers[0].status, LayerStatus::Missing);
    }

    #[test]
    fn test_documentation_with_readme() {
        let tmp = TempDir::new().unwrap();
        let _dir = make_module_dir(&tmp, &[
            ("index.ts", "/** @param x */\nexport function foo(x: number) {}"),
            ("README.md", "# Module docs"),
        ]);

        let result = analyze_module_layers(
            tmp.path(),
            "test_module",
            None,
            &["documentation".to_string()],
        );

        assert_eq!(result.layers.len(), 1);
        assert_eq!(result.layers[0].layer_type, LayerType::Documentation);
        assert!(matches!(result.layers[0].status, LayerStatus::Complete | LayerStatus::Partial));
    }

    #[test]
    fn test_connections_complete() {
        let tmp = TempDir::new().unwrap();
        let _dir = make_module_dir(&tmp, &[("index.ts", "export {}")]);

        let conn = ModuleConnectionInfo {
            dependencies: vec!["auth".to_string()],
            dependents: vec!["api".to_string()],
        };

        let result = analyze_module_layers(
            tmp.path(),
            "test_module",
            Some(&conn),
            &["connections".to_string()],
        );

        assert_eq!(result.layers[0].status, LayerStatus::Complete);
    }

    #[test]
    fn test_connections_orphan() {
        let tmp = TempDir::new().unwrap();
        let _dir = make_module_dir(&tmp, &[("index.ts", "export {}")]);

        let result = analyze_module_layers(
            tmp.path(),
            "test_module",
            None,
            &["connections".to_string()],
        );

        assert_eq!(result.layers[0].status, LayerStatus::Missing);
    }

    #[test]
    fn test_status_clean() {
        let tmp = TempDir::new().unwrap();
        let _dir = make_module_dir(&tmp, &[
            ("index.ts", "export function foo() { return 42; }"),
        ]);

        let result = analyze_module_layers(
            tmp.path(),
            "test_module",
            None,
            &["status".to_string()],
        );

        assert_eq!(result.layers[0].status, LayerStatus::Complete);
    }

    #[test]
    fn test_status_with_issues() {
        let tmp = TempDir::new().unwrap();
        let _dir = make_module_dir(&tmp, &[
            ("index.ts", "// TODO: fix this\n// FIXME: broken\nexport {}"),
        ]);

        let result = analyze_module_layers(
            tmp.path(),
            "test_module",
            None,
            &["status".to_string()],
        );

        assert_eq!(result.layers[0].status, LayerStatus::Partial);
        let total = result.layers[0].details.get("total_issues")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        assert_eq!(total, 2);
    }

    #[test]
    fn test_multiple_layers_different_heights() {
        let tmp = TempDir::new().unwrap();
        // Module with source, no tests, no context → tests layer won't appear
        let _dir = make_module_dir(&tmp, &[
            ("index.ts", "export {}"),
        ]);

        let result = analyze_module_layers(
            tmp.path(),
            "test_module",
            None,
            &[
                "context".to_string(),
                "tests".to_string(),
                "documentation".to_string(),
                "connections".to_string(),
                "status".to_string(),
            ],
        );

        // All 5 layers should be present (context, tests, docs, connections, status)
        // Tests has source files but no test files → Missing (still present)
        // Docs has source but no readme/docs → Missing (still present)
        assert!(result.layers.len() >= 3, "Should have at least context + connections + status");
    }

    #[test]
    fn test_no_source_files_skips_optional_layers() {
        let tmp = TempDir::new().unwrap();
        // Empty module
        let dir = tmp.path().join("empty_module");
        fs::create_dir_all(&dir).unwrap();

        let result = analyze_module_layers(
            tmp.path(),
            "empty_module",
            None,
            &[
                "context".to_string(),
                "tests".to_string(),
                "documentation".to_string(),
                "connections".to_string(),
                "status".to_string(),
            ],
        );

        // Only context and connections (always applicable)
        let types: Vec<_> = result.layers.iter().map(|l| l.layer_type).collect();
        assert!(types.contains(&LayerType::Context));
        assert!(types.contains(&LayerType::Connections));
        assert!(!types.contains(&LayerType::Tests));
        assert!(!types.contains(&LayerType::Documentation));
        assert!(!types.contains(&LayerType::Status));
    }

    // =========================================================================
    // Python docstring detection
    // =========================================================================

    #[test]
    fn test_python_module_docstring() {
        assert!(has_python_docstrings("\"\"\"Module documentation.\"\"\"\n\nimport os"));
        assert!(has_python_docstrings("# Comment\n\n\"\"\"Module doc.\"\"\""));
        assert!(has_python_docstrings("'''Single-quote module doc.'''"));
    }

    #[test]
    fn test_python_function_docstring() {
        assert!(has_python_docstrings("def foo():\n    \"\"\"Do something.\"\"\""));
        assert!(has_python_docstrings("async def bar():\n    \"\"\"Async doc.\"\"\""));
        assert!(has_python_docstrings("class MyClass:\n    \"\"\"Class doc.\"\"\""));
    }

    #[test]
    fn test_python_raw_docstring() {
        assert!(has_python_docstrings("def foo():\n    r\"\"\"Raw docstring.\"\"\""));
    }

    #[test]
    fn test_python_rejects_string_literals() {
        // Triple-quote NOT after def/class and NOT at module level → not a docstring
        assert!(!has_python_docstrings("x = 1\nresult = \"\"\"some template\"\"\""));
        assert!(!has_python_docstrings("x = 1\nif True:\n    msg = '''hello'''"));
    }

    // =========================================================================
    // Go doc comment detection
    // =========================================================================

    #[test]
    fn test_go_func_doc() {
        assert!(has_go_doc_comments("// Handler processes requests.\nfunc Handler() {}"));
    }

    #[test]
    fn test_go_type_doc() {
        assert!(has_go_doc_comments("// Config holds app settings.\ntype Config struct {}"));
    }

    #[test]
    fn test_go_multiline_doc() {
        assert!(has_go_doc_comments(
            "// Server is the main HTTP server.\n// It handles all incoming connections.\nfunc Server() {}"
        ));
    }

    #[test]
    fn test_go_package_doc() {
        assert!(has_go_doc_comments("// Package http provides HTTP client and server."));
    }

    #[test]
    fn test_go_var_const_doc() {
        assert!(has_go_doc_comments("// ErrNotFound indicates a missing resource.\nvar ErrNotFound = errors.New(\"not found\")"));
        assert!(has_go_doc_comments("// MaxRetries is the default retry count.\nconst MaxRetries = 3"));
    }

    #[test]
    fn test_go_rejects_lowercase_comments() {
        // lowercase comment is NOT a godoc comment
        assert!(!has_go_doc_comments("// this is a regular comment\nfunc handler() {}"));
    }

    #[test]
    fn test_go_rejects_no_declaration() {
        // Uppercase comment but no declaration following
        assert!(!has_go_doc_comments("// Warning: this is dangerous\nfmt.Println(\"hello\")"));
    }
}
