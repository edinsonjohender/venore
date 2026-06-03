//! Logbook Repository
//!
//! SQLite + FTS5 persistence for the KNOWLEDGE logbook index — the markdown
//! sections of ocean knowledge nodes. Separate tables from `rag_*` (code) so
//! knowledge search never contaminates code search and vice versa.
//!
//! Mirrors `RagRepository` (same FTS5 + embeddings pattern) but keyed by
//! `(project_id, node_id, section_id)` instead of files, with a `content_hash`
//! column so the Index Current can skip unchanged sections cheaply.

use sqlx::sqlite::SqlitePool;
use sqlx::Row;

use crate::{Result, VenoreError};

/// A chunk of logbook content — one knowledge-node section, indexed for search.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LogbookChunk {
    /// = NodeSection.id (so reindexing a section overwrites its chunk).
    pub id: String,
    pub project_id: String,
    /// Ocean node id the section belongs to (NOT a rag_files FK).
    pub node_id: String,
    pub name: String,
    pub content: String,
    /// SHA256(name + "\0" + content) — change detection for the Index Current.
    pub content_hash: String,
    /// "user" or "ai" — provenance carried for display/filtering.
    pub source: String,
    pub updated_at: i64,
}

/// SQLite-backed logbook index with FTS5 search.
pub struct LogbookRepository {
    pool: SqlitePool,
}

impl LogbookRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Access the underlying connection pool (for ad-hoc queries in sibling modules)
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Initialize tables, indexes, FTS5 virtual table, and triggers.
    pub async fn initialize(&self) -> Result<()> {
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to enable foreign keys: {}", e)))?;

        // Chunks table — one row per knowledge-node section.
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS logbook_chunks (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                node_id TEXT NOT NULL,
                name TEXT NOT NULL,
                content TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                source TEXT,
                updated_at INTEGER NOT NULL
            )"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create logbook_chunks: {}", e)))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_logbook_chunks_project ON logbook_chunks(project_id)")
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to create logbook project index: {}", e)))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_logbook_chunks_node ON logbook_chunks(project_id, node_id)")
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to create logbook node index: {}", e)))?;

        // FTS5 virtual table (content-sync with logbook_chunks)
        sqlx::query(
            "CREATE VIRTUAL TABLE IF NOT EXISTS logbook_chunks_fts USING fts5(
                name, content,
                content=logbook_chunks, content_rowid=rowid
            )"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create logbook FTS5 table: {}", e)))?;

        // Embeddings table (linked to logbook_chunks via FK)
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS logbook_embeddings (
                chunk_id TEXT PRIMARY KEY,
                embedding BLOB NOT NULL,
                model TEXT NOT NULL,
                dimensions INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY (chunk_id) REFERENCES logbook_chunks(id) ON DELETE CASCADE
            )"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create logbook_embeddings: {}", e)))?;

        // Triggers to keep FTS5 in sync. Unlike rag_* (which deletes-then-inserts
        // on file change), logbook chunks are upserted in place, so we ALSO need an
        // AFTER UPDATE trigger to re-sync the FTS index on content edits.
        sqlx::query(
            "CREATE TRIGGER IF NOT EXISTS logbook_fts_insert AFTER INSERT ON logbook_chunks BEGIN
                INSERT INTO logbook_chunks_fts(rowid, name, content)
                VALUES (new.rowid, new.name, new.content);
            END"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create logbook FTS insert trigger: {}", e)))?;

        sqlx::query(
            "CREATE TRIGGER IF NOT EXISTS logbook_fts_delete AFTER DELETE ON logbook_chunks BEGIN
                INSERT INTO logbook_chunks_fts(logbook_chunks_fts, rowid, name, content)
                VALUES ('delete', old.rowid, old.name, old.content);
            END"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create logbook FTS delete trigger: {}", e)))?;

        sqlx::query(
            "CREATE TRIGGER IF NOT EXISTS logbook_fts_update AFTER UPDATE ON logbook_chunks BEGIN
                INSERT INTO logbook_chunks_fts(logbook_chunks_fts, rowid, name, content)
                VALUES ('delete', old.rowid, old.name, old.content);
                INSERT INTO logbook_chunks_fts(rowid, name, content)
                VALUES (new.rowid, new.name, new.content);
            END"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create logbook FTS update trigger: {}", e)))?;

        tracing::info!("Logbook repository initialized");
        Ok(())
    }

    // ========================================================================
    // CHANGE DETECTION
    // ========================================================================

    /// Get the stored content hash for a section (None if not indexed yet).
    pub async fn get_section_hash(&self, project_id: &str, section_id: &str) -> Result<Option<String>> {
        let row = sqlx::query(
            "SELECT content_hash FROM logbook_chunks WHERE project_id = ? AND id = ?"
        )
        .bind(project_id)
        .bind(section_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to get section hash: {}", e)))?;

        Ok(row.map(|r| r.get("content_hash")))
    }

    /// Get all indexed section ids for a node (used to detect deletions).
    pub async fn get_node_section_ids(&self, project_id: &str, node_id: &str) -> Result<Vec<String>> {
        let rows = sqlx::query(
            "SELECT id FROM logbook_chunks WHERE project_id = ? AND node_id = ?"
        )
        .bind(project_id)
        .bind(node_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to get node section ids: {}", e)))?;

        Ok(rows.iter().map(|r| r.get::<String, _>("id")).collect())
    }

    // ========================================================================
    // CHUNKS
    // ========================================================================

    /// Insert or update a chunk (by section id). Changing content re-syncs FTS
    /// via the update trigger; the stale embedding is dropped so it re-embeds.
    pub async fn upsert_chunk(&self, chunk: &LogbookChunk) -> Result<()> {
        // Drop a now-stale embedding when content changed: an UPDATE that alters
        // `content` must invalidate the old vector. Cheapest correct approach is
        // to delete the embedding row whenever the hash differs from what's stored.
        let prev_hash = self.get_section_hash(&chunk.project_id, &chunk.id).await?;
        let content_changed = prev_hash.as_deref() != Some(chunk.content_hash.as_str());

        sqlx::query(
            "INSERT INTO logbook_chunks (id, project_id, node_id, name, content, content_hash, source, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                node_id = excluded.node_id,
                name = excluded.name,
                content = excluded.content,
                content_hash = excluded.content_hash,
                source = excluded.source,
                updated_at = excluded.updated_at"
        )
        .bind(&chunk.id)
        .bind(&chunk.project_id)
        .bind(&chunk.node_id)
        .bind(&chunk.name)
        .bind(&chunk.content)
        .bind(&chunk.content_hash)
        .bind(&chunk.source)
        .bind(chunk.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to upsert logbook chunk: {}", e)))?;

        if content_changed {
            sqlx::query("DELETE FROM logbook_embeddings WHERE chunk_id = ?")
                .bind(&chunk.id)
                .execute(&self.pool)
                .await
                .map_err(|e| VenoreError::DatabaseError(format!("Failed to drop stale embedding: {}", e)))?;
        }

        Ok(())
    }

    /// Delete a single chunk by section id (cascades its embedding).
    pub async fn delete_chunk(&self, section_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM logbook_chunks WHERE id = ?")
            .bind(section_id)
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to delete logbook chunk: {}", e)))?;

        Ok(())
    }

    /// Delete all chunks for a node (cascades embeddings). Used when a node is removed.
    pub async fn delete_node(&self, project_id: &str, node_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM logbook_chunks WHERE project_id = ? AND node_id = ?")
            .bind(project_id)
            .bind(node_id)
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to delete logbook node: {}", e)))?;

        Ok(())
    }

    // ========================================================================
    // EMBEDDINGS
    // ========================================================================

    /// Insert or replace an embedding for a chunk.
    pub async fn upsert_embedding(
        &self,
        chunk_id: &str,
        embedding: &[u8],
        model: &str,
        dimensions: u32,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO logbook_embeddings (chunk_id, embedding, model, dimensions, created_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(chunk_id) DO UPDATE SET
                embedding = excluded.embedding,
                model = excluded.model,
                dimensions = excluded.dimensions,
                created_at = excluded.created_at"
        )
        .bind(chunk_id)
        .bind(embedding)
        .bind(model)
        .bind(dimensions as i64)
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to upsert logbook embedding: {}", e)))?;

        Ok(())
    }

    /// Get chunk IDs that don't have embeddings (or have a different model).
    pub async fn get_chunks_without_embeddings(
        &self,
        project_id: &str,
        model: &str,
        limit: u32,
    ) -> Result<Vec<(String, String)>> {
        let rows = sqlx::query(
            "SELECT c.id, c.content FROM logbook_chunks c
             LEFT JOIN logbook_embeddings e ON c.id = e.chunk_id AND e.model = ?
             WHERE c.project_id = ? AND e.chunk_id IS NULL
             LIMIT ?"
        )
        .bind(model)
        .bind(project_id)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to get logbook chunks without embeddings: {}", e)))?;

        Ok(rows.iter().map(|r| {
            (r.get::<String, _>("id"), r.get::<String, _>("content"))
        }).collect())
    }

    /// Check whether a project has any logbook embeddings stored.
    pub async fn has_embeddings(&self, project_id: &str) -> Result<bool> {
        let count: i64 = sqlx::query(
            "SELECT COUNT(*) as cnt FROM logbook_embeddings e
             JOIN logbook_chunks c ON c.id = e.chunk_id
             WHERE c.project_id = ?"
        )
        .bind(project_id)
        .fetch_one(&self.pool)
        .await
        .map(|r| r.get("cnt"))
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to check logbook embeddings: {}", e)))?;

        Ok(count > 0)
    }

    // ========================================================================
    // SEARCH
    // ========================================================================

    /// Full-text search using FTS5 BM25 ranking. Returns (chunk, rank_score).
    pub async fn search(&self, project_id: &str, query: &str, limit: u32) -> Result<Vec<(LogbookChunk, f64)>> {
        let rows = sqlx::query(
            "SELECT c.id, c.project_id, c.node_id, c.name, c.content, c.content_hash, c.source, c.updated_at,
                    rank
             FROM logbook_chunks_fts fts
             JOIN logbook_chunks c ON c.rowid = fts.rowid
             WHERE logbook_chunks_fts MATCH ?
               AND c.project_id = ?
             ORDER BY rank
             LIMIT ?"
        )
        .bind(query)
        .bind(project_id)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::RagSearchError(format!("Logbook FTS5 search failed: {}", e)))?;

        Ok(rows.iter().map(|r| {
            let chunk = row_to_chunk(r);
            let score: f64 = r.get("rank");
            (chunk, score)
        }).collect())
    }

    /// Search by embedding similarity — returns (chunk, cosine_similarity).
    pub async fn search_by_embedding(
        &self,
        project_id: &str,
        query_embedding: &[f32],
        limit: u32,
    ) -> Result<Vec<(LogbookChunk, f64)>> {
        use crate::rag::embeddings::{blob_to_embedding, cosine_similarity};

        let rows = sqlx::query(
            "SELECT c.id, c.project_id, c.node_id, c.name, c.content, c.content_hash, c.source, c.updated_at,
                    e.embedding
             FROM logbook_embeddings e
             JOIN logbook_chunks c ON c.id = e.chunk_id
             WHERE c.project_id = ?"
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::RagSearchError(format!("Logbook embedding search failed: {}", e)))?;

        let mut scored: Vec<(LogbookChunk, f64)> = rows.iter().map(|r| {
            let chunk = row_to_chunk(r);
            let blob: Vec<u8> = r.get("embedding");
            let stored_emb = blob_to_embedding(&blob);
            let sim = cosine_similarity(query_embedding, &stored_emb);
            (chunk, sim)
        }).collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit as usize);

        Ok(scored)
    }
}

/// Build a `LogbookChunk` from a query row that selects the full chunk columns.
fn row_to_chunk(r: &sqlx::sqlite::SqliteRow) -> LogbookChunk {
    LogbookChunk {
        id: r.get("id"),
        project_id: r.get("project_id"),
        node_id: r.get("node_id"),
        name: r.get("name"),
        content: r.get("content"),
        content_hash: r.get("content_hash"),
        source: r.get("source"),
        updated_at: r.get::<i64, _>("updated_at"),
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn create_test_repo() -> LogbookRepository {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let repo = LogbookRepository::new(pool);
        repo.initialize().await.unwrap();
        repo
    }

    fn test_chunk(id: &str, project_id: &str, node_id: &str, name: &str, content: &str) -> LogbookChunk {
        LogbookChunk {
            id: id.to_string(),
            project_id: project_id.to_string(),
            node_id: node_id.to_string(),
            name: name.to_string(),
            content: content.to_string(),
            content_hash: format!("hash-of-{}", content),
            source: "user".to_string(),
            updated_at: 1_700_000_000,
        }
    }

    #[tokio::test]
    async fn test_upsert_and_search() {
        let repo = create_test_repo().await;

        repo.upsert_chunk(&test_chunk("s1", "proj1", "n1", "Auth flow", "The login process validates credentials")).await.unwrap();
        repo.upsert_chunk(&test_chunk("s2", "proj1", "n1", "Storage", "Data is persisted to disk")).await.unwrap();

        let results = repo.search("proj1", "login", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.id, "s1");
    }

    #[tokio::test]
    async fn test_upsert_updates_in_place_and_resyncs_fts() {
        let repo = create_test_repo().await;

        repo.upsert_chunk(&test_chunk("s1", "proj1", "n1", "Topic", "original text about cats")).await.unwrap();
        // Edit the content — same id.
        repo.upsert_chunk(&test_chunk("s1", "proj1", "n1", "Topic", "rewritten text about dogs")).await.unwrap();

        // Old term gone, new term found (update trigger re-synced FTS).
        assert!(repo.search("proj1", "cats", 10).await.unwrap().is_empty());
        let hits = repo.search("proj1", "dogs", 10).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0.content, "rewritten text about dogs");
    }

    #[tokio::test]
    async fn test_get_section_hash() {
        let repo = create_test_repo().await;

        assert!(repo.get_section_hash("proj1", "s1").await.unwrap().is_none());

        let chunk = test_chunk("s1", "proj1", "n1", "T", "body");
        repo.upsert_chunk(&chunk).await.unwrap();

        assert_eq!(repo.get_section_hash("proj1", "s1").await.unwrap(), Some(chunk.content_hash));
    }

    #[tokio::test]
    async fn test_get_node_section_ids_and_delete_node() {
        let repo = create_test_repo().await;

        repo.upsert_chunk(&test_chunk("s1", "proj1", "n1", "A", "x")).await.unwrap();
        repo.upsert_chunk(&test_chunk("s2", "proj1", "n1", "B", "y")).await.unwrap();
        repo.upsert_chunk(&test_chunk("s3", "proj1", "n2", "C", "z")).await.unwrap();

        let mut ids = repo.get_node_section_ids("proj1", "n1").await.unwrap();
        ids.sort();
        assert_eq!(ids, vec!["s1".to_string(), "s2".to_string()]);

        repo.delete_node("proj1", "n1").await.unwrap();
        assert!(repo.get_node_section_ids("proj1", "n1").await.unwrap().is_empty());
        // n2 untouched
        assert_eq!(repo.get_node_section_ids("proj1", "n2").await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_delete_chunk() {
        let repo = create_test_repo().await;
        repo.upsert_chunk(&test_chunk("s1", "proj1", "n1", "A", "hello")).await.unwrap();
        repo.delete_chunk("s1").await.unwrap();
        assert!(repo.search("proj1", "hello", 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_embeddings_upsert_has_and_pending() {
        let repo = create_test_repo().await;
        repo.upsert_chunk(&test_chunk("s1", "proj1", "n1", "A", "alpha")).await.unwrap();
        repo.upsert_chunk(&test_chunk("s2", "proj1", "n1", "B", "beta")).await.unwrap();

        assert!(!repo.has_embeddings("proj1").await.unwrap());

        let pending = repo.get_chunks_without_embeddings("proj1", "test-model", 100).await.unwrap();
        assert_eq!(pending.len(), 2);

        let blob = crate::rag::embeddings::embedding_to_blob(&[0.1f32, 0.2]);
        repo.upsert_embedding("s1", &blob, "test-model", 2).await.unwrap();

        assert!(repo.has_embeddings("proj1").await.unwrap());
        let pending = repo.get_chunks_without_embeddings("proj1", "test-model", 100).await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].0, "s2");
    }

    #[tokio::test]
    async fn test_content_change_drops_stale_embedding() {
        let repo = create_test_repo().await;
        repo.upsert_chunk(&test_chunk("s1", "proj1", "n1", "A", "first")).await.unwrap();

        let blob = crate::rag::embeddings::embedding_to_blob(&[1.0f32, 0.0]);
        repo.upsert_embedding("s1", &blob, "test-model", 2).await.unwrap();
        assert!(repo.has_embeddings("proj1").await.unwrap());

        // Re-upsert with changed content (different hash) → stale embedding dropped.
        repo.upsert_chunk(&test_chunk("s1", "proj1", "n1", "A", "second")).await.unwrap();
        assert!(!repo.has_embeddings("proj1").await.unwrap());

        // Re-upsert with SAME content (same hash) → embedding preserved.
        let blob2 = crate::rag::embeddings::embedding_to_blob(&[0.0f32, 1.0]);
        repo.upsert_embedding("s1", &blob2, "test-model", 2).await.unwrap();
        repo.upsert_chunk(&test_chunk("s1", "proj1", "n1", "A", "second")).await.unwrap();
        assert!(repo.has_embeddings("proj1").await.unwrap());
    }

    #[tokio::test]
    async fn test_search_by_embedding_ranks_by_similarity() {
        let repo = create_test_repo().await;
        repo.upsert_chunk(&test_chunk("s1", "proj1", "n1", "auth", "auth content")).await.unwrap();
        repo.upsert_chunk(&test_chunk("s2", "proj1", "n1", "config", "config content")).await.unwrap();

        let emb1 = crate::rag::embeddings::embedding_to_blob(&[1.0f32, 0.0, 0.0]);
        let emb2 = crate::rag::embeddings::embedding_to_blob(&[0.0f32, 1.0, 0.0]);
        repo.upsert_embedding("s1", &emb1, "test", 3).await.unwrap();
        repo.upsert_embedding("s2", &emb2, "test", 3).await.unwrap();

        let query_vec = vec![0.9f32, 0.1, 0.0];
        let results = repo.search_by_embedding("proj1", &query_vec, 10).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0.id, "s1");
    }

    #[tokio::test]
    async fn test_project_isolation() {
        let repo = create_test_repo().await;
        repo.upsert_chunk(&test_chunk("s1", "projA", "n1", "T", "shared term apple")).await.unwrap();
        repo.upsert_chunk(&test_chunk("s2", "projB", "n1", "T", "shared term apple")).await.unwrap();

        let hits = repo.search("projA", "apple", 10).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0.project_id, "projA");
    }
}
