//! Interactive wizard command (V1-style)

use crate::wizard as wiz;
use crate::analysis;
use crate::context_generation;
use venore_core::checkpoint::CheckpointManager;
use std::path::PathBuf;

pub fn run() -> anyhow::Result<()> {
    // Initialize async runtime for this command
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async_run())
}

async fn async_run() -> anyhow::Result<()> {
    use console::style;
    use inquire::{Confirm, Select};

    println!("\n{}", style("╔═══════════════════════════════════════════════════╗").cyan());
    println!("{}", style("║           Context Agent - Venore V2              ║").cyan());
    println!("{}", style("╚═══════════════════════════════════════════════════╝").cyan());

    // Step 0: Get project path (not counted as a visible step)
    let project_path = wiz::step0_project_path()?;

    // Check for existing checkpoint immediately after getting project path
    let checkpoint_mgr = CheckpointManager::new(&project_path);

    if checkpoint_mgr.exists() {
        if let Ok(Some(checkpoint)) = checkpoint_mgr.load() {
            // If checkpoint exists in .venore/, it belongs to this project
            let info = checkpoint_mgr.get_info();

            println!();
            println!("{}", style("⚠️  Found checkpoint from previous session").yellow().bold());
            println!();
            println!("  Project: {}", style(&checkpoint.wizard_config.project_name).cyan());
            println!("  Progress: {}/{} modules completed ({}%)",
                     info.completed_count,
                     info.total_count,
                     info.progress_percent);
            println!("  Last run: {}", checkpoint.last_updated_at.format("%Y-%m-%d %H:%M"));
            println!();

            let resume_choice = Select::new(
                "Resume from checkpoint?",
                vec!["Yes, resume generation", "No, start fresh"]
            ).prompt()?;

            if resume_choice == "Yes, resume generation" {
                return resume_from_checkpoint(checkpoint_mgr, checkpoint, project_path.clone()).await;
            } else {
                checkpoint_mgr.delete()?;
                println!("\n{}", style("✓ Starting fresh wizard").green());
            }
        }
    }

    // Normal wizard flow (Steps 1-5)
    // Step 1: Project Context (additional info beyond path)
    println!("\n{}", style("Step 1 of 5: Project Context").green().bold());
    let project_context = wiz::step1_project_context(project_path)?;

    // Step 2: Analysis Rules
    println!("\n{}", style("Step 2 of 5: Analysis Rules").green().bold());
    let analysis_rules = wiz::step2_analysis_rules()?;

    // Step 2.5: Project Type Detection
    println!("\n{}", style("Step 3 of 5: Project Type Detection").green().bold());
    let project_type_info = wiz::step2_5_detect_project_type(&project_context.project_path).await?;

    // Step 3: Analysis
    println!("\n{}", style("Step 4 of 5: Analyzing project...").green().bold());
    let result = analysis::step3_analyze(
        &project_context,
        &analysis_rules,
        &project_type_info
    ).await?;

    // Display results
    println!("\n{}", style("═══════════════════════════════════════════════════").cyan());
    println!("{}", style("Analysis Results").green().bold());
    println!("{}", style("═══════════════════════════════════════════════════").cyan());
    println!("✓ Files: {}", style(result.total_files).yellow());
    println!("✓ Modules: {}", style(result.modules.len()).yellow());

    println!("\n{}", style("Detected modules:").bold());
    for (i, module) in result.modules.iter().enumerate().take(10) {
        println!("  {}. {} ({} files) [{}]",
            i + 1,
            style(&module.name).cyan(),
            module.file_count,
            style(format!("{:?}", module.confidence)).dim()
        );
    }

    if result.modules.len() > 10 {
        println!("  ... and {} more", result.modules.len() - 10);
    }

    // Ask if user wants to generate contexts
    println!();
    let generate = Confirm::new("Would you like to generate .context.md files for modules?")
        .with_default(true)
        .with_help_message("This will use an LLM (requires API key in environment)")
        .prompt()?;

    if generate {
        // Step 4: Generate Contexts
        println!("\n{}", style("Step 5 of 5: Generate Contexts").green().bold());
        let generation_result = context_generation::step4_generate_contexts(
            &project_context,
            &analysis_rules,
            &result,
            &project_type_info,
        ).await?;

        println!("{}", style("═══════════════════════════════════════════════════").cyan());
        println!("{}", style("Wizard Complete!").green().bold());
        println!("{}", style("═══════════════════════════════════════════════════").cyan());
        println!("✓ Contexts generated: {}", style(generation_result.generated).green());

        if generation_result.failed > 0 {
            println!("✗ Failed: {}", style(generation_result.failed).red());
        }
    } else {
        println!("\n{}", style("═══════════════════════════════════════════════════").cyan());
        println!("{}", style("Analysis Complete!").green().bold());
        println!("{}", style("═══════════════════════════════════════════════════").cyan());
        println!("⚠️  Context generation skipped");
    }

    println!();

    Ok(())
}

/// Resume wizard from checkpoint (skip Steps 1-3, go directly to generation)
async fn resume_from_checkpoint(
    checkpoint_mgr: CheckpointManager,
    checkpoint: venore_core::checkpoint::Checkpoint,
    actual_project_path: PathBuf,
) -> anyhow::Result<()> {
    use console::style;
    use venore_core::context::DepthLevel;

    println!("\n{}", style("✓ Resuming from checkpoint...").green());

    // Reconstruct ProjectContext from checkpoint using ACTUAL project path
    // (not the one saved in checkpoint, as project may have been moved)
    let project_context = wiz::ProjectContext {
        project_path: actual_project_path,
        name: checkpoint.wizard_config.project_name.clone(),
        description: checkpoint.wizard_config.project_description.clone(),
        project_state: checkpoint.wizard_config.project_state.clone(),
        team_size: checkpoint.wizard_config.team_size.clone(),
        goals: checkpoint.wizard_config.goals.clone(),
    };

    // Reconstruct AnalysisRules from checkpoint
    let depth_level = match checkpoint.wizard_config.depth_level.as_str() {
        "Minimal" => DepthLevel::Minimal,
        "Normal" => DepthLevel::Normal,
        "Detailed" => DepthLevel::Detailed,
        "Expert" => DepthLevel::Expert,
        _ => DepthLevel::Normal,
    };

    let analysis_rules = wiz::AnalysisRules {
        depth_level,
        layers_to_generate: checkpoint.wizard_config.layers_to_generate.clone(),
        exclusions: checkpoint.wizard_config.exclusions.clone(),
    };

    // Reconstruct ProjectTypeInfo from checkpoint
    let project_type_info = wiz::ProjectTypeInfo {
        detected_type: checkpoint.wizard_config.project_type,
        confidence: checkpoint.wizard_config.project_type_confidence,
        evidence: vec![], // Not needed for resume
        metadata: checkpoint.wizard_config.project_metadata.clone(),
        user_confirmed: true,
    };

    // Get selected modules from checkpoint (don't re-detect, use saved selection)
    let selected_module_names = &checkpoint.wizard_config.selected_module_names;
    let completed_ids = &checkpoint.completed_module_ids;

    // Filter to get remaining modules
    let remaining_module_names: Vec<String> = selected_module_names.iter()
        .filter(|name| !completed_ids.contains(name))
        .cloned()
        .collect();

    println!("\n{}", style("═══════════════════════════════════════════════════").cyan());
    println!("{}", style("Resume Summary").green().bold());
    println!("{}", style("═══════════════════════════════════════════════════").cyan());
    println!("✓ Completed: {}/{} modules", completed_ids.len(), selected_module_names.len());
    println!("✓ Remaining: {} modules", remaining_module_names.len());
    println!();

    if remaining_module_names.is_empty() {
        println!("{}", style("🎉 All modules already completed!").green().bold());
        checkpoint_mgr.delete()?;
        return Ok(());
    }

    // Now re-analyze to get fresh module content for the remaining modules
    println!("{}", style("📊 Re-analyzing project...").dim());
    let result = analysis::step3_analyze(
        &project_context,
        &analysis_rules,
        &project_type_info
    ).await?;

    // Continue with generation (Step 5)
    println!("\n{}", style("Step 5 of 5: Generate Contexts (resumed)").green().bold());
    let generation_result = context_generation::step4_generate_contexts_resume(
        &project_context,
        &analysis_rules,
        &result,
        &project_type_info,
        checkpoint_mgr,
        remaining_module_names,
    ).await?;

    println!("{}", style("═══════════════════════════════════════════════════").cyan());
    println!("{}", style("Wizard Complete!").green().bold());
    println!("{}", style("═══════════════════════════════════════════════════").cyan());
    println!("✓ Contexts generated: {}", style(generation_result.generated).green());

    if generation_result.failed > 0 {
        println!("✗ Failed: {}", style(generation_result.failed).red());
    }

    println!();

    Ok(())
}
