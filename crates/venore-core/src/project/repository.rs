//! Project Repository
//!
//! SQLite persistence for registered projects.

use sqlx::sqlite::SqlitePool;
use sqlx::Row;

use crate::{Result, VenoreError};
use super::identity::RegisteredProject;

/// SQLite-backed project repository
pub struct ProjectRepository {
    pool: SqlitePool,
}

impl ProjectRepository {
    /// Create a new ProjectRepository with the given connection pool
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Initialize tables (CREATE IF NOT EXISTS)
    pub async fn initialize(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS projects (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                path TEXT NOT NULL,
                created_at TEXT NOT NULL,
                last_opened_at TEXT NOT NULL DEFAULT (datetime('now'))
            )"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create projects table: {}", e)))?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_projects_path ON projects(path)"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create projects index: {}", e)))?;

        // Migration: add project_type column (idempotent)
        sqlx::query(
            "ALTER TABLE projects ADD COLUMN project_type TEXT NOT NULL DEFAULT 'code'"
        )
        .execute(&self.pool)
        .await
        .ok(); // Ignore error if column already exists

        tracing::info!("Project repository initialized");
        Ok(())
    }

    /// Insert or update a project. On conflict (same ID), updates path and last_opened_at.
    pub async fn upsert(
        &self,
        id: &str,
        name: &str,
        path: &str,
        created_at: &str,
        project_type: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO projects (id, name, path, created_at, last_opened_at, project_type)
             VALUES (?, ?, ?, ?, datetime('now'), ?)
             ON CONFLICT(id) DO UPDATE SET
                path = excluded.path,
                last_opened_at = datetime('now')"
        )
        .bind(id)
        .bind(name)
        .bind(path)
        .bind(created_at)
        .bind(project_type)
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to upsert project: {}", e)))?;

        Ok(())
    }

    /// Map a SQLite row to a RegisteredProject
    fn row_to_project(row: &sqlx::sqlite::SqliteRow) -> RegisteredProject {
        let created_at_str: String = row.get("created_at");
        let last_opened_str: String = row.get("last_opened_at");
        RegisteredProject {
            id: row.get::<String, _>("id")
                .parse()
                .unwrap_or_default(),
            name: row.get("name"),
            path: row.get("path"),
            project_type: row.get("project_type"),
            created_at: chrono::DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now()),
            last_opened_at: chrono::DateTime::parse_from_rfc3339(&last_opened_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now()),
        }
    }

    /// List all projects, ordered by last_opened_at DESC
    pub async fn list(&self) -> Result<Vec<RegisteredProject>> {
        let rows = sqlx::query(
            "SELECT id, name, path, project_type, created_at, last_opened_at
             FROM projects ORDER BY last_opened_at DESC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to list projects: {}", e)))?;

        let projects = rows.iter().map(Self::row_to_project).collect();
        Ok(projects)
    }

    /// Find a project by ID
    pub async fn find_by_id(&self, id: &str) -> Result<Option<RegisteredProject>> {
        let row = sqlx::query(
            "SELECT id, name, path, project_type, created_at, last_opened_at
             FROM projects WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to find project: {}", e)))?;

        Ok(row.as_ref().map(Self::row_to_project))
    }

    /// Find a project by path
    pub async fn find_by_path(&self, path: &str) -> Result<Option<RegisteredProject>> {
        let row = sqlx::query(
            "SELECT id, name, path, project_type, created_at, last_opened_at
             FROM projects WHERE path = ?"
        )
        .bind(path)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to find project by path: {}", e)))?;

        Ok(row.as_ref().map(Self::row_to_project))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn create_test_repo() -> ProjectRepository {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let repo = ProjectRepository::new(pool);
        repo.initialize().await.unwrap();
        repo
    }

    #[tokio::test]
    async fn test_upsert_and_find_by_id() {
        let repo = create_test_repo().await;
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        repo.upsert(&id, "my-project", "/path/to/project", &now, "code").await.unwrap();

        let found = repo.find_by_id(&id).await.unwrap();
        assert!(found.is_some());
        let project = found.unwrap();
        assert_eq!(project.name, "my-project");
        assert_eq!(project.path, "/path/to/project");
        assert_eq!(project.project_type, "code");
    }

    #[tokio::test]
    async fn test_upsert_updates_path() {
        let repo = create_test_repo().await;
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        repo.upsert(&id, "my-project", "/old/path", &now, "code").await.unwrap();
        repo.upsert(&id, "my-project", "/new/path", &now, "code").await.unwrap();

        let found = repo.find_by_id(&id).await.unwrap().unwrap();
        assert_eq!(found.path, "/new/path");
    }

    #[tokio::test]
    async fn test_upsert_knowledge_project() {
        let repo = create_test_repo().await;
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        repo.upsert(&id, "my-research", "/knowledge/path", &now, "knowledge").await.unwrap();

        let found = repo.find_by_id(&id).await.unwrap().unwrap();
        assert_eq!(found.project_type, "knowledge");
    }

    #[tokio::test]
    async fn test_find_by_path() {
        let repo = create_test_repo().await;
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        repo.upsert(&id, "path-test", "/specific/path", &now, "code").await.unwrap();

        let found = repo.find_by_path("/specific/path").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id.to_string(), id);

        let missing = repo.find_by_path("/nonexistent").await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_list_returns_all_projects() {
        let repo = create_test_repo().await;
        let now = chrono::Utc::now().to_rfc3339();

        let id1 = uuid::Uuid::new_v4().to_string();
        let id2 = uuid::Uuid::new_v4().to_string();

        repo.upsert(&id1, "project-1", "/path/1", &now, "code").await.unwrap();
        repo.upsert(&id2, "project-2", "/path/2", &now, "knowledge").await.unwrap();

        let projects = repo.list().await.unwrap();
        assert_eq!(projects.len(), 2);

        let names: Vec<&str> = projects.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"project-1"));
        assert!(names.contains(&"project-2"));
    }

    #[tokio::test]
    async fn test_find_nonexistent() {
        let repo = create_test_repo().await;
        let found = repo.find_by_id("nonexistent-id").await.unwrap();
        assert!(found.is_none());
    }
}
