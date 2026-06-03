//! Knowledge Repository
//!
//! SQLite persistence for knowledge features, hexagons, and evidence.

use sqlx::sqlite::SqlitePool;
use sqlx::Row;

use crate::{Result, VenoreError};
use super::types::{KnowledgeFeature, KnowledgeHexagon, KnowledgeEvidence, KnowledgeFile, KnowledgeProjectLink};

/// SQLite-backed knowledge repository
pub struct KnowledgeRepository {
    pool: SqlitePool,
}

impl KnowledgeRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Initialize tables (CREATE IF NOT EXISTS) with cascade FKs
    pub async fn initialize(&self) -> Result<()> {
        // Enable foreign keys
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to enable FKs: {}", e)))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS knowledge_features (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'active',
                priority TEXT NOT NULL DEFAULT 'medium',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
            )"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create knowledge_features: {}", e)))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS knowledge_hexagons (
                id TEXT PRIMARY KEY,
                feature_id TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                phase TEXT NOT NULL DEFAULT 'discover',
                percentage INTEGER NOT NULL DEFAULT 0,
                confidence TEXT NOT NULL DEFAULT 'low',
                risk TEXT NOT NULL DEFAULT 'unknown',
                priority TEXT NOT NULL DEFAULT 'medium',
                is_dead_end INTEGER NOT NULL DEFAULT 0,
                blocked_by TEXT NOT NULL DEFAULT '[]',
                notes_user TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (feature_id) REFERENCES knowledge_features(id) ON DELETE CASCADE
            )"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create knowledge_hexagons: {}", e)))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS knowledge_evidence (
                id TEXT PRIMARY KEY,
                hexagon_id TEXT NOT NULL,
                content TEXT NOT NULL,
                source_url TEXT NOT NULL DEFAULT '',
                source_type TEXT NOT NULL DEFAULT 'manual',
                confidence TEXT NOT NULL DEFAULT 'medium',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (hexagon_id) REFERENCES knowledge_hexagons(id) ON DELETE CASCADE
            )"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create knowledge_evidence: {}", e)))?;

        // Idempotent migrations — add new columns if missing
        self.migrate_add_column("knowledge_features", "objective", "TEXT NOT NULL DEFAULT 'explore'").await;
        self.migrate_add_column("knowledge_features", "intensity", "TEXT NOT NULL DEFAULT 'moderate'").await;
        self.migrate_add_column("knowledge_features", "max_hexagons_per_phase", "INTEGER NOT NULL DEFAULT 7").await;
        self.migrate_add_column("knowledge_features", "auto_advance", "INTEGER NOT NULL DEFAULT 0").await;
        self.migrate_add_column("knowledge_features", "tags", "TEXT NOT NULL DEFAULT '[]'").await;
        self.migrate_add_column("knowledge_hexagons", "agent_status", "TEXT NOT NULL DEFAULT 'idle'").await;

        // New tables
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS knowledge_files (
                id TEXT PRIMARY KEY,
                feature_id TEXT NOT NULL,
                filename TEXT NOT NULL,
                filepath TEXT NOT NULL,
                filetype TEXT NOT NULL,
                filesize INTEGER NOT NULL DEFAULT 0,
                indexed INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (feature_id) REFERENCES knowledge_features(id) ON DELETE CASCADE
            )"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create knowledge_files: {}", e)))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS knowledge_project_links (
                id TEXT PRIMARY KEY,
                feature_id TEXT NOT NULL,
                project_id TEXT NOT NULL,
                project_path TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (feature_id) REFERENCES knowledge_features(id) ON DELETE CASCADE
            )"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create knowledge_project_links: {}", e)))?;

        tracing::info!("Knowledge repository initialized");
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
    // Features CRUD
    // =========================================================================

    pub async fn create_feature(&self, feature: &KnowledgeFeature) -> Result<()> {
        sqlx::query(
            "INSERT INTO knowledge_features (id, project_id, name, description, status, priority, objective, intensity, max_hexagons_per_phase, auto_advance, tags, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&feature.id)
        .bind(&feature.project_id)
        .bind(&feature.name)
        .bind(&feature.description)
        .bind(&feature.status)
        .bind(&feature.priority)
        .bind(&feature.objective)
        .bind(&feature.intensity)
        .bind(feature.max_hexagons_per_phase)
        .bind(feature.auto_advance)
        .bind(&feature.tags)
        .bind(&feature.created_at)
        .bind(&feature.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create feature: {}", e)))?;
        Ok(())
    }

    pub async fn get_feature(&self, id: &str) -> Result<Option<KnowledgeFeature>> {
        let row = sqlx::query(
            "SELECT id, project_id, name, description, status, priority, objective, intensity, max_hexagons_per_phase, auto_advance, tags, created_at, updated_at
             FROM knowledge_features WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to get feature: {}", e)))?;

        Ok(row.as_ref().map(Self::row_to_feature))
    }

    pub async fn list_features_by_project(&self, project_id: &str) -> Result<Vec<KnowledgeFeature>> {
        let rows = sqlx::query(
            "SELECT id, project_id, name, description, status, priority, objective, intensity, max_hexagons_per_phase, auto_advance, tags, created_at, updated_at
             FROM knowledge_features WHERE project_id = ? ORDER BY created_at DESC"
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to list features: {}", e)))?;

        Ok(rows.iter().map(Self::row_to_feature).collect())
    }

    pub async fn update_feature(&self, feature: &KnowledgeFeature) -> Result<()> {
        sqlx::query(
            "UPDATE knowledge_features SET name = ?, description = ?, status = ?, priority = ?, objective = ?, intensity = ?, max_hexagons_per_phase = ?, auto_advance = ?, tags = ?, updated_at = ?
             WHERE id = ?"
        )
        .bind(&feature.name)
        .bind(&feature.description)
        .bind(&feature.status)
        .bind(&feature.priority)
        .bind(&feature.objective)
        .bind(&feature.intensity)
        .bind(feature.max_hexagons_per_phase)
        .bind(feature.auto_advance)
        .bind(&feature.tags)
        .bind(&feature.updated_at)
        .bind(&feature.id)
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to update feature: {}", e)))?;
        Ok(())
    }

    pub async fn delete_feature(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM knowledge_features WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to delete feature: {}", e)))?;
        Ok(())
    }

    fn row_to_feature(row: &sqlx::sqlite::SqliteRow) -> KnowledgeFeature {
        KnowledgeFeature {
            id: row.get("id"),
            project_id: row.get("project_id"),
            name: row.get("name"),
            description: row.get("description"),
            status: row.get("status"),
            priority: row.get("priority"),
            objective: row.get("objective"),
            intensity: row.get("intensity"),
            max_hexagons_per_phase: row.get("max_hexagons_per_phase"),
            auto_advance: row.get::<i32, _>("auto_advance") != 0,
            tags: row.get("tags"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }

    // =========================================================================
    // Hexagons CRUD
    // =========================================================================

    pub async fn create_hexagon(&self, hex: &KnowledgeHexagon) -> Result<()> {
        sqlx::query(
            "INSERT INTO knowledge_hexagons (id, feature_id, title, description, phase, percentage, confidence, risk, priority, is_dead_end, blocked_by, notes_user, agent_status, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&hex.id)
        .bind(&hex.feature_id)
        .bind(&hex.title)
        .bind(&hex.description)
        .bind(&hex.phase)
        .bind(hex.percentage)
        .bind(&hex.confidence)
        .bind(&hex.risk)
        .bind(&hex.priority)
        .bind(hex.is_dead_end)
        .bind(&hex.blocked_by)
        .bind(&hex.notes_user)
        .bind(&hex.agent_status)
        .bind(&hex.created_at)
        .bind(&hex.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create hexagon: {}", e)))?;
        Ok(())
    }

    pub async fn get_hexagon(&self, id: &str) -> Result<Option<KnowledgeHexagon>> {
        let row = sqlx::query(
            "SELECT id, feature_id, title, description, phase, percentage, confidence, risk, priority, is_dead_end, blocked_by, notes_user, agent_status, created_at, updated_at
             FROM knowledge_hexagons WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to get hexagon: {}", e)))?;

        Ok(row.as_ref().map(Self::row_to_hexagon))
    }

    pub async fn list_hexagons_by_feature(&self, feature_id: &str) -> Result<Vec<KnowledgeHexagon>> {
        let rows = sqlx::query(
            "SELECT id, feature_id, title, description, phase, percentage, confidence, risk, priority, is_dead_end, blocked_by, notes_user, agent_status, created_at, updated_at
             FROM knowledge_hexagons WHERE feature_id = ? ORDER BY created_at ASC"
        )
        .bind(feature_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to list hexagons: {}", e)))?;

        Ok(rows.iter().map(Self::row_to_hexagon).collect())
    }

    pub async fn update_hexagon(&self, hex: &KnowledgeHexagon) -> Result<()> {
        sqlx::query(
            "UPDATE knowledge_hexagons SET title = ?, description = ?, phase = ?, percentage = ?, confidence = ?, risk = ?, priority = ?, is_dead_end = ?, blocked_by = ?, notes_user = ?, agent_status = ?, updated_at = ?
             WHERE id = ?"
        )
        .bind(&hex.title)
        .bind(&hex.description)
        .bind(&hex.phase)
        .bind(hex.percentage)
        .bind(&hex.confidence)
        .bind(&hex.risk)
        .bind(&hex.priority)
        .bind(hex.is_dead_end)
        .bind(&hex.blocked_by)
        .bind(&hex.notes_user)
        .bind(&hex.agent_status)
        .bind(&hex.updated_at)
        .bind(&hex.id)
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to update hexagon: {}", e)))?;
        Ok(())
    }

    pub async fn delete_hexagon(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM knowledge_hexagons WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to delete hexagon: {}", e)))?;
        Ok(())
    }

    fn row_to_hexagon(row: &sqlx::sqlite::SqliteRow) -> KnowledgeHexagon {
        KnowledgeHexagon {
            id: row.get("id"),
            feature_id: row.get("feature_id"),
            title: row.get("title"),
            description: row.get("description"),
            phase: row.get("phase"),
            percentage: row.get("percentage"),
            confidence: row.get("confidence"),
            risk: row.get("risk"),
            priority: row.get("priority"),
            is_dead_end: row.get::<i32, _>("is_dead_end") != 0,
            blocked_by: row.get("blocked_by"),
            notes_user: row.get("notes_user"),
            agent_status: row.get("agent_status"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }

    // =========================================================================
    // Evidence CRUD
    // =========================================================================

    pub async fn create_evidence(&self, ev: &KnowledgeEvidence) -> Result<()> {
        sqlx::query(
            "INSERT INTO knowledge_evidence (id, hexagon_id, content, source_url, source_type, confidence, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&ev.id)
        .bind(&ev.hexagon_id)
        .bind(&ev.content)
        .bind(&ev.source_url)
        .bind(&ev.source_type)
        .bind(&ev.confidence)
        .bind(&ev.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create evidence: {}", e)))?;
        Ok(())
    }

    pub async fn list_evidence_by_hexagon(&self, hexagon_id: &str) -> Result<Vec<KnowledgeEvidence>> {
        let rows = sqlx::query(
            "SELECT id, hexagon_id, content, source_url, source_type, confidence, created_at
             FROM knowledge_evidence WHERE hexagon_id = ? ORDER BY created_at ASC"
        )
        .bind(hexagon_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to list evidence: {}", e)))?;

        Ok(rows.iter().map(Self::row_to_evidence).collect())
    }

    pub async fn delete_evidence(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM knowledge_evidence WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to delete evidence: {}", e)))?;
        Ok(())
    }

    fn row_to_evidence(row: &sqlx::sqlite::SqliteRow) -> KnowledgeEvidence {
        KnowledgeEvidence {
            id: row.get("id"),
            hexagon_id: row.get("hexagon_id"),
            content: row.get("content"),
            source_url: row.get("source_url"),
            source_type: row.get("source_type"),
            confidence: row.get("confidence"),
            created_at: row.get("created_at"),
        }
    }

    /// Count evidence entries for a hexagon (used by context builder).
    pub async fn count_evidence_by_hexagon(&self, hexagon_id: &str) -> Result<usize> {
        let row = sqlx::query("SELECT COUNT(*) as cnt FROM knowledge_evidence WHERE hexagon_id = ?")
            .bind(hexagon_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to count evidence: {}", e)))?;
        Ok(row.get::<i32, _>("cnt") as usize)
    }

    // =========================================================================
    // Files CRUD
    // =========================================================================

    pub async fn create_file(&self, file: &KnowledgeFile) -> Result<()> {
        sqlx::query(
            "INSERT INTO knowledge_files (id, feature_id, filename, filepath, filetype, filesize, indexed, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&file.id)
        .bind(&file.feature_id)
        .bind(&file.filename)
        .bind(&file.filepath)
        .bind(&file.filetype)
        .bind(file.filesize)
        .bind(file.indexed)
        .bind(&file.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create file: {}", e)))?;
        Ok(())
    }

    pub async fn list_files_by_feature(&self, feature_id: &str) -> Result<Vec<KnowledgeFile>> {
        let rows = sqlx::query(
            "SELECT id, feature_id, filename, filepath, filetype, filesize, indexed, created_at
             FROM knowledge_files WHERE feature_id = ? ORDER BY created_at ASC"
        )
        .bind(feature_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to list files: {}", e)))?;

        Ok(rows.iter().map(Self::row_to_file).collect())
    }

    pub async fn delete_file(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM knowledge_files WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to delete file: {}", e)))?;
        Ok(())
    }

    fn row_to_file(row: &sqlx::sqlite::SqliteRow) -> KnowledgeFile {
        KnowledgeFile {
            id: row.get("id"),
            feature_id: row.get("feature_id"),
            filename: row.get("filename"),
            filepath: row.get("filepath"),
            filetype: row.get("filetype"),
            filesize: row.get("filesize"),
            indexed: row.get::<i32, _>("indexed") != 0,
            created_at: row.get("created_at"),
        }
    }

    // =========================================================================
    // Project Links CRUD
    // =========================================================================

    pub async fn create_project_link(&self, link: &KnowledgeProjectLink) -> Result<()> {
        sqlx::query(
            "INSERT INTO knowledge_project_links (id, feature_id, project_id, project_path, created_at)
             VALUES (?, ?, ?, ?, ?)"
        )
        .bind(&link.id)
        .bind(&link.feature_id)
        .bind(&link.project_id)
        .bind(&link.project_path)
        .bind(&link.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create project link: {}", e)))?;
        Ok(())
    }

    pub async fn list_project_links_by_feature(&self, feature_id: &str) -> Result<Vec<KnowledgeProjectLink>> {
        let rows = sqlx::query(
            "SELECT id, feature_id, project_id, project_path, created_at
             FROM knowledge_project_links WHERE feature_id = ? ORDER BY created_at ASC"
        )
        .bind(feature_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to list project links: {}", e)))?;

        Ok(rows.iter().map(Self::row_to_project_link).collect())
    }

    pub async fn delete_project_link(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM knowledge_project_links WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to delete project link: {}", e)))?;
        Ok(())
    }

    fn row_to_project_link(row: &sqlx::sqlite::SqliteRow) -> KnowledgeProjectLink {
        KnowledgeProjectLink {
            id: row.get("id"),
            feature_id: row.get("feature_id"),
            project_id: row.get("project_id"),
            project_path: row.get("project_path"),
            created_at: row.get("created_at"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn create_test_repo() -> (KnowledgeRepository, SqlitePool) {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        // Enable FKs
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();

        // Create projects table (dependency)
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS projects (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                path TEXT NOT NULL,
                project_type TEXT NOT NULL DEFAULT 'code',
                created_at TEXT NOT NULL,
                last_opened_at TEXT NOT NULL DEFAULT (datetime('now'))
            )"
        )
        .execute(&pool)
        .await
        .unwrap();

        let repo = KnowledgeRepository::new(pool.clone());
        repo.initialize().await.unwrap();
        (repo, pool)
    }

    async fn insert_project(pool: &SqlitePool, id: &str) {
        sqlx::query(
            "INSERT INTO projects (id, name, path, project_type, created_at)
             VALUES (?, 'test', '/test', 'knowledge', datetime('now'))"
        )
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    }

    fn make_feature(id: &str, project_id: &str) -> KnowledgeFeature {
        KnowledgeFeature {
            id: id.to_string(),
            project_id: project_id.to_string(),
            name: format!("Feature {}", id),
            description: "Test feature".to_string(),
            status: "active".to_string(),
            priority: "high".to_string(),
            objective: "explore".to_string(),
            intensity: "moderate".to_string(),
            max_hexagons_per_phase: 7,
            auto_advance: false,
            tags: "[]".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn make_hexagon(id: &str, feature_id: &str) -> KnowledgeHexagon {
        KnowledgeHexagon {
            id: id.to_string(),
            feature_id: feature_id.to_string(),
            title: format!("Hexagon {}", id),
            description: "Test hexagon".to_string(),
            phase: "discover".to_string(),
            percentage: 0,
            confidence: "low".to_string(),
            risk: "unknown".to_string(),
            priority: "medium".to_string(),
            is_dead_end: false,
            blocked_by: "[]".to_string(),
            notes_user: "".to_string(),
            agent_status: "idle".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn make_evidence(id: &str, hexagon_id: &str) -> KnowledgeEvidence {
        KnowledgeEvidence {
            id: id.to_string(),
            hexagon_id: hexagon_id.to_string(),
            content: "Some evidence".to_string(),
            source_url: "https://example.com".to_string(),
            source_type: "web".to_string(),
            confidence: "high".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn test_feature_crud() {
        let (repo, pool) = create_test_repo().await;
        insert_project(&pool, "proj-1").await;

        let feature = make_feature("f-1", "proj-1");
        repo.create_feature(&feature).await.unwrap();

        let found = repo.get_feature("f-1").await.unwrap().unwrap();
        assert_eq!(found.name, "Feature f-1");

        let list = repo.list_features_by_project("proj-1").await.unwrap();
        assert_eq!(list.len(), 1);

        let mut updated = found;
        updated.name = "Updated Feature".to_string();
        updated.updated_at = "2026-02-01T00:00:00Z".to_string();
        repo.update_feature(&updated).await.unwrap();

        let reloaded = repo.get_feature("f-1").await.unwrap().unwrap();
        assert_eq!(reloaded.name, "Updated Feature");

        repo.delete_feature("f-1").await.unwrap();
        assert!(repo.get_feature("f-1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_hexagon_crud() {
        let (repo, pool) = create_test_repo().await;
        insert_project(&pool, "proj-1").await;
        repo.create_feature(&make_feature("f-1", "proj-1")).await.unwrap();

        let hex = make_hexagon("h-1", "f-1");
        repo.create_hexagon(&hex).await.unwrap();

        let found = repo.get_hexagon("h-1").await.unwrap().unwrap();
        assert_eq!(found.title, "Hexagon h-1");
        assert_eq!(found.phase, "discover");
        assert!(!found.is_dead_end);

        let mut updated = found;
        updated.phase = "validate".to_string();
        updated.percentage = 75;
        updated.updated_at = "2026-02-01T00:00:00Z".to_string();
        repo.update_hexagon(&updated).await.unwrap();

        let reloaded = repo.get_hexagon("h-1").await.unwrap().unwrap();
        assert_eq!(reloaded.phase, "validate");
        assert_eq!(reloaded.percentage, 75);

        let list = repo.list_hexagons_by_feature("f-1").await.unwrap();
        assert_eq!(list.len(), 1);

        repo.delete_hexagon("h-1").await.unwrap();
        assert!(repo.get_hexagon("h-1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_evidence_crud() {
        let (repo, pool) = create_test_repo().await;
        insert_project(&pool, "proj-1").await;
        repo.create_feature(&make_feature("f-1", "proj-1")).await.unwrap();
        repo.create_hexagon(&make_hexagon("h-1", "f-1")).await.unwrap();

        let ev = make_evidence("e-1", "h-1");
        repo.create_evidence(&ev).await.unwrap();

        let list = repo.list_evidence_by_hexagon("h-1").await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].content, "Some evidence");

        repo.delete_evidence("e-1").await.unwrap();
        let list = repo.list_evidence_by_hexagon("h-1").await.unwrap();
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn test_cascade_delete_feature() {
        let (repo, pool) = create_test_repo().await;
        insert_project(&pool, "proj-1").await;
        repo.create_feature(&make_feature("f-1", "proj-1")).await.unwrap();
        repo.create_hexagon(&make_hexagon("h-1", "f-1")).await.unwrap();
        repo.create_evidence(&make_evidence("e-1", "h-1")).await.unwrap();

        // Deleting feature should cascade to hexagons and evidence
        repo.delete_feature("f-1").await.unwrap();

        assert!(repo.get_hexagon("h-1").await.unwrap().is_none());
        let evidence = repo.list_evidence_by_hexagon("h-1").await.unwrap();
        assert!(evidence.is_empty());
    }

    #[tokio::test]
    async fn test_cascade_delete_hexagon() {
        let (repo, pool) = create_test_repo().await;
        insert_project(&pool, "proj-1").await;
        repo.create_feature(&make_feature("f-1", "proj-1")).await.unwrap();
        repo.create_hexagon(&make_hexagon("h-1", "f-1")).await.unwrap();
        repo.create_evidence(&make_evidence("e-1", "h-1")).await.unwrap();

        // Deleting hexagon should cascade to evidence
        repo.delete_hexagon("h-1").await.unwrap();

        let evidence = repo.list_evidence_by_hexagon("h-1").await.unwrap();
        assert!(evidence.is_empty());
    }

    #[tokio::test]
    async fn test_file_crud() {
        let (repo, pool) = create_test_repo().await;
        insert_project(&pool, "proj-1").await;
        repo.create_feature(&make_feature("f-1", "proj-1")).await.unwrap();

        let file = KnowledgeFile {
            id: "file-1".to_string(),
            feature_id: "f-1".to_string(),
            filename: "notes.pdf".to_string(),
            filepath: "/tmp/notes.pdf".to_string(),
            filetype: "pdf".to_string(),
            filesize: 1024,
            indexed: false,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };
        repo.create_file(&file).await.unwrap();

        let list = repo.list_files_by_feature("f-1").await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].filename, "notes.pdf");

        repo.delete_file("file-1").await.unwrap();
        let list = repo.list_files_by_feature("f-1").await.unwrap();
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn test_project_link_crud() {
        let (repo, pool) = create_test_repo().await;
        insert_project(&pool, "proj-1").await;
        repo.create_feature(&make_feature("f-1", "proj-1")).await.unwrap();

        let link = KnowledgeProjectLink {
            id: "link-1".to_string(),
            feature_id: "f-1".to_string(),
            project_id: "proj-1".to_string(),
            project_path: "/home/user/project".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };
        repo.create_project_link(&link).await.unwrap();

        let list = repo.list_project_links_by_feature("f-1").await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].project_path, "/home/user/project");

        repo.delete_project_link("link-1").await.unwrap();
        let list = repo.list_project_links_by_feature("f-1").await.unwrap();
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn test_count_evidence_by_hexagon() {
        let (repo, pool) = create_test_repo().await;
        insert_project(&pool, "proj-1").await;
        repo.create_feature(&make_feature("f-1", "proj-1")).await.unwrap();
        repo.create_hexagon(&make_hexagon("h-1", "f-1")).await.unwrap();

        assert_eq!(repo.count_evidence_by_hexagon("h-1").await.unwrap(), 0);

        repo.create_evidence(&make_evidence("e-1", "h-1")).await.unwrap();
        repo.create_evidence(&make_evidence("e-2", "h-1")).await.unwrap();

        assert_eq!(repo.count_evidence_by_hexagon("h-1").await.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_feature_new_fields() {
        let (repo, pool) = create_test_repo().await;
        insert_project(&pool, "proj-1").await;

        let mut feature = make_feature("f-1", "proj-1");
        feature.objective = "validate".to_string();
        feature.intensity = "deep".to_string();
        feature.max_hexagons_per_phase = 12;
        feature.auto_advance = true;
        feature.tags = r#"["rust","ai"]"#.to_string();
        repo.create_feature(&feature).await.unwrap();

        let found = repo.get_feature("f-1").await.unwrap().unwrap();
        assert_eq!(found.objective, "validate");
        assert_eq!(found.intensity, "deep");
        assert_eq!(found.max_hexagons_per_phase, 12);
        assert!(found.auto_advance);
        assert_eq!(found.tags, r#"["rust","ai"]"#);
    }

    #[tokio::test]
    async fn test_hexagon_agent_status() {
        let (repo, pool) = create_test_repo().await;
        insert_project(&pool, "proj-1").await;
        repo.create_feature(&make_feature("f-1", "proj-1")).await.unwrap();

        let hex = make_hexagon("h-1", "f-1");
        repo.create_hexagon(&hex).await.unwrap();

        let found = repo.get_hexagon("h-1").await.unwrap().unwrap();
        assert_eq!(found.agent_status, "idle");

        let mut updated = found;
        updated.agent_status = "running".to_string();
        updated.updated_at = "2026-02-01T00:00:00Z".to_string();
        repo.update_hexagon(&updated).await.unwrap();

        let reloaded = repo.get_hexagon("h-1").await.unwrap().unwrap();
        assert_eq!(reloaded.agent_status, "running");
    }
}
