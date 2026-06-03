//! Memory repository — SQLite CRUD for project memory

use sqlx::sqlite::SqlitePool;
use sqlx::Row;

use crate::{MapDbErr, Result};
use super::models::ProjectMemory;

pub struct MemoryRepository {
    pool: SqlitePool,
}

impl MemoryRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // =========================================================================
    // Initialize
    // =========================================================================

    pub async fn initialize(&self) -> Result<()> {
        self.create_tables().await?;
        self.migrate_add_column("project_memory", "tech_debt", "TEXT NOT NULL DEFAULT ''").await;
        tracing::info!("Memory repository initialized");
        Ok(())
    }

    async fn create_tables(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS project_memory (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL DEFAULT '',
                description TEXT NOT NULL DEFAULT '',
                state TEXT NOT NULL DEFAULT 'active',
                team_size TEXT NOT NULL DEFAULT 'solo',
                goals_json TEXT NOT NULL DEFAULT '[]',
                architecture TEXT NOT NULL DEFAULT '',
                tech_debt TEXT NOT NULL DEFAULT '',
                response_language TEXT NOT NULL DEFAULT 'en',
                conventions_json TEXT NOT NULL DEFAULT '[]',
                project_summary TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )"
        )
        .execute(&self.pool)
        .await
        .db_err("Failed to create project_memory table")?;

        Ok(())
    }

    /// Idempotent column migration — adds a column if it doesn't exist.
    async fn migrate_add_column(&self, table: &str, column: &str, col_type: &str) {
        let rows = sqlx::query(&format!("PRAGMA table_info({})", table))
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default();
        let exists = rows.iter().any(|r| {
            let name: String = r.get("name");
            name == column
        });
        if !exists {
            let sql = format!("ALTER TABLE {} ADD COLUMN {} {}", table, column, col_type);
            if let Err(e) = sqlx::query(&sql).execute(&self.pool).await {
                tracing::warn!("Migration {}.{} failed (may already exist): {}", table, column, e);
            } else {
                tracing::info!("Migrated {}.{}", table, column);
            }
        }
    }

    // =========================================================================
    // CRUD
    // =========================================================================

    /// Fetch a project memory by its primary key (memory id, not project_id).
    /// Used by callers that have only the memory id and need to resolve the
    /// owning project (e.g. the delete Tauri command, which must locate the
    /// project_path to delete the portable `.venore/project-memory.json`).
    pub async fn get_by_id(&self, id: &str) -> Result<Option<ProjectMemory>> {
        let row = sqlx::query(
            "SELECT id, project_id, name, description, state, team_size,
                    goals_json, architecture, tech_debt, response_language,
                    conventions_json, project_summary,
                    created_at, updated_at
             FROM project_memory WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .db_err("Failed to get project memory by id")?;

        Ok(row.map(|r| row_to_memory(&r)))
    }

    pub async fn get_by_project(&self, project_id: &str) -> Result<Option<ProjectMemory>> {
        let row = sqlx::query(
            "SELECT id, project_id, name, description, state, team_size,
                    goals_json, architecture, tech_debt, response_language,
                    conventions_json, project_summary,
                    created_at, updated_at
             FROM project_memory WHERE project_id = ?"
        )
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await
        .db_err("Failed to get project memory")?;

        Ok(row.map(|r| row_to_memory(&r)))
    }

    /// INSERT OR REPLACE — upserts on the UNIQUE project_id.
    pub async fn save(&self, memory: &ProjectMemory) -> Result<()> {
        let goals_json = serde_json::to_string(&memory.goals)
            .db_err("Failed to serialize goals")?;
        let conventions_json = serde_json::to_string(&memory.conventions)
            .db_err("Failed to serialize conventions")?;

        sqlx::query(
            "INSERT INTO project_memory
                (id, project_id, name, description, state, team_size,
                 goals_json, architecture, tech_debt, response_language,
                 conventions_json, project_summary,
                 created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (project_id) DO UPDATE SET
                name = excluded.name,
                description = excluded.description,
                state = excluded.state,
                team_size = excluded.team_size,
                goals_json = excluded.goals_json,
                architecture = excluded.architecture,
                tech_debt = excluded.tech_debt,
                response_language = excluded.response_language,
                conventions_json = excluded.conventions_json,
                project_summary = excluded.project_summary,
                updated_at = excluded.updated_at"
        )
        .bind(&memory.id)
        .bind(&memory.project_id)
        .bind(&memory.name)
        .bind(&memory.description)
        .bind(&memory.state)
        .bind(&memory.team_size)
        .bind(&goals_json)
        .bind(&memory.architecture)
        .bind(&memory.tech_debt)
        .bind(&memory.response_language)
        .bind(&conventions_json)
        .bind(&memory.project_summary)
        .bind(&memory.created_at)
        .bind(&memory.updated_at)
        .execute(&self.pool)
        .await
        .db_err("Failed to save project memory")?;

        tracing::info!(project_id = %memory.project_id, name = %memory.name, "Project memory saved");
        Ok(())
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM project_memory WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .db_err("Failed to delete project memory")?;

        if result.rows_affected() == 0 {
            return Err(crate::VenoreError::NotFound(format!("Project memory '{}'", id)));
        }

        tracing::info!(id = %id, "Project memory deleted");
        Ok(())
    }
}

// =============================================================================
// Row mapping
// =============================================================================

fn row_to_memory(row: &sqlx::sqlite::SqliteRow) -> ProjectMemory {
    let goals_str: String = row.get("goals_json");
    let goals: Vec<String> = serde_json::from_str(&goals_str).unwrap_or_else(|e| {
        tracing::warn!(json = %goals_str, error = %e, "Corrupt goals JSON in project memory, using empty");
        Vec::new()
    });

    let conventions_str: String = row.get("conventions_json");
    let conventions: Vec<String> = serde_json::from_str(&conventions_str).unwrap_or_else(|e| {
        tracing::warn!(json = %conventions_str, error = %e, "Corrupt conventions JSON in project memory, using empty");
        Vec::new()
    });

    ProjectMemory {
        id: row.get("id"),
        project_id: row.get("project_id"),
        name: row.get("name"),
        description: row.get("description"),
        state: row.get("state"),
        team_size: row.get("team_size"),
        goals,
        architecture: row.get("architecture"),
        tech_debt: row.get("tech_debt"),
        response_language: row.get("response_language"),
        conventions,
        project_summary: row.get("project_summary"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}
