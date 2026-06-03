//! Context repository — SQLite CRUD for module contexts and layer analysis
//!
//! Stores LLM-generated module summaries and heuristic layer results in DB
//! instead of .context.md files. Enables chat injection, staleness detection,
//! and context evolution.

use sqlx::sqlite::SqlitePool;
use sqlx::Row;

use crate::{MapDbErr, Result};

// =============================================================================
// Record types
// =============================================================================

/// LLM-generated module context stored in DB
#[derive(Debug, Clone)]
pub struct ModuleContextRecord {
    pub id: String,
    pub project_id: String,
    pub module_name: String,
    pub module_path: String,
    /// Full markdown summary (what it does, why, API, decisions)
    pub summary: String,
    /// Serialized ContextMetadata (identity, connections, deps, agent_context)
    pub metadata_json: String,
    pub depth_level: String,
    /// SHA-256 of module source code (for staleness detection)
    pub code_hash: Option<String>,
    pub model: String,
    pub provider: String,
    pub tokens_used: Option<i64>,
    pub generation_time_ms: Option<i64>,
    pub stale: bool,
    pub generated_at: String,
    pub updated_at: String,
}

/// Layer analysis result stored in DB
#[derive(Debug, Clone)]
pub struct ModuleLayerRecord {
    pub id: String,
    pub project_id: String,
    pub module_name: String,
    pub module_path: String,
    /// context|tests|documentation|connections|status
    pub layer_type: String,
    /// complete|partial|missing
    pub status: String,
    /// Serialized HashMap<String, Value> with layer-specific details
    pub details_json: String,
    pub analyzed_at: String,
}

// =============================================================================
// Repository
// =============================================================================

pub struct ContextRepository {
    pool: SqlitePool,
}

impl ContextRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Access the underlying pool (for sharing with other repos)
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    // =========================================================================
    // Initialize
    // =========================================================================

    pub async fn initialize(&self) -> Result<()> {
        self.create_tables().await?;
        tracing::info!("Context repository initialized");
        Ok(())
    }

    async fn create_tables(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS module_contexts (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                module_name TEXT NOT NULL,
                module_path TEXT NOT NULL,
                summary TEXT NOT NULL,
                metadata_json TEXT NOT NULL,
                depth_level TEXT NOT NULL DEFAULT 'normal',
                code_hash TEXT,
                model TEXT NOT NULL,
                provider TEXT NOT NULL,
                tokens_used INTEGER,
                generation_time_ms INTEGER,
                stale INTEGER NOT NULL DEFAULT 0,
                generated_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(project_id, module_name)
            )"
        )
        .execute(&self.pool)
        .await
        .db_err("Failed to create module_contexts table")?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_module_contexts_project
             ON module_contexts(project_id)"
        )
        .execute(&self.pool)
        .await
        .db_err("Failed to create module_contexts index")?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_module_contexts_stale
             ON module_contexts(project_id, stale)"
        )
        .execute(&self.pool)
        .await
        .db_err("Failed to create module_contexts stale index")?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS module_layers (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                module_name TEXT NOT NULL,
                module_path TEXT NOT NULL,
                layer_type TEXT NOT NULL,
                status TEXT NOT NULL,
                details_json TEXT NOT NULL,
                analyzed_at TEXT NOT NULL,
                UNIQUE(project_id, module_name, layer_type)
            )"
        )
        .execute(&self.pool)
        .await
        .db_err("Failed to create module_layers table")?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_module_layers_project
             ON module_layers(project_id)"
        )
        .execute(&self.pool)
        .await
        .db_err("Failed to create module_layers index")?;

        Ok(())
    }

    // =========================================================================
    // Module Contexts — CRUD
    // =========================================================================

    /// Upsert a module context (INSERT or UPDATE on project_id + module_name)
    pub async fn save_module_context(&self, ctx: &ModuleContextRecord) -> Result<()> {
        sqlx::query(
            "INSERT INTO module_contexts
                (id, project_id, module_name, module_path, summary, metadata_json,
                 depth_level, code_hash, model, provider, tokens_used,
                 generation_time_ms, stale, generated_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (project_id, module_name) DO UPDATE SET
                module_path = excluded.module_path,
                summary = excluded.summary,
                metadata_json = excluded.metadata_json,
                depth_level = excluded.depth_level,
                code_hash = excluded.code_hash,
                model = excluded.model,
                provider = excluded.provider,
                tokens_used = excluded.tokens_used,
                generation_time_ms = excluded.generation_time_ms,
                stale = excluded.stale,
                generated_at = excluded.generated_at,
                updated_at = excluded.updated_at"
        )
        .bind(&ctx.id)
        .bind(&ctx.project_id)
        .bind(&ctx.module_name)
        .bind(&ctx.module_path)
        .bind(&ctx.summary)
        .bind(&ctx.metadata_json)
        .bind(&ctx.depth_level)
        .bind(&ctx.code_hash)
        .bind(&ctx.model)
        .bind(&ctx.provider)
        .bind(ctx.tokens_used)
        .bind(ctx.generation_time_ms)
        .bind(ctx.stale as i32)
        .bind(&ctx.generated_at)
        .bind(&ctx.updated_at)
        .execute(&self.pool)
        .await
        .db_err("Failed to save module context")?;

        tracing::debug!(
            project_id = %ctx.project_id,
            module = %ctx.module_name,
            "Module context saved"
        );
        Ok(())
    }

    /// Get a single module context
    pub async fn get_module_context(
        &self,
        project_id: &str,
        module_name: &str,
    ) -> Result<Option<ModuleContextRecord>> {
        let row = sqlx::query(
            "SELECT id, project_id, module_name, module_path, summary, metadata_json,
                    depth_level, code_hash, model, provider, tokens_used,
                    generation_time_ms, stale, generated_at, updated_at
             FROM module_contexts
             WHERE project_id = ? AND module_name = ?"
        )
        .bind(project_id)
        .bind(module_name)
        .fetch_optional(&self.pool)
        .await
        .db_err("Failed to get module context")?;

        Ok(row.map(|r| row_to_module_context(&r)))
    }

    /// Get all module contexts for a project
    pub async fn get_all_module_contexts(
        &self,
        project_id: &str,
    ) -> Result<Vec<ModuleContextRecord>> {
        let rows = sqlx::query(
            "SELECT id, project_id, module_name, module_path, summary, metadata_json,
                    depth_level, code_hash, model, provider, tokens_used,
                    generation_time_ms, stale, generated_at, updated_at
             FROM module_contexts
             WHERE project_id = ?
             ORDER BY module_name"
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .db_err("Failed to get all module contexts")?;

        Ok(rows.iter().map(row_to_module_context).collect())
    }

    /// Get stale module names
    pub async fn get_stale_modules(&self, project_id: &str) -> Result<Vec<String>> {
        let rows = sqlx::query(
            "SELECT module_name FROM module_contexts
             WHERE project_id = ? AND stale = 1
             ORDER BY module_name"
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .db_err("Failed to get stale modules")?;

        Ok(rows.iter().map(|r| r.get("module_name")).collect())
    }

    /// Mark a single module as stale
    pub async fn mark_stale(&self, project_id: &str, module_name: &str) -> Result<()> {
        sqlx::query(
            "UPDATE module_contexts SET stale = 1, updated_at = datetime('now')
             WHERE project_id = ? AND module_name = ?"
        )
        .bind(project_id)
        .bind(module_name)
        .execute(&self.pool)
        .await
        .db_err("Failed to mark module stale")?;

        Ok(())
    }

    /// Mark all modules as stale for a project
    pub async fn mark_all_stale(&self, project_id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE module_contexts SET stale = 1, updated_at = datetime('now')
             WHERE project_id = ?"
        )
        .bind(project_id)
        .execute(&self.pool)
        .await
        .db_err("Failed to mark all modules stale")?;

        Ok(())
    }

    /// Delete a single module context
    pub async fn delete_module_context(
        &self,
        project_id: &str,
        module_name: &str,
    ) -> Result<()> {
        sqlx::query(
            "DELETE FROM module_contexts WHERE project_id = ? AND module_name = ?"
        )
        .bind(project_id)
        .bind(module_name)
        .execute(&self.pool)
        .await
        .db_err("Failed to delete module context")?;

        Ok(())
    }

    /// Delete all module contexts for a project
    pub async fn delete_all_module_contexts(&self, project_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM module_contexts WHERE project_id = ?")
            .bind(project_id)
            .execute(&self.pool)
            .await
            .db_err("Failed to delete all module contexts")?;

        Ok(())
    }

    /// Count module contexts for a project
    pub async fn count_module_contexts(&self, project_id: &str) -> Result<u32> {
        let row = sqlx::query(
            "SELECT COUNT(*) as cnt FROM module_contexts WHERE project_id = ?"
        )
        .bind(project_id)
        .fetch_one(&self.pool)
        .await
        .db_err("Failed to count module contexts")?;

        let count: i64 = row.get("cnt");
        Ok(count as u32)
    }

    // =========================================================================
    // Module Layers — CRUD
    // =========================================================================

    /// Upsert a single layer analysis result
    pub async fn save_module_layer(&self, layer: &ModuleLayerRecord) -> Result<()> {
        sqlx::query(
            "INSERT INTO module_layers
                (id, project_id, module_name, module_path, layer_type,
                 status, details_json, analyzed_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (project_id, module_name, layer_type) DO UPDATE SET
                module_path = excluded.module_path,
                status = excluded.status,
                details_json = excluded.details_json,
                analyzed_at = excluded.analyzed_at"
        )
        .bind(&layer.id)
        .bind(&layer.project_id)
        .bind(&layer.module_name)
        .bind(&layer.module_path)
        .bind(&layer.layer_type)
        .bind(&layer.status)
        .bind(&layer.details_json)
        .bind(&layer.analyzed_at)
        .execute(&self.pool)
        .await
        .db_err("Failed to save module layer")?;

        Ok(())
    }

    /// Save all layers for a module (batch upsert)
    pub async fn save_module_layers(
        &self,
        project_id: &str,
        module_name: &str,
        module_path: &str,
        layers: &[(String, String, String)], // (layer_type, status, details_json)
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();

        for (layer_type, status, details_json) in layers {
            let id = uuid::Uuid::new_v4().to_string();
            let record = ModuleLayerRecord {
                id,
                project_id: project_id.to_string(),
                module_name: module_name.to_string(),
                module_path: module_path.to_string(),
                layer_type: layer_type.clone(),
                status: status.clone(),
                details_json: details_json.clone(),
                analyzed_at: now.clone(),
            };
            self.save_module_layer(&record).await?;
        }

        Ok(())
    }

    /// Get all layers for a specific module
    pub async fn get_module_layers(
        &self,
        project_id: &str,
        module_name: &str,
    ) -> Result<Vec<ModuleLayerRecord>> {
        let rows = sqlx::query(
            "SELECT id, project_id, module_name, module_path, layer_type,
                    status, details_json, analyzed_at
             FROM module_layers
             WHERE project_id = ? AND module_name = ?
             ORDER BY layer_type"
        )
        .bind(project_id)
        .bind(module_name)
        .fetch_all(&self.pool)
        .await
        .db_err("Failed to get module layers")?;

        Ok(rows.iter().map(row_to_module_layer).collect())
    }

    /// Get all layers for all modules in a project
    pub async fn get_all_layers(&self, project_id: &str) -> Result<Vec<ModuleLayerRecord>> {
        let rows = sqlx::query(
            "SELECT id, project_id, module_name, module_path, layer_type,
                    status, details_json, analyzed_at
             FROM module_layers
             WHERE project_id = ?
             ORDER BY module_name, layer_type"
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .db_err("Failed to get all layers")?;

        Ok(rows.iter().map(row_to_module_layer).collect())
    }

    /// Delete all layers for a project
    pub async fn delete_all_layers(&self, project_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM module_layers WHERE project_id = ?")
            .bind(project_id)
            .execute(&self.pool)
            .await
            .db_err("Failed to delete all layers")?;

        Ok(())
    }
}

// =============================================================================
// Row mapping
// =============================================================================

fn row_to_module_context(row: &sqlx::sqlite::SqliteRow) -> ModuleContextRecord {
    ModuleContextRecord {
        id: row.get("id"),
        project_id: row.get("project_id"),
        module_name: row.get("module_name"),
        module_path: row.get("module_path"),
        summary: row.get("summary"),
        metadata_json: row.get("metadata_json"),
        depth_level: row.get("depth_level"),
        code_hash: row.get("code_hash"),
        model: row.get("model"),
        provider: row.get("provider"),
        tokens_used: row.get("tokens_used"),
        generation_time_ms: row.get("generation_time_ms"),
        stale: row.get::<i32, _>("stale") != 0,
        generated_at: row.get("generated_at"),
        updated_at: row.get("updated_at"),
    }
}

fn row_to_module_layer(row: &sqlx::sqlite::SqliteRow) -> ModuleLayerRecord {
    ModuleLayerRecord {
        id: row.get("id"),
        project_id: row.get("project_id"),
        module_name: row.get("module_name"),
        module_path: row.get("module_path"),
        layer_type: row.get("layer_type"),
        status: row.get("status"),
        details_json: row.get("details_json"),
        analyzed_at: row.get("analyzed_at"),
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn create_test_repo() -> ContextRepository {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let repo = ContextRepository::new(pool);
        repo.initialize().await.unwrap();
        repo
    }

    fn test_context(project_id: &str, module_name: &str) -> ModuleContextRecord {
        ModuleContextRecord {
            id: uuid::Uuid::new_v4().to_string(),
            project_id: project_id.to_string(),
            module_name: module_name.to_string(),
            module_path: format!("src/{}", module_name),
            summary: format!("# {}\n\nThis module handles...", module_name),
            metadata_json: "{}".to_string(),
            depth_level: "normal".to_string(),
            code_hash: Some("sha256-abc123".to_string()),
            model: "claude-sonnet-4-5".to_string(),
            provider: "anthropic".to_string(),
            tokens_used: Some(1500),
            generation_time_ms: Some(3200),
            stale: false,
            generated_at: "2026-03-18T00:00:00Z".to_string(),
            updated_at: "2026-03-18T00:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn test_save_and_get_module_context() {
        let repo = create_test_repo().await;
        let ctx = test_context("proj1", "auth");

        repo.save_module_context(&ctx).await.unwrap();

        let loaded = repo.get_module_context("proj1", "auth").await.unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.module_name, "auth");
        assert_eq!(loaded.summary, "# auth\n\nThis module handles...");
        assert!(!loaded.stale);
    }

    #[tokio::test]
    async fn test_upsert_module_context() {
        let repo = create_test_repo().await;
        let mut ctx = test_context("proj1", "auth");

        repo.save_module_context(&ctx).await.unwrap();

        // Update summary
        ctx.summary = "# auth\n\nUpdated summary.".to_string();
        ctx.stale = false;
        repo.save_module_context(&ctx).await.unwrap();

        let loaded = repo.get_module_context("proj1", "auth").await.unwrap().unwrap();
        assert_eq!(loaded.summary, "# auth\n\nUpdated summary.");
    }

    #[tokio::test]
    async fn test_get_all_module_contexts() {
        let repo = create_test_repo().await;

        repo.save_module_context(&test_context("proj1", "auth")).await.unwrap();
        repo.save_module_context(&test_context("proj1", "payments")).await.unwrap();
        repo.save_module_context(&test_context("proj2", "other")).await.unwrap();

        let contexts = repo.get_all_module_contexts("proj1").await.unwrap();
        assert_eq!(contexts.len(), 2);
        assert_eq!(contexts[0].module_name, "auth");
        assert_eq!(contexts[1].module_name, "payments");
    }

    #[tokio::test]
    async fn test_mark_stale() {
        let repo = create_test_repo().await;
        repo.save_module_context(&test_context("proj1", "auth")).await.unwrap();

        repo.mark_stale("proj1", "auth").await.unwrap();

        let stale = repo.get_stale_modules("proj1").await.unwrap();
        assert_eq!(stale, vec!["auth"]);

        let ctx = repo.get_module_context("proj1", "auth").await.unwrap().unwrap();
        assert!(ctx.stale);
    }

    #[tokio::test]
    async fn test_mark_all_stale() {
        let repo = create_test_repo().await;
        repo.save_module_context(&test_context("proj1", "auth")).await.unwrap();
        repo.save_module_context(&test_context("proj1", "payments")).await.unwrap();

        repo.mark_all_stale("proj1").await.unwrap();

        let stale = repo.get_stale_modules("proj1").await.unwrap();
        assert_eq!(stale.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_module_context() {
        let repo = create_test_repo().await;
        repo.save_module_context(&test_context("proj1", "auth")).await.unwrap();

        repo.delete_module_context("proj1", "auth").await.unwrap();

        let loaded = repo.get_module_context("proj1", "auth").await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_count_module_contexts() {
        let repo = create_test_repo().await;
        repo.save_module_context(&test_context("proj1", "auth")).await.unwrap();
        repo.save_module_context(&test_context("proj1", "payments")).await.unwrap();

        let count = repo.count_module_contexts("proj1").await.unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_save_and_get_module_layers() {
        let repo = create_test_repo().await;

        let layers = vec![
            ("tests".to_string(), "partial".to_string(), r#"{"test_files":3,"source_files":10,"coverage_ratio":0.3}"#.to_string()),
            ("documentation".to_string(), "missing".to_string(), r#"{"has_readme":false,"doc_ratio":0.0}"#.to_string()),
        ];

        repo.save_module_layers("proj1", "auth", "src/auth", &layers).await.unwrap();

        let loaded = repo.get_module_layers("proj1", "auth").await.unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].layer_type, "documentation"); // sorted by layer_type
        assert_eq!(loaded[1].layer_type, "tests");
    }

    #[tokio::test]
    async fn test_get_all_layers() {
        let repo = create_test_repo().await;

        let layers = vec![
            ("tests".to_string(), "complete".to_string(), "{}".to_string()),
        ];

        repo.save_module_layers("proj1", "auth", "src/auth", &layers).await.unwrap();
        repo.save_module_layers("proj1", "payments", "src/payments", &layers).await.unwrap();

        let all = repo.get_all_layers("proj1").await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_layer_upsert() {
        let repo = create_test_repo().await;

        let layers_v1 = vec![
            ("tests".to_string(), "missing".to_string(), "{}".to_string()),
        ];
        repo.save_module_layers("proj1", "auth", "src/auth", &layers_v1).await.unwrap();

        // Update status
        let layers_v2 = vec![
            ("tests".to_string(), "complete".to_string(), r#"{"coverage_ratio":0.8}"#.to_string()),
        ];
        repo.save_module_layers("proj1", "auth", "src/auth", &layers_v2).await.unwrap();

        let loaded = repo.get_module_layers("proj1", "auth").await.unwrap();
        assert_eq!(loaded.len(), 1); // upsert, not duplicate
        assert_eq!(loaded[0].status, "complete");
    }

    #[tokio::test]
    async fn test_empty_project() {
        let repo = create_test_repo().await;

        let contexts = repo.get_all_module_contexts("nonexistent").await.unwrap();
        assert!(contexts.is_empty());

        let stale = repo.get_stale_modules("nonexistent").await.unwrap();
        assert!(stale.is_empty());

        let layers = repo.get_all_layers("nonexistent").await.unwrap();
        assert!(layers.is_empty());

        let count = repo.count_module_contexts("nonexistent").await.unwrap();
        assert_eq!(count, 0);
    }
}
