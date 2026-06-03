//! Analysis Output command - Shows consolidated analysis structure
//!
//! This command generates the complete analysis structure that will be
//! passed to the LLM in the future (TASK-004).
//!
//! NO AI/LLM integration - just shows the prepared data.

use anyhow::Result;
use std::path::PathBuf;
use venore_core::analysis::{
    ScanConfig, scan_directory,
    ParseConfig, parse_file, Language,
    DetectorConfig, detect_modules,
    AnalysisConfig, AnalysisBuilder, AnalysisDepth,
};
use crate::cli::{OutputFormat, AnalysisDepthArg};
use crate::formatter;

pub struct AnalysisOutputArgs {
    pub path: PathBuf,
    pub format: OutputFormat,
    pub extensions: Vec<String>,
    pub ignore: Vec<String>,
    pub module: Option<String>,
    pub depth: AnalysisDepthArg,
}

impl From<AnalysisDepthArg> for AnalysisDepth {
    fn from(arg: AnalysisDepthArg) -> Self {
        match arg {
            AnalysisDepthArg::Minimal => AnalysisDepth::Minimal,
            AnalysisDepthArg::Normal => AnalysisDepth::Normal,
            AnalysisDepthArg::Detailed => AnalysisDepth::Detailed,
            AnalysisDepthArg::Expert => AnalysisDepth::Expert,
        }
    }
}

pub fn run(args: AnalysisOutputArgs) -> Result<()> {
    // 1. Scan files
    let scan_config = ScanConfig {
        project_path: args.path.clone(),
        target_extensions: args.extensions,
        ignore_patterns: args.ignore,
        max_file_size_kb: 500,
    };

    let scan_result = scan_directory(scan_config)?;

    // 2. Parse files
    let mut parse_results = Vec::new();

    for file in &scan_result.files {
        let content = std::fs::read_to_string(&file.path)?;

        if let Some(language) = Language::from_extension(&file.extension) {
            match parse_file(ParseConfig {
                file_path: file.path.clone(),
                language,
                content,
            }) {
                Ok(result) => parse_results.push(result),
                Err(_) => {
                    // Skip files that fail to parse
                }
            }
        }
    }

    // 3. Detect modules
    let detector_config = DetectorConfig {
        files: scan_result.files.clone(),
        parse_results: parse_results.clone(),
        project_root: args.path.clone(),
        detection_strategy: None,
    };

    let modules_result = detect_modules(detector_config)?;

    // 4. Build analysis output
    let analysis_config = AnalysisConfig {
        scan_result: scan_result.clone(),
        parse_results,
        modules: modules_result,
        project_root: args.path,
        depth: args.depth.into(),
    };

    let builder = AnalysisBuilder::new(analysis_config);
    let analysis = builder.build();

    // 5. Output
    match args.format {
        OutputFormat::Json => {
            if let Some(module_name) = args.module {
                // Show only specific module
                if let Some(module) = analysis.modules.iter().find(|m| m.name == module_name) {
                    println!("{}", serde_json::to_string_pretty(module)?);
                } else {
                    eprintln!("Module '{}' not found", module_name);
                    std::process::exit(1);
                }
            } else {
                // Show complete analysis
                println!("{}", serde_json::to_string_pretty(&analysis)?);
            }
        }
        OutputFormat::Pretty => {
            formatter::pretty::print_analysis_output(&analysis, &args.module);
        }
        OutputFormat::Text => {
            formatter::text::print_analysis_output(&analysis, &args.module);
        }
    }

    Ok(())
}
