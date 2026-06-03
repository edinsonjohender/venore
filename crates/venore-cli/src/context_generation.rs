//! Context Generation (Step 4)
//!
//! Generates .context.md files for selected modules using LLM.

use crate::wizard::{ProjectContext, AnalysisRules, ProjectTypeInfo};
use crate::analysis::AnalysisResult;
use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use inquire::{MultiSelect, Select};
use venore_core::analysis::{
    ScanConfig, scan_directory, ParseConfig, parse_file, Language,
    DetectorConfig, detect_modules, AnalysisConfig, AnalysisBuilder, AnalysisDepth,
    project_analyzer,
};
use venore_core::llm::prelude::*;
use venore_core::context::{ContextWriter, ContextMetadata, ModuleIdentity, ModuleType, ModuleStatus};
use venore_core::context::{Layer, LayerType, LayerStatus, Dependencies, GenerationMetadata};
use venore_core::context::calculate_code_hash;
use venore_core::infrastructure::config::MockConfigStore;
use venore_core::checkpoint::{CheckpointManager, WizardConfig};
use chrono::Utc;

#[derive(Debug)]
pub struct GenerationResult {
    pub total_selected: usize,
    pub generated: usize,
    pub failed: usize,
    pub skipped: usize,
}

/// Step 4: Generate contexts for selected modules
pub async fn step4_generate_contexts(
    project_ctx: &ProjectContext,
    rules: &AnalysisRules,
    analysis: &AnalysisResult,
    project_type_info: &ProjectTypeInfo,
) -> Result<GenerationResult> {
    println!("\n");
    println!("═══════════════════════════════════════════════════════════");
    println!("  STEP 4: GENERATE CONTEXTS");
    println!("═══════════════════════════════════════════════════════════");
    println!();

    // 1. Let user select modules to generate
    let module_options: Vec<String> = analysis
        .modules
        .iter()
        .map(|m| format!("{} ({} files)", m.name, m.file_count))
        .collect();

    if module_options.is_empty() {
        println!("❌ No modules detected. Cannot generate contexts.");
        return Ok(GenerationResult {
            total_selected: 0,
            generated: 0,
            failed: 0,
            skipped: 0,
        });
    }

    // Ask user if they want to select all or choose specific modules
    let selection_mode = Select::new(
        "How do you want to select modules?",
        vec![
            format!("✓ All modules ({})", module_options.len()),
            "⚙ Custom selection".to_string(),
        ],
    )
    .prompt()?;

    let selected_indices = if selection_mode.starts_with("✓ All") {
        // Select all modules - return all option strings
        module_options.clone()
    } else {
        // Custom selection with MultiSelect
        println!("\nSelect modules to generate contexts for:");
        println!("(Use Space to select, Enter to confirm)\n");

        let selected = MultiSelect::new("Modules:", module_options.clone())
            .with_help_message("↑↓ to move, Space to select, Enter to confirm, 'a' to select all")
            .prompt()?;

        if selected.is_empty() {
            println!("\n⚠️  No modules selected. Skipping generation.");
            return Ok(GenerationResult {
                total_selected: 0,
                generated: 0,
                failed: 0,
                skipped: 0,
            });
        }

        selected
    };

    println!("\n✓ Selected {} modules\n", selected_indices.len());

    // 1.5. Checkpoint: Check for existing checkpoint and handle resume
    let checkpoint_mgr = CheckpointManager::new(&project_ctx.project_path);

    let mut modules_to_generate = selected_indices.clone();

    if checkpoint_mgr.exists() {
        if let Ok(Some(_)) = checkpoint_mgr.load() {
            let info = checkpoint_mgr.get_info();

            println!("⚠️  Found checkpoint: {}/{} modules completed ({}%)",
                     info.completed_count, info.total_count, info.progress_percent);

            let resume = Select::new(
                "Continue from checkpoint?",
                vec!["Yes, resume", "No, start fresh"]
            ).prompt()? == "Yes, resume";

            if resume {
                let completed = checkpoint_mgr.get_completed_ids();
                modules_to_generate.retain(|m| {
                    let module_index = module_options.iter().position(|opt| opt == m).unwrap();
                    let module_name = &analysis.modules[module_index].name;
                    !completed.contains(module_name)
                });

                println!("✓ Resuming: {} modules remaining\n", modules_to_generate.len());
            } else {
                let _ = checkpoint_mgr.delete();
                println!("✓ Starting fresh generation\n");
            }
        }
    }

    // 2. Setup LLM (check for API key)
    // Load .env file
    let _ = dotenvy::dotenv();

    // Priority: Gemini > Anthropic > OpenAI
    let api_key = match std::env::var("GEMINI_API_KEY")
        .or_else(|_| std::env::var("ANTHROPIC_API_KEY"))
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
    {
        Ok(key) => key,
        Err(_) => {
            println!("❌ No API key found in .env or environment variables.");
            println!("   Set one of: GEMINI_API_KEY, ANTHROPIC_API_KEY, OPENAI_API_KEY");
            println!("   Example .env file:");
            println!("   GEMINI_API_KEY=your-key-here");
            return Ok(GenerationResult {
                total_selected: selected_indices.len(),
                generated: 0,
                failed: 0,
                skipped: selected_indices.len(),
            });
        }
    };

    let provider = if std::env::var("GEMINI_API_KEY").is_ok() {
        LlmProviderType::Gemini
    } else if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        LlmProviderType::Anthropic
    } else {
        LlmProviderType::OpenAI
    };

    let model = match provider {
        LlmProviderType::Gemini => "gemini-2.5-flash",
        LlmProviderType::Anthropic => "claude-haiku-4-5",
        LlmProviderType::OpenAI => "gpt-4.1-mini",
        LlmProviderType::Ollama => "qwen3:8b",
        LlmProviderType::Tavily => unreachable!("Tavily is not an LLM provider"),
    };

    println!("🤖 Using {} with model {}", provider.as_str(), model);
    println!();

    // Setup LLM Gateway
    let store = MockConfigStore::new();
    store.store_api_key(provider, api_key).await?;
    let gateway = LlmGateway::new(Box::new(store));

    // Test connection
    print!("🔍 Testing connection... ");
    match gateway.test_connection(provider, Some(model.to_string())).await {
        Ok(test_result) if test_result.success => {
            println!("✓ Connected ({}ms)", test_result.latency_ms);
        }
        Ok(test_result) => {
            println!("✗ Failed: {:?}", test_result.error);
            return Ok(GenerationResult {
                total_selected: selected_indices.len(),
                generated: 0,
                failed: 0,
                skipped: selected_indices.len(),
            });
        }
        Err(e) => {
            println!("✗ Error: {}", e);
            return Ok(GenerationResult {
                total_selected: selected_indices.len(),
                generated: 0,
                failed: 0,
                skipped: selected_indices.len(),
            });
        }
    }

    println!();

    // 3. Re-scan and build full analysis with depth
    println!("📊 Building full analysis...");

    let scan_config = ScanConfig {
        project_path: project_ctx.project_path.clone(),
        target_extensions: vec![
            "ts".into(), "tsx".into(), "js".into(), "jsx".into(),
            "rs".into(), "py".into(), "go".into(),
        ],
        ignore_patterns: rules.exclusions.clone(),
        max_file_size_kb: 500,
    };

    let scan_result = scan_directory(scan_config)?;

    let mut parse_results = Vec::new();
    for file in &scan_result.files {
        if let Ok(content) = std::fs::read_to_string(&file.path) {
            if let Some(language) = Language::from_extension(&file.extension) {
                if let Ok(result) = parse_file(ParseConfig {
                    file_path: file.path.clone(),
                    language,
                    content,
                }) {
                    parse_results.push(result);
                }
            }
        }
    }

    // Get detection strategy from project type info
    let detection_strategy = if project_type_info.user_confirmed {
        project_analyzer::factory::get_analyzer(project_type_info.detected_type)
            .ok()
            .map(|analyzer| analyzer.module_detection_strategy())
    } else {
        None
    };

    let detector_config = DetectorConfig {
        files: scan_result.files.clone(),
        parse_results: parse_results.clone(),
        project_root: project_ctx.project_path.clone(),
        detection_strategy,
    };

    let modules_result = detect_modules(detector_config)?;

    let depth = match &rules.depth_level {
        venore_core::context::DepthLevel::Minimal => AnalysisDepth::Minimal,
        venore_core::context::DepthLevel::Normal => AnalysisDepth::Normal,
        venore_core::context::DepthLevel::Detailed => AnalysisDepth::Detailed,
        venore_core::context::DepthLevel::Expert => AnalysisDepth::Expert,
    };

    let analysis_config = AnalysisConfig {
        scan_result: scan_result.clone(),
        parse_results,
        modules: modules_result,
        project_root: project_ctx.project_path.clone(),
        depth,
    };

    let builder = AnalysisBuilder::new(analysis_config);
    let full_analysis = builder.build();

    println!("✓ Analysis built\n");

    // 3.5. Initialize checkpoint if starting fresh
    if !checkpoint_mgr.exists() {
        let provider_name = match provider {
            LlmProviderType::Gemini => "gemini",
            LlmProviderType::Anthropic => "anthropic",
            LlmProviderType::OpenAI => "openai",
            LlmProviderType::Ollama => "ollama",
            LlmProviderType::Tavily => unreachable!("Tavily is not an LLM provider"),
        };

        let depth_level_str = match &rules.depth_level {
            venore_core::context::DepthLevel::Minimal => "Minimal",
            venore_core::context::DepthLevel::Normal => "Normal",
            venore_core::context::DepthLevel::Detailed => "Detailed",
            venore_core::context::DepthLevel::Expert => "Expert",
        };

        // Get module names from selected indices
        let module_names: Vec<String> = analysis.modules.iter()
            .map(|m| m.name.clone())
            .collect();

        let selected_module_names: Vec<String> = modules_to_generate.iter()
            .filter_map(|selected| {
                let module_index = module_options.iter().position(|opt| opt == selected)?;
                Some(analysis.modules.get(module_index)?.name.clone())
            })
            .collect();

        checkpoint_mgr.initialize_with_wizard_config(
            project_ctx.project_path.clone(),
            WizardConfig {
                // Step 1
                project_name: project_ctx.name.clone(),
                project_description: project_ctx.description.clone(),
                project_state: project_ctx.project_state.clone(),
                team_size: project_ctx.team_size.clone(),
                goals: project_ctx.goals.clone(),

                // Step 2
                depth_level: depth_level_str.to_string(),
                layers_to_generate: rules.layers_to_generate.clone(),
                exclusions: rules.exclusions.clone(),

                // Step 2.5
                project_type: project_type_info.detected_type,
                project_type_confidence: project_type_info.confidence,
                project_metadata: project_type_info.metadata.clone(),

                // Step 3
                total_files_scanned: scan_result.files.len(),
                total_modules_detected: analysis.modules.len(),
                module_names,

                // Step 4
                selected_module_names,
                llm_provider: provider_name.to_string(),
                llm_model: Some(model.to_string()),
                analysis_depth: depth,
            },
            modules_to_generate.len(),
        )?;
    }

    // 4. Generate contexts for selected modules
    let pb = ProgressBar::new(modules_to_generate.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("=>-"),
    );

    let mut generated = 0;
    let mut failed = 0;
    let mut rate_limited = false;
    let total_modules = modules_to_generate.len();

    for (idx, selected) in modules_to_generate.iter().enumerate() {
        // Find module by matching display string
        let module_index = module_options
            .iter()
            .position(|opt| opt == selected)
            .unwrap();

        let detected_module = &analysis.modules[module_index];

        let module_analysis = full_analysis
            .modules
            .iter()
            .find(|m| m.name == detected_module.name)
            .unwrap_or_else(|| {
                panic!(
                    "Module '{}' not found in full_analysis. Available modules: {:?}",
                    detected_module.name,
                    full_analysis.modules.iter().map(|m| &m.name).collect::<Vec<_>>()
                )
            });

        pb.set_message(format!("Generating {}...", module_analysis.name));

        // Build prompt
        let prompt = build_module_prompt(module_analysis, &rules.depth_level);

        // Call LLM
        let request = LlmRequest {
            model: model.to_string(),
            messages: vec![
                LlmMessage {
                    role: MessageRole::System,
                    content: "You are a code documentation expert. Generate comprehensive context documentation following the structure provided.".to_string(),
                    tool_call_id: None,
                    tool_calls: None,
                    content_parts: None,
                },
                LlmMessage {
                    role: MessageRole::User,
                    content: prompt,
                    tool_call_id: None,
                    tool_calls: None,
                    content_parts: None,
                },
            ],
            temperature: Some(0.3),
            max_tokens: Some(4000),
            tools: None,
            json_schema: None,
            timeout_secs: Some(120), // Increased from 90s to 120s for larger modules
            web_search: false,
        };

        let options = GatewayOptions::for_task(LlmTask::Analysis).with_provider(provider);

        match gateway.complete(request, options).await {
            Ok(response) => {
                // Write .context.md
                let module_path = project_ctx.project_path.join(&module_analysis.path);

                let metadata = build_metadata(
                    &module_analysis.name,
                    model,
                    provider,
                    &response,
                    &module_analysis.code_snippets,
                );

                let writer = ContextWriter::new();
                match writer.write(&module_path, &response, &metadata) {
                    Ok(_) => {
                        generated += 1;

                        // Mark module as completed in checkpoint
                        if let Err(e) = checkpoint_mgr.mark_completed(module_analysis.name.clone()) {
                            pb.println(format!("  ⚠️  Failed to save checkpoint: {}", e));
                        }
                    }
                    Err(e) => {
                        pb.println(format!("  ✗ Failed to write {}: {}", module_analysis.name, e));
                        failed += 1;
                    }
                }
            }
            Err(e) => {
                // Check if it's a rate limit error
                if e.to_string().contains("429") || e.to_string().contains("rate") || e.to_string().contains("quota") {
                    rate_limited = true;
                }
                pb.println(format!("  ✗ LLM failed for {}: {}", module_analysis.name, e));
                failed += 1;
            }
        }

        pb.inc(1);

        // Add delay between modules to avoid rate limits (except for last module)
        if idx < total_modules - 1 {
            let delay_secs = if rate_limited {
                30 // 30s delay after rate limit
            } else {
                match provider {
                    LlmProviderType::Gemini => 4,  // Gemini free: 15 req/min
                    LlmProviderType::Anthropic => 2, // Anthropic: 30 req/min
                    LlmProviderType::OpenAI => 2,   // OpenAI: 30 req/min
                    LlmProviderType::Ollama => 0,   // Ollama: local, no rate limit
                    LlmProviderType::Tavily => 0,   // Not used as LLM provider
                }
            };

            pb.set_message(format!("Waiting {}s before next module...", delay_secs));
            tokio::time::sleep(tokio::time::Duration::from_secs(delay_secs)).await;

            // Reset rate limit flag after waiting
            if rate_limited {
                rate_limited = false;
            }
        }
    }

    pb.finish_with_message("Done!");

    println!();
    println!("═══════════════════════════════════════════════════════════");
    println!("  GENERATION COMPLETE");
    println!("═══════════════════════════════════════════════════════════");
    println!();
    println!("✓ Generated: {}", generated);
    if failed > 0 {
        println!("✗ Failed: {}", failed);
    }
    println!();

    // Generate project lighthouse before cleanup
    if generated > 0 && failed == 0 {
        if let Ok(Some(checkpoint)) = checkpoint_mgr.load() {
            if let Err(e) = generate_project_lighthouse(
                &project_ctx.project_path,
                &checkpoint.wizard_config,
                generated,
                provider,
                model,
            ) {
                println!("⚠️  Failed to generate project lighthouse: {}", e);
            } else {
                println!("✓ Project lighthouse generated");
            }
        }

        // Cleanup checkpoint on successful completion
        if let Err(e) = checkpoint_mgr.delete() {
            println!("⚠️  Failed to delete checkpoint: {}", e);
        }
    }

    Ok(GenerationResult {
        total_selected: modules_to_generate.len(),
        generated,
        failed,
        skipped: 0,
    })
}

/// Build prompt for module context generation
fn build_module_prompt(
    module: &venore_core::analysis::ModuleAnalysis,
    depth: &venore_core::context::DepthLevel,
) -> String {
    let depth_str = match depth {
        venore_core::context::DepthLevel::Minimal => "minimal",
        venore_core::context::DepthLevel::Normal => "normal",
        venore_core::context::DepthLevel::Detailed => "detailed",
        venore_core::context::DepthLevel::Expert => "expert",
    };

    format!(
        r#"You are a code documentation expert. Analyze this module and generate comprehensive context documentation.

MODULE: {}
PATH: {}
FILES: {}
DEPTH: {}

ARCHITECTURE:
- Dependencies: {}
- Dependents: {}
- External: {}

EXPORTS (detected by AST analysis):
{}

NOTE: If no exports are listed above, the module may:
- Use re-exports (export * from "./file") which are not yet fully supported by the analyzer
- Be an internal/private module without public exports
- Contain only type definitions or configuration

In such cases, analyze the CODE SNIPPETS and module structure to infer the likely exports.

CODE SNIPPETS:
{}

IMPORTANT: Generate markdown content directly. Do NOT wrap your response in code blocks (no ```markdown). Your entire response should be valid markdown that starts with a heading.

Generate your response following this EXACT structure:

# {}

> **Quick Summary**: One-sentence description of what this module does

## Purpose

2-3 paragraphs explaining:
- What problem does this solve?
- Why does this module exist?
- When should you use it?

## API Reference

Organize exports into these sections (skip empty sections):

### Functions
List only exported **functions, methods, and custom hooks** (React hooks like useX, withX are functions). For each:
*   **functionName(params): ReturnType** - Brief description of what it does
*   **useCustomHook(params): ReturnType** - Brief description (hooks are functions, NOT constants)

### Components
List only React/UI **components** (functions that return JSX/elements). For each:
*   **ComponentName** - Brief description of the component's purpose

### Types & Interfaces
List only TypeScript **types, interfaces, and type aliases** (NOT components, NOT constants). For each:
*   **TypeName** - Brief description of what it represents

### Constants & Variables
List only exported **primitive constants and configuration objects** (NOT functions, NOT components, NOT hooks). For each:
*   **CONSTANT_NAME** - Brief description and example value if relevant
*   Examples: `const MAX_SIZE = 100`, `const CONFIG = {{ ... }}`, `const COLORS = [...]`

IMPORTANT: Do NOT duplicate items across sections. Each export should appear in ONLY ONE section based on what it actually is.

## Architecture

Explain:
- How does this module fit in the larger system?
- What are its dependencies and why?
- What patterns does it follow?

## Usage Examples

Provide 1-2 realistic code examples

## Notes

Any important caveats, gotchas, or best practices
"#,
        module.name,
        module.path,
        module.file_count,
        depth_str,
        module.architecture.dependencies.join(", "),
        module.architecture.dependents.join(", "),
        module.architecture.external_deps.join(", "),
        format_exports(&module.symbols.exports),
        module.code_snippets,
        module.name,
    )
}

fn format_exports(exports: &[venore_core::analysis::SymbolInfo]) -> String {
    if exports.is_empty() {
        return "None".to_string();
    }

    exports
        .iter()
        .take(20) // Limit to avoid huge prompts
        .map(|e| format!("- {} ({})", e.name, e.kind))
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_metadata(
    module_name: &str,
    model: &str,
    provider: LlmProviderType,
    response: &LlmResponse,
    code: &str,
) -> ContextMetadata {
    ContextMetadata {
        identity: ModuleIdentity {
            name: module_name.to_string(),
            module_type: ModuleType::Service,
            status: ModuleStatus::Stable,
            owner: None,
            tags: vec!["auto-generated".to_string()],
        },
        layers: vec![Layer {
            layer_type: LayerType::Context,
            status: LayerStatus::Complete,
            coverage: Some(100),
            notes: None,
        }],
        connections: vec![],
        dependencies: Dependencies {
            internal: vec![],
            external: vec![],
            optional: vec![],
        },
        agent_context: None,
        generation: GenerationMetadata {
            analyzed_at: Utc::now().to_rfc3339(),
            agent: "venore-cli-wizard".to_string(),
            model: model.to_string(),
            provider: provider.as_str().to_string(),
            code_hash: Some(calculate_code_hash(code)),
            stale: false,
            tokens_used: response.usage.as_ref().map(|u| u.total_tokens),
            generation_time_ms: None,
        },
    }
}

/// Step 4 (Resume): Generate contexts for remaining modules using existing checkpoint
pub async fn step4_generate_contexts_resume(
    project_ctx: &ProjectContext,
    rules: &AnalysisRules,
    _analysis: &AnalysisResult,
    project_type_info: &ProjectTypeInfo,
    checkpoint_mgr: CheckpointManager,
    remaining_module_names: Vec<String>,
) -> Result<GenerationResult> {
    println!();
    println!("═══════════════════════════════════════════════════════════");
    println!("  STEP 4: GENERATE CONTEXTS (RESUMED)");
    println!("═══════════════════════════════════════════════════════════");
    println!();

    if remaining_module_names.is_empty() {
        println!("✓ All modules already completed!");
        checkpoint_mgr.delete()?;
        return Ok(GenerationResult {
            total_selected: 0,
            generated: 0,
            failed: 0,
            skipped: 0,
        });
    }

    println!("✓ Resuming generation for {} remaining modules\n", remaining_module_names.len());

    // Setup LLM (load from checkpoint config)
    let _ = dotenvy::dotenv();

    let checkpoint = checkpoint_mgr.load()?.expect("Checkpoint should exist");
    let llm_provider_str = &checkpoint.wizard_config.llm_provider;
    let model = checkpoint.wizard_config.llm_model.as_deref()
        .unwrap_or("gemini-2.5-flash");

    let provider = match llm_provider_str.as_str() {
        "gemini" => LlmProviderType::Gemini,
        "anthropic" => LlmProviderType::Anthropic,
        "openai" => LlmProviderType::OpenAI,
        _ => LlmProviderType::Gemini,
    };

    let api_key = match provider {
        LlmProviderType::Gemini => std::env::var("GEMINI_API_KEY"),
        LlmProviderType::Anthropic => std::env::var("ANTHROPIC_API_KEY"),
        LlmProviderType::OpenAI => std::env::var("OPENAI_API_KEY"),
        LlmProviderType::Ollama => Ok("not-needed".to_string()), // Ollama doesn't need API key
        LlmProviderType::Tavily => unreachable!("Tavily is not an LLM provider"),
    }?;

    println!("🤖 Using {} with model {}", provider.as_str(), model);
    println!();

    let store = MockConfigStore::new();
    store.store_api_key(provider, api_key).await?;
    let gateway = LlmGateway::new(Box::new(store));

    // Test connection
    print!("🔍 Testing connection... ");
    match gateway.test_connection(provider, Some(model.to_string())).await {
        Ok(test_result) if test_result.success => {
            println!("✓ Connected ({}ms)", test_result.latency_ms);
        }
        Ok(test_result) => {
            println!("✗ Failed: {:?}", test_result.error);
            return Ok(GenerationResult {
                total_selected: remaining_module_names.len(),
                generated: 0,
                failed: 0,
                skipped: remaining_module_names.len(),
            });
        }
        Err(e) => {
            println!("✗ Error: {}", e);
            return Ok(GenerationResult {
                total_selected: remaining_module_names.len(),
                generated: 0,
                failed: 0,
                skipped: remaining_module_names.len(),
            });
        }
    }

    println!();

    // Re-scan and build full analysis
    println!("📊 Building full analysis...");

    let scan_config = ScanConfig {
        project_path: project_ctx.project_path.clone(),
        target_extensions: vec![
            "ts".into(), "tsx".into(), "js".into(), "jsx".into(),
            "rs".into(), "py".into(), "go".into(),
        ],
        ignore_patterns: rules.exclusions.clone(),
        max_file_size_kb: 500,
    };

    let scan_result = scan_directory(scan_config)?;

    let mut parse_results = Vec::new();
    for file in &scan_result.files {
        if let Ok(content) = std::fs::read_to_string(&file.path) {
            if let Some(language) = Language::from_extension(&file.extension) {
                if let Ok(result) = parse_file(ParseConfig {
                    file_path: file.path.clone(),
                    language,
                    content,
                }) {
                    parse_results.push(result);
                }
            }
        }
    }

    let detection_strategy = if project_type_info.user_confirmed {
        project_analyzer::factory::get_analyzer(project_type_info.detected_type)
            .ok()
            .map(|analyzer| analyzer.module_detection_strategy())
    } else {
        None
    };

    let detector_config = DetectorConfig {
        files: scan_result.files.clone(),
        parse_results: parse_results.clone(),
        project_root: project_ctx.project_path.clone(),
        detection_strategy,
    };

    let modules_result = detect_modules(detector_config)?;

    let depth = checkpoint.wizard_config.analysis_depth;

    let analysis_config = AnalysisConfig {
        scan_result: scan_result.clone(),
        parse_results,
        modules: modules_result,
        project_root: project_ctx.project_path.clone(),
        depth,
    };

    let builder = AnalysisBuilder::new(analysis_config);
    let full_analysis = builder.build();

    println!("✓ Analysis built\n");

    // Generate contexts
    let pb = ProgressBar::new(remaining_module_names.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("=>-"),
    );

    let mut generated = 0;
    let mut failed = 0;
    let mut rate_limited = false;

    for module_name in &remaining_module_names {
        let module_analysis = full_analysis
            .modules
            .iter()
            .find(|m| &m.name == module_name);

        // If module not found in analysis, skip it (may have been deleted)
        let module_analysis = match module_analysis {
            Some(m) => m,
            None => {
                pb.println(format!("  ⚠️  Module '{}' not found, skipping", module_name));
                failed += 1;
                pb.inc(1);
                continue;
            }
        };

        pb.set_message(format!("Generating {}...", module_analysis.name));

        let prompt = build_module_prompt(module_analysis, &rules.depth_level);

        let request = LlmRequest {
            model: model.to_string(),
            messages: vec![
                LlmMessage {
                    role: MessageRole::System,
                    content: "You are a code documentation expert. Generate comprehensive context documentation following the structure provided.".to_string(),
                    tool_call_id: None,
                    tool_calls: None,
                    content_parts: None,
                },
                LlmMessage {
                    role: MessageRole::User,
                    content: prompt,
                    tool_call_id: None,
                    tool_calls: None,
                    content_parts: None,
                },
            ],
            temperature: Some(0.3),
            max_tokens: Some(4000),
            tools: None,
            json_schema: None,
            timeout_secs: Some(120),
            web_search: false,
        };

        let options = GatewayOptions::for_task(LlmTask::Analysis).with_provider(provider);

        match gateway.complete(request, options).await {
            Ok(response) => {
                let module_path = project_ctx.project_path.join(&module_analysis.path);

                let metadata = build_metadata(
                    &module_analysis.name,
                    model,
                    provider,
                    &response,
                    &module_analysis.code_snippets,
                );

                let writer = ContextWriter::new();
                match writer.write(&module_path, &response, &metadata) {
                    Ok(_) => {
                        generated += 1;
                        if let Err(e) = checkpoint_mgr.mark_completed(module_analysis.name.clone()) {
                            pb.println(format!("  ⚠️  Failed to save checkpoint: {}", e));
                        }
                    }
                    Err(e) => {
                        pb.println(format!("  ✗ Failed to write {}: {}", module_analysis.name, e));
                        failed += 1;
                    }
                }
            }
            Err(e) => {
                if e.to_string().contains("429") || e.to_string().contains("rate limit") {
                    pb.println(format!("  ⚠️  Rate limited: {}", e));
                    failed += 1;
                    rate_limited = true;
                } else {
                    pb.println(format!("  ✗ Error: {}", e));
                    failed += 1;
                }
            }
        }

        pb.inc(1);

        if rate_limited {
            let delay_secs = 4;
            pb.set_message(format!("Waiting {}s before next module...", delay_secs));
            tokio::time::sleep(tokio::time::Duration::from_secs(delay_secs)).await;
            rate_limited = false;
        }
    }

    pb.finish_with_message("Done!");

    println!();
    println!("═══════════════════════════════════════════════════════════");
    println!("  GENERATION COMPLETE");
    println!("═══════════════════════════════════════════════════════════");
    println!();
    println!("✓ Generated: {}", generated);
    if failed > 0 {
        println!("✗ Failed: {}", failed);
    }
    println!();

    // Generate project lighthouse before cleanup
    if generated > 0 && failed == 0 {
        if let Ok(Some(checkpoint)) = checkpoint_mgr.load() {
            if let Err(e) = generate_project_lighthouse(
                &project_ctx.project_path,
                &checkpoint.wizard_config,
                generated,
                provider,
                model,
            ) {
                println!("⚠️  Failed to generate project lighthouse: {}", e);
            } else {
                println!("✓ Project lighthouse generated");
            }
        }

        // Cleanup checkpoint on successful completion
        if let Err(e) = checkpoint_mgr.delete() {
            println!("⚠️  Failed to delete checkpoint: {}", e);
        }
    }

    Ok(GenerationResult {
        total_selected: remaining_module_names.len(),
        generated,
        failed,
        skipped: 0,
    })
}

/// Generate project lighthouse file with full wizard configuration
fn generate_project_lighthouse(
    project_path: &std::path::Path,
    wizard_config: &WizardConfig,
    modules_generated: usize,
    provider: LlmProviderType,
    model: &str,
) -> Result<()> {
    use std::fs;

    // Create .context directory in project root
    let context_dir = project_path.join(".context");
    fs::create_dir_all(&context_dir)?;

    // Sanitize project name for filename
    let safe_name = wizard_config
        .project_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect::<String>();

    let lighthouse_path = context_dir.join(format!("{}.context.md", safe_name));

    // Build YAML frontmatter
    let mut frontmatter = String::from("---\n");
    frontmatter.push_str(&format!("name: \"{}\"\n", wizard_config.project_name));
    frontmatter.push_str("nodeType: lighthouse\n");
    frontmatter.push_str("isProjectRoot: true\n");
    frontmatter.push_str("status: active\n");
    frontmatter.push_str("tags: [project-config, auto-generated]\n\n");

    // Wizard Configuration
    frontmatter.push_str("wizardConfig:\n");
    frontmatter.push_str(&format!("  depth_level: \"{}\"\n", wizard_config.depth_level));
    frontmatter.push_str("  layers_to_generate:\n");
    for layer in &wizard_config.layers_to_generate {
        frontmatter.push_str(&format!("    - \"{}\"\n", layer));
    }
    let depth_str = format!("{:?}", wizard_config.analysis_depth).to_lowercase();
    frontmatter.push_str(&format!("  analysis_depth: \"{}\"\n", depth_str));
    frontmatter.push_str(&format!("  llm_provider: \"{}\"\n", wizard_config.llm_provider));
    if let Some(llm_model) = &wizard_config.llm_model {
        frontmatter.push_str(&format!("  llm_model: \"{}\"\n", llm_model));
    }
    if !wizard_config.exclusions.is_empty() {
        frontmatter.push_str("  exclusions:\n");
        for exclusion in &wizard_config.exclusions {
            frontmatter.push_str(&format!("    - \"{}\"\n", exclusion));
        }
    }

    // Project Information
    frontmatter.push_str("\nprojectInfo:\n");
    frontmatter.push_str(&format!("  description: \"{}\"\n", wizard_config.project_description));
    frontmatter.push_str(&format!("  state: \"{}\"\n", wizard_config.project_state));
    frontmatter.push_str(&format!("  team_size: \"{}\"\n", wizard_config.team_size));
    if !wizard_config.goals.is_empty() {
        frontmatter.push_str("  goals:\n");
        for goal in &wizard_config.goals {
            frontmatter.push_str(&format!("    - \"{}\"\n", goal));
        }
    }

    // Project Type
    frontmatter.push_str("\nprojectType:\n");
    frontmatter.push_str(&format!("  type: \"{:?}\"\n", wizard_config.project_type));
    frontmatter.push_str(&format!("  confidence: {:.2}\n", wizard_config.project_type_confidence));
    if !wizard_config.project_metadata.is_empty() {
        frontmatter.push_str("  metadata:\n");
        for (key, value) in &wizard_config.project_metadata {
            frontmatter.push_str(&format!("    {}: \"{}\"\n", key, value));
        }
    }

    // Stats
    frontmatter.push_str("\nstats:\n");
    frontmatter.push_str(&format!("  totalModules: {}\n", wizard_config.total_modules_detected));
    frontmatter.push_str(&format!("  selectedModules: {}\n", wizard_config.selected_module_names.len()));
    frontmatter.push_str(&format!("  generatedModules: {}\n", modules_generated));
    frontmatter.push_str(&format!("  filesScanned: {}\n", wizard_config.total_files_scanned));
    frontmatter.push_str(&format!("  generatedAt: \"{}\"\n", Utc::now().to_rfc3339()));

    // Analysis Metadata
    frontmatter.push_str("\nanalyzed:\n");
    frontmatter.push_str(&format!("  at: \"{}\"\n", Utc::now().to_rfc3339()));
    frontmatter.push_str("  agent: \"venore-cli-wizard\"\n");
    frontmatter.push_str(&format!("  model: \"{}\"\n", model));
    frontmatter.push_str(&format!("  provider: \"{}\"\n", provider.as_str()));
    frontmatter.push_str("  stale: false\n");
    frontmatter.push_str("---\n\n");

    // Build markdown content
    let mut content = frontmatter;
    content.push_str(&format!("# {}\n\n", wizard_config.project_name));
    content.push_str("> **Project Lighthouse**: Configuration and metadata for this project's context generation.\n\n");

    content.push_str("## Overview\n\n");
    content.push_str(&format!("{}\n\n", wizard_config.project_description));
    content.push_str(&format!("**Project State**: {}\n\n", wizard_config.project_state));
    content.push_str(&format!("**Team Size**: {}\n\n", wizard_config.team_size));

    if !wizard_config.goals.is_empty() {
        content.push_str("**Goals**:\n");
        for goal in &wizard_config.goals {
            content.push_str(&format!("- {}\n", goal));
        }
        content.push('\n');
    }

    content.push_str("## Configuration Used\n\n");
    content.push_str("This project was analyzed using the following configuration:\n\n");
    content.push_str(&format!("- **Depth Level**: {}\n", wizard_config.depth_level));
    content.push_str(&format!("- **Analysis Depth**: {:?}\n", wizard_config.analysis_depth));
    content.push_str(&format!("- **Layers**: {}\n", wizard_config.layers_to_generate.join(", ")));
    content.push_str(&format!(
        "- **LLM Provider**: {} ({})\n",
        wizard_config.llm_provider,
        wizard_config.llm_model.as_deref().unwrap_or("default model")
    ));

    if !wizard_config.exclusions.is_empty() {
        content.push_str(&format!("- **Exclusions**: {}\n", wizard_config.exclusions.join(", ")));
    }
    content.push('\n');

    content.push_str("## Project Statistics\n\n");
    content.push_str(&format!("- **Total Modules Detected**: {}\n", wizard_config.total_modules_detected));
    content.push_str(&format!("- **Modules Selected**: {}\n", wizard_config.selected_module_names.len()));
    content.push_str(&format!("- **Modules Generated**: {}\n", modules_generated));
    content.push_str(&format!("- **Files Scanned**: {}\n", wizard_config.total_files_scanned));
    content.push_str(&format!("- **Generation Date**: {}\n\n", Utc::now().format("%Y-%m-%d")));

    content.push_str("## Project Type\n\n");
    content.push_str(&format!(
        "**Type**: {:?} ({:.0}% confidence)\n\n",
        wizard_config.project_type,
        wizard_config.project_type_confidence * 100.0
    ));

    if !wizard_config.project_metadata.is_empty() {
        content.push_str("**Detected Metadata**:\n");
        for (key, value) in &wizard_config.project_metadata {
            content.push_str(&format!("- {}: {}\n", key, value));
        }
        content.push('\n');
    }

    content.push_str("## Modules Generated\n\n");
    for (i, module_name) in wizard_config.selected_module_names.iter().enumerate() {
        content.push_str(&format!("{}. {}\n", i + 1, module_name));
    }
    content.push('\n');

    content.push_str("---\n");
    content.push_str(&format!(
        "<!-- Generated by Venore venore-cli-wizard using {} -->\n",
        model
    ));

    // Write file
    fs::write(&lighthouse_path, content)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use venore_core::analysis::AnalysisDepth;
    use venore_core::analysis::project_analyzer::traits::ProjectType;

    #[test]
    fn test_generate_project_lighthouse() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        let mut metadata = HashMap::new();
        metadata.insert("framework".to_string(), "React".to_string());
        metadata.insert("package_manager".to_string(), "npm".to_string());

        let wizard_config = WizardConfig {
            project_name: "Test Project".to_string(),
            project_description: "A test project for lighthouse generation".to_string(),
            project_state: "Active development".to_string(),
            team_size: "5-10 developers".to_string(),
            goals: vec![
                "Understanding legacy code".to_string(),
                "Maintaining living docs".to_string(),
            ],
            depth_level: "Normal".to_string(),
            layers_to_generate: vec!["Basic Context".to_string()],
            exclusions: vec!["node_modules".to_string(), "dist".to_string()],
            project_type: ProjectType::NodeMonorepo,
            project_type_confidence: 0.95,
            project_metadata: metadata,
            total_files_scanned: 150,
            total_modules_detected: 5,
            module_names: vec![
                "components".to_string(),
                "hooks".to_string(),
                "utils".to_string(),
            ],
            selected_module_names: vec![
                "components".to_string(),
                "hooks".to_string(),
                "utils".to_string(),
            ],
            llm_provider: "gemini".to_string(),
            llm_model: Some("gemini-2.5-flash".to_string()),
            analysis_depth: AnalysisDepth::Normal,
        };

        let result = generate_project_lighthouse(
            project_path,
            &wizard_config,
            3,
            LlmProviderType::Gemini,
            "gemini-2.5-flash",
        );

        assert!(result.is_ok(), "Failed to generate lighthouse: {:?}", result.err());

        // Verify file was created
        let lighthouse_path = project_path.join(".context/Test_Project.context.md");
        assert!(lighthouse_path.exists(), "Lighthouse file was not created");

        // Verify content
        let content = std::fs::read_to_string(&lighthouse_path).unwrap();

        // Check frontmatter
        assert!(content.contains("---"), "Missing YAML frontmatter");
        assert!(content.contains("name: \"Test Project\""), "Missing project name");
        assert!(content.contains("nodeType: lighthouse"), "Missing lighthouse type");
        assert!(content.contains("isProjectRoot: true"), "Missing isProjectRoot flag");

        // Check wizard config
        assert!(content.contains("wizardConfig:"), "Missing wizardConfig section");
        assert!(content.contains("depth_level: \"Normal\""), "Missing depth_level");
        assert!(content.contains("llm_provider: \"gemini\""), "Missing llm_provider");
        assert!(content.contains("analysis_depth: \"normal\""), "Missing analysis_depth");

        // Check project info
        assert!(content.contains("projectInfo:"), "Missing projectInfo section");
        assert!(content.contains("description: \"A test project for lighthouse generation\""));
        assert!(content.contains("state: \"Active development\""));

        // Check project type
        assert!(content.contains("projectType:"), "Missing projectType section");
        assert!(content.contains("type: \"NodeMonorepo\""), "Missing project type");
        assert!(content.contains("confidence: 0.95"), "Missing confidence");

        // Check stats
        assert!(content.contains("stats:"), "Missing stats section");
        assert!(content.contains("totalModules: 5"), "Missing totalModules");
        assert!(content.contains("generatedModules: 3"), "Missing generatedModules");
        assert!(content.contains("filesScanned: 150"), "Missing filesScanned");

        // Check markdown content
        assert!(content.contains("# Test Project"), "Missing markdown title");
        assert!(content.contains("## Overview"), "Missing Overview section");
        assert!(content.contains("## Configuration Used"), "Missing Configuration section");
        assert!(content.contains("## Project Statistics"), "Missing Statistics section");
        assert!(content.contains("## Modules Generated"), "Missing Modules section");

        println!("✓ Lighthouse generated successfully at: {}", lighthouse_path.display());
    }
}
