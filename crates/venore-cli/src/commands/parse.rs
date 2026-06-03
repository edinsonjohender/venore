//! Parse command - File Scanner + AST Parser (TASK-001 + TASK-002)

use anyhow::Result;
use std::path::PathBuf;
use venore_core::analysis::file_scanner::{scan_directory, ScanConfig};
use venore_core::analysis::ast_parser::{parse_file, ParseConfig, Language, ParseResult};

use crate::cli::OutputFormat;
use crate::formatter;

pub struct ParseArgs {
    pub path: PathBuf,
    pub format: OutputFormat,
    pub extensions: Vec<String>,
    pub ignore: Vec<String>,
}

pub struct ParseOutput {
    pub results: Vec<ParseResult>,
    pub errors: Vec<ParseError>,
}

pub struct ParseError {
    pub file: PathBuf,
    pub error: String,
}

pub fn run(args: ParseArgs) -> Result<()> {
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
        // Read file content
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

        // Detect language from extension
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

    let output = ParseOutput {
        results: parse_results,
        errors: parse_errors,
    };

    // Format output
    match args.format {
        OutputFormat::Pretty => formatter::pretty::print_parse(&scan_result, &output),
        OutputFormat::Text => formatter::text::print_parse(&scan_result, &output),
        OutputFormat::Json => formatter::json::print_parse(&scan_result, &output)?,
    }

    Ok(())
}
