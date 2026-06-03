//! Scan command - File Scanner (TASK-001)

use anyhow::Result;
use std::path::PathBuf;
use venore_core::analysis::file_scanner::{scan_directory, ScanConfig};

use crate::cli::OutputFormat;
use crate::formatter;

pub struct ScanArgs {
    pub path: PathBuf,
    pub format: OutputFormat,
    pub extensions: Vec<String>,
    pub ignore: Vec<String>,
    pub max_size: u64,
}

pub fn run(args: ScanArgs) -> Result<()> {
    // Configure scanner
    let config = ScanConfig {
        project_path: args.path.clone(),
        target_extensions: args.extensions,
        ignore_patterns: args.ignore,
        max_file_size_kb: args.max_size,
    };

    // Run scanner
    let result = scan_directory(config)?;

    // Format output
    match args.format {
        OutputFormat::Pretty => formatter::pretty::print_scan(&result),
        OutputFormat::Text => formatter::text::print_scan(&result),
        OutputFormat::Json => formatter::json::print_scan(&result)?,
    }

    Ok(())
}
