//! Text formatter (simple, no colors)

use venore_core::analysis::file_scanner::ScanResult;

use crate::commands::parse::ParseOutput;
use crate::commands::modules::ModulesOutput;

pub fn print_scan(result: &ScanResult) {
    println!("SCAN_RESULTS");
    println!("files_found: {}", result.files.len());
    println!("total_size_bytes: {}", result.total_size_bytes);
    println!("scan_duration_ms: {}", result.scan_duration_ms);
}

pub fn print_parse(scan_result: &ScanResult, parse_output: &ParseOutput) {
    println!("PARSE_RESULTS");
    println!("files_scanned: {}", scan_result.files.len());
    println!("files_parsed: {}", parse_output.results.len());
    println!("parse_errors: {}", parse_output.errors.len());

    let mut total_symbols = 0;
    for result in &parse_output.results {
        total_symbols += result.symbols.len();
    }
    println!("total_symbols: {}", total_symbols);
}

pub fn print_modules(scan_result: &ScanResult, modules_output: &ModulesOutput) {
    println!("MODULE_RESULTS");
    println!("files_scanned: {}", scan_result.files.len());
    println!("files_parsed: {}", modules_output.parse_results.len());
    println!("modules_detected: {}", modules_output.modules.modules.len());
    println!("orphan_files: {}", modules_output.modules.orphan_files.len());
    println!("detection_duration_ms: {}", modules_output.modules.detection_duration_ms);
}

pub fn print_analysis_output(analysis: &venore_core::analysis::AnalysisOutput, module_filter: &Option<String>) {
    println!("ANALYSIS_OUTPUT");
    println!("repository_name: {}", analysis.repository.name);
    if let Some(lang) = &analysis.repository.language {
        println!("repository_language: {:?}", lang);
    }
    println!("repository_technologies: {}", analysis.repository.technologies.join(","));
    println!("repository_total_files: {}", analysis.repository.total_files);
    println!("repository_total_modules: {}", analysis.repository.total_modules);

    let modules_to_show: Vec<_> = if let Some(filter) = module_filter {
        analysis.modules.iter().filter(|m| &m.name == filter).collect()
    } else {
        analysis.modules.iter().collect()
    };

    println!("modules_shown: {}", modules_to_show.len());

    for (idx, module) in modules_to_show.iter().enumerate() {
        println!("\nmodule_{}_name: {}", idx, module.name);
        println!("module_{}_path: {}", idx, module.path);
        println!("module_{}_file_count: {}", idx, module.file_count);
        if let Some(entry) = &module.entry_point {
            println!("module_{}_entry_point: {}", idx, entry);
        }
        println!("module_{}_dependencies: {}", idx, module.architecture.dependencies.join(","));
        println!("module_{}_dependents: {}", idx, module.architecture.dependents.join(","));
        println!("module_{}_external_deps: {}", idx, module.architecture.external_deps.join(","));
        println!("module_{}_total_symbols: {}", idx, module.symbols.all.len());
        println!("module_{}_exported_symbols: {}", idx, module.symbols.exports.len());
    }

    println!("\norphan_files_count: {}", analysis.orphan_files.len());
}
