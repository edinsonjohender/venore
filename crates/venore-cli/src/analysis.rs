//! Analysis execution (Step 3)

use crate::wizard::{ProjectContext, AnalysisRules, ProjectTypeInfo};
use venore_core::analysis::{file_scanner, ast_parser, module_detector, project_analyzer};
use indicatif::{ProgressBar, ProgressStyle};

#[derive(Debug)]
pub struct AnalysisResult {
    pub total_files: usize,
    pub modules: Vec<DetectedModule>,
    pub scan_time_ms: u64,
}

#[derive(Debug)]
pub struct DetectedModule {
    pub name: String,
    pub path: String,
    pub file_count: usize,
    pub confidence: Confidence,
    pub has_existing_context: bool,
}

#[derive(Debug)]
pub enum Confidence {
    High,
    Medium,
    Low,
}

pub async fn step3_analyze(
    context: &ProjectContext,
    rules: &AnalysisRules,
    project_type_info: &ProjectTypeInfo
) -> anyhow::Result<AnalysisResult> {
    let start_time = std::time::Instant::now();

    // Progress bar
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap()
    );
    spinner.set_message("Scanning files...");

    // 1. Scan files
    let scan_config = file_scanner::ScanConfig {
        project_path: context.project_path.clone(),
        target_extensions: vec![
            "ts".into(), "tsx".into(), "js".into(), "jsx".into(),
            "rs".into(), "py".into(), "go".into()
        ],
        ignore_patterns: rules.exclusions.clone(),
        max_file_size_kb: 1024, // 1MB
    };

    let scan_result = file_scanner::scan_directory(scan_config)?;
    spinner.set_message(format!("Found {} files, parsing...", scan_result.files.len()));

    // 2. Parse files
    let parse_results: Vec<_> = scan_result
        .files
        .iter()
        .filter_map(|file| {
            let content = std::fs::read_to_string(&file.path).ok()?;
            let language = ast_parser::Language::from_extension(&file.extension)?;

            let config = ast_parser::ParseConfig {
                file_path: file.path.clone(),
                language,
                content,
            };

            ast_parser::parse_file(config).ok()
        })
        .collect();

    spinner.set_message("Detecting modules...");

    // 3. Get detection strategy from project analyzer
    let detection_strategy = if project_type_info.user_confirmed {
        // User confirmed detection, use the strategy
        project_analyzer::factory::get_analyzer(project_type_info.detected_type)
            .ok()
            .map(|analyzer| analyzer.module_detection_strategy())
    } else {
        // User rejected detection or low confidence, use fallback
        None
    };

    // 4. Detect modules
    let detector_config = module_detector::DetectorConfig {
        files: scan_result.files.clone(),
        parse_results,
        project_root: context.project_path.clone(),
        detection_strategy,
    };

    let detection_result = module_detector::detect_modules(detector_config)?;

    spinner.finish_with_message(format!("✓ Analysis complete in {:.2}s", start_time.elapsed().as_secs_f32()));

    // Convert to CLI result format
    let detected_modules: Vec<DetectedModule> = detection_result
        .modules
        .into_iter()
        .map(|m| DetectedModule {
            name: m.name,
            path: m.path.display().to_string(),
            file_count: m.files.len(),
            confidence: Confidence::High, // TODO: Calculate based on entry_point existence
            has_existing_context: false,  // TODO: Check for .context.md files
        })
        .collect();

    Ok(AnalysisResult {
        total_files: scan_result.files.len(),
        modules: detected_modules,
        scan_time_ms: start_time.elapsed().as_millis() as u64,
    })
}
