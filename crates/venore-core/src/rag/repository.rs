//! RAG Repository
//!
//! SQLite + FTS5 persistence for code index. Provides CRUD for files/chunks
//! and full-text search via BM25 ranking.

use sqlx::sqlite::SqlitePool;
use sqlx::Row;

use crate::rag::types::{IndexStatus, ModuleDep, ModuleInfo, RagChunk, RagFile, SymbolRef};
use crate::{Result, VenoreError};

/// SQLite-backed RAG repository with FTS5 search
pub struct RagRepository {
    pool: SqlitePool,
}

impl RagRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Access the underlying connection pool (for ad-hoc queries in sibling modules)
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Initialize tables, indexes, FTS5 virtual table, and triggers
    pub async fn initialize(&self) -> Result<()> {
        // Enable foreign keys
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to enable foreign keys: {}", e)))?;

        // Files table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS rag_files (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                file_path TEXT NOT NULL,
                relative_path TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                language TEXT,
                indexed_at TEXT NOT NULL,
                UNIQUE(project_id, relative_path)
            )"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create rag_files: {}", e)))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_rag_files_project ON rag_files(project_id)")
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to create rag_files index: {}", e)))?;

        // Chunks table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS rag_chunks (
                id TEXT PRIMARY KEY,
                file_id TEXT NOT NULL,
                project_id TEXT NOT NULL,
                chunk_type TEXT NOT NULL,
                name TEXT NOT NULL,
                content TEXT NOT NULL,
                line_start INTEGER NOT NULL,
                line_end INTEGER NOT NULL,
                relative_path TEXT NOT NULL,
                metadata TEXT,
                FOREIGN KEY (file_id) REFERENCES rag_files(id) ON DELETE CASCADE
            )"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create rag_chunks: {}", e)))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_rag_chunks_file ON rag_chunks(file_id)")
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to create chunks file index: {}", e)))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_rag_chunks_project ON rag_chunks(project_id)")
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to create chunks project index: {}", e)))?;

        // Composite index used by the symbol-ref linking pass in
        // `populate_graph` (UPDATE rag_symbol_refs … WHERE c.project_id = ?
        // AND c.name = ?). Without this index that query degrades to a full
        // chunks scan per symbol-ref row — fine when ref counts are small
        // (low-thousands) but explodes once a graph has 10k+ refs: the
        // opencode-dev repro logged a 66s update. With the composite index
        // the same update runs in well under a second.
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_rag_chunks_project_name ON rag_chunks(project_id, name)")
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to create chunks project_name index: {}", e)))?;

        // FTS5 virtual table (content-sync with rag_chunks)
        sqlx::query(
            "CREATE VIRTUAL TABLE IF NOT EXISTS rag_chunks_fts USING fts5(
                name, content, chunk_type, relative_path,
                content=rag_chunks, content_rowid=rowid
            )"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create FTS5 table: {}", e)))?;

        // Embeddings table (linked to rag_chunks via FK)
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS rag_embeddings (
                chunk_id TEXT PRIMARY KEY,
                embedding BLOB NOT NULL,
                model TEXT NOT NULL,
                dimensions INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY (chunk_id) REFERENCES rag_chunks(id) ON DELETE CASCADE
            )"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create rag_embeddings: {}", e)))?;

        // Triggers to keep FTS5 in sync
        sqlx::query(
            "CREATE TRIGGER IF NOT EXISTS rag_fts_insert AFTER INSERT ON rag_chunks BEGIN
                INSERT INTO rag_chunks_fts(rowid, name, content, chunk_type, relative_path)
                VALUES (new.rowid, new.name, new.content, new.chunk_type, new.relative_path);
            END"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create FTS insert trigger: {}", e)))?;

        sqlx::query(
            "CREATE TRIGGER IF NOT EXISTS rag_fts_delete AFTER DELETE ON rag_chunks BEGIN
                INSERT INTO rag_chunks_fts(rag_chunks_fts, rowid, name, content, chunk_type, relative_path)
                VALUES ('delete', old.rowid, old.name, old.content, old.chunk_type, old.relative_path);
            END"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create FTS delete trigger: {}", e)))?;

        // =================================================================
        // GRAPH TABLES — structural relationships
        // =================================================================

        // File-to-module mapping
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS rag_file_modules (
                file_id TEXT NOT NULL,
                project_id TEXT NOT NULL,
                module_name TEXT NOT NULL,
                module_path TEXT NOT NULL,
                is_entry_point INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (file_id),
                FOREIGN KEY (file_id) REFERENCES rag_files(id) ON DELETE CASCADE
            )"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create rag_file_modules: {}", e)))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_rag_file_modules_project ON rag_file_modules(project_id)")
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to create file_modules project index: {}", e)))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_rag_file_modules_module ON rag_file_modules(project_id, module_name)")
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to create file_modules module index: {}", e)))?;

        // Module-level dependency edges
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS rag_module_deps (
                project_id TEXT NOT NULL,
                from_module TEXT NOT NULL,
                to_module TEXT NOT NULL,
                dep_type TEXT NOT NULL DEFAULT 'import',
                PRIMARY KEY (project_id, from_module, to_module)
            )"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create rag_module_deps: {}", e)))?;

        // Symbol-level reference edges
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS rag_symbol_refs (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                from_chunk_id TEXT NOT NULL,
                to_chunk_id TEXT,
                to_symbol_name TEXT NOT NULL,
                to_file_path TEXT,
                ref_type TEXT NOT NULL,
                line_number INTEGER,
                FOREIGN KEY (from_chunk_id) REFERENCES rag_chunks(id) ON DELETE CASCADE
            )"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create rag_symbol_refs: {}", e)))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_rag_symbol_refs_project ON rag_symbol_refs(project_id)")
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to create symbol_refs project index: {}", e)))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_rag_symbol_refs_from ON rag_symbol_refs(from_chunk_id)")
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to create symbol_refs from index: {}", e)))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_rag_symbol_refs_to ON rag_symbol_refs(to_chunk_id)")
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to create symbol_refs to index: {}", e)))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_rag_symbol_refs_name ON rag_symbol_refs(project_id, to_symbol_name)")
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to create symbol_refs name index: {}", e)))?;

        tracing::info!("RAG repository initialized (with graph tables)");
        Ok(())
    }

    // ========================================================================
    // FILES
    // ========================================================================

    /// Upsert a file record (insert or update on conflict)
    pub async fn upsert_file(&self, file: &RagFile) -> Result<()> {
        sqlx::query(
            "INSERT INTO rag_files (id, project_id, file_path, relative_path, content_hash, language, indexed_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(project_id, relative_path) DO UPDATE SET
                file_path = excluded.file_path,
                content_hash = excluded.content_hash,
                language = excluded.language,
                indexed_at = excluded.indexed_at"
        )
        .bind(&file.id)
        .bind(&file.project_id)
        .bind(&file.file_path)
        .bind(&file.relative_path)
        .bind(&file.content_hash)
        .bind(&file.language)
        .bind(&file.indexed_at)
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to upsert file: {}", e)))?;

        Ok(())
    }

    /// Get all indexed files for a project
    pub async fn get_files(&self, project_id: &str) -> Result<Vec<RagFile>> {
        let rows = sqlx::query(
            "SELECT id, project_id, file_path, relative_path, content_hash, language, indexed_at
             FROM rag_files WHERE project_id = ?"
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to get files: {}", e)))?;

        Ok(rows.iter().map(|r| RagFile {
            id: r.get("id"),
            project_id: r.get("project_id"),
            file_path: r.get("file_path"),
            relative_path: r.get("relative_path"),
            content_hash: r.get("content_hash"),
            language: r.get("language"),
            indexed_at: r.get("indexed_at"),
        }).collect())
    }

    /// Get a file by its relative path within a project
    pub async fn get_file_by_path(&self, project_id: &str, relative_path: &str) -> Result<Option<RagFile>> {
        let row = sqlx::query(
            "SELECT id, project_id, file_path, relative_path, content_hash, language, indexed_at
             FROM rag_files WHERE project_id = ? AND relative_path = ?"
        )
        .bind(project_id)
        .bind(relative_path)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to get file by path: {}", e)))?;

        Ok(row.map(|r| RagFile {
            id: r.get("id"),
            project_id: r.get("project_id"),
            file_path: r.get("file_path"),
            relative_path: r.get("relative_path"),
            content_hash: r.get("content_hash"),
            language: r.get("language"),
            indexed_at: r.get("indexed_at"),
        }))
    }

    /// Delete a file and its chunks, module mapping, and symbol refs
    pub async fn delete_file(&self, file_id: &str) -> Result<()> {
        // Delete symbol refs that originate from this file's chunks
        self.delete_symbol_refs_for_file(file_id).await?;

        // Delete file-module mapping
        sqlx::query("DELETE FROM rag_file_modules WHERE file_id = ?")
            .bind(file_id)
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to delete file module mapping: {}", e)))?;

        // Manually delete chunks first (FTS trigger fires on delete)
        sqlx::query("DELETE FROM rag_chunks WHERE file_id = ?")
            .bind(file_id)
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to delete chunks for file: {}", e)))?;

        sqlx::query("DELETE FROM rag_files WHERE id = ?")
            .bind(file_id)
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to delete file: {}", e)))?;

        Ok(())
    }

    /// Delete entire project index (files, chunks, graph data)
    pub async fn delete_project_index(&self, project_id: &str) -> Result<()> {
        // Delete graph data first
        self.delete_symbol_refs(project_id).await?;
        self.delete_module_deps(project_id).await?;
        self.delete_module_mappings(project_id).await?;

        // Delete chunks (triggers FTS cleanup)
        sqlx::query("DELETE FROM rag_chunks WHERE project_id = ?")
            .bind(project_id)
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to delete project chunks: {}", e)))?;

        sqlx::query("DELETE FROM rag_files WHERE project_id = ?")
            .bind(project_id)
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to delete project files: {}", e)))?;

        Ok(())
    }

    // ========================================================================
    // CHUNKS
    // ========================================================================

    /// Insert multiple chunks (within a transaction)
    pub async fn insert_chunks(&self, chunks: &[RagChunk]) -> Result<()> {
        for chunk in chunks {
            sqlx::query(
                "INSERT INTO rag_chunks (id, file_id, project_id, chunk_type, name, content, line_start, line_end, relative_path, metadata)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(&chunk.id)
            .bind(&chunk.file_id)
            .bind(&chunk.project_id)
            .bind(&chunk.chunk_type)
            .bind(&chunk.name)
            .bind(&chunk.content)
            .bind(chunk.line_start as i64)
            .bind(chunk.line_end as i64)
            .bind(&chunk.relative_path)
            .bind(&chunk.metadata)
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to insert chunk: {}", e)))?;
        }

        Ok(())
    }

    /// Delete all chunks for a file
    pub async fn delete_chunks_for_file(&self, file_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM rag_chunks WHERE file_id = ?")
            .bind(file_id)
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to delete chunks: {}", e)))?;

        Ok(())
    }

    // ========================================================================
    // SEARCH
    // ========================================================================

    /// Full-text search using FTS5 BM25 ranking
    pub async fn search(&self, project_id: &str, query: &str, limit: u32) -> Result<Vec<(RagChunk, f64)>> {
        let rows = sqlx::query(
            "SELECT c.id, c.file_id, c.project_id, c.chunk_type, c.name, c.content,
                    c.line_start, c.line_end, c.relative_path, c.metadata,
                    rank
             FROM rag_chunks_fts fts
             JOIN rag_chunks c ON c.rowid = fts.rowid
             WHERE rag_chunks_fts MATCH ?
               AND c.project_id = ?
             ORDER BY rank
             LIMIT ?"
        )
        .bind(query)
        .bind(project_id)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::RagSearchError(format!("FTS5 search failed: {}", e)))?;

        Ok(rows.iter().map(|r| {
            let chunk = RagChunk {
                id: r.get("id"),
                file_id: r.get("file_id"),
                project_id: r.get("project_id"),
                chunk_type: r.get("chunk_type"),
                name: r.get("name"),
                content: r.get("content"),
                line_start: r.get::<i64, _>("line_start") as u32,
                line_end: r.get::<i64, _>("line_end") as u32,
                relative_path: r.get("relative_path"),
                metadata: r.get("metadata"),
            };
            let score: f64 = r.get("rank");
            (chunk, score)
        }).collect())
    }

    // ========================================================================
    // EMBEDDINGS
    // ========================================================================

    /// Insert or replace an embedding for a chunk
    pub async fn upsert_embedding(
        &self,
        chunk_id: &str,
        embedding: &[u8],
        model: &str,
        dimensions: u32,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO rag_embeddings (chunk_id, embedding, model, dimensions, created_at)
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
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to upsert embedding: {}", e)))?;

        Ok(())
    }

    /// Get chunk IDs that don't have embeddings (or have a different model)
    pub async fn get_chunks_without_embeddings(
        &self,
        project_id: &str,
        model: &str,
        limit: u32,
    ) -> Result<Vec<(String, String)>> {
        let rows = sqlx::query(
            "SELECT c.id, c.content FROM rag_chunks c
             LEFT JOIN rag_embeddings e ON c.id = e.chunk_id AND e.model = ?
             WHERE c.project_id = ? AND e.chunk_id IS NULL
             LIMIT ?"
        )
        .bind(model)
        .bind(project_id)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to get chunks without embeddings: {}", e)))?;

        Ok(rows.iter().map(|r| {
            (r.get::<String, _>("id"), r.get::<String, _>("content"))
        }).collect())
    }

    /// Check whether a project has any embeddings stored
    pub async fn has_embeddings(&self, project_id: &str) -> Result<bool> {
        let count: i64 = sqlx::query(
            "SELECT COUNT(*) as cnt FROM rag_embeddings e
             JOIN rag_chunks c ON c.id = e.chunk_id
             WHERE c.project_id = ?"
        )
        .bind(project_id)
        .fetch_one(&self.pool)
        .await
        .map(|r| r.get("cnt"))
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to check embeddings: {}", e)))?;

        Ok(count > 0)
    }

    /// Search by embedding similarity — returns (chunk, cosine_similarity)
    pub async fn search_by_embedding(
        &self,
        project_id: &str,
        query_embedding: &[f32],
        limit: u32,
    ) -> Result<Vec<(RagChunk, f64)>> {
        use crate::rag::embeddings::{blob_to_embedding, cosine_similarity};

        // Load all embeddings for the project (in-memory cosine similarity)
        // This is acceptable for ≤10K chunks (30MB). For larger projects,
        // a vector DB extension would be needed.
        let rows = sqlx::query(
            "SELECT c.id, c.file_id, c.project_id, c.chunk_type, c.name, c.content,
                    c.line_start, c.line_end, c.relative_path, c.metadata,
                    e.embedding
             FROM rag_embeddings e
             JOIN rag_chunks c ON c.id = e.chunk_id
             WHERE c.project_id = ?"
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::RagSearchError(format!("Embedding search failed: {}", e)))?;

        let mut scored: Vec<(RagChunk, f64)> = rows.iter().map(|r| {
            let chunk = RagChunk {
                id: r.get("id"),
                file_id: r.get("file_id"),
                project_id: r.get("project_id"),
                chunk_type: r.get("chunk_type"),
                name: r.get("name"),
                content: r.get("content"),
                line_start: r.get::<i64, _>("line_start") as u32,
                line_end: r.get::<i64, _>("line_end") as u32,
                relative_path: r.get("relative_path"),
                metadata: r.get("metadata"),
            };
            let blob: Vec<u8> = r.get("embedding");
            let stored_emb = blob_to_embedding(&blob);
            let sim = cosine_similarity(query_embedding, &stored_emb);
            (chunk, sim)
        }).collect();

        // Sort by similarity descending
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit as usize);

        Ok(scored)
    }

    // ========================================================================
    // GRAPH — File-module mapping
    // ========================================================================

    /// Upsert a file-to-module mapping
    pub async fn upsert_file_module(
        &self,
        file_id: &str,
        project_id: &str,
        module_name: &str,
        module_path: &str,
        is_entry_point: bool,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO rag_file_modules (file_id, project_id, module_name, module_path, is_entry_point)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(file_id) DO UPDATE SET
                module_name = excluded.module_name,
                module_path = excluded.module_path,
                is_entry_point = excluded.is_entry_point"
        )
        .bind(file_id)
        .bind(project_id)
        .bind(module_name)
        .bind(module_path)
        .bind(is_entry_point as i32)
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to upsert file module: {}", e)))?;

        Ok(())
    }

    /// Get all files belonging to a module
    pub async fn get_module_files(&self, project_id: &str, module_name: &str) -> Result<Vec<RagFile>> {
        let rows = sqlx::query(
            "SELECT f.id, f.project_id, f.file_path, f.relative_path, f.content_hash, f.language, f.indexed_at
             FROM rag_files f
             JOIN rag_file_modules fm ON fm.file_id = f.id
             WHERE fm.project_id = ? AND fm.module_name = ?"
        )
        .bind(project_id)
        .bind(module_name)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to get module files: {}", e)))?;

        Ok(rows.iter().map(|r| RagFile {
            id: r.get("id"),
            project_id: r.get("project_id"),
            file_path: r.get("file_path"),
            relative_path: r.get("relative_path"),
            content_hash: r.get("content_hash"),
            language: r.get("language"),
            indexed_at: r.get("indexed_at"),
        }).collect())
    }

    /// Get the module name for a given file
    pub async fn get_file_module(&self, file_id: &str) -> Result<Option<String>> {
        let row = sqlx::query("SELECT module_name FROM rag_file_modules WHERE file_id = ?")
            .bind(file_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to get file module: {}", e)))?;

        Ok(row.map(|r| r.get("module_name")))
    }

    /// Get all modules for a project (with file counts)
    pub async fn get_modules(&self, project_id: &str) -> Result<Vec<ModuleInfo>> {
        let rows = sqlx::query(
            "SELECT module_name, module_path, COUNT(*) as file_count
             FROM rag_file_modules
             WHERE project_id = ?
             GROUP BY module_name, module_path
             ORDER BY module_name"
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to get modules: {}", e)))?;

        Ok(rows.iter().map(|r| ModuleInfo {
            project_id: project_id.to_string(),
            module_name: r.get("module_name"),
            module_path: r.get("module_path"),
            file_count: r.get::<i64, _>("file_count") as u32,
        }).collect())
    }

    /// Delete all file-module mappings for a project
    pub async fn delete_module_mappings(&self, project_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM rag_file_modules WHERE project_id = ?")
            .bind(project_id)
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to delete module mappings: {}", e)))?;

        Ok(())
    }

    // ========================================================================
    // GRAPH — Module dependencies
    // ========================================================================

    /// Upsert a module dependency edge
    pub async fn upsert_module_dep(
        &self,
        project_id: &str,
        from_module: &str,
        to_module: &str,
        dep_type: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO rag_module_deps (project_id, from_module, to_module, dep_type)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(project_id, from_module, to_module) DO UPDATE SET
                dep_type = excluded.dep_type"
        )
        .bind(project_id)
        .bind(from_module)
        .bind(to_module)
        .bind(dep_type)
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to upsert module dep: {}", e)))?;

        Ok(())
    }

    /// Get every module dep for a project in a single query.
    /// Used to build a `module_name → ModuleConnectionInfo` map in one pass
    /// instead of N+1 queries when analyzing layers for many modules.
    pub async fn get_all_module_deps(&self, project_id: &str) -> Result<Vec<ModuleDep>> {
        let rows = sqlx::query(
            "SELECT from_module, to_module, dep_type
             FROM rag_module_deps
             WHERE project_id = ?"
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to get all module deps: {}", e)))?;

        Ok(rows.iter().map(|r| ModuleDep {
            from_module: r.get("from_module"),
            to_module: r.get("to_module"),
            dep_type: r.get("dep_type"),
        }).collect())
    }

    /// Get modules that a given module depends on
    pub async fn get_module_deps(&self, project_id: &str, module_name: &str) -> Result<Vec<ModuleDep>> {
        let rows = sqlx::query(
            "SELECT from_module, to_module, dep_type
             FROM rag_module_deps
             WHERE project_id = ? AND from_module = ?"
        )
        .bind(project_id)
        .bind(module_name)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to get module deps: {}", e)))?;

        Ok(rows.iter().map(|r| ModuleDep {
            from_module: r.get("from_module"),
            to_module: r.get("to_module"),
            dep_type: r.get("dep_type"),
        }).collect())
    }

    /// Get modules that depend on a given module
    pub async fn get_module_dependents(&self, project_id: &str, module_name: &str) -> Result<Vec<ModuleDep>> {
        let rows = sqlx::query(
            "SELECT from_module, to_module, dep_type
             FROM rag_module_deps
             WHERE project_id = ? AND to_module = ?"
        )
        .bind(project_id)
        .bind(module_name)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to get module dependents: {}", e)))?;

        Ok(rows.iter().map(|r| ModuleDep {
            from_module: r.get("from_module"),
            to_module: r.get("to_module"),
            dep_type: r.get("dep_type"),
        }).collect())
    }

    /// Delete all module dependencies for a project
    pub async fn delete_module_deps(&self, project_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM rag_module_deps WHERE project_id = ?")
            .bind(project_id)
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to delete module deps: {}", e)))?;

        Ok(())
    }

    // ========================================================================
    // GRAPH — Symbol references
    // ========================================================================

    /// Insert symbol references in batch
    pub async fn insert_symbol_refs(&self, refs: &[SymbolRef]) -> Result<()> {
        for r in refs {
            sqlx::query(
                "INSERT INTO rag_symbol_refs (id, project_id, from_chunk_id, to_chunk_id, to_symbol_name, to_file_path, ref_type, line_number)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(&r.id)
            .bind(&r.project_id)
            .bind(&r.from_chunk_id)
            .bind(&r.to_chunk_id)
            .bind(&r.to_symbol_name)
            .bind(&r.to_file_path)
            .bind(&r.ref_type)
            .bind(r.line_number.map(|n| n as i64))
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to insert symbol ref: {}", e)))?;
        }

        Ok(())
    }

    /// Get all references FROM a chunk (what does this symbol reference?)
    pub async fn get_symbol_refs_from(&self, chunk_id: &str) -> Result<Vec<SymbolRef>> {
        let rows = sqlx::query(
            "SELECT id, project_id, from_chunk_id, to_chunk_id, to_symbol_name, to_file_path, ref_type, line_number
             FROM rag_symbol_refs WHERE from_chunk_id = ?"
        )
        .bind(chunk_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to get symbol refs from: {}", e)))?;

        Ok(rows.iter().map(|r| SymbolRef {
            id: r.get("id"),
            project_id: r.get("project_id"),
            from_chunk_id: r.get("from_chunk_id"),
            to_chunk_id: r.get("to_chunk_id"),
            to_symbol_name: r.get("to_symbol_name"),
            to_file_path: r.get("to_file_path"),
            ref_type: r.get("ref_type"),
            line_number: r.get::<Option<i64>, _>("line_number").map(|n| n as u32),
        }).collect())
    }

    /// Get all references TO a symbol name (who references this symbol?)
    pub async fn get_symbol_refs_to(&self, project_id: &str, symbol_name: &str) -> Result<Vec<SymbolRef>> {
        let rows = sqlx::query(
            "SELECT id, project_id, from_chunk_id, to_chunk_id, to_symbol_name, to_file_path, ref_type, line_number
             FROM rag_symbol_refs WHERE project_id = ? AND to_symbol_name = ?"
        )
        .bind(project_id)
        .bind(symbol_name)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to get symbol refs to: {}", e)))?;

        Ok(rows.iter().map(|r| SymbolRef {
            id: r.get("id"),
            project_id: r.get("project_id"),
            from_chunk_id: r.get("from_chunk_id"),
            to_chunk_id: r.get("to_chunk_id"),
            to_symbol_name: r.get("to_symbol_name"),
            to_file_path: r.get("to_file_path"),
            ref_type: r.get("ref_type"),
            line_number: r.get::<Option<i64>, _>("line_number").map(|n| n as u32),
        }).collect())
    }

    /// Delete all symbol references for a file (via from_chunk_id → file_id)
    pub async fn delete_symbol_refs_for_file(&self, file_id: &str) -> Result<()> {
        sqlx::query(
            "DELETE FROM rag_symbol_refs WHERE from_chunk_id IN (
                SELECT id FROM rag_chunks WHERE file_id = ?
            )"
        )
        .bind(file_id)
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to delete symbol refs for file: {}", e)))?;

        Ok(())
    }

    /// Delete all symbol references for a project
    pub async fn delete_symbol_refs(&self, project_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM rag_symbol_refs WHERE project_id = ?")
            .bind(project_id)
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to delete symbol refs: {}", e)))?;

        Ok(())
    }

    /// Resolve dangling symbol references by matching to_symbol_name → chunk name
    pub async fn resolve_symbol_refs(&self, project_id: &str) -> Result<u32> {
        let result = sqlx::query(
            "UPDATE rag_symbol_refs SET to_chunk_id = (
                SELECT c.id FROM rag_chunks c
                WHERE c.project_id = rag_symbol_refs.project_id
                  AND c.name = rag_symbol_refs.to_symbol_name
                LIMIT 1
            )
            WHERE project_id = ? AND to_chunk_id IS NULL"
        )
        .bind(project_id)
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to resolve symbol refs: {}", e)))?;

        Ok(result.rows_affected() as u32)
    }

    /// FTS5 search scoped to a specific module
    pub async fn search_within_module(
        &self,
        project_id: &str,
        module_name: &str,
        query: &str,
        limit: u32,
    ) -> Result<Vec<(RagChunk, f64)>> {
        let rows = sqlx::query(
            "SELECT c.id, c.file_id, c.project_id, c.chunk_type, c.name, c.content,
                    c.line_start, c.line_end, c.relative_path, c.metadata,
                    rank
             FROM rag_chunks_fts fts
             JOIN rag_chunks c ON c.rowid = fts.rowid
             JOIN rag_file_modules fm ON fm.file_id = c.file_id
             WHERE rag_chunks_fts MATCH ?
               AND c.project_id = ?
               AND fm.module_name = ?
             ORDER BY rank
             LIMIT ?"
        )
        .bind(query)
        .bind(project_id)
        .bind(module_name)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::RagSearchError(format!("Module-scoped FTS5 search failed: {}", e)))?;

        Ok(rows.iter().map(|r| {
            let chunk = RagChunk {
                id: r.get("id"),
                file_id: r.get("file_id"),
                project_id: r.get("project_id"),
                chunk_type: r.get("chunk_type"),
                name: r.get("name"),
                content: r.get("content"),
                line_start: r.get::<i64, _>("line_start") as u32,
                line_end: r.get::<i64, _>("line_end") as u32,
                relative_path: r.get("relative_path"),
                metadata: r.get("metadata"),
            };
            let score: f64 = r.get("rank");
            (chunk, score)
        }).collect())
    }

    // ========================================================================
    // STATUS
    // ========================================================================

    /// Get indexing status for a project
    pub async fn get_index_status(&self, project_id: &str) -> Result<IndexStatus> {
        let file_count: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM rag_files WHERE project_id = ?")
            .bind(project_id)
            .fetch_one(&self.pool)
            .await
            .map(|r| r.get("cnt"))
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to count files: {}", e)))?;

        let chunk_count: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM rag_chunks WHERE project_id = ?")
            .bind(project_id)
            .fetch_one(&self.pool)
            .await
            .map(|r| r.get("cnt"))
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to count chunks: {}", e)))?;

        let last_indexed: Option<String> = sqlx::query(
            "SELECT MAX(indexed_at) as last_at FROM rag_files WHERE project_id = ?"
        )
        .bind(project_id)
        .fetch_one(&self.pool)
        .await
        .map(|r| r.get("last_at"))
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to get last indexed: {}", e)))?;

        Ok(IndexStatus {
            project_id: project_id.to_string(),
            is_indexed: file_count > 0,
            total_files: file_count as u32,
            total_chunks: chunk_count as u32,
            last_indexed_at: last_indexed,
        })
    }

    // ========================================================================
    // Overview Chunks (fallback when FTS returns nothing)
    // ========================================================================

    /// Fetch high-value overview chunks for a project.
    ///
    /// Language-agnostic: no hardcoded filenames. Returns `file`-type chunks
    /// ordered by heuristic priority based on path depth and name patterns:
    /// 1. README (score 100) — project description
    /// 2. Root-level files (score 90) — whatever manifests/configs the project has
    /// 3. Shallow files, depth=1 (score 70) — top-level source dirs
    /// 4. Deeper file chunks (score 50)
    ///
    /// Used as fallback when FTS5 returns no results, ensuring the LLM
    /// always has project context regardless of query language.
    pub async fn fetch_overview_chunks(
        &self,
        project_id: &str,
        limit: u32,
    ) -> Result<Vec<(RagChunk, f64)>> {
        let rows = sqlx::query(
            r#"
            SELECT
                id, file_id, project_id, chunk_type, name, content,
                line_start, line_end, relative_path, metadata,
                CASE
                    WHEN LOWER(relative_path) LIKE '%readme%' THEN 100.0
                    WHEN relative_path NOT LIKE '%/%' THEN 90.0
                    WHEN LENGTH(relative_path) - LENGTH(REPLACE(relative_path, '/', '')) = 1 THEN 70.0
                    ELSE 50.0
                END as priority
            FROM rag_chunks
            WHERE project_id = ?
              AND chunk_type = 'file'
            ORDER BY priority DESC, LENGTH(content) DESC
            LIMIT ?
            "#,
        )
        .bind(project_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to fetch overview chunks: {}", e)))?;

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            let chunk = RagChunk {
                id: row.get("id"),
                file_id: row.get("file_id"),
                project_id: row.get("project_id"),
                chunk_type: row.get("chunk_type"),
                name: row.get("name"),
                content: row.get("content"),
                line_start: row.get::<i64, _>("line_start") as u32,
                line_end: row.get::<i64, _>("line_end") as u32,
                relative_path: row.get("relative_path"),
                metadata: row.get("metadata"),
            };
            let score: f64 = row.get("priority");
            results.push((chunk, score));
        }

        Ok(results)
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn create_test_repo() -> RagRepository {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let repo = RagRepository::new(pool);
        repo.initialize().await.unwrap();
        repo
    }

    fn test_file(id: &str, project_id: &str, relative_path: &str) -> RagFile {
        RagFile {
            id: id.to_string(),
            project_id: project_id.to_string(),
            file_path: format!("/project/{}", relative_path),
            relative_path: relative_path.to_string(),
            content_hash: "abc123".to_string(),
            language: Some("typescript".to_string()),
            indexed_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn test_chunk(id: &str, file_id: &str, project_id: &str, name: &str, content: &str) -> RagChunk {
        RagChunk {
            id: id.to_string(),
            file_id: file_id.to_string(),
            project_id: project_id.to_string(),
            chunk_type: "function".to_string(),
            name: name.to_string(),
            content: content.to_string(),
            line_start: 1,
            line_end: 5,
            relative_path: "src/utils.ts".to_string(),
            metadata: None,
        }
    }

    #[tokio::test]
    async fn test_upsert_and_get_files() {
        let repo = create_test_repo().await;

        let file = test_file("f1", "proj1", "src/index.ts");
        repo.upsert_file(&file).await.unwrap();

        let files = repo.get_files("proj1").await.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].relative_path, "src/index.ts");
    }

    #[tokio::test]
    async fn test_upsert_updates_on_conflict() {
        let repo = create_test_repo().await;

        let mut file = test_file("f1", "proj1", "src/index.ts");
        repo.upsert_file(&file).await.unwrap();

        // Update hash
        file.content_hash = "new_hash".to_string();
        repo.upsert_file(&file).await.unwrap();

        let files = repo.get_files("proj1").await.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].content_hash, "new_hash");
    }

    #[tokio::test]
    async fn test_get_file_by_path() {
        let repo = create_test_repo().await;

        let file = test_file("f1", "proj1", "src/utils.ts");
        repo.upsert_file(&file).await.unwrap();

        let found = repo.get_file_by_path("proj1", "src/utils.ts").await.unwrap();
        assert!(found.is_some());

        let missing = repo.get_file_by_path("proj1", "nonexistent.ts").await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_delete_file_cascades_chunks() {
        let repo = create_test_repo().await;

        let file = test_file("f1", "proj1", "src/utils.ts");
        repo.upsert_file(&file).await.unwrap();

        let chunk = test_chunk("c1", "f1", "proj1", "greet", "function greet() {}");
        repo.insert_chunks(&[chunk]).await.unwrap();

        repo.delete_file("f1").await.unwrap();

        let files = repo.get_files("proj1").await.unwrap();
        assert!(files.is_empty());

        // Chunks should also be gone
        let status = repo.get_index_status("proj1").await.unwrap();
        assert_eq!(status.total_chunks, 0);
    }

    #[tokio::test]
    async fn test_insert_and_search_chunks() {
        let repo = create_test_repo().await;

        let file = test_file("f1", "proj1", "src/utils.ts");
        repo.upsert_file(&file).await.unwrap();

        let chunks = vec![
            test_chunk("c1", "f1", "proj1", "getUserById", "async function getUserById(id: string) { return db.find(id); }"),
            test_chunk("c2", "f1", "proj1", "createUser", "async function createUser(name: string) { return db.insert(name); }"),
        ];
        repo.insert_chunks(&chunks).await.unwrap();

        let results = repo.search("proj1", "getUserById", 10).await.unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].0.name, "getUserById");
    }

    #[tokio::test]
    async fn test_delete_project_index() {
        let repo = create_test_repo().await;

        let file = test_file("f1", "proj1", "src/utils.ts");
        repo.upsert_file(&file).await.unwrap();

        let chunk = test_chunk("c1", "f1", "proj1", "test", "function test() {}");
        repo.insert_chunks(&[chunk]).await.unwrap();

        repo.delete_project_index("proj1").await.unwrap();

        let status = repo.get_index_status("proj1").await.unwrap();
        assert!(!status.is_indexed);
        assert_eq!(status.total_files, 0);
        assert_eq!(status.total_chunks, 0);
    }

    #[tokio::test]
    async fn test_index_status() {
        let repo = create_test_repo().await;

        // Empty project
        let status = repo.get_index_status("proj1").await.unwrap();
        assert!(!status.is_indexed);
        assert_eq!(status.total_files, 0);

        // Add file + chunks
        let file = test_file("f1", "proj1", "src/index.ts");
        repo.upsert_file(&file).await.unwrap();

        let chunks = vec![
            test_chunk("c1", "f1", "proj1", "a", "fn a() {}"),
            test_chunk("c2", "f1", "proj1", "b", "fn b() {}"),
        ];
        repo.insert_chunks(&chunks).await.unwrap();

        let status = repo.get_index_status("proj1").await.unwrap();
        assert!(status.is_indexed);
        assert_eq!(status.total_files, 1);
        assert_eq!(status.total_chunks, 2);
    }

    #[tokio::test]
    async fn test_embedding_upsert_and_has() {
        let repo = create_test_repo().await;

        let file = test_file("f1", "proj1", "src/utils.ts");
        repo.upsert_file(&file).await.unwrap();

        let chunk = test_chunk("c1", "f1", "proj1", "greet", "function greet() {}");
        repo.insert_chunks(&[chunk]).await.unwrap();

        // No embeddings initially
        assert!(!repo.has_embeddings("proj1").await.unwrap());

        // Insert an embedding
        let fake_embedding = crate::rag::embeddings::embedding_to_blob(&[0.1f32, 0.2, 0.3]);
        repo.upsert_embedding("c1", &fake_embedding, "test-model", 3).await.unwrap();

        // Now has embeddings
        assert!(repo.has_embeddings("proj1").await.unwrap());
    }

    #[tokio::test]
    async fn test_get_chunks_without_embeddings() {
        let repo = create_test_repo().await;

        let file = test_file("f1", "proj1", "src/utils.ts");
        repo.upsert_file(&file).await.unwrap();

        let chunks = vec![
            test_chunk("c1", "f1", "proj1", "a", "fn a() {}"),
            test_chunk("c2", "f1", "proj1", "b", "fn b() {}"),
        ];
        repo.insert_chunks(&chunks).await.unwrap();

        // All chunks should be missing embeddings
        let pending = repo.get_chunks_without_embeddings("proj1", "test-model", 100).await.unwrap();
        assert_eq!(pending.len(), 2);

        // Embed one chunk
        let blob = crate::rag::embeddings::embedding_to_blob(&[0.1f32, 0.2]);
        repo.upsert_embedding("c1", &blob, "test-model", 2).await.unwrap();

        // Only one should remain
        let pending = repo.get_chunks_without_embeddings("proj1", "test-model", 100).await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].0, "c2");
    }

    // ========================================================================
    // GRAPH TESTS
    // ========================================================================

    #[tokio::test]
    async fn test_upsert_and_get_file_module() {
        let repo = create_test_repo().await;

        let file = test_file("f1", "proj1", "src/auth/index.ts");
        repo.upsert_file(&file).await.unwrap();

        repo.upsert_file_module("f1", "proj1", "auth", "src/auth", true).await.unwrap();

        let module = repo.get_file_module("f1").await.unwrap();
        assert_eq!(module, Some("auth".to_string()));

        let files = repo.get_module_files("proj1", "auth").await.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].id, "f1");
    }

    #[tokio::test]
    async fn test_get_modules_with_file_counts() {
        let repo = create_test_repo().await;

        let f1 = test_file("f1", "proj1", "src/auth/index.ts");
        let f2 = test_file("f2", "proj1", "src/auth/login.ts");
        let f3 = test_file("f3", "proj1", "src/utils/helpers.ts");
        repo.upsert_file(&f1).await.unwrap();
        repo.upsert_file(&f2).await.unwrap();
        repo.upsert_file(&f3).await.unwrap();

        repo.upsert_file_module("f1", "proj1", "auth", "src/auth", true).await.unwrap();
        repo.upsert_file_module("f2", "proj1", "auth", "src/auth", false).await.unwrap();
        repo.upsert_file_module("f3", "proj1", "utils", "src/utils", true).await.unwrap();

        let modules = repo.get_modules("proj1").await.unwrap();
        assert_eq!(modules.len(), 2);
        assert_eq!(modules[0].module_name, "auth");
        assert_eq!(modules[0].file_count, 2);
        assert_eq!(modules[1].module_name, "utils");
        assert_eq!(modules[1].file_count, 1);
    }

    #[tokio::test]
    async fn test_module_deps_and_dependents() {
        let repo = create_test_repo().await;

        repo.upsert_module_dep("proj1", "auth", "utils", "import").await.unwrap();
        repo.upsert_module_dep("proj1", "auth", "db", "import").await.unwrap();
        repo.upsert_module_dep("proj1", "api", "auth", "import").await.unwrap();

        // auth depends on utils and db
        let deps = repo.get_module_deps("proj1", "auth").await.unwrap();
        assert_eq!(deps.len(), 2);

        // auth is depended on by api
        let dependents = repo.get_module_dependents("proj1", "auth").await.unwrap();
        assert_eq!(dependents.len(), 1);
        assert_eq!(dependents[0].from_module, "api");
    }

    #[tokio::test]
    async fn test_module_dep_upsert_idempotent() {
        let repo = create_test_repo().await;

        repo.upsert_module_dep("proj1", "auth", "utils", "import").await.unwrap();
        repo.upsert_module_dep("proj1", "auth", "utils", "import").await.unwrap();

        let deps = repo.get_module_deps("proj1", "auth").await.unwrap();
        assert_eq!(deps.len(), 1);
    }

    #[tokio::test]
    async fn test_symbol_refs_insert_and_query() {
        let repo = create_test_repo().await;
        use crate::rag::types::SymbolRef;

        let file = test_file("f1", "proj1", "src/auth.ts");
        repo.upsert_file(&file).await.unwrap();

        let chunk = test_chunk("c1", "f1", "proj1", "login", "function login() {}");
        repo.insert_chunks(&[chunk]).await.unwrap();

        let refs = vec![
            SymbolRef {
                id: "ref1".to_string(),
                project_id: "proj1".to_string(),
                from_chunk_id: "c1".to_string(),
                to_chunk_id: None,
                to_symbol_name: "hashPassword".to_string(),
                to_file_path: Some("src/utils.ts".to_string()),
                ref_type: "import".to_string(),
                line_number: Some(1),
            },
            SymbolRef {
                id: "ref2".to_string(),
                project_id: "proj1".to_string(),
                from_chunk_id: "c1".to_string(),
                to_chunk_id: None,
                to_symbol_name: "db".to_string(),
                to_file_path: None,
                ref_type: "import".to_string(),
                line_number: Some(2),
            },
        ];
        repo.insert_symbol_refs(&refs).await.unwrap();

        // Query refs FROM chunk c1
        let from_refs = repo.get_symbol_refs_from("c1").await.unwrap();
        assert_eq!(from_refs.len(), 2);

        // Query refs TO "hashPassword"
        let to_refs = repo.get_symbol_refs_to("proj1", "hashPassword").await.unwrap();
        assert_eq!(to_refs.len(), 1);
        assert_eq!(to_refs[0].from_chunk_id, "c1");
    }

    #[tokio::test]
    async fn test_resolve_symbol_refs() {
        let repo = create_test_repo().await;
        use crate::rag::types::SymbolRef;

        // Create two files with chunks
        let f1 = test_file("f1", "proj1", "src/auth.ts");
        let f2 = test_file("f2", "proj1", "src/utils.ts");
        repo.upsert_file(&f1).await.unwrap();
        repo.upsert_file(&f2).await.unwrap();

        let c1 = test_chunk("c1", "f1", "proj1", "login", "function login() { hashPassword(); }");
        let c2 = test_chunk("c2", "f2", "proj1", "hashPassword", "function hashPassword() {}");
        repo.insert_chunks(&[c1, c2]).await.unwrap();

        // Insert unresolved ref
        let refs = vec![SymbolRef {
            id: "ref1".to_string(),
            project_id: "proj1".to_string(),
            from_chunk_id: "c1".to_string(),
            to_chunk_id: None,
            to_symbol_name: "hashPassword".to_string(),
            to_file_path: None,
            ref_type: "import".to_string(),
            line_number: Some(1),
        }];
        repo.insert_symbol_refs(&refs).await.unwrap();

        // Resolve
        let resolved = repo.resolve_symbol_refs("proj1").await.unwrap();
        assert_eq!(resolved, 1);

        // Verify to_chunk_id is now set
        let updated = repo.get_symbol_refs_from("c1").await.unwrap();
        assert_eq!(updated[0].to_chunk_id, Some("c2".to_string()));
    }

    #[tokio::test]
    async fn test_delete_file_cascades_graph_data() {
        let repo = create_test_repo().await;
        use crate::rag::types::SymbolRef;

        let file = test_file("f1", "proj1", "src/auth.ts");
        repo.upsert_file(&file).await.unwrap();

        let chunk = test_chunk("c1", "f1", "proj1", "login", "function login() {}");
        repo.insert_chunks(&[chunk]).await.unwrap();

        repo.upsert_file_module("f1", "proj1", "auth", "src/auth", false).await.unwrap();
        repo.insert_symbol_refs(&[SymbolRef {
            id: "ref1".to_string(),
            project_id: "proj1".to_string(),
            from_chunk_id: "c1".to_string(),
            to_chunk_id: None,
            to_symbol_name: "db".to_string(),
            to_file_path: None,
            ref_type: "import".to_string(),
            line_number: None,
        }]).await.unwrap();

        // Delete file — should cascade
        repo.delete_file("f1").await.unwrap();

        assert!(repo.get_file_module("f1").await.unwrap().is_none());
        assert!(repo.get_symbol_refs_from("c1").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_delete_project_cleans_graph() {
        let repo = create_test_repo().await;

        let file = test_file("f1", "proj1", "src/auth.ts");
        repo.upsert_file(&file).await.unwrap();

        repo.upsert_file_module("f1", "proj1", "auth", "src/auth", false).await.unwrap();
        repo.upsert_module_dep("proj1", "auth", "utils", "import").await.unwrap();

        repo.delete_project_index("proj1").await.unwrap();

        assert!(repo.get_modules("proj1").await.unwrap().is_empty());
        assert!(repo.get_module_deps("proj1", "auth").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_search_within_module() {
        let repo = create_test_repo().await;

        // Two files in different modules
        let f1 = test_file("f1", "proj1", "src/auth/login.ts");
        let f2 = test_file("f2", "proj1", "src/utils/hash.ts");
        repo.upsert_file(&f1).await.unwrap();
        repo.upsert_file(&f2).await.unwrap();

        repo.upsert_file_module("f1", "proj1", "auth", "src/auth", false).await.unwrap();
        repo.upsert_file_module("f2", "proj1", "utils", "src/utils", false).await.unwrap();

        let c1 = test_chunk("c1", "f1", "proj1", "login", "async function login(user: string) { return authenticate(user); }");
        let c2 = test_chunk("c2", "f2", "proj1", "hashPassword", "function hashPassword(pwd: string) { return hash(pwd); }");
        repo.insert_chunks(&[c1, c2]).await.unwrap();

        // Search within auth module only
        let results = repo.search_within_module("proj1", "auth", "login", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.name, "login");

        // Search within utils module — should NOT find login
        let results = repo.search_within_module("proj1", "utils", "login", 10).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_search_by_embedding() {
        let repo = create_test_repo().await;

        let file = test_file("f1", "proj1", "src/utils.ts");
        repo.upsert_file(&file).await.unwrap();

        let chunks = vec![
            test_chunk("c1", "f1", "proj1", "auth", "function auth() {}"),
            test_chunk("c2", "f1", "proj1", "config", "function config() {}"),
        ];
        repo.insert_chunks(&chunks).await.unwrap();

        // Insert embeddings: c1 is similar to query, c2 is different
        let emb1 = crate::rag::embeddings::embedding_to_blob(&[1.0f32, 0.0, 0.0]);
        let emb2 = crate::rag::embeddings::embedding_to_blob(&[0.0f32, 1.0, 0.0]);
        repo.upsert_embedding("c1", &emb1, "test", 3).await.unwrap();
        repo.upsert_embedding("c2", &emb2, "test", 3).await.unwrap();

        // Query vector close to c1
        let query_vec = vec![0.9f32, 0.1, 0.0];
        let results = repo.search_by_embedding("proj1", &query_vec, 10).await.unwrap();
        assert_eq!(results.len(), 2);
        // c1 should rank first (higher cosine similarity)
        assert_eq!(results[0].0.name, "auth");
    }
}
