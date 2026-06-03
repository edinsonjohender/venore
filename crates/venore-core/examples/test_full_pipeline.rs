//! End-to-end test: analysis-output.json → index_project_with_graph → verify graph
//!
//! Usage: cargo run --example test_full_pipeline -- <project_path>

use std::path::Path;
use venore_core::analysis::AnalysisOutput;
use venore_core::rag::{self, IndexConfig, RagRepository};
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().compact().init();

    let project_path = std::env::args().nth(1)
        .expect("Usage: test_full_pipeline <project_path>");
    let project_path = Path::new(&project_path);

    // 1. Load analysis output
    println!("\n=== Step 1: Load analysis-output.json ===");
    let analysis = AnalysisOutput::load_from_disk(project_path)
        .expect("Failed to read")
        .expect("No analysis-output.json found — run wizard first");

    println!("  Modules: {}", analysis.modules.len());
    for m in &analysis.modules {
        println!("    {} → {} files, {} deps, {} imports",
            m.name, m.file_count, m.architecture.dependencies.len(), m.imports.len());
    }

    // 2. Create in-memory SQLite + index with graph
    println!("\n=== Step 2: Index + populate graph (in-memory DB) ===");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory DB");

    let repo = RagRepository::new(pool);
    repo.initialize().await.expect("Failed to init DB");

    let config = IndexConfig::default();
    let project_id = "test-project";

    let result = rag::index_project_with_graph(
        &repo, project_id, project_path, &config, None, &analysis, None,
    ).await.expect("index_project_with_graph failed");

    println!("  Indexed: {}", result.indexed);
    println!("  Skipped: {}", result.skipped);
    println!("  Removed: {}", result.removed);
    println!("  Modules mapped: {}", result.modules_mapped);
    println!("  Deps created: {}", result.deps_created);
    println!("  Refs created: {}", result.refs_created);

    // 3. Verify graph data
    println!("\n=== Step 3: Verify graph ===");

    let modules = repo.get_modules(project_id).await.expect("get_modules failed");
    println!("  Modules in DB: {}", modules.len());
    for m in &modules {
        println!("    {} ({}) — {} files", m.module_name, m.module_path, m.file_count);

        let deps = repo.get_module_deps(project_id, &m.module_name).await.unwrap();
        if !deps.is_empty() {
            println!("      deps: {:?}", deps.iter().map(|d| &d.to_module).collect::<Vec<_>>());
        }

        let dependents = repo.get_module_dependents(project_id, &m.module_name).await.unwrap();
        if !dependents.is_empty() {
            println!("      dependents: {:?}", dependents.iter().map(|d| &d.from_module).collect::<Vec<_>>());
        }
    }

    // 4. Test FTS search
    println!("\n=== Step 4: FTS search test ===");
    let search_results = rag::search_code(&repo, project_id, "confetti", 5, 500).await.unwrap();
    println!("  Search 'confetti': {} results", search_results.len());
    for r in &search_results {
        println!("    {} ({}) — {}:{}-{} score={:.2}",
            r.chunk.name, r.chunk.chunk_type, r.chunk.relative_path,
            r.chunk.line_start, r.chunk.line_end, r.score);
    }

    // 5. Test graph query
    println!("\n=== Step 5: Graph query test ===");
    if let Some(query) = rag::classify_query("modules") {
        let qr = rag::execute_graph_query(&repo, project_id, query).await.unwrap();
        println!("  Query 'modules': type={}, {} modules, {} deps",
            qr.query_type, qr.modules.len(), qr.deps.len());
    } else {
        println!("  'modules' not classified as graph query (falling back to FTS)");
    }

    if let Some(query) = rag::classify_query("deps of src") {
        let qr = rag::execute_graph_query(&repo, project_id, query).await.unwrap();
        println!("  Query 'deps of src': type={}, {} modules, {} deps",
            qr.query_type, qr.modules.len(), qr.deps.len());
    }

    println!("\n=== ALL CHECKS PASSED ===");
}
