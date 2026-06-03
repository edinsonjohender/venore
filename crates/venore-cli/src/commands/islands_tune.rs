//! Islands Tune command - Find optimal island detection parameters
//!
//! This command:
//! 1. Scans and analyzes the project once
//! 2. Tests multiple parameter combinations
//! 3. Compares results in a table
//! 4. Recommends the best configuration

use anyhow::Result;
use std::path::PathBuf;
use std::time::Instant;
use venore_core::analysis::file_scanner::{scan_directory, ScanConfig};
use venore_core::analysis::ast_parser::{parse_file, ParseConfig, Language};
use venore_core::analysis::module_detector::{detect_modules, DetectorConfig};
use venore_core::analysis::analysis_output::{AnalysisBuilder, AnalysisConfig, AnalysisDepth};
use venore_core::analysis::island_detector::{
    detect_islands, IslandDetectorConfig, IslandParams, IslandDetectionResult,
};
use serde::Serialize;

pub struct IslandsTuneArgs {
    pub path: PathBuf,
    pub extensions: Vec<String>,
    pub ignore: Vec<String>,
    pub output: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
struct ConfigResult {
    // Parameters
    min_modules: usize,
    max_depth: usize,
    cohesion_threshold: f32,

    // Results
    islands_found: usize,
    avg_cohesion: f32,
    total_modules_in_islands: usize,
    critical_modules: usize,

    // Computed metrics
    coverage: f32, // % of modules in islands
    score: f32,    // Overall quality score
}

pub fn run(args: IslandsTuneArgs) -> Result<()> {
    println!("🔧 Islands Parameter Tuning");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();

    // Step 1: Analyze project once
    println!("📊 Analyzing project (this will take a moment)...");
    let start = Instant::now();

    let scan_config = ScanConfig {
        project_path: args.path.clone(),
        target_extensions: args.extensions,
        ignore_patterns: args.ignore,
        max_file_size_kb: 500,
    };

    let scan_result = scan_directory(scan_config)?;
    println!("   ✓ Scanned {} files", scan_result.files.len());

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
    println!("   ✓ Parsed {} files", parse_results.len());

    let detector_config = DetectorConfig {
        files: scan_result.files.clone(),
        parse_results: parse_results.clone(),
        project_root: args.path.clone(),
        detection_strategy: None,
    };

    let modules_result = detect_modules(detector_config)?;
    let total_modules = modules_result.modules.len();
    println!("   ✓ Detected {} modules", total_modules);

    let analysis_config = AnalysisConfig {
        scan_result,
        parse_results,
        modules: modules_result,
        project_root: args.path.clone(),
        depth: AnalysisDepth::Normal,
    };

    let full_analysis = AnalysisBuilder::new(analysis_config).build();
    println!("   ✓ Built dependency graph");
    println!("   ⏱️  Analysis took {:.2}s", start.elapsed().as_secs_f32());
    println!();

    // Step 2: Test multiple configurations
    println!("🧪 Testing parameter combinations...");
    println!();

    let cohesion_thresholds = vec![0.0, 0.05, 0.1, 0.2, 0.3, 0.5];
    let min_modules_values = vec![2, 3, 4, 5];
    let max_depth_values = vec![1, 2, 3];

    let mut results: Vec<ConfigResult> = Vec::new();
    let mut test_count = 0;
    let total_tests = cohesion_thresholds.len() * min_modules_values.len() * max_depth_values.len();

    for &cohesion in &cohesion_thresholds {
        for &min_mods in &min_modules_values {
            for &depth in &max_depth_values {
                test_count += 1;

                // Print progress every 10 tests
                if test_count % 10 == 0 || test_count == 1 {
                    print!("\r   Testing config {}/{} ", test_count, total_tests);
                    use std::io::Write;
                    std::io::stdout().flush().ok();
                }

                let config = IslandDetectorConfig {
                    modules: full_analysis.modules.clone(),
                    params: IslandParams {
                        min_modules: min_mods,
                        max_depth: depth,
                        cohesion_threshold: cohesion,
                        weight_threshold: 3,
                        dependency_score: 3,
                    },
                };

                if let Ok(result) = detect_islands(config) {
                    let coverage = calculate_coverage(&result, total_modules);
                    let score = calculate_score(&result, coverage, total_modules);

                    results.push(ConfigResult {
                        min_modules: min_mods,
                        max_depth: depth,
                        cohesion_threshold: cohesion,
                        islands_found: result.islands.len(),
                        avg_cohesion: result.metrics.avg_cohesion,
                        total_modules_in_islands: result.islands.iter().map(|i| i.weight).sum(),
                        critical_modules: result.metrics.critical_modules.len(),
                        coverage,
                        score,
                    });
                }
            }
        }
    }

    println!("\r   ✓ Tested {} configurations", test_count);
    println!();

    // Step 3: Sort by score and display results
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

    // Display top 10 configurations
    println!("🏆 Top 10 Configurations (ranked by quality score):");
    println!();
    println!("┌────┬───────┬───────┬──────────┬────────┬──────────┬──────────┬───────┐");
    println!("│Rank│MinMods│ Depth │ Cohesion │Islands │AvgCohes% │ Coverage │ Score │");
    println!("├────┼───────┼───────┼──────────┼────────┼──────────┼──────────┼───────┤");

    for (i, result) in results.iter().take(10).enumerate() {
        println!(
            "│{:^4}│{:^7}│{:^7}│{:^10.2}│{:^8}│{:^10.1}│{:^10.1}│{:^7.2}│",
            i + 1,
            result.min_modules,
            result.max_depth,
            result.cohesion_threshold,
            result.islands_found,
            result.avg_cohesion * 100.0,
            result.coverage,
            result.score
        );
    }
    println!("└────┴───────┴───────┴──────────┴────────┴──────────┴──────────┴───────┘");
    println!();

    // Step 4: Recommend best configuration
    if let Some(best) = results.first() {
        println!("✨ Recommended Configuration:");
        println!();
        println!("   --min-modules {}      # Minimum modules per island", best.min_modules);
        println!("   --depth {}             # Path clustering depth", best.max_depth);
        println!("   --cohesion {}        # Cohesion threshold", best.cohesion_threshold);
        println!();
        println!("   Expected results:");
        println!("   • {} islands detected", best.islands_found);
        println!("   • {:.1}% average cohesion", best.avg_cohesion * 100.0);
        println!("   • {:.1}% module coverage ({}/{})",
            best.coverage,
            best.total_modules_in_islands,
            total_modules
        );
        println!();
        println!("   Run with:");
        println!("   venore islands --min-modules {} --depth {} --cohesion {:.2} {}",
            best.min_modules,
            best.max_depth,
            best.cohesion_threshold,
            args.path.display()
        );
        println!();
    }

    // Step 5: Save to file if requested
    if let Some(output_path) = args.output {
        let json = serde_json::to_string_pretty(&results)?;
        std::fs::write(&output_path, json)?;
        println!("💾 Full results saved to: {}", output_path.display());
        println!();
    }

    Ok(())
}

/// Calculate what % of total modules are included in islands
fn calculate_coverage(result: &IslandDetectionResult, total_modules: usize) -> f32 {
    if total_modules == 0 {
        return 0.0;
    }
    let modules_in_islands: usize = result.islands.iter().map(|i| i.weight).sum();
    (modules_in_islands as f32 / total_modules as f32) * 100.0
}

/// Calculate overall quality score for a configuration
///
/// Scoring formula:
/// - 40% weight on coverage (we want most modules in islands)
/// - 30% weight on cohesion (higher is better)
/// - 20% weight on island count (sweet spot: 3-8 islands)
/// - 10% weight on critical module detection
fn calculate_score(result: &IslandDetectionResult, coverage: f32, total_modules: usize) -> f32 {
    // Coverage score (0-40)
    let coverage_score = (coverage / 100.0) * 40.0;

    // Cohesion score (0-30)
    let cohesion_score = result.metrics.avg_cohesion * 30.0;

    // Island count score (0-20)
    // Sweet spot: 3-8 islands
    let island_score = if result.islands.is_empty() {
        0.0
    } else if result.islands.len() >= 3 && result.islands.len() <= 8 {
        20.0 // Perfect range
    } else if result.islands.len() < 3 {
        (result.islands.len() as f32 / 3.0) * 20.0 // Too few
    } else {
        20.0 * (8.0 / result.islands.len() as f32) // Too many
    };

    // Critical modules score (0-10)
    let critical_ratio = result.metrics.critical_modules.len() as f32 / total_modules as f32;
    let critical_score = if critical_ratio > 0.0 && critical_ratio < 0.3 {
        10.0 // Good: some critical modules but not too many
    } else if critical_ratio == 0.0 {
        5.0 // OK: no critical modules detected
    } else {
        5.0 // Too many critical modules
    };

    coverage_score + cohesion_score + island_score + critical_score
}
