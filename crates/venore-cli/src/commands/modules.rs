//! Modules command - Complete pipeline (TASK-001 + TASK-002 + TASK-003)

use anyhow::Result;
use std::path::PathBuf;
use venore_core::analysis::file_scanner::{scan_directory, ScanConfig};
use venore_core::analysis::ast_parser::{parse_file, ParseConfig, Language, ParseResult};
use venore_core::analysis::module_detector::{detect_modules, DetectorConfig, ModuleDetectionResult};

use crate::cli::OutputFormat;
use crate::formatter;
use crate::commands::parse::ParseError;

pub struct ModulesArgs {
    pub path: PathBuf,
    pub format: OutputFormat,
    pub extensions: Vec<String>,
    pub ignore: Vec<String>,
}

pub struct ModulesOutput {
    pub parse_results: Vec<ParseResult>,
    pub parse_errors: Vec<ParseError>,
    pub modules: ModuleDetectionResult,
}

pub fn run(args: ModulesArgs) -> Result<()> {
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
    let mut parse_errors = Vec::new();

    for file in &scan_result.files {
        let content = match std::fs::read_to_string(&file.path) {
            Ok(c) => c,
            Err(e) => {
                parse_errors.push(ParseError {
                    file: file.path.clone(),
                    error: format!("Failed to read file: {}", e),
                });
                continue;
            }
        };

        if let Some(language) = Language::from_extension(&file.extension) {
            let config = ParseConfig {
                file_path: file.path.clone(),
                language,
                content,
            };

            match parse_file(config) {
                Ok(result) => parse_results.push(result),
                Err(e) => {
                    parse_errors.push(ParseError {
                        file: file.path.clone(),
                        error: e.to_string(),
                    });
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

    let output = ModulesOutput {
        parse_results,
        parse_errors,
        modules: modules_result,
    };

    // Format output
    match args.format {
        OutputFormat::Pretty => formatter::pretty::print_modules(&scan_result, &output),
        OutputFormat::Text => formatter::text::print_modules(&scan_result, &output),
        OutputFormat::Json => formatter::json::print_modules(&scan_result, &output)?,
    }

    Ok(())
}
