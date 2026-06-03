//! Pretty formatter with colors and emojis

use colored::*;
use std::collections::HashMap;
use venore_core::analysis::file_scanner::ScanResult;
use venore_core::analysis::ast_parser::SymbolKind;

use crate::commands::parse::ParseOutput;
use crate::commands::modules::ModulesOutput;

pub fn print_scan(result: &ScanResult) {
    println!("\n{}", "═══════════════════════════════════".cyan());
    println!("{}", "  📁 FILE SCAN".green().bold());
    println!("{}", "═══════════════════════════════════".cyan());

    println!("\n   Found: {}", result.files.len().to_string().yellow());
    println!("   Total size: {}", format_bytes(result.total_size_bytes).yellow());
    println!("   Time: {}", format_duration(result.scan_duration_ms).yellow());

    // Group by extension
    let mut by_ext: HashMap<String, usize> = HashMap::new();
    for file in &result.files {
        *by_ext.entry(file.extension.clone()).or_insert(0) += 1;
    }

    if !by_ext.is_empty() {
        println!("\n   {} By extension:", "📊".cyan());
        let mut sorted: Vec<_> = by_ext.iter().collect();
        sorted.sort_by_key(|(_, count)| std::cmp::Reverse(**count));

        for (ext, count) in sorted {
            let percentage = (*count as f32 / result.files.len() as f32) * 100.0;
            let ext_str = if ext.is_empty() {
                "(no extension)".to_string()
            } else {
                format!(".{}", ext)
            };
            println!(
                "      {} : {} files ({:.1}%)",
                ext_str.cyan(),
                count.to_string().yellow(),
                percentage
            );
        }
    }

    println!("\n{}", "═══════════════════════════════════".cyan());
}

pub fn print_parse(scan_result: &ScanResult, parse_output: &ParseOutput) {
    println!("\n{}", "═══════════════════════════════════".cyan());
    println!("{}", "  🔍 AST PARSE".green().bold());
    println!("{}", "═══════════════════════════════════".cyan());

    println!("\n   Files scanned: {}", scan_result.files.len().to_string().yellow());
    println!("   Files parsed: {}", parse_output.results.len().to_string().yellow());

    // Count symbols by kind
    let mut functions = 0;
    let mut classes = 0;
    let mut interfaces = 0;
    let mut enums = 0;
    let mut types = 0;

    for result in &parse_output.results {
        for symbol in &result.symbols {
            match symbol.kind {
                SymbolKind::Function => functions += 1,
                SymbolKind::Class => classes += 1,
                SymbolKind::Interface => interfaces += 1,
                SymbolKind::Enum => enums += 1,
                SymbolKind::Type => types += 1,
                _ => {}
            }
        }
    }

    println!("\n   {} Symbols extracted:", "✨".cyan());
    println!("      Functions  : {}", functions.to_string().yellow());
    println!("      Classes    : {}", classes.to_string().yellow());
    println!("      Interfaces : {}", interfaces.to_string().yellow());
    println!("      Enums      : {}", enums.to_string().yellow());
    println!("      Types      : {}", types.to_string().yellow());

    // Show parse errors
    if !parse_output.errors.is_empty() {
        println!("\n   {} Parse errors: {}", "⚠️".yellow(), parse_output.errors.len().to_string().red());
        for error in parse_output.errors.iter().take(5) {
            println!(
                "      {} {}",
                "✗".red(),
                error.file.display().to_string().dimmed()
            );
            println!("        {}", error.error.dimmed());
        }
        if parse_output.errors.len() > 5 {
            println!("      {} ... and {} more", "...".dimmed(), (parse_output.errors.len() - 5).to_string().dimmed());
        }
    }

    println!("\n{}", "═══════════════════════════════════".cyan());
}

pub fn print_modules(scan_result: &ScanResult, modules_output: &ModulesOutput) {
    println!("\n{}", "═══════════════════════════════════".cyan());
    println!("{}", "  VENORE CONTEXT GENERATOR".cyan().bold());
    println!("{}", "═══════════════════════════════════".cyan());

    // Scan info
    println!("\n{} {}", "📁".green(), "SCAN".green().bold());
    println!("   Files: {}", scan_result.files.len().to_string().yellow());
    println!("   Size: {}", format_bytes(scan_result.total_size_bytes).yellow());

    // Parse info
    println!("\n{} {}", "🔍".green(), "PARSE".green().bold());
    println!("   Parsed: {}", modules_output.parse_results.len().to_string().yellow());

    let mut total_symbols = 0;
    for result in &modules_output.parse_results {
        total_symbols += result.symbols.len();
    }
    println!("   Symbols: {}", total_symbols.to_string().yellow());

    // Modules info
    println!("\n{} {}", "📦".green(), "MODULES".green().bold());
    println!("   Detected: {}", modules_output.modules.modules.len().to_string().yellow());
    println!("   Orphans: {}", modules_output.modules.orphan_files.len().to_string().yellow());

    // List modules
    if !modules_output.modules.modules.is_empty() {
        println!("\n   Modules (sorted by size):");

        let mut modules = modules_output.modules.modules.clone();
        modules.sort_by_key(|m| std::cmp::Reverse(m.files.len()));

        for module in modules.iter().take(10) {
            println!("\n   {} {}", "📦".cyan(), module.name.bold());

            if let Some(entry) = &module.entry_point {
                println!(
                    "      Entry: {}",
                    entry.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .dimmed()
                );
            }

            println!("      Files: {}", module.files.len().to_string().yellow());

            if !module.dependencies.is_empty() {
                println!(
                    "      {} {}",
                    "→".cyan(),
                    module.dependencies.join(", ").cyan()
                );
            }
        }

        if modules.len() > 10 {
            println!("\n   {} ... and {} more modules", "...".dimmed(), (modules.len() - 10).to_string().dimmed());
        }
    }

    // Orphan files
    if !modules_output.modules.orphan_files.is_empty() {
        println!("\n   {} Orphan files: {}", "📄".yellow(), modules_output.modules.orphan_files.len().to_string().yellow());
        for orphan in modules_output.modules.orphan_files.iter().take(5) {
            println!(
                "      {}",
                orphan.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .dimmed()
            );
        }
        if modules_output.modules.orphan_files.len() > 5 {
            println!("      {} ... and {} more", "...".dimmed(), (modules_output.modules.orphan_files.len() - 5).to_string().dimmed());
        }
    }

    // Footer
    let total_time = scan_result.scan_duration_ms + modules_output.modules.detection_duration_ms;
    println!("\n{}", "═══════════════════════════════════".cyan());
    println!("   {} Completed in {}", "✓".green(), format_duration(total_time).green());
    println!("{}", "═══════════════════════════════════".cyan());
    println!();
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn format_duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{}ms", ms)
    } else {
        format!("{:.1}s", ms as f64 / 1000.0)
    }
}

pub fn print_analysis_output(analysis: &venore_core::analysis::AnalysisOutput, module_filter: &Option<String>) {
    println!("\n{}", "═══════════════════════════════════".cyan());
    println!("{}", "  📊 ANALYSIS OUTPUT".green().bold());
    println!("{}", "═══════════════════════════════════".cyan());

    // Repository info
    println!("\n{} {}", "📁".green(), "REPOSITORY".green().bold());
    println!("   Name: {}", analysis.repository.name.yellow());
    if let Some(lang) = &analysis.repository.language {
        println!("   Language: {}", format!("{:?}", lang).yellow());
    }
    if !analysis.repository.technologies.is_empty() {
        println!("   Technologies: {}", analysis.repository.technologies.join(", ").yellow());
    }
    println!("   Total files: {}", analysis.repository.total_files.to_string().yellow());
    println!("   Total modules: {}", analysis.repository.total_modules.to_string().yellow());

    // Modules
    let modules_to_show: Vec<_> = if let Some(filter) = module_filter {
        analysis.modules.iter().filter(|m| &m.name == filter).collect()
    } else {
        analysis.modules.iter().collect()
    };

    if modules_to_show.is_empty() {
        if module_filter.is_some() {
            println!("\n{} Module not found", "❌".red());
        }
        return;
    }

    println!("\n{} {} (showing {})", "📦".green(), "MODULES".green().bold(), modules_to_show.len().to_string().yellow());

    for module in modules_to_show.iter().take(5) {
        println!("\n   {} {}", "📦".cyan(), module.name.bold());
        println!("      Path: {}", module.path.dimmed());
        println!("      Files: {}", module.file_count.to_string().yellow());

        if let Some(entry) = &module.entry_point {
            println!("      Entry: {}", entry.cyan());
        }

        // Architecture
        if !module.architecture.dependencies.is_empty() {
            println!("      Dependencies: {}", module.architecture.dependencies.join(", ").cyan());
        }
        if !module.architecture.dependents.is_empty() {
            println!("      Dependents: {}", module.architecture.dependents.join(", ").green());
        }
        if !module.architecture.external_deps.is_empty() {
            println!("      External: {}", module.architecture.external_deps.join(", ").magenta());
        }

        // Symbols
        println!("      Symbols: {} total, {} exported",
            module.symbols.all.len().to_string().yellow(),
            module.symbols.exports.len().to_string().green()
        );

        if !module.symbols.exports.is_empty() && module.symbols.exports.len() <= 5 {
            println!("      Exports:");
            for exp in &module.symbols.exports {
                println!("         • {} ({})", exp.name.green(), exp.kind.dimmed());
            }
        }

        // Imports
        if !module.imports.is_empty() {
            println!("      Imports: {} total", module.imports.len().to_string().yellow());
            if module.imports.len() <= 10 {
                for imp in &module.imports {
                    if imp.items.is_empty() {
                        println!("         • {} (default)", imp.module.blue());
                    } else {
                        println!("         • {}: {}", imp.module.blue(), imp.items.join(", ").dimmed());
                    }
                }
            }
        }
    }

    if modules_to_show.len() > 5 {
        println!("\n   {} Showing first 5 of {} modules", "ℹ️".blue(), modules_to_show.len().to_string().yellow());
        println!("   {}", "Use --module <name> to see a specific module".dimmed());
    }

    // Orphan files
    if !analysis.orphan_files.is_empty() {
        println!("\n{} {} ({})", "📄".yellow(), "ORPHAN FILES".yellow().bold(), analysis.orphan_files.len().to_string().yellow());
        for file in analysis.orphan_files.iter().take(10) {
            println!("      • {}", file.dimmed());
        }
        if analysis.orphan_files.len() > 10 {
            println!("      ... and {} more", (analysis.orphan_files.len() - 10).to_string().dimmed());
        }
    }

    println!("\n{}", "═══════════════════════════════════".cyan());
}
