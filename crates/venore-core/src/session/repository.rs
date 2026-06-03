//! Session Repository
//!
//! SQLite persistence for session metadata.

use sqlx::sqlite::SqlitePool;
use sqlx::Row;

use crate::{Result, VenoreError};
use super::types::{Session, SessionStatus};

/// SQLite-backed session repository
pub struct SessionRepository {
    pool: SqlitePool,
}

impl SessionRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Initialize the sessions table
    pub async fn initialize(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                objective TEXT NOT NULL DEFAULT '',
                project_id TEXT NOT NULL,
                base_branch TEXT NOT NULL,
                session_branch TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'active',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create sessions table: {}", e)))?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_sessions_project ON sessions(project_id)"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create sessions index: {}", e)))?;

        // Migration: add worktree_path column (ignore error if it already exists)
        let _ = sqlx::query(
            "ALTER TABLE sessions ADD COLUMN worktree_path TEXT NOT NULL DEFAULT ''"
        )
        .execute(&self.pool)
        .await;

        tracing::info!("Session repository initialized");
        Ok(())
    }

    /// Create a new session
    pub async fn create(&self, session: &Session) -> Result<()> {
        sqlx::query(
            "INSERT INTO sessions (id, name, objective, project_id, base_branch, session_branch, worktree_path, status, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&session.id)
        .bind(&session.name)
        .bind(&session.objective)
        .bind(&session.project_id)
        .bind(&session.base_branch)
        .bind(&session.session_branch)
        .bind(&session.worktree_path)
        .bind(session.status.as_str())
        .bind(&session.created_at)
        .bind(&session.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create session: {}", e)))?;

        Ok(())
    }

    /// Get a session by ID
    pub async fn get(&self, id: &str) -> Result<Option<Session>> {
        let row = sqlx::query(
            "SELECT id, name, objective, project_id, base_branch, session_branch, worktree_path, status, created_at, updated_at
             FROM sessions WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to get session: {}", e)))?;

        Ok(row.map(|r| self.row_to_session(&r)))
    }

    /// List sessions by project, ordered by updated_at DESC
    pub async fn list_by_project(&self, project_id: &str) -> Result<Vec<Session>> {
        let rows = sqlx::query(
            "SELECT id, name, objective, project_id, base_branch, session_branch, worktree_path, status, created_at, updated_at
             FROM sessions WHERE project_id = ?
             ORDER BY updated_at DESC"
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to list sessions: {}", e)))?;

        Ok(rows.iter().map(|r| self.row_to_session(r)).collect())
    }

    /// Update session status
    pub async fn update_status(&self, id: &str, status: SessionStatus) -> Result<()> {
        sqlx::query(
            "UPDATE sessions SET status = ?, updated_at = datetime('now') WHERE id = ?"
        )
        .bind(status.as_str())
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to update session status: {}", e)))?;

        Ok(())
    }

    /// Touch session (update updated_at timestamp)
    pub async fn touch(&self, id: &str) -> Result<()> {
        sqlx::query("UPDATE sessions SET updated_at = datetime('now') WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to touch session: {}", e)))?;

        Ok(())
    }

    /// Delete a session
    pub async fn delete(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to delete session: {}", e)))?;

        Ok(())
    }

    fn row_to_session(&self, row: &sqlx::sqlite::SqliteRow) -> Session {
        let status_str: String = row.get("status");
        Session {
            id: row.get("id"),
            name: row.get("name"),
            objective: row.get("objective"),
            project_id: row.get("project_id"),
            base_branch: row.get("base_branch"),
            session_branch: row.get("session_branch"),
            worktree_path: row.get("worktree_path"),
            status: SessionStatus::from_str(&status_str).unwrap_or(SessionStatus::Active),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }
}
