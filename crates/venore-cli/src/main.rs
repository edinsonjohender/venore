//! Venore CLI - Context Generator Testing Tool

use clap::Parser;
use cli::{Cli, Commands, parse_extensions, parse_ignore};
use commands::*;

mod cli;
mod commands;
mod formatter;
mod wizard;
mod analysis;
mod context_generation;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Analyze {
            path,
            format,
            extensions,
            ignore,
            max_size,
            output,
            verbose,
        } => {
            let args = AnalyzeArgs {
                path,
                format,
                extensions: parse_extensions(&extensions),
                ignore: parse_ignore(&ignore),
                max_size,
                output,
                verbose,
            };
            analyze::run(args)?;
        }

        Commands::Scan {
            path,
            format,
            extensions,
            ignore,
            max_size,
        } => {
            let args = ScanArgs {
                path,
                format,
                extensions: parse_extensions(&extensions),
                ignore: parse_ignore(&ignore),
                max_size,
            };
            scan::run(args)?;
        }

        Commands::Parse {
            path,
            format,
            extensions,
            ignore,
        } => {
            let args = ParseArgs {
                path,
                format,
                extensions: parse_extensions(&extensions),
                ignore: parse_ignore(&ignore),
            };
            parse::run(args)?;
        }

        Commands::Modules {
            path,
            format,
            extensions,
            ignore,
        } => {
            let args = ModulesArgs {
                path,
                format,
                extensions: parse_extensions(&extensions),
                ignore: parse_ignore(&ignore),
            };
            modules::run(args)?;
        }

        Commands::AnalysisOutput {
            path,
            format,
            extensions,
            ignore,
            module,
            depth,
        } => {
            let args = AnalysisOutputArgs {
                path,
                format,
                extensions: parse_extensions(&extensions),
                ignore: parse_ignore(&ignore),
                module,
                depth,
            };
            analysis_output::run(args)?;
        }

        Commands::Wizard => {
            commands::wizard::run()?;
        }

        Commands::Islands {
            path,
            format,
            extensions,
            ignore,
            min_modules,
            depth,
            cohesion,
            critical,
        } => {
            let args = IslandsArgs {
                path,
                format,
                extensions: parse_extensions(&extensions),
                ignore: parse_ignore(&ignore),
                min_modules,
                depth,
                cohesion,
                critical,
            };
            commands::islands::run(args)?;
        }

        Commands::IslandsTune {
            path,
            extensions,
            ignore,
            output,
        } => {
            let args = IslandsTuneArgs {
                path,
                extensions: parse_extensions(&extensions),
                ignore: parse_ignore(&ignore),
                output,
            };
            commands::islands_tune::run(args)?;
        }
    }

    Ok(())
}
