//! CLI argument parsing

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "venore-cli")]
#[command(version = "0.1.0")]
#[command(about = "CLI for exercising the Context Generator pipeline", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run the full pipeline (scan + parse + modules)
    Analyze {
        /// Path of the project to analyze
        path: PathBuf,

        /// Output format
        #[arg(long, default_value = "pretty")]
        format: OutputFormat,

        /// File extensions to analyze (comma-separated)
        #[arg(long, default_value = ".ts,.tsx,.js,.jsx")]
        extensions: String,

        /// Patterns to ignore (comma-separated)
        #[arg(long, default_value = "node_modules,dist,.git,target")]
        ignore: String,

        /// Maximum file size in KB
        #[arg(long, default_value = "500")]
        max_size: u64,

        /// Save the result to a JSON file
        #[arg(long)]
        output: Option<PathBuf>,

        /// Print verbose logs
        #[arg(long, short)]
        verbose: bool,
    },

    /// Scan files only (TASK-001)
    Scan {
        /// Path of the project to scan
        path: PathBuf,

        /// Output format
        #[arg(long, default_value = "pretty")]
        format: OutputFormat,

        /// File extensions to scan
        #[arg(long, default_value = ".ts,.tsx,.js,.jsx")]
        extensions: String,

        /// Patterns to ignore
        #[arg(long, default_value = "node_modules,dist,.git,target")]
        ignore: String,

        /// Maximum file size in KB
        #[arg(long, default_value = "500")]
        max_size: u64,
    },

    /// Scan and parse files (TASK-001 + TASK-002)
    Parse {
        /// Path of the project to parse
        path: PathBuf,

        /// Output format
        #[arg(long, default_value = "pretty")]
        format: OutputFormat,

        /// File extensions to parse
        #[arg(long, default_value = ".ts,.tsx,.js,.jsx")]
        extensions: String,

        /// Patterns to ignore
        #[arg(long, default_value = "node_modules,dist,.git,target")]
        ignore: String,
    },

    /// Detect modules (TASK-001 + TASK-002 + TASK-003)
    Modules {
        /// Path of the project to analyze
        path: PathBuf,

        /// Output format
        #[arg(long, default_value = "pretty")]
        format: OutputFormat,

        /// File extensions to analyze
        #[arg(long, default_value = ".ts,.tsx,.js,.jsx")]
        extensions: String,

        /// Patterns to ignore
        #[arg(long, default_value = "node_modules,dist,.git,target")]
        ignore: String,
    },

    /// Emit the consolidated analysis structure (for future AI consumption)
    AnalysisOutput {
        /// Path of the project to analyze
        path: PathBuf,

        /// Output format
        #[arg(long, default_value = "json")]
        format: OutputFormat,

        /// File extensions to analyze
        #[arg(long, default_value = ".ts,.tsx,.js,.jsx")]
        extensions: String,

        /// Patterns to ignore
        #[arg(long, default_value = "node_modules,dist,.git,target")]
        ignore: String,

        /// Show output for a single module only
        #[arg(long)]
        module: Option<String>,

        /// Analysis depth level
        #[arg(long, default_value = "normal")]
        depth: AnalysisDepthArg,
    },

    /// Interactive wizard for project analysis (V1-style)
    Wizard,

    /// Detect sub-islands (logical clusters of modules)
    Islands {
        /// Path of the project to analyze
        path: PathBuf,

        /// Output format
        #[arg(long, default_value = "text")]
        format: IslandOutputFormat,

        /// File extensions to analyze
        #[arg(long, default_value = ".ts,.tsx,.js,.jsx,.rs,.go,.py")]
        extensions: String,

        /// Patterns to ignore
        #[arg(long, default_value = "node_modules,dist,.git,target")]
        ignore: String,

        /// Minimum modules required to form an island
        #[arg(long, default_value = "2")]
        min_modules: usize,

        /// Path depth used for clustering (e.g. 2 = "src/components")
        #[arg(long, default_value = "2")]
        depth: usize,

        /// Minimum cohesion threshold (0.0-1.0)
        #[arg(long, default_value = "0.3")]
        cohesion: f32,

        /// Minimum incoming-dependency count to flag a module as critical
        #[arg(long, default_value = "3")]
        critical: usize,
    },

    /// Try several island-detection configurations and pick the best
    IslandsTune {
        /// Path of the project to analyze
        path: PathBuf,

        /// File extensions to analyze
        #[arg(long, default_value = ".ts,.tsx,.js,.jsx,.rs,.go,.py")]
        extensions: String,

        /// Patterns to ignore
        #[arg(long, default_value = "node_modules,dist,.git,target")]
        ignore: String,

        /// Save results to a file
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

#[derive(Clone, ValueEnum)]
pub enum AnalysisDepthArg {
    /// Minimal — no code snippets
    Minimal,
    /// Normal — 1 snippet (~100 chars)
    Normal,
    /// Detailed — 3 snippets (~300 chars each)
    Detailed,
    /// Expert — 5 snippets (~500 chars each)
    Expert,
}

#[derive(Clone, ValueEnum)]
pub enum OutputFormat {
    /// Colorized, human-readable output
    Pretty,
    /// Plain text output, no colors
    Text,
    /// Structured JSON output
    Json,
}

#[derive(Clone, ValueEnum)]
pub enum IslandOutputFormat {
    /// Plain text output, no colors
    Text,
    /// Structured JSON output
    Json,
    /// Markdown output for documentation
    Markdown,
}

/// Parse comma-separated extensions into a vector
pub fn parse_extensions(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Parse comma-separated ignore patterns into a vector
pub fn parse_ignore(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}
