//! JSON formatter

use anyhow::Result;
use serde_json::json;
use venore_core::analysis::file_scanner::ScanResult;

use crate::commands::parse::ParseOutput;
use crate::commands::modules::ModulesOutput;

pub fn print_scan(result: &ScanResult) -> Result<()> {
    let output = json!({
        "files_found": result.files.len(),
        "total_size_bytes": result.total_size_bytes,
        "scan_duration_ms": result.scan_duration_ms,
        "files": result.files,
    });

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

pub fn print_parse(scan_result: &ScanResult, parse_output: &ParseOutput) -> Result<()> {
    let output = json!({
        "files_scanned": scan_result.files.len(),
        "files_parsed": parse_output.results.len(),
        "parse_errors": parse_output.errors.len(),
        "scan_duration_ms": scan_result.scan_duration_ms,
        "results": parse_output.results,
    });

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

pub fn print_modules(scan_result: &ScanResult, modules_output: &ModulesOutput) -> Result<()> {
    let output = json!({
        "files_scanned": scan_result.files.len(),
        "files_parsed": modules_output.parse_results.len(),
        "modules_detected": modules_output.modules.modules.len(),
        "orphan_files": modules_output.modules.orphan_files.len(),
        "scan_duration_ms": scan_result.scan_duration_ms,
        "detection_duration_ms": modules_output.modules.detection_duration_ms,
        "modules": modules_output.modules,
    });

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
