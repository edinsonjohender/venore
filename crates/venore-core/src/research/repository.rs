//! Research Repository
//!
//! SQLite persistence for research runs.

use sqlx::sqlite::SqlitePool;
use sqlx::Row;

use crate::{Result, VenoreError};
use super::types::ResearchRun;

/// SQLite-backed repository for research engine runs
pub struct ResearchRepository {
    pool: SqlitePool,
}

impl ResearchRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Initialize table (CREATE IF NOT EXISTS)
    pub async fn initialize(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS research_runs (
                id TEXT PRIMARY KEY,
                feature_id TEXT NOT NULL,
                phase TEXT NOT NULL DEFAULT 'decomposing',
                status TEXT NOT NULL DEFAULT 'running',
                intensity TEXT NOT NULL DEFAULT 'moderate',
                max_workers INTEGER NOT NULL DEFAULT 3,
                evaluation_round INTEGER NOT NULL DEFAULT 0,
                total_workers_spawned INTEGER NOT NULL DEFAULT 0,
                total_tool_calls INTEGER NOT NULL DEFAULT 0,
                total_tokens INTEGER NOT NULL DEFAULT 0,
                manager_model TEXT NOT NULL DEFAULT '',
                worker_model TEXT NOT NULL DEFAULT '',
                user_instructions TEXT NOT NULL DEFAULT '[]',
                started_at TEXT NOT NULL,
                finished_at TEXT,
                duration_ms INTEGER NOT NULL DEFAULT 0,
                error TEXT,
                FOREIGN KEY (feature_id) REFERENCES knowledge_features(id) ON DELETE CASCADE
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create research_runs: {e}")))?;

        Ok(())
    }

    // -------------------------------------------------------------------------
    // CRUD
    // -------------------------------------------------------------------------

    pub async fn create_run(&self, run: &ResearchRun) -> Result<()> {
        sqlx::query(
            "INSERT INTO research_runs (
                id, feature_id, phase, status, intensity, max_workers,
                evaluation_round, total_workers_spawned, total_tool_calls, total_tokens,
                manager_model, worker_model, user_instructions,
                started_at, finished_at, duration_ms, error
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
        )
        .bind(&run.id)
        .bind(&run.feature_id)
        .bind(&run.phase)
        .bind(&run.status)
        .bind(&run.intensity)
        .bind(run.max_workers)
        .bind(run.evaluation_round)
        .bind(run.total_workers_spawned)
        .bind(run.total_tool_calls)
        .bind(run.total_tokens)
        .bind(&run.manager_model)
        .bind(&run.worker_model)
        .bind(&run.user_instructions)
        .bind(&run.started_at)
        .bind(&run.finished_at)
        .bind(run.duration_ms)
        .bind(&run.error)
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create research run: {e}")))?;

        Ok(())
    }

    pub async fn get_run(&self, id: &str) -> Result<Option<ResearchRun>> {
        let row = sqlx::query(
            "SELECT id, feature_id, phase, status, intensity, max_workers,
                    evaluation_round, total_workers_spawned, total_tool_calls, total_tokens,
                    manager_model, worker_model, user_instructions,
                    started_at, finished_at, duration_ms, error
             FROM research_runs WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to get research run: {e}")))?;

        Ok(row.map(|r| row_to_run(&r)))
    }

    /// Get the latest active (non-terminal) run for a feature
    pub async fn get_active_run_for_feature(&self, feature_id: &str) -> Result<Option<ResearchRun>> {
        let row = sqlx::query(
            "SELECT id, feature_id, phase, status, intensity, max_workers,
                    evaluation_round, total_workers_spawned, total_tool_calls, total_tokens,
                    manager_model, worker_model, user_instructions,
                    started_at, finished_at, duration_ms, error
             FROM research_runs
             WHERE feature_id = ?1 AND status IN ('running', 'paused')
             ORDER BY started_at DESC LIMIT 1",
        )
        .bind(feature_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to get active run: {e}")))?;

        Ok(row.map(|r| row_to_run(&r)))
    }

    pub async fn update_run(&self, run: &ResearchRun) -> Result<()> {
        sqlx::query(
            "UPDATE research_runs SET
                phase = ?2, status = ?3, evaluation_round = ?4,
                total_workers_spawned = ?5, total_tool_calls = ?6, total_tokens = ?7,
                user_instructions = ?8, finished_at = ?9, duration_ms = ?10, error = ?11
             WHERE id = ?1",
        )
        .bind(&run.id)
        .bind(&run.phase)
        .bind(&run.status)
        .bind(run.evaluation_round)
        .bind(run.total_workers_spawned)
        .bind(run.total_tool_calls)
        .bind(run.total_tokens)
        .bind(&run.user_instructions)
        .bind(&run.finished_at)
        .bind(run.duration_ms)
        .bind(&run.error)
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to update research run: {e}")))?;

        Ok(())
    }

    /// Mark all "running" runs as "paused" — called on app startup for recovery
    pub async fn pause_stale_runs(&self) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE research_runs SET status = 'paused', phase = 'paused'
             WHERE status = 'running'",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to pause stale runs: {e}")))?;

        Ok(result.rows_affected())
    }

    /// Append a user instruction to a run's instruction list
    pub async fn append_user_instruction(&self, run_id: &str, instruction: &str) -> Result<()> {
        // Read current, parse, append, write back
        let run = self.get_run(run_id).await?;
        let run = run.ok_or_else(|| VenoreError::NotFound(format!("Research run {run_id}")))?;

        let mut instructions: Vec<String> =
            serde_json::from_str(&run.user_instructions).unwrap_or_default();
        instructions.push(instruction.to_string());
        let updated = serde_json::to_string(&instructions).unwrap_or_else(|_| "[]".to_string());

        sqlx::query("UPDATE research_runs SET user_instructions = ?2 WHERE id = ?1")
            .bind(run_id)
            .bind(&updated)
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to append instruction: {e}")))?;

        Ok(())
    }
}

fn row_to_run(row: &sqlx::sqlite::SqliteRow) -> ResearchRun {
    ResearchRun {
        id: row.get("id"),
        feature_id: row.get("feature_id"),
        phase: row.get("phase"),
        status: row.get("status"),
        intensity: row.get("intensity"),
        max_workers: row.get("max_workers"),
        evaluation_round: row.get("evaluation_round"),
        total_workers_spawned: row.get("total_workers_spawned"),
        total_tool_calls: row.get("total_tool_calls"),
        total_tokens: row.get("total_tokens"),
        manager_model: row.get("manager_model"),
        worker_model: row.get("worker_model"),
        user_instructions: row.get("user_instructions"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
        duration_ms: row.get("duration_ms"),
        error: row.get("error"),
    }
}
