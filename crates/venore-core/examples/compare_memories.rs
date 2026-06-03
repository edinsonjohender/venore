//! One-off: dump and compare project_memory rows for two project paths.
//! Usage: `cargo run -p venore-core --example compare_memories`

use sqlx::sqlite::SqlitePool;
use sqlx::Row;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = std::env::var("TEMP").unwrap_or_else(|_| "/tmp".to_string())
        + "/venore-dev/config.db";
    let pool = SqlitePool::connect(&format!("sqlite:{}", db)).await?;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let needles: Vec<String> = if args.is_empty() {
        vec![
            "skalar-design-system-main-5".to_string(),
            "skalar-design-system-main-6".to_string(),
        ]
    } else {
        args
    };
    let needles: Vec<&str> = needles.iter().map(|s| s.as_str()).collect();

    // Inspect schema first
    println!("=== Tables in DB ===");
    let tables = sqlx::query("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .fetch_all(&pool).await?;
    for t in &tables { let n: String = t.get("name"); println!("  {}", n); }

    println!("\n=== project_memory columns ===");
    let cols = sqlx::query("PRAGMA table_info(project_memory)").fetch_all(&pool).await?;
    for c in &cols { let n: String = c.get("name"); println!("  {}", n); }

    println!("\n=== projects columns ===");
    let cols = sqlx::query("PRAGMA table_info(projects)").fetch_all(&pool).await?;
    for c in &cols { let n: String = c.get("name"); println!("  {}", n); }

    // Try without the join — just pull the memory rows directly, name should
    // be enough to identify them by needle.
    for needle in &needles {
        let row = sqlx::query(
            "SELECT pm.name, pm.description, pm.state, pm.team_size,
                    pm.goals_json, pm.architecture, pm.tech_debt,
                    pm.project_summary, pm.created_at, pm.updated_at,
                    pm.project_id
             FROM project_memory pm
             WHERE pm.name LIKE ?
             ORDER BY pm.updated_at DESC
             LIMIT 1"
        )
        .bind(format!("%{}%", needle))
        .fetch_optional(&pool)
        .await?;

        println!("\n=================================================================");
        println!("MATCH: {}", needle);
        println!("=================================================================");
        match row {
            Some(r) => {
                let name: String = r.get("name");
                let desc: String = r.get("description");
                let state: String = r.get("state");
                let team: String = r.get("team_size");
                let goals: String = r.get("goals_json");
                let arch: String = r.get("architecture");
                let debt: String = r.get("tech_debt");
                let summary: String = r.get("project_summary");
                let pid: String = r.get("project_id");
                let updated: String = r.get("updated_at");

                println!("project_id:  {}", pid);
                println!("name:        {}", name);
                println!("state:       {}", state);
                println!("team_size:   {}", team);
                println!("goals:       {}", goals);
                println!("updated_at:  {}", updated);
                println!();
                println!("--- description ({} chars, {} words) ---", desc.len(), desc.split_whitespace().count());
                println!("{}", desc);
                println!();
                println!("--- architecture ({} chars, {} words) ---", arch.len(), arch.split_whitespace().count());
                println!("{}", arch);
                println!();
                println!("--- tech_debt ({} chars, {} words) ---", debt.len(), debt.split_whitespace().count());
                println!("{}", debt);
                println!();
                println!("--- project_summary ({} chars, {} words, {} lines) ---",
                    summary.len(),
                    summary.split_whitespace().count(),
                    summary.lines().count(),
                );
                println!("{}", summary);
            }
            None => println!("(no row)"),
        }
    }

    Ok(())
}
