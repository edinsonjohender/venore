//! One-off: inspect what was persisted for a given project — memory, layers,
//! module deps, rag files. Usage:
//! `cargo run -p venore-core --example inspect_project`

use sqlx::sqlite::SqlitePool;
use sqlx::Row;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = std::env::var("TEMP").unwrap_or_else(|_| "/tmp".to_string())
        + "/venore-dev/config.db";
    let pool = SqlitePool::connect(&format!("sqlite:{}", db)).await?;

    let needle = std::env::args().nth(1).unwrap_or_else(|| "skalar-design-system-main-10".to_string());

    // Find project_id
    let proj = sqlx::query(
        "SELECT p.id, p.name, p.path FROM projects p WHERE p.path LIKE ? ORDER BY p.last_opened_at DESC LIMIT 1"
    )
    .bind(format!("%{}%", needle))
    .fetch_optional(&pool)
    .await?;

    let project_id: String = match proj {
        Some(r) => {
            let id: String = r.get("id");
            let name: String = r.get("name");
            let path: String = r.get("path");
            println!("=== Project ===");
            println!("id:    {}", id);
            println!("name:  {}", name);
            println!("path:  {}", path);
            id
        }
        None => {
            // Fallback: try by memory name
            let m = sqlx::query("SELECT project_id, name FROM project_memory WHERE name LIKE ? ORDER BY updated_at DESC LIMIT 1")
                .bind(format!("%{}%", needle))
                .fetch_optional(&pool).await?;
            match m {
                Some(r) => {
                    let id: String = r.get("project_id");
                    let name: String = r.get("name");
                    println!("=== Project (from memory) ===");
                    println!("id:   {}", id);
                    println!("name: {}", name);
                    id
                }
                None => {
                    println!("No project found matching '{}'", needle);
                    return Ok(());
                }
            }
        }
    };

    // module_layers
    println!("\n=== module_layers ===");
    let layers_rows = sqlx::query(
        "SELECT layer_type, status, COUNT(*) as cnt
         FROM module_layers
         WHERE project_id = ?
         GROUP BY layer_type, status
         ORDER BY layer_type, status"
    ).bind(&project_id).fetch_all(&pool).await?;
    if layers_rows.is_empty() {
        println!("(none)");
    } else {
        println!("{:<16} {:<14} count", "layer_type", "status");
        for r in &layers_rows {
            let l: String = r.get("layer_type");
            let s: String = r.get("status");
            let c: i64 = r.get("cnt");
            println!("{:<16} {:<14} {}", l, s, c);
        }
    }

    let total_layers: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM module_layers WHERE project_id = ?"
    ).bind(&project_id).fetch_one(&pool).await?;
    println!("total rows: {}", total_layers.0);

    let distinct_modules: (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT module_name) FROM module_layers WHERE project_id = ?"
    ).bind(&project_id).fetch_one(&pool).await?;
    println!("distinct modules with layers: {}", distinct_modules.0);

    // rag_module_deps (connections)
    println!("\n=== rag_module_deps (connections) ===");
    let deps_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM rag_module_deps WHERE project_id = ?"
    ).bind(&project_id).fetch_one(&pool).await?;
    println!("total deps: {}", deps_count.0);

    let sample = sqlx::query(
        "SELECT from_module, to_module, dep_type FROM rag_module_deps
         WHERE project_id = ? ORDER BY from_module"
    ).bind(&project_id).fetch_all(&pool).await?;
    if !sample.is_empty() {
        println!("all deps:");
        for r in &sample {
            let from: String = r.get("from_module");
            let to: String = r.get("to_module");
            let dep: String = r.get("dep_type");
            println!("  {} → {} ({})", from, to, dep);
        }
    }

    // Distinct destination modules
    let distinct_to: Vec<String> = sqlx::query(
        "SELECT DISTINCT to_module FROM rag_module_deps WHERE project_id = ? ORDER BY to_module"
    ).bind(&project_id).fetch_all(&pool).await?
        .iter().map(|r| r.get::<String, _>("to_module")).collect();
    println!("\ndistinct destinations: {} → {:?}", distinct_to.len(), distinct_to);

    // rag_file_modules (file→module mappings)
    println!("\n=== rag_file_modules ===");
    let fm_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM rag_file_modules WHERE project_id = ?"
    ).bind(&project_id).fetch_one(&pool).await?;
    println!("total file→module mappings: {}", fm_count.0);

    let distinct_mod: (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT module_name) FROM rag_file_modules WHERE project_id = ?"
    ).bind(&project_id).fetch_one(&pool).await?;
    println!("distinct modules mapped: {}", distinct_mod.0);

    let by_module = sqlx::query(
        "SELECT module_name, COUNT(*) as cnt
         FROM rag_file_modules
         WHERE project_id = ?
         GROUP BY module_name
         ORDER BY cnt DESC LIMIT 20"
    ).bind(&project_id).fetch_all(&pool).await?;
    if !by_module.is_empty() {
        println!("top modules by file count:");
        for r in &by_module {
            let n: String = r.get("module_name");
            let c: i64 = r.get("cnt");
            println!("  {:<32} {}", n, c);
        }
    }

    // Compare with analysis.modules (from module_layers, which the wizard
    // populates from analysis.modules — same source of truth).
    let analysis_mod_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT module_name) FROM module_layers WHERE project_id = ?"
    ).bind(&project_id).fetch_one(&pool).await?;
    println!("\nanalysis.modules count (from module_layers):    {}", analysis_mod_count.0);
    println!("modules mapped to files (rag_file_modules):     {}", distinct_mod.0);
    println!("DELTA (modules with no files mapped):           {}",
        analysis_mod_count.0 - distinct_mod.0);

    // rag_files
    println!("\n=== rag_files ===");
    let files_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM rag_files WHERE project_id = ?"
    ).bind(&project_id).fetch_one(&pool).await?;
    println!("indexed files: {}", files_count.0);

    // rag_symbol_refs
    println!("\n=== rag_symbol_refs ===");
    let refs_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM rag_symbol_refs WHERE project_id = ?"
    ).bind(&project_id).fetch_one(&pool).await?;
    println!("total refs: {}", refs_count.0);

    Ok(())
}
