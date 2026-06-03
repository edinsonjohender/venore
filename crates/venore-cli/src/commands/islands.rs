//! Islands command - Detect logical groupings of modules
//!
//! This command:
//! 1. Scans the project
//! 2. Parses AST
//! 3. Detects modules
//! 4. Detects islands (sub-groups) based on:
//!    - Path-based clustering
//!    - Dependency cohesion
//!    - Criticality scores

use anyhow::Result;
use std::path::PathBuf;
use venore_core::analysis::file_scanner::{scan_directory, ScanConfig};
use venore_core::analysis::ast_parser::{parse_file, ParseConfig, Language};
use venore_core::analysis::module_detector::{detect_modules, DetectorConfig};
use venore_core::analysis::analysis_output::{AnalysisBuilder, AnalysisConfig, AnalysisDepth};
use venore_core::analysis::island_detector::{
    detect_islands, IslandDetectorConfig, IslandParams, IslandDetectionResult,
};

use crate::cli::IslandOutputFormat;

pub struct IslandsArgs {
    pub path: PathBuf,
    pub format: IslandOutputFormat,
    pub extensions: Vec<String>,
    pub ignore: Vec<String>,
    pub min_modules: usize,
    pub depth: usize,
    pub cohesion: f32,
    pub critical: usize,
}

pub fn run(args: IslandsArgs) -> Result<()> {
    // 1. Scan files
    println!("🔍 Scanning project...");
    let scan_config = ScanConfig {
        project_path: args.path.clone(),
        target_extensions: args.extensions,
        ignore_patterns: args.ignore,
        max_file_size_kb: 500,
    };

    let scan_result = scan_directory(scan_config)?;
    println!("   Found {} files", scan_result.files.len());

    // 2. Parse AST
    println!("📝 Parsing files...");
    let mut parse_results = Vec::new();

    for file in &scan_result.files {
        let content = match std::fs::read_to_string(&file.path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        if let Some(language) = Language::from_extension(&file.extension) {
            let config = ParseConfig {
                file_path: file.path.clone(),
                language,
                content,
            };

            if let Ok(result) = parse_file(config) {
                parse_results.push(result);
            }
        }
    }
    println!("   Parsed {} files", parse_results.len());

    // 3. Detect modules
    println!("📦 Detecting modules...");
    let detector_config = DetectorConfig {
        files: scan_result.files.clone(),
        parse_results: parse_results.clone(),
        project_root: args.path.clone(),
        detection_strategy: None,
    };

    let modules_result = detect_modules(detector_config)?;
    println!("   Found {} modules", modules_result.modules.len());

    // 4. Build full analysis (to get ModuleAnalysis with dependencies)
    println!("🔗 Building dependency graph...");
    let analysis_config = AnalysisConfig {
        scan_result,
        parse_results,
        modules: modules_result,
        project_root: args.path.clone(),
        depth: AnalysisDepth::Normal, // Default depth for CLI
    };

    let full_analysis = AnalysisBuilder::new(analysis_config).build();
    println!("   Analyzed {} modules", full_analysis.modules.len());

    // 5. Detect islands
    println!("🏝️  Detecting islands...");
    let island_config = IslandDetectorConfig {
        modules: full_analysis.modules,
        params: IslandParams {
            min_modules: args.min_modules,
            max_depth: args.depth,
            cohesion_threshold: args.cohesion,
            weight_threshold: 3,
            dependency_score: args.critical,
        },
    };

    let result = detect_islands(island_config)?;
    println!("   Found {} islands\n", result.islands.len());

    // 6. Format output
    match args.format {
        IslandOutputFormat::Text => print_text(&result),
        IslandOutputFormat::Json => print_json(&result)?,
        IslandOutputFormat::Markdown => print_markdown(&result),
    }

    Ok(())
}

fn print_text(result: &IslandDetectionResult) {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🏝️  Island Detection Results");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
    println!("📊 Summary:");
    println!("   Total modules analyzed: {}", result.metrics.total_modules);
    println!("   Islands detected: {}", result.metrics.islands_detected);
    println!(
        "   Average cohesion: {:.1}%",
        result.metrics.avg_cohesion * 100.0
    );
    println!(
        "   Critical modules: {}",
        result.metrics.critical_modules.len()
    );
    println!();

    if result.islands.is_empty() {
        println!("⚠️  No islands detected with current parameters.");
        println!("   Try adjusting --min-modules, --cohesion, or --depth");
        return;
    }

    println!("🏝️  Detected Islands:");
    println!();

    for (i, island) in result.islands.iter().enumerate() {
        println!(
            "{}. {} ({} modules)",
            i + 1,
            island.name,
            island.weight
        );
        println!("   Cohesion: {:.1}%", island.cohesion * 100.0);
        println!("   Criticality: {} avg dependencies", island.criticality);
        println!("   Description: {}", island.description);
        println!("   Modules:");
        for module in &island.modules {
            println!("     • {}", module);
        }
        println!();
    }

    if !result.metrics.critical_modules.is_empty() {
        println!("⚠️  Critical Modules (high dependencies):");
        for module in &result.metrics.critical_modules {
            println!("   • {}", module);
        }
        println!();
    }
}

fn print_json(result: &IslandDetectionResult) -> Result<()> {
    let json = serde_json::to_string_pretty(result)?;
    println!("{}", json);
    Ok(())
}

fn print_markdown(result: &IslandDetectionResult) {
    println!("# Island Detection Results\n");
    println!("## Summary\n");
    println!(
        "- **Total modules analyzed**: {}",
        result.metrics.total_modules
    );
    println!(
        "- **Islands detected**: {}",
        result.metrics.islands_detected
    );
    println!(
        "- **Average cohesion**: {:.1}%",
        result.metrics.avg_cohesion * 100.0
    );
    println!(
        "- **Critical modules**: {}\n",
        result.metrics.critical_modules.len()
    );

    if result.islands.is_empty() {
        println!("⚠️ **No islands detected** with current parameters.\n");
        println!("Try adjusting `--min-modules`, `--cohesion`, or `--depth`\n");
        return;
    }

    println!("## Detected Islands\n");

    for (i, island) in result.islands.iter().enumerate() {
        println!(
            "### {}. {} ({} modules)\n",
            i + 1,
            island.name,
            island.weight
        );
        println!("- **Cohesion**: {:.1}%", island.cohesion * 100.0);
        println!("- **Criticality**: {}", island.criticality);
        println!("- **Description**: {}\n", island.description);
        println!("**Modules**:\n");
        for module in &island.modules {
            println!("- `{}`", module);
        }
        println!();
    }

    if !result.metrics.critical_modules.is_empty() {
        println!("## Critical Modules\n");
        println!("Modules with high incoming dependencies:\n");
        for module in &result.metrics.critical_modules {
            println!("- `{}`", module);
        }
        println!();
    }
}
