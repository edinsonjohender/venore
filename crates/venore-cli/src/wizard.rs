//! Wizard steps for CLI interaction

use inquire::{Select, Text, MultiSelect};
use std::path::{Path, PathBuf};
use venore_core::context::DepthLevel;
use venore_core::analysis::project_analyzer::{factory, traits::ProjectType};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Step 1: Project Context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectContext {
    pub project_path: PathBuf,
    pub name: String,
    pub description: String,
    pub project_state: String,
    pub team_size: String,
    pub goals: Vec<String>,
}

/// Step 0: Get project path only (for checkpoint detection)
pub fn step0_project_path() -> anyhow::Result<PathBuf> {
    println!();

    let current_dir = std::env::current_dir()?;
    let project_path = Text::new("Project path:")
        .with_default(&current_dir.display().to_string())
        .prompt()?;

    Ok(PathBuf::from(project_path))
}

/// Step 2: Analysis Rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisRules {
    pub depth_level: DepthLevel,
    pub layers_to_generate: Vec<String>,
    pub exclusions: Vec<String>,
}

/// Step 2.5: Project Type Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTypeInfo {
    pub detected_type: ProjectType,
    pub confidence: f32,
    pub evidence: Vec<String>,
    pub metadata: HashMap<String, String>,
    pub user_confirmed: bool,
}

pub fn step1_project_context(project_path: PathBuf) -> anyhow::Result<ProjectContext> {
    println!();

    // Project name (auto-suggest from path)
    let name = Text::new("Project name:")
        .with_default("my-project")
        .prompt()?;

    // Description
    let description = Text::new("Tell me about your project:")
        .with_help_message("Brief description of what this project does")
        .prompt()?;

    // Project state
    let project_state = Select::new(
        "Project state:",
        vec![
            "New (< 3 months)",
            "Active development",
            "Maintenance",
            "Legacy"
        ]
    ).prompt()?;

    // Team size
    let team_size = Select::new(
        "Team size:",
        vec![
            "Just me",
            "2-5 people",
            "6-15 people",
            "15+ people"
        ]
    ).prompt()?;

    // Goals
    let goals = MultiSelect::new(
        "Goals with Venore:",
        vec![
            "Onboarding new members",
            "Understanding legacy code",
            "Planning refactor",
            "Client documentation",
            "Architecture audit",
            "Maintaining living docs"
        ]
    ).prompt()?;

    Ok(ProjectContext {
        project_path,
        name: name.to_string(),
        description: description.to_string(),
        project_state: project_state.to_string(),
        team_size: team_size.to_string(),
        goals: goals.into_iter().map(|s| s.to_string()).collect(),
    })
}

pub fn step2_analysis_rules() -> anyhow::Result<AnalysisRules> {
    println!();

    // Depth level
    let depth_options = vec![
        "Minimal - Essential files only (~500-800 tokens)",
        "Normal - Standard depth (~1.5-2K tokens) [DEFAULT]",
        "Detailed - Comprehensive analysis (~3-4K tokens)",
        "Expert - Maximum depth (~5-8K tokens)"
    ];

    let depth_choice = Select::new("Depth level:", depth_options).prompt()?;

    let depth_level = match depth_choice {
        s if s.starts_with("Minimal") => DepthLevel::Minimal,
        s if s.starts_with("Normal") => DepthLevel::Normal,
        s if s.starts_with("Detailed") => DepthLevel::Detailed,
        s if s.starts_with("Expert") => DepthLevel::Expert,
        _ => DepthLevel::Normal,
    };

    // Layers to generate
    let layer_options = vec![
        "Basic Context (required)",
        "Status",
        "Connections",
        "Tests",
        "Documentation"
    ];

    let layers = MultiSelect::new("Layers to generate:", layer_options)
        .with_default(&[0]) // Basic Context pre-selected
        .prompt()?;

    // Exclusions
    let exclusions_input = Text::new("Exclusions (comma-separated):")
        .with_default("node_modules,dist,.git,build,target")
        .with_help_message("Folders/patterns to ignore during analysis")
        .prompt()?;

    let exclusions: Vec<String> = exclusions_input
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    Ok(AnalysisRules {
        depth_level,
        layers_to_generate: layers.into_iter().map(|s| s.to_string()).collect(),
        exclusions,
    })
}

/// Step 2.5: Detect Project Type
pub async fn step2_5_detect_project_type(
    project_path: &Path
) -> anyhow::Result<ProjectTypeInfo> {
    use console::style;

    println!();
    println!("{}", style("Detecting project type...").dim());

    // 1. Auto-detect project type
    let detection = factory::detect_project_type(project_path).await?;

    // 2. Display detection result
    println!();
    println!("{}", style("Detection Result:").bold());

    let type_display = detection.project_type.display_name();

    println!("  Type: {}", style(type_display).cyan().bold());
    println!("  Confidence: {}%", style(format!("{:.0}", detection.confidence * 100.0)).yellow());

    if !detection.evidence.is_empty() {
        println!();
        println!("{}", style("Evidence:").bold());
        for evidence in &detection.evidence {
            println!("  • {}", evidence);
        }
    }

    if !detection.metadata.is_empty() {
        println!();
        println!("{}", style("Metadata:").bold());
        for (key, value) in &detection.metadata {
            println!("  {} → {}", key, style(value).cyan());
        }
    }

    // 3. If confidence < 80%, ask for confirmation
    let user_confirmed = if detection.confidence < 0.8 {
        println!();
        println!("{}", style("⚠️  Low confidence detection").yellow());

        let confirm = Select::new(
            "Is this project type correct?",
            vec!["Yes, continue", "No, use fallback detection"]
        ).prompt()?;

        confirm == "Yes, continue"
    } else {
        println!();
        println!("{}", style("✓ Project type confirmed").green());
        true
    };

    Ok(ProjectTypeInfo {
        detected_type: detection.project_type,
        confidence: detection.confidence,
        evidence: detection.evidence,
        metadata: detection.metadata,
        user_confirmed,
    })
}
