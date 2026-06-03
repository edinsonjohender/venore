//! PR Analysis Service
//!
//! Assembles context from the PR diff and project `.context.md` files,
//! builds a structured prompt, and delegates to the LLM for analysis.
//! Supports 4 depth levels for configurable context enrichment.

use std::collections::HashSet;
use std::path::Path;
use tracing::{debug, info, warn};

use super::types::GitHubPullRequestFile;

// =============================================================================
// Analysis Depth Level
// =============================================================================

/// Configurable depth for PR analysis context enrichment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum AnalysisDepthLevel {
    /// Patches + PR metadata only. No `.context.md`.
    Minimal,
    /// Patches + root `.context.md` + affected modules. (default)
    #[default]
    Normal,
    /// Normal + RAG: symbol definitions from the diff + module layer analysis
    Detailed,
    /// Detailed + full content of changed files + module history
    Expert,
}


impl AnalysisDepthLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Normal => "normal",
            Self::Detailed => "detailed",
            Self::Expert => "expert",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "minimal" => Some(Self::Minimal),
            "normal" => Some(Self::Normal),
            "detailed" => Some(Self::Detailed),
            "expert" => Some(Self::Expert),
            _ => None,
        }
    }
}

// =============================================================================
// Context Assembly
// =============================================================================

/// All the context needed to analyze a PR.
pub struct PrAnalysisContext {
    pub pr_title: String,
    pub pr_body: Option<String>,
    pub pr_author: String,
    pub head_ref: String,
    pub base_ref: String,
    pub files: Vec<PrFileContext>,
    /// Root .context.md content
    pub project_context: Option<String>,
    /// Module-level .context.md files: (module_name, content)
    pub module_contexts: Vec<(String, String)>,
    /// RAG results: symbol definitions found in code (Detailed+)
    pub code_definitions: Vec<CodeDefinition>,
    /// Layer analysis results: (module_name, analysis_text) (Detailed+)
    pub module_health: Vec<(String, String)>,
    /// Full file contents: (filename, content) (Expert only)
    pub full_file_contents: Vec<(String, String)>,
}

/// A code definition found via RAG search.
pub struct CodeDefinition {
    pub symbol_name: String,
    pub chunk_type: String,
    pub content: String,
    pub relative_path: String,
    pub line_start: u32,
}

pub struct PrFileContext {
    pub filename: String,
    pub status: String,
    pub additions: u64,
    pub deletions: u64,
    pub patch: Option<String>,
}

/// Max total characters for patches in the prompt to avoid exceeding token limits.
const MAX_PATCH_CHARS: usize = 100_000;

/// Budget limits for enrichment sections
const RAG_DEFINITIONS_BUDGET: usize = 20_000;
const MODULE_HEALTH_BUDGET: usize = 5_000;
const FULL_FILES_BUDGET: usize = 150_000;

/// Assemble PR context from files and project path.
///
/// Reads the root `.context.md` and identifies affected modules' `.context.md`
/// files to provide architectural context for the analysis.
pub fn assemble_pr_context(
    project_path: &Path,
    pr_title: &str,
    pr_body: Option<&str>,
    pr_author: &str,
    head_ref: &str,
    base_ref: &str,
    files: &[GitHubPullRequestFile],
) -> PrAnalysisContext {
    info!(pr_title, file_count = files.len(), "Assembling PR analysis context");

    // Read root .context.md
    let root_context_path = project_path.join(".context.md");
    let project_context = std::fs::read_to_string(&root_context_path).ok();
    if project_context.is_some() {
        debug!("Found root .context.md");
    }

    // Identify affected modules by looking at directory prefixes of changed files
    let mut seen_modules = HashSet::new();
    let mut module_contexts = Vec::new();

    for file in files {
        // Take the first directory component as the potential module
        let parts: Vec<&str> = file.filename.split('/').collect();
        if parts.len() >= 2 {
            let module_name = parts[0].to_string();
            if seen_modules.insert(module_name.clone()) {
                // Check for .context.md in this module's directory
                let module_context_path = project_path.join(&module_name).join(".context.md");
                if let Ok(content) = std::fs::read_to_string(&module_context_path) {
                    debug!(module = %module_name, "Found module .context.md");
                    module_contexts.push((module_name, content));
                }
            }
        }
    }

    // Build file contexts with truncation
    let mut total_patch_chars = 0usize;
    let file_contexts: Vec<PrFileContext> = files
        .iter()
        .map(|f| {
            let patch = if let Some(ref p) = f.patch {
                if total_patch_chars + p.len() <= MAX_PATCH_CHARS {
                    total_patch_chars += p.len();
                    Some(p.clone())
                } else if total_patch_chars < MAX_PATCH_CHARS {
                    let remaining = MAX_PATCH_CHARS - total_patch_chars;
                    total_patch_chars = MAX_PATCH_CHARS;
                    warn!(filename = %f.filename, "Truncating patch to fit within limit");
                    Some(format!("{}...[truncated]", &p[..remaining]))
                } else {
                    None // Skip patches beyond limit
                }
            } else {
                None
            };

            PrFileContext {
                filename: f.filename.clone(),
                status: f.status.clone(),
                additions: f.additions,
                deletions: f.deletions,
                patch,
            }
        })
        .collect();

    PrAnalysisContext {
        pr_title: pr_title.to_string(),
        pr_body: pr_body.map(|s| s.to_string()),
        pr_author: pr_author.to_string(),
        head_ref: head_ref.to_string(),
        base_ref: base_ref.to_string(),
        files: file_contexts,
        project_context,
        module_contexts,
        code_definitions: Vec::new(),
        module_health: Vec::new(),
        full_file_contents: Vec::new(),
    }
}

// =============================================================================
// Context Enrichment
// =============================================================================

/// Enrich the PR analysis context based on the configured depth level.
///
/// - **Minimal**: Strips .context.md data (patches only)
/// - **Normal**: No-op (already has .context.md from assembly)
/// - **Detailed**: Adds RAG symbol definitions + layer analysis
/// - **Expert**: Detailed + full file contents of changed files
pub async fn enrich_context(
    ctx: &mut PrAnalysisContext,
    depth: AnalysisDepthLevel,
    project_path: &Path,
    rag_repo: Option<&crate::rag::RagRepository>,
    project_id: Option<&str>,
) {
    info!(depth = depth.as_str(), "Enriching PR context");

    match depth {
        AnalysisDepthLevel::Minimal => {
            ctx.project_context = None;
            ctx.module_contexts.clear();
        }
        AnalysisDepthLevel::Normal => {
            // No-op — already assembled with .context.md
        }
        AnalysisDepthLevel::Detailed => {
            enrich_with_rag(ctx, rag_repo, project_id).await;
            enrich_with_layers(ctx, project_path);
        }
        AnalysisDepthLevel::Expert => {
            enrich_with_rag(ctx, rag_repo, project_id).await;
            enrich_with_layers(ctx, project_path);
            enrich_with_full_files(ctx, project_path);
        }
    }
}

/// Extract symbol identifiers from the added lines in patches.
fn extract_symbols_from_patches(files: &[PrFileContext]) -> Vec<String> {
    let mut symbols = HashSet::new();

    // Common language keywords to filter out
    let keywords: HashSet<&str> = [
        "if", "else", "for", "while", "return", "const", "let", "var", "function",
        "class", "interface", "type", "import", "export", "from", "async", "await",
        "pub", "fn", "struct", "enum", "impl", "use", "mod", "self", "Self",
        "true", "false", "null", "undefined", "None", "Some", "Ok", "Err",
        "new", "this", "super", "extends", "implements", "static", "readonly",
        "string", "number", "boolean", "void", "any", "never", "unknown",
        "String", "Option", "Result", "Vec", "Box", "Arc", "Mutex",
        "match", "break", "continue", "loop", "in", "of", "as", "is",
        "try", "catch", "throw", "finally", "yield", "delete", "typeof",
        "where", "trait", "dyn", "ref", "mut", "move", "crate",
    ].into_iter().collect();

    let type_re = regex::Regex::new(r"\b([A-Z][a-zA-Z0-9]{2,})\b").unwrap();
    let func_re = regex::Regex::new(r"\b([a-z][a-zA-Z0-9_]{2,})\s*\(").unwrap();

    for file in files {
        let patch = match &file.patch {
            Some(p) => p,
            None => continue,
        };

        for line in patch.lines() {
            // Only scan added lines
            if !line.starts_with('+') || line.starts_with("+++") {
                continue;
            }

            // Types/classes (PascalCase)
            for cap in type_re.captures_iter(line) {
                let sym = &cap[1];
                if !keywords.contains(sym) {
                    symbols.insert(sym.to_string());
                }
            }

            // Function calls (camelCase/snake_case followed by parenthesis)
            for cap in func_re.captures_iter(line) {
                let sym = &cap[1];
                if !keywords.contains(sym) {
                    symbols.insert(sym.to_string());
                }
            }
        }
    }

    // Limit to ~30 symbols
    let mut result: Vec<String> = symbols.into_iter().collect();
    result.sort();
    result.truncate(30);

    debug!(count = result.len(), "Extracted symbols from patches");
    result
}

/// Enrich context with RAG symbol definitions from the code index.
async fn enrich_with_rag(
    ctx: &mut PrAnalysisContext,
    rag_repo: Option<&crate::rag::RagRepository>,
    project_id: Option<&str>,
) {
    let (repo, pid) = match (rag_repo, project_id) {
        (Some(r), Some(p)) => (r, p),
        _ => {
            warn!("RAG not available (no repository or project_id), skipping symbol enrichment");
            return;
        }
    };

    let symbols = extract_symbols_from_patches(&ctx.files);
    if symbols.is_empty() {
        debug!("No symbols extracted from patches, skipping RAG enrichment");
        return;
    }

    let mut total_chars = 0usize;
    let mut definitions_found = 0u32;

    for symbol in &symbols {
        if total_chars >= RAG_DEFINITIONS_BUDGET {
            break;
        }

        match crate::rag::search_code(repo, pid, symbol, 3, 2000).await {
            Ok(results) => {
                for result in results {
                    let content_len = result.chunk.content.len();
                    if total_chars + content_len > RAG_DEFINITIONS_BUDGET {
                        break;
                    }
                    total_chars += content_len;
                    definitions_found += 1;

                    ctx.code_definitions.push(CodeDefinition {
                        symbol_name: result.chunk.name.clone(),
                        chunk_type: result.chunk.chunk_type.clone(),
                        content: result.chunk.content,
                        relative_path: result.chunk.relative_path,
                        line_start: result.chunk.line_start,
                    });
                }
            }
            Err(e) => {
                debug!(symbol = %symbol, error = %e, "RAG search failed for symbol");
            }
        }
    }

    info!(
        definitions = definitions_found,
        total_chars,
        "Enriching with RAG: found {} definitions",
        definitions_found
    );
}

/// Enrich context with layer analysis (tests, status) for affected modules.
fn enrich_with_layers(ctx: &mut PrAnalysisContext, project_path: &Path) {
    use crate::layers::analyzer::analyze_module_layers;

    let layers_config = vec!["tests".to_string(), "status".to_string()];
    let mut total_chars = 0usize;

    // Analyze each module that has a .context.md (already detected)
    let module_names: Vec<String> = ctx.module_contexts.iter()
        .map(|(name, _)| name.clone())
        .collect();

    for module_name in &module_names {
        if total_chars >= MODULE_HEALTH_BUDGET {
            break;
        }

        let analysis = analyze_module_layers(
            project_path,
            module_name,
            None,
            &layers_config,
        );

        // Serialize to readable text
        let mut text = String::new();
        for layer in &analysis.layers {
            let status_str = layer.status.as_str();
            let layer_name = layer.layer_type.as_str();

            text.push_str(&format!("- {}: {} ", layer_name, status_str));

            // Add key details
            match layer.layer_type {
                crate::layers::types::LayerType::Tests => {
                    if let Some(count) = layer.details.get("test_files").and_then(|v| v.as_u64()) {
                        let ratio = layer.details.get("coverage_ratio")
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.0);
                        text.push_str(&format!("({} test files, {:.0}% ratio)", count, ratio * 100.0));
                    }
                }
                crate::layers::types::LayerType::Status => {
                    let todo = layer.details.get("todo_count").and_then(|v| v.as_u64()).unwrap_or(0);
                    let fixme = layer.details.get("fixme_count").and_then(|v| v.as_u64()).unwrap_or(0);
                    let hack = layer.details.get("hack_count").and_then(|v| v.as_u64()).unwrap_or(0);
                    text.push_str(&format!("({} TODOs, {} FIXMEs, {} HACKs)", todo, fixme, hack));
                }
                _ => {}
            }
            text.push('\n');
        }

        if total_chars + text.len() <= MODULE_HEALTH_BUDGET {
            total_chars += text.len();
            ctx.module_health.push((module_name.clone(), text));
        }
    }

    info!(
        modules = ctx.module_health.len(),
        total_chars,
        "Enriched with layer analysis"
    );
}

/// Enrich context with full file contents of changed files (Expert level).
fn enrich_with_full_files(ctx: &mut PrAnalysisContext, project_path: &Path) {
    // Collect files sorted by size (smallest first to maximize coverage)
    let mut file_sizes: Vec<(String, u64)> = ctx.files.iter()
        .filter_map(|f| {
            let path = project_path.join(&f.filename);
            std::fs::metadata(&path).ok().map(|m| (f.filename.clone(), m.len()))
        })
        .collect();

    file_sizes.sort_by_key(|(_, size)| *size);

    let mut total_chars = 0usize;

    for (filename, _) in &file_sizes {
        if total_chars >= FULL_FILES_BUDGET {
            break;
        }

        let path = project_path.join(filename);
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                if total_chars + content.len() > FULL_FILES_BUDGET {
                    // Skip files that would exceed budget
                    continue;
                }
                total_chars += content.len();
                ctx.full_file_contents.push((filename.clone(), content));
            }
            Err(_) => {
                debug!(filename = %filename, "File not found (may be deleted in PR), skipping");
            }
        }
    }

    info!(
        files = ctx.full_file_contents.len(),
        total_chars,
        "Enriched with full file contents"
    );
}

// =============================================================================
// Prompt Builder
// =============================================================================

/// Build the analysis prompt from the assembled context.
///
/// The `depth` parameter controls the Analysis Instructions section:
/// higher depths instruct the LLM to leverage the enrichment data
/// (Code Context, Module Health, Full File Contents) that was injected.
pub fn build_pr_analysis_prompt(ctx: &PrAnalysisContext, depth: AnalysisDepthLevel) -> String {
    let mut prompt = String::with_capacity(8192);

    prompt.push_str("You are a senior code reviewer analyzing a pull request. ");
    prompt.push_str("Evaluate the changes against the project's established patterns and conventions.\n\n");

    // Project context
    if let Some(ref project_ctx) = ctx.project_context {
        prompt.push_str("## Project Context (.context.md)\n\n");
        prompt.push_str(project_ctx);
        prompt.push_str("\n\n");
    }

    // Module contexts
    for (name, content) in &ctx.module_contexts {
        prompt.push_str(&format!("## Module Context: {} (.context.md)\n\n", name));
        prompt.push_str(content);
        prompt.push_str("\n\n");
    }

    // PR info
    prompt.push_str("---\n\n## Pull Request\n\n");
    prompt.push_str(&format!("**Title:** {}\n", ctx.pr_title));
    prompt.push_str(&format!("**Author:** {}\n", ctx.pr_author));
    prompt.push_str(&format!("**Branch:** {} → {}\n", ctx.head_ref, ctx.base_ref));

    if let Some(ref body) = ctx.pr_body {
        prompt.push_str(&format!("\n**Description:**\n{}\n", body));
    }

    // Files changed
    prompt.push_str(&format!("\n## Files Changed ({})\n\n", ctx.files.len()));

    for file in &ctx.files {
        prompt.push_str(&format!(
            "### `{}` ({}, +{} -{})\n",
            file.filename, file.status, file.additions, file.deletions,
        ));

        if let Some(ref patch) = file.patch {
            prompt.push_str("\n```diff\n");
            prompt.push_str(patch);
            prompt.push_str("\n```\n\n");
        } else {
            prompt.push_str("*(patch not available)*\n\n");
        }
    }

    // Code definitions (Detailed+)
    if !ctx.code_definitions.is_empty() {
        prompt.push_str("---\n\n## Code Context (Referenced Definitions)\n\n");
        for def in &ctx.code_definitions {
            prompt.push_str(&format!(
                "### `{}` ({}) — {}\n\n```\n{}\n```\n\n",
                def.symbol_name,
                def.chunk_type,
                def.relative_path,
                def.content,
            ));
        }
    }

    // Module health (Detailed+)
    if !ctx.module_health.is_empty() {
        prompt.push_str("---\n\n## Module Health\n\n");
        for (module_name, health_text) in &ctx.module_health {
            prompt.push_str(&format!("### {}/\n{}\n", module_name, health_text));
        }
    }

    // Full file contents (Expert)
    if !ctx.full_file_contents.is_empty() {
        prompt.push_str("---\n\n## Full File Contents\n\n");
        for (filename, content) in &ctx.full_file_contents {
            prompt.push_str(&format!("### `{}`\n\n```\n{}\n```\n\n", filename, content));
        }
    }

    // Analysis instructions (depth-aware)
    prompt.push_str(&build_analysis_instructions(depth));

    prompt
}

/// Build the "Analysis Instructions" section based on the depth level.
///
/// Higher depths instruct the LLM to explicitly reference enrichment data
/// (Code Context, Module Health, Full File Contents) in its findings.
fn build_analysis_instructions(depth: AnalysisDepthLevel) -> String {
    let mut s = String::with_capacity(2048);
    s.push_str("---\n\n## Analysis Instructions\n\n");

    match depth {
        AnalysisDepthLevel::Minimal => {
            s.push_str("Perform a lightweight review based ONLY on the diff patches above.\n\n");
            s.push_str("Analyze this PR and provide a structured review. For each category below, assign a verdict:\n");
            s.push_str("- **PASS** — Meets expectations\n");
            s.push_str("- **WARNING** — Minor concerns or suggestions\n");
            s.push_str("- **FAIL** — Significant issues that should be addressed\n\n");
            s.push_str("### Categories\n\n");
            s.push_str("1. **Code Quality** — Is the code clean, readable, and well-structured?\n");
            s.push_str("2. **Naming & Style** — Are naming conventions consistent?\n");
            s.push_str("3. **Potential Issues** — Any bugs, edge cases, or security concerns visible in the diff?\n");
            s.push_str("4. **Documentation** — Are public APIs documented where appropriate?\n\n");
            s.push_str("Note: This is a lightweight review without project context. Findings are limited to what is visible in the patches.\n\n");
        }

        AnalysisDepthLevel::Normal => {
            s.push_str("Analyze this PR and provide a structured review. For each category below, assign a verdict:\n");
            s.push_str("- **PASS** — Meets expectations\n");
            s.push_str("- **WARNING** — Minor concerns or suggestions\n");
            s.push_str("- **FAIL** — Significant issues that should be addressed\n\n");
            s.push_str("### Categories\n\n");
            s.push_str("1. **Pattern Compliance** — Does the code follow patterns documented in .context.md?\n");
            s.push_str("2. **Naming & Style** — Are naming conventions consistent with the project?\n");
            s.push_str("3. **Test Coverage** — Are there tests for new functionality?\n");
            s.push_str("4. **Architecture Alignment** — Does the change fit the documented architecture?\n");
            s.push_str("5. **Potential Issues** — Any bugs, edge cases, or security concerns?\n");
            s.push_str("6. **Documentation** — Are public APIs documented where appropriate?\n\n");
        }

        AnalysisDepthLevel::Detailed => {
            s.push_str("You have been given enriched context for a deep review. The following sections are available:\n");
            s.push_str("- **Patches**: The actual code changes (diffs)\n");
            s.push_str("- **Project/Module .context.md**: Documented patterns, conventions, and architecture\n");
            s.push_str("- **Code Context (Referenced Definitions)**: Symbol definitions from the codebase found via RAG search\n");
            s.push_str("- **Module Health**: Layer analysis with test coverage ratios, TODOs, FIXMEs, and HACKs\n\n");

            s.push_str("### How to use the enriched context\n\n");
            s.push_str("**Code Context** — Use these definitions to:\n");
            s.push_str("- Verify the PR uses APIs/types correctly (check function signatures, parameter types)\n");
            s.push_str("- Detect broken contracts (e.g., calling a function with wrong arguments, missing required fields)\n");
            s.push_str("- Identify code duplication (is the PR reimplementing something that already exists?)\n\n");
            s.push_str("**Module Health** — Use these metrics to:\n");
            s.push_str("- Flag changes to modules with low test coverage (the PR should add tests)\n");
            s.push_str("- Note existing TODOs/FIXMEs that the PR might resolve or worsen\n");
            s.push_str("- Identify unhealthy modules where extra care is warranted\n\n");

            s.push_str("Analyze this PR and provide a structured review. For each category below, assign a verdict:\n");
            s.push_str("- **PASS** — Meets expectations\n");
            s.push_str("- **WARNING** — Minor concerns or suggestions\n");
            s.push_str("- **FAIL** — Significant issues that should be addressed\n\n");
            s.push_str("### Categories\n\n");
            s.push_str("1. **Pattern Compliance** — Does the code follow patterns documented in .context.md? Cross-reference with Code Context definitions.\n");
            s.push_str("2. **Naming & Style** — Are naming conventions consistent with the project and existing symbol names?\n");
            s.push_str("3. **Test Coverage** — Are there tests for new functionality? Reference Module Health test ratios.\n");
            s.push_str("4. **Architecture Alignment** — Does the change fit the documented architecture? Check for API misuse using Code Context.\n");
            s.push_str("5. **Potential Issues** — Bugs, edge cases, security concerns, or broken contracts visible in diffs and Code Context.\n");
            s.push_str("6. **Code Health** — Does this PR improve or worsen the module's health? Reference Module Health metrics.\n\n");

            s.push_str("**Be specific**: reference actual symbol names from Code Context and metrics from Module Health in your findings.\n\n");
        }

        AnalysisDepthLevel::Expert => {
            s.push_str("You have been given the MAXIMUM enriched context for an expert-level review. The following sections are available:\n");
            s.push_str("- **Patches**: The actual code changes (diffs)\n");
            s.push_str("- **Project/Module .context.md**: Documented patterns, conventions, and architecture\n");
            s.push_str("- **Code Context (Referenced Definitions)**: Symbol definitions from the codebase found via RAG search\n");
            s.push_str("- **Module Health**: Layer analysis with test coverage ratios, TODOs, FIXMEs, and HACKs\n");
            s.push_str("- **Full File Contents**: Complete source files for every changed file\n\n");

            s.push_str("### How to use the enriched context\n\n");
            s.push_str("**Code Context** — Use these definitions to:\n");
            s.push_str("- Verify the PR uses APIs/types correctly (check function signatures, parameter types)\n");
            s.push_str("- Detect broken contracts (e.g., calling a function with wrong arguments, missing required fields)\n");
            s.push_str("- Identify code duplication (is the PR reimplementing something that already exists?)\n\n");
            s.push_str("**Module Health** — Use these metrics to:\n");
            s.push_str("- Flag changes to modules with low test coverage (the PR should add tests)\n");
            s.push_str("- Note existing TODOs/FIXMEs that the PR might resolve or worsen\n");
            s.push_str("- Identify unhealthy modules where extra care is warranted\n\n");
            s.push_str("**Full File Contents** — Use the complete files to:\n");
            s.push_str("- Understand the full function/class context, not just the changed lines\n");
            s.push_str("- Detect inconsistencies between the changes and surrounding code\n");
            s.push_str("- Verify error handling patterns match the rest of the file\n");
            s.push_str("- Spot duplication within the same file that the diff alone wouldn't reveal\n\n");

            s.push_str("Analyze this PR and provide a structured review. For each category below, assign a verdict:\n");
            s.push_str("- **PASS** — Meets expectations\n");
            s.push_str("- **WARNING** — Minor concerns or suggestions\n");
            s.push_str("- **FAIL** — Significant issues that should be addressed\n\n");
            s.push_str("### Categories\n\n");
            s.push_str("1. **Pattern Compliance** — Does the code follow patterns documented in .context.md? Cross-reference with Code Context definitions.\n");
            s.push_str("2. **Naming & Style** — Are naming conventions consistent with the project and existing symbol names?\n");
            s.push_str("3. **Test Coverage** — Are there tests for new functionality? Reference Module Health test ratios.\n");
            s.push_str("4. **Architecture Alignment** — Does the change fit the documented architecture? Check for API misuse using Code Context.\n");
            s.push_str("5. **Potential Issues** — Bugs, edge cases, security concerns, or broken contracts visible in diffs, Code Context, and Full File Contents.\n");
            s.push_str("6. **Code Health** — Does this PR improve or worsen the module's health? Reference Module Health metrics and Full File Contents.\n\n");

            s.push_str("**CRITICAL**: Your findings MUST be SPECIFIC — cite exact function names, line references, and concrete code from the provided context. Generic observations like 'consider adding tests' or 'looks good' are NOT acceptable at this depth level.\n\n");
        }
    }

    // Common output format — markdown + mandatory JSON schema
    s.push_str("### Output Format\n\n");
    s.push_str("Provide your analysis in two parts:\n\n");
    s.push_str("**Part 1: Detailed Analysis** (markdown)\n\n");
    s.push_str("## Summary\n[1-2 sentence overview]\n\n");
    s.push_str("## Detailed Analysis\n");
    s.push_str("One section per category with verdict and explanation.\n\n");
    s.push_str("## Recommendations\n[Specific actionable suggestions]\n\n");

    s.push_str("**Part 2: Structured Report** (mandatory JSON block)\n\n");
    s.push_str("After the markdown analysis, you MUST include a ```json fenced block with EXACTLY this schema.\n");
    s.push_str("Use EXACTLY these category names — do NOT rename, reorder, add, or remove categories:\n\n");
    s.push_str("```json\n");

    match depth {
        AnalysisDepthLevel::Minimal => {
            s.push_str(r#"{
  "overall_score": 0,
  "summary": "...",
  "categories": [
    { "name": "Code Quality", "score": 0, "status": "good", "findings_count": 0 },
    { "name": "Naming & Style", "score": 0, "status": "good", "findings_count": 0 },
    { "name": "Potential Issues", "score": 0, "status": "good", "findings_count": 0 },
    { "name": "Documentation", "score": 0, "status": "good", "findings_count": 0 }
  ],
  "findings": [
    { "title": "...", "category": "Code Quality", "severity": "warning", "description": "..." }
  ]
}"#);
        }
        _ => {
            s.push_str(r#"{
  "overall_score": 0,
  "summary": "...",
  "categories": [
    { "name": "Pattern Compliance", "score": 0, "status": "good", "findings_count": 0 },
    { "name": "Naming & Style", "score": 0, "status": "good", "findings_count": 0 },
    { "name": "Test Coverage", "score": 0, "status": "good", "findings_count": 0 },
    { "name": "Architecture Alignment", "score": 0, "status": "good", "findings_count": 0 },
    { "name": "Potential Issues", "score": 0, "status": "good", "findings_count": 0 },
    { "name": "Code Health", "score": 0, "status": "good", "findings_count": 0 }
  ],
  "findings": [
    { "title": "...", "category": "Pattern Compliance", "severity": "warning", "description": "..." }
  ]
}"#);
        }
    }

    s.push_str("\n```\n\n");
    s.push_str("Rules for the JSON:\n");
    s.push_str("- `overall_score` and `score`: integer 0-100\n");
    s.push_str("- `status`: exactly one of `\"good\"`, `\"warning\"`, `\"critical\"`\n");
    s.push_str("- `severity` in findings: exactly one of `\"info\"`, `\"warning\"`, `\"critical\"`\n");
    s.push_str("- `category` in findings must match one of the category names above\n");
    s.push_str("- Every finding must have a non-empty `description`\n\n");

    // Scoring calibration — prevent lazy all-100 outputs
    s.push_str("### Scoring Calibration\n\n");
    s.push_str("Apply these scoring guidelines strictly:\n");
    s.push_str("- **100** = Flawless. Zero concerns of any kind. Reserve this only when there is genuinely nothing to note.\n");
    s.push_str("- **85-99** = Very good. Minor stylistic or subjective observations, no real issues.\n");
    s.push_str("- **70-84** = Good with concerns. Missing tests, minor inconsistencies, or small gaps.\n");
    s.push_str("- **50-69** = Needs attention. Clear issues like missing error handling, broken patterns, or gaps.\n");
    s.push_str("- **0-49** = Significant problems. Bugs, security issues, or major architectural violations.\n\n");
    s.push_str("A score of 100 across ALL categories is extremely rare. Most real PRs have at least one area with room for improvement. ");
    s.push_str("If you give 100, you must be certain there is literally nothing to flag — not even a minor suggestion.\n\n");
    s.push_str("For each category, state at least ONE concrete observation (positive or negative) to justify the score.\n");

    s
}
