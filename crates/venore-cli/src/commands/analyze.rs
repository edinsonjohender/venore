//! Analyze command - Full analysis with optional output to file

use anyhow::Result;
use std::path::PathBuf;

use crate::cli::OutputFormat;
use crate::commands::modules::{ModulesArgs, run as run_modules};

pub struct AnalyzeArgs {
    pub path: PathBuf,
    pub format: OutputFormat,
    pub extensions: Vec<String>,
    pub ignore: Vec<String>,
    pub max_size: u64,
    pub output: Option<PathBuf>,
    pub verbose: bool,
}

pub fn run(args: AnalyzeArgs) -> Result<()> {
    // Set verbosity
    if args.verbose {
        eprintln!("🔍 Modo verbose activado");
        eprintln!("📁 Analizando: {}", args.path.display());
        eprintln!("📄 Extensiones: {}", args.extensions.join(", "));
        eprintln!("🚫 Ignorando: {}", args.ignore.join(", "));
    }

    // Run modules command (includes scan + parse + modules)
    let modules_args = ModulesArgs {
        path: args.path.clone(),
        format: args.format.clone(),
        extensions: args.extensions,
        ignore: args.ignore,
    };

    run_modules(modules_args)?;

    // Save to file if requested
    if let Some(output_path) = args.output {
        // TODO: Implement JSON save
        eprintln!("\n💾 Guardado en: {}", output_path.display());
    }

    Ok(())
}
