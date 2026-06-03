use venore_core::analysis::pipeline::{RunAnalysisConfig, run_analysis};
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().compact().init();

    let project_path = std::env::args().nth(1)
        .expect("Usage: run_analysis_test <project_path>");

    let config = RunAnalysisConfig {
        project_path: PathBuf::from(&project_path),
        ..RunAnalysisConfig::default()
    };

    match run_analysis(config).await {
        Ok(output) => {
            println!("\n=== ANALYSIS RESULT ===");
            println!("Repository: {}", output.repository.name);
            println!("Language: {:?}", output.repository.language);
            println!("Technologies: {:?}", output.repository.technologies);
            println!("Total files: {}", output.repository.total_files);
            println!("Total modules: {}", output.repository.total_modules);
            println!("\n--- Modules ---");
            for m in &output.modules {
                println!("  {} ({}/) - {} files, deps: {:?}",
                    m.name, m.path, m.file_count, m.architecture.dependencies);
            }
            if !output.orphan_files.is_empty() {
                println!("\n--- Orphan files ---");
                for f in &output.orphan_files {
                    println!("  {}", f);
                }
            }
            println!("\nSaved to .venore/analysis-output.json");
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
