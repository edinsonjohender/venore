//! Reusable analysis pipeline
//!
//! Provides a single `run_analysis()` function that runs the full
//! scan → parse → detect → build pipeline and persists the result.

use std::path::PathBuf;

use crate::error::Result;
use crate::analysis::analysis_output::{AnalysisBuilder, AnalysisConfig, AnalysisDepth, AnalysisOutput};
use crate::analysis::ast_parser::{Language, ParseConfig, parse_file};
use crate::analysis::file_scanner::{ScanConfig, scan_directory};
use crate::analysis::module_detector::{DetectorConfig, detect_modules};
use crate::analysis::project_analyzer;

/// Configuration for `run_analysis`.
#[derive(Debug, Clone)]
pub struct RunAnalysisConfig {
    pub project_path: PathBuf,
    pub target_extensions: Vec<String>,
    pub ignore_patterns: Vec<String>,
    pub max_file_size_kb: u64,
    pub depth: AnalysisDepth,
}

impl Default for RunAnalysisConfig {
    fn default() -> Self {
        Self {
            project_path: PathBuf::new(),
            target_extensions: Language::all_extensions()
                .iter()
                .map(|s| s.to_string())
                .collect(),
            ignore_patterns: vec![
                "node_modules", "dist", ".git", "target", ".venore",
                "__pycache__", ".next", "build", "coverage", ".turbo",
            ].into_iter().map(String::from).collect(),
            max_file_size_kb: 1024,
            depth: AnalysisDepth::Normal,
        }
    }
}

/// Run the full analysis pipeline: scan → parse → detect project type →
/// detect modules → build output → save to disk.
///
/// Returns the produced `AnalysisOutput` (also persisted at
/// `.venore/analysis-output.json`).
pub async fn run_analysis(config: RunAnalysisConfig) -> Result<AnalysisOutput> {
    let project_path = &config.project_path;
    tracing::info!("run_analysis: starting for {}", project_path.display());

    // 1. Scan directory
    let scan_config = ScanConfig {
        project_path: project_path.clone(),
        target_extensions: config.target_extensions.clone(),
        ignore_patterns: config.ignore_patterns.clone(),
        max_file_size_kb: config.max_file_size_kb,
    };

    let scan_result = scan_directory(scan_config)?;
    tracing::info!("run_analysis: scanned {} files", scan_result.files.len());

    // 2. Parse each file with AST parser
    let mut parse_results = Vec::new();
    for file_info in &scan_result.files {
        let language = Language::from_extension(&file_info.extension);
        let language = match language {
            Some(lang) => lang,
            None => continue, // skip files without AST support
        };

        let content = match std::fs::read_to_string(&file_info.path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        match parse_file(ParseConfig {
            file_path: file_info.path.clone(),
            language,
            content,
        }) {
            Ok(pr) => parse_results.push(pr),
            Err(e) => {
                tracing::debug!("parse error for {}: {}", file_info.path.display(), e);
            }
        }
    }

    tracing::info!("run_analysis: parsed {} files", parse_results.len());

    // 3. Detect project type → get module detection strategy
    let detection = project_analyzer::detect_project_type(project_path).await?;
    let strategy = project_analyzer::get_analyzer(detection.project_type)
        .ok()
        .map(|a| a.module_detection_strategy());

    tracing::info!(
        "run_analysis: project type {:?} (confidence {:.0}%)",
        detection.project_type,
        detection.confidence * 100.0
    );

    // 4. Detect modules
    let detector_config = DetectorConfig {
        files: scan_result.files.clone(),
        parse_results: parse_results.clone(),
        project_root: project_path.clone(),
        detection_strategy: strategy,
    };

    let modules = detect_modules(detector_config)?;
    tracing::info!("run_analysis: detected {} modules", modules.modules.len());

    // 5. Build analysis output
    let analysis_config = AnalysisConfig {
        scan_result,
        parse_results,
        modules,
        project_root: project_path.clone(),
        depth: config.depth,
    };

    let analysis = AnalysisBuilder::new(analysis_config).build();

    // 6. Persist to disk
    analysis.save_to_disk(project_path)?;
    tracing::info!("run_analysis: saved analysis output to disk");

    Ok(analysis)
}
