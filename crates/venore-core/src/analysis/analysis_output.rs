//! Analysis Output - Consolidated structure for AI consumption (future TASK-004)
//!
//! This module prepares the analysis data from TASK-001, 002, 003 into a
//! structured format ready to be passed to LLM in the future.
//!
//! NO AI/LLM integration here - just data preparation.

use std::path::{Path, PathBuf};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use serde_json::Value;
use tracing::{debug, warn};
use crate::analysis::{ScanResult, ParseResult, ModuleDetectionResult, Language};

/// Analysis depth levels (how much code to extract)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum AnalysisDepth {
    /// Minimal - No code snippets, just metadata
    Minimal,
    /// Normal - 1 snippet (~100 chars)
    #[default]
    Normal,
    /// Detailed - 3 snippets (~300 chars each)
    Detailed,
    /// Expert - 5 snippets (~500 chars each)
    Expert,
}


/// Repository-level context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryInfo {
    /// Repository name (from directory)
    pub name: String,
    /// Main programming language detected
    pub language: Option<Language>,
    /// Technologies detected (e.g., ["TypeScript", "React", "Node.js"])
    pub technologies: Vec<String>,
    /// Total files scanned
    pub total_files: usize,
    /// Total modules detected
    pub total_modules: usize,
}

/// Module analysis output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleAnalysis {
    /// Module name
    pub name: String,
    /// Relative path from project root
    pub path: String,
    /// Number of files in module
    pub file_count: usize,
    /// Entry point file if detected
    pub entry_point: Option<String>,

    /// Architecture information
    pub architecture: ModuleArchitecture,

    /// Code symbols
    pub symbols: ModuleSymbols,

    /// Import statements
    pub imports: Vec<ImportInfo>,

    /// Code snippets (extracted based on depth level)
    pub code_snippets: String,

    /// File list
    pub files: Vec<String>,
}

/// Module architecture information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleArchitecture {
    /// Modules this module depends on
    pub dependencies: Vec<String>,
    /// Modules that depend on this module (reverse dependencies)
    pub dependents: Vec<String>,
    /// External dependencies (npm packages, cargo crates, etc)
    pub external_deps: Vec<String>,
}

/// Module symbols (functions, classes, etc)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleSymbols {
    /// Exported symbols (functions, classes that are exported)
    pub exports: Vec<SymbolInfo>,
    /// All symbols in the module
    pub all: Vec<SymbolInfo>,
}

/// Import information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportInfo {
    /// Module being imported from
    pub module: String,
    /// Items being imported (empty for default imports)
    pub items: Vec<String>,
    /// File where import occurs
    pub file: String,
}

/// Simplified symbol information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInfo {
    /// Symbol name
    pub name: String,
    /// Symbol kind (function, class, interface, etc)
    pub kind: String,
    /// File where symbol is defined
    pub file: String,
    /// Line number
    pub line: usize,
    /// Whether it's exported
    pub exported: bool,
}

/// Complete analysis output for a repository
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisOutput {
    /// Repository information
    pub repository: RepositoryInfo,
    /// All modules analyzed
    pub modules: Vec<ModuleAnalysis>,
    /// Orphan files (not in any module)
    pub orphan_files: Vec<String>,
}

// =============================================================================
// Persistence — .venore/analysis-output.json
// =============================================================================

const ANALYSIS_FILE: &str = ".venore/analysis-output.json";

impl AnalysisOutput {
    /// Save analysis output to `.venore/analysis-output.json` (atomic write).
    pub fn save_to_disk(&self, project_path: &Path) -> crate::Result<()> {
        let path = project_path.join(ANALYSIS_FILE);
        crate::utils::atomic_json::write_atomic(&path, self)?;
        debug!(path = ?path, "Analysis output saved to disk");
        Ok(())
    }

    /// Load analysis output from `.venore/analysis-output.json`.
    /// Returns None if file doesn't exist. Backs up corrupt files.
    pub fn load_from_disk(project_path: &Path) -> crate::Result<Option<Self>> {
        let path = project_path.join(ANALYSIS_FILE);
        let result: Option<Self> = crate::utils::atomic_json::read_or_backup_corrupt(&path)?;
        if result.is_some() {
            debug!(path = ?path, "Analysis output loaded from disk");
        } else if path.with_extension("json.corrupt").exists() {
            warn!(path = ?path, "Corrupt analysis file was backed up");
        }
        Ok(result)
    }
}

// =============================================================================
// Builder
// =============================================================================

/// Configuration for building analysis output
pub struct AnalysisConfig {
    pub scan_result: ScanResult,
    pub parse_results: Vec<ParseResult>,
    pub modules: ModuleDetectionResult,
    pub project_root: PathBuf,
    pub depth: AnalysisDepth,
}

/// Builds consolidated analysis output from scanner, parser, and detector results
pub struct AnalysisBuilder {
    config: AnalysisConfig,
    external_deps_cache: Option<Vec<String>>,
}

impl AnalysisBuilder {
    pub fn new(config: AnalysisConfig) -> Self {
        let external_deps_cache = Self::read_package_json(&config.project_root);
        Self {
            config,
            external_deps_cache,
        }
    }

    /// Build complete analysis output
    pub fn build(&self) -> AnalysisOutput {
        AnalysisOutput {
            repository: self.build_repository_info(),
            modules: self.build_modules(),
            orphan_files: self.build_orphan_files(),
        }
    }

    fn build_repository_info(&self) -> RepositoryInfo {
        let name = self.config.project_root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let language = self.detect_main_language();
        let technologies = self.detect_technologies();

        RepositoryInfo {
            name,
            language,
            technologies,
            total_files: self.config.scan_result.files.len(),
            total_modules: self.config.modules.modules.len(),
        }
    }

    fn build_modules(&self) -> Vec<ModuleAnalysis> {
        self.config.modules.modules.iter()
            .map(|module| self.build_module_analysis(module))
            .collect()
    }

    fn build_module_analysis(&self, module: &crate::analysis::module_detector::Module) -> ModuleAnalysis {
        // Always store the module path with forward slashes. The RAG indexer
        // normalizes `rag_files.relative_path` the same way before writing,
        // and `populate_graph` does prefix matching between them. On Windows
        // `to_string_lossy()` keeps backslashes, so without this normalization
        // a module at `src\shared\ui\Alert` never matched files under
        // `src/shared/ui/Alert/...` and only modules with single-segment
        // paths (no separators) ended up with files mapped.
        let path = module.path.to_string_lossy().replace('\\', "/");
        let entry_point = module.entry_point.as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|s| s.to_string());

        let architecture = self.build_architecture(module);
        let symbols = self.build_symbols(module);
        let imports = self.build_imports(module);
        let code_snippets = self.extract_code_snippets(module);

        let files: Vec<String> = module.files.iter()
            .filter_map(|p| p.file_name())
            .filter_map(|n| n.to_str())
            .map(|s| s.to_string())
            .collect();

        ModuleAnalysis {
            name: module.name.clone(),
            path,
            file_count: module.files.len(),
            entry_point,
            architecture,
            symbols,
            imports,
            code_snippets,
            files,
        }
    }

    fn build_architecture(&self, module: &crate::analysis::module_detector::Module) -> ModuleArchitecture {
        let dependencies = module.dependencies.clone();
        let dependents = self.compute_dependents(&module.name);
        let external_deps = self.extract_external_deps(module);

        ModuleArchitecture {
            dependencies,
            dependents,
            external_deps,
        }
    }

    fn build_symbols(&self, module: &crate::analysis::module_detector::Module) -> ModuleSymbols {
        let mut all_symbols = Vec::new();
        let mut exports_list = Vec::new();

        for file_path in &module.files {
            if let Some(parse_result) = self.config.parse_results.iter()
                .find(|pr| pr.file_path == *file_path)
            {
                // Collect all symbols
                for symbol in &parse_result.symbols {
                    all_symbols.push(SymbolInfo {
                        name: symbol.name.clone(),
                        kind: format!("{:?}", symbol.kind).to_lowercase(),
                        file: file_path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown")
                            .to_string(),
                        line: symbol.line_start,
                        exported: false, // Will be marked true if in exports
                    });
                }

                // Collect exports
                for export in &parse_result.exports {
                    exports_list.push(SymbolInfo {
                        name: export.name.clone(),
                        kind: format!("{:?}", export.kind).to_lowercase(),
                        file: file_path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown")
                            .to_string(),
                        line: 0, // Exports don't have line info
                        exported: true,
                    });
                }
            }
        }

        // Mark symbols as exported if they match export names
        for symbol in &mut all_symbols {
            if exports_list.iter().any(|e| e.name == symbol.name) {
                symbol.exported = true;
            }
        }

        ModuleSymbols {
            exports: exports_list,
            all: all_symbols,
        }
    }

    fn build_imports(&self, module: &crate::analysis::module_detector::Module) -> Vec<ImportInfo> {
        let mut imports_list = Vec::new();

        for file_path in &module.files {
            if let Some(parse_result) = self.config.parse_results.iter()
                .find(|pr| pr.file_path == *file_path)
            {
                let file_name = file_path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                for import in &parse_result.imports {
                    imports_list.push(ImportInfo {
                        module: import.module.clone(),
                        items: import.items.clone(),
                        file: file_name.clone(),
                    });
                }
            }
        }

        imports_list
    }

    fn build_orphan_files(&self) -> Vec<String> {
        self.config.modules.orphan_files.iter()
            .filter_map(|p| p.file_name())
            .filter_map(|n| n.to_str())
            .map(|s| s.to_string())
            .collect()
    }

    fn detect_main_language(&self) -> Option<Language> {
        let mut lang_count: HashMap<String, (Language, usize)> = HashMap::new();

        for file in &self.config.scan_result.files {
            if let Some(lang) = Language::from_extension(&file.extension) {
                let key = format!("{:?}", lang);
                lang_count.entry(key)
                    .and_modify(|(_, count)| *count += 1)
                    .or_insert((lang, 1));
            }
        }

        lang_count.into_values()
            .max_by_key(|(_, count)| *count)
            .map(|(lang, _)| lang)
    }

    fn detect_technologies(&self) -> Vec<String> {
        let mut technologies = Vec::new();
        let mut seen: std::collections::HashSet<&'static str> = std::collections::HashSet::new();

        // Derive the language list straight from the `Language` enum:
        // any extension Venore recognises that appears in the scanned
        // files counts as a detected technology. New `Language` variants
        // light up here automatically — no edit needed.
        for file in &self.config.scan_result.files {
            if let Some(lang) = Language::from_extension(&file.extension) {
                let label = lang.display_name();
                if seen.insert(label) {
                    technologies.push(label.to_string());
                }
            }
        }

        // Extension-level facts that aren't derivable from `Language`:
        // React is a UI framework on top of JS/TS but lives in the file
        // extension (`tsx`/`jsx`); Node.js shows up as a `package.json`
        // file at the root. Both are kept as heuristics for projects
        // that don't surface a typed analyzer detection here.
        if self
            .config
            .scan_result
            .files
            .iter()
            .any(|f| f.extension == "tsx" || f.extension == "jsx")
        {
            technologies.push("React".to_string());
        }

        if self.config.scan_result.files.iter().any(|f| {
            f.path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n == "package.json")
                .unwrap_or(false)
        }) {
            technologies.push("Node.js".to_string());
        }

        technologies
    }

    fn compute_dependents(&self, module_name: &str) -> Vec<String> {
        self.config.modules.modules.iter()
            .filter(|m| m.dependencies.contains(&module_name.to_string()))
            .map(|m| m.name.clone())
            .collect()
    }

    fn extract_external_deps(&self, _module: &crate::analysis::module_detector::Module) -> Vec<String> {
        // Return cached external dependencies from package.json
        self.external_deps_cache.clone().unwrap_or_default()
    }

    /// Read package.json and extract dependencies
    fn read_package_json(project_root: &PathBuf) -> Option<Vec<String>> {
        let package_json_path = project_root.join("package.json");

        if !package_json_path.exists() {
            return None;
        }

        let content = std::fs::read_to_string(&package_json_path).ok()?;
        let json: Value = serde_json::from_str(&content).ok()?;

        let mut deps = Vec::new();

        // Extract dependencies
        if let Some(dependencies) = json.get("dependencies").and_then(|d| d.as_object()) {
            for (name, version) in dependencies {
                let version_str = version.as_str().unwrap_or("*");
                deps.push(format!("{}@{}", name, version_str));
            }
        }

        // Extract devDependencies
        if let Some(dev_dependencies) = json.get("devDependencies").and_then(|d| d.as_object()) {
            for (name, version) in dev_dependencies {
                let version_str = version.as_str().unwrap_or("*");
                deps.push(format!("{}@{} (dev)", name, version_str));
            }
        }

        if deps.is_empty() {
            None
        } else {
            Some(deps)
        }
    }

    fn extract_code_snippets(&self, module: &crate::analysis::module_detector::Module) -> String {
        // Determine how many snippets to extract based on depth
        let (max_snippets, max_length) = match self.config.depth {
            AnalysisDepth::Minimal => return String::new(),
            AnalysisDepth::Normal => (1, 100),
            AnalysisDepth::Detailed => (3, 300),
            AnalysisDepth::Expert => (5, 500),
        };

        let mut snippets = Vec::new();

        // Prioritize entry point first, then other files
        let files_to_check: Vec<_> = if let Some(entry) = &module.entry_point {
            std::iter::once(entry.as_path())
                .chain(module.files.iter().map(|p| p.as_path()).filter(|p| Some(*p) != module.entry_point.as_deref()))
                .take(max_snippets)
                .collect()
        } else {
            module.files.iter().map(|p| p.as_path()).take(max_snippets).collect()
        };

        for file_path in files_to_check {
            if let Ok(content) = std::fs::read_to_string(file_path) {
                let file_name = file_path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");

                // Skip files that only contain re-exports (not useful for LLM inference)
                let trimmed = content.trim();
                let is_reexport_only = trimmed.lines()
                    .all(|line| {
                        let l = line.trim();
                        l.is_empty() ||
                        l.starts_with("//") ||
                        l.starts_with("/*") ||
                        l.starts_with("*") ||
                        l.starts_with("export * from") ||
                        l.starts_with("export {") && l.contains("} from")
                    });

                if is_reexport_only {
                    continue; // Skip re-export-only files
                }

                // Extract snippet (first N chars)
                let snippet: String = content.chars().take(max_length).collect();

                if !snippet.trim().is_empty() {
                    snippets.push(format!("// File: {}\n{}", file_name, snippet));
                }
            }

            if snippets.len() >= max_snippets {
                break;
            }
        }

        snippets.join("\n\n---\n\n")
    }
}

#[cfg(test)]
mod tests_analysis_builder {
    use super::*;
    use std::path::PathBuf;
    use std::time::SystemTime;
    use crate::analysis::file_scanner::FileInfo;
    use crate::analysis::module_detector::Module;
    use crate::analysis::ast_parser::{Symbol, SymbolKind, Export};

    fn make_file(path: &str, ext: &str, size: u64) -> FileInfo {
        let name = PathBuf::from(path)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        FileInfo {
            path: PathBuf::from(path),
            name,
            extension: ext.to_string(),
            size_bytes: size,
            last_modified: SystemTime::UNIX_EPOCH,
        }
    }

    fn empty_config_with_files(files: Vec<FileInfo>) -> AnalysisConfig {
        AnalysisConfig {
            scan_result: ScanResult {
                total_size_bytes: files.iter().map(|f| f.size_bytes).sum(),
                files,
                scan_duration_ms: 0,
            },
            parse_results: vec![],
            modules: ModuleDetectionResult {
                modules: vec![],
                orphan_files: vec![],
                detection_duration_ms: 0,
            },
            project_root: PathBuf::from("/test"),
            depth: AnalysisDepth::Normal,
        }
    }

    #[test]
    fn test_detect_main_language() {
        let config = empty_config_with_files(vec![
            make_file("a.ts", "ts", 100),
            make_file("b.ts", "ts", 100),
            make_file("c.js", "js", 100),
        ]);

        let builder = AnalysisBuilder::new(config);
        let language = builder.detect_main_language();

        assert_eq!(language, Some(Language::TypeScript));
    }

    #[test]
    fn test_detect_technologies() {
        let config = empty_config_with_files(vec![
            make_file("App.tsx", "tsx", 100),
            make_file("package.json", "json", 200),
        ]);

        let builder = AnalysisBuilder::new(config);
        let technologies = builder.detect_technologies();

        assert!(technologies.contains(&"TypeScript".to_string()));
        assert!(technologies.contains(&"React".to_string()));
        assert!(technologies.contains(&"Node.js".to_string()));
    }

    #[test]
    fn test_compute_dependents() {
        let modules = vec![
            Module {
                name: "auth".to_string(),
                path: PathBuf::from("auth"),
                files: vec![],
                entry_point: None,
                dependencies: vec![],
            },
            Module {
                name: "api".to_string(),
                path: PathBuf::from("api"),
                files: vec![],
                entry_point: None,
                dependencies: vec!["auth".to_string()],
            },
            Module {
                name: "ui".to_string(),
                path: PathBuf::from("ui"),
                files: vec![],
                entry_point: None,
                dependencies: vec!["auth".to_string()],
            },
        ];

        let config = AnalysisConfig {
            scan_result: ScanResult {
                files: vec![],
                total_size_bytes: 0,
                scan_duration_ms: 0,
            },
            parse_results: vec![],
            modules: ModuleDetectionResult {
                modules,
                orphan_files: vec![],
                detection_duration_ms: 0,
            },
            project_root: PathBuf::from("/test"),
            depth: AnalysisDepth::Normal,
        };

        let builder = AnalysisBuilder::new(config);
        let dependents = builder.compute_dependents("auth");

        assert_eq!(dependents.len(), 2);
        assert!(dependents.contains(&"api".to_string()));
        assert!(dependents.contains(&"ui".to_string()));
    }

    #[test]
    fn test_build_symbols_filters_exports() {
        let parse_results = vec![
            ParseResult {
                file_path: PathBuf::from("test.ts"),
                symbols: vec![
                    Symbol {
                        name: "publicFunc".to_string(),
                        kind: SymbolKind::Function,
                        line_start: 1,
                        line_end: 3,
                        signature: None,
                    },
                    Symbol {
                        name: "privateFunc".to_string(),
                        kind: SymbolKind::Function,
                        line_start: 5,
                        line_end: 7,
                        signature: None,
                    },
                ],
                exports: vec![
                    Export {
                        name: "publicFunc".to_string(),
                        kind: SymbolKind::Function,
                        line: 1,
                    },
                ],
                imports: vec![],
                language: Language::TypeScript,
                parse_duration_ms: 0,
            },
        ];

        let module = Module {
            name: "test".to_string(),
            path: PathBuf::from("test"),
            files: vec![PathBuf::from("test.ts")],
            entry_point: None,
            dependencies: vec![],
        };

        let config = AnalysisConfig {
            scan_result: ScanResult {
                files: vec![],
                total_size_bytes: 0,
                scan_duration_ms: 0,
            },
            parse_results,
            modules: ModuleDetectionResult {
                modules: vec![],
                orphan_files: vec![],
                detection_duration_ms: 0,
            },
            project_root: PathBuf::from("/test"),
            depth: AnalysisDepth::Normal,
        };

        let builder = AnalysisBuilder::new(config);
        let symbols = builder.build_symbols(&module);

        assert_eq!(symbols.all.len(), 2);
        assert_eq!(symbols.exports.len(), 1);
        assert_eq!(symbols.exports[0].name, "publicFunc");
    }
}
