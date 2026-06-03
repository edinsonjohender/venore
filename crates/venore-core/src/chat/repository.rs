//! Chat Repository
//!
//! SQLite persistence for chat sessions and messages.

use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;

use crate::{MapDbErr, Result, VenoreError};

// ============================================================================
// TYPES
// ============================================================================

/// A chat session (conversation thread)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: String,
    pub name: String,
    pub project_id: Option<String>,
    pub dev_session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl ChatSession {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Self {
        Self {
            id: row.get("id"),
            name: row.get("name"),
            project_id: row.get("project_id"),
            dev_session_id: row.get("dev_session_id"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }
}

/// A persisted snapshot (tool_call_id → commit_hash), enriched with tool call data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotRecord {
    pub tool_call_id: String,
    pub session_id: String,
    pub commit_hash: String,
    pub created_at: String,
    pub tool_name: Option<String>,
    pub arguments: Option<String>,
}

/// A persisted tool call record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub id: String,
    pub session_id: String,
    pub tool_name: String,
    pub arguments: String,
    pub success: Option<bool>,
    pub output: Option<String>,
    pub commit_hash: Option<String>,
    pub created_at: String,
}

/// Aggregated token usage for a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSummary {
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    pub message_count: u32,
}

/// A persisted chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageRecord {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub created_at: String,
    /// JSON array of attachment metadata: [{"name":"file.png","mimeType":"image/png"}]
    pub attachments_json: Option<String>,
}

impl ChatMessageRecord {
    /// Create a user message record.
    pub fn new_user(session_id: &str, content: &str, attachments_json: Option<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            role: "user".to_string(),
            content: content.to_string(),
            provider: None,
            model: None,
            prompt_tokens: None,
            completion_tokens: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            attachments_json,
        }
    }

    /// Create an assistant message record with token usage.
    pub fn new_assistant(
        session_id: &str,
        content: &str,
        provider: &str,
        model: &str,
        prompt_tokens: u32,
        completion_tokens: u32,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            role: "assistant".to_string(),
            content: content.to_string(),
            provider: Some(provider.to_string()),
            model: Some(model.to_string()),
            prompt_tokens: Some(prompt_tokens),
            completion_tokens: Some(completion_tokens),
            created_at: chrono::Utc::now().to_rfc3339(),
            attachments_json: None,
        }
    }
}

// ============================================================================
// REPOSITORY
// ============================================================================

/// SQLite-backed chat repository
pub struct ChatRepository {
    pool: SqlitePool,
}

impl ChatRepository {
    /// Create a new ChatRepository with the given connection pool
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Initialize tables with automatic migration from old schema
    pub async fn initialize(&self) -> Result<()> {
        // Check if table exists and has old schema (project_path column)
        let needs_migration = self.detect_legacy_schema().await?;

        if needs_migration {
            tracing::info!("Migrating chat_sessions: project_path -> project_id");
            self.migrate_to_project_id().await?;
        } else {
            // Create table with new schema (or no-op if already exists)
            self.create_tables().await?;
        }

        // Migration: add dev_session_id column (ignore error if already exists)
        sqlx::query("ALTER TABLE chat_sessions ADD COLUMN dev_session_id TEXT")
            .execute(&self.pool)
            .await
            .ok();

        // Migration: add attachments_json column to chat_messages (ignore if exists)
        sqlx::query("ALTER TABLE chat_messages ADD COLUMN attachments_json TEXT")
            .execute(&self.pool)
            .await
            .ok();

        // Create chat_snapshots table (idempotent)
        // Uses strftime with %f for millisecond precision (needed for delete_snapshots_after)
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS chat_snapshots (
                tool_call_id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                commit_hash TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now'))
            )"
        )
        .execute(&self.pool)
        .await
        .db_err("Failed to create chat_snapshots")?;

        // Create chat_tool_calls table (idempotent)
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS chat_tool_calls (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                tool_name TEXT NOT NULL,
                arguments TEXT NOT NULL DEFAULT '{}',
                success INTEGER,
                output TEXT,
                commit_hash TEXT,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now')),
                FOREIGN KEY (session_id) REFERENCES chat_sessions(id) ON DELETE CASCADE
            )"
        )
        .execute(&self.pool)
        .await
        .db_err("Failed to create chat_tool_calls")?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_chat_tool_calls_session ON chat_tool_calls(session_id)"
        )
        .execute(&self.pool)
        .await
        .db_err("Failed to create tool_calls index")?;

        tracing::info!("Chat repository initialized");
        Ok(())
    }

    /// Detect if the old schema with project_path exists
    async fn detect_legacy_schema(&self) -> Result<bool> {
        // Check if table exists at all
        let table_exists: bool = sqlx::query(
            "SELECT COUNT(*) as cnt FROM sqlite_master WHERE type='table' AND name='chat_sessions'"
        )
        .fetch_one(&self.pool)
        .await
        .map(|row| row.get::<i64, _>("cnt") > 0)
        .db_err("Failed to check table existence")?;

        if !table_exists {
            return Ok(false);
        }

        // Check if project_path column exists (old schema)
        let columns: Vec<_> = sqlx::query("PRAGMA table_info(chat_sessions)")
            .fetch_all(&self.pool)
            .await
            .db_err("Failed to read table info")?;

        let has_project_path = columns.iter().any(|row| {
            let name: String = row.get("name");
            name == "project_path"
        });

        Ok(has_project_path)
    }

    /// Migrate from project_path to project_id using the projects table
    async fn migrate_to_project_id(&self) -> Result<()> {
        // Create new table with project_id
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS chat_sessions_new (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                project_id TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )"
        )
        .execute(&self.pool)
        .await
        .db_err("Failed to create new chat_sessions")?;

        // Copy data, joining with projects table to resolve path -> id
        sqlx::query(
            "INSERT OR IGNORE INTO chat_sessions_new (id, name, project_id, created_at, updated_at)
             SELECT cs.id, cs.name, p.id, cs.created_at, cs.updated_at
             FROM chat_sessions cs
             LEFT JOIN projects p ON cs.project_path = p.path"
        )
        .execute(&self.pool)
        .await
        .db_err("Failed to migrate data")?;

        // Drop old table, rename new
        sqlx::query("DROP TABLE chat_sessions")
            .execute(&self.pool)
            .await
            .db_err("Failed to drop old table")?;

        sqlx::query("ALTER TABLE chat_sessions_new RENAME TO chat_sessions")
            .execute(&self.pool)
            .await
            .db_err("Failed to rename table")?;

        // Ensure messages table and index exist
        self.create_messages_table().await?;

        tracing::info!("Chat sessions migrated from project_path to project_id");
        Ok(())
    }

    /// Create tables with the new schema (project_id + dev_session_id)
    async fn create_tables(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS chat_sessions (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                project_id TEXT,
                dev_session_id TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )"
        )
        .execute(&self.pool)
        .await
        .db_err("Failed to create chat_sessions")?;

        self.create_messages_table().await
    }

    /// Create messages table and index (shared by migration and fresh create)
    async fn create_messages_table(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS chat_messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                provider TEXT,
                model TEXT,
                prompt_tokens INTEGER,
                completion_tokens INTEGER,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (session_id) REFERENCES chat_sessions(id) ON DELETE CASCADE
            )"
        )
        .execute(&self.pool)
        .await
        .db_err("Failed to create chat_messages")?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_chat_messages_session ON chat_messages(session_id)"
        )
        .execute(&self.pool)
        .await
        .db_err("Failed to create index")?;

        Ok(())
    }

    // ========================================================================
    // SESSIONS
    // ========================================================================

    /// Create a new chat session
    pub async fn create_session(&self, name: &str, project_id: Option<&str>) -> Result<ChatSession> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO chat_sessions (id, name, project_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(name)
        .bind(project_id)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await
        .db_err("Failed to create session")?;

        Ok(ChatSession {
            id,
            name: name.to_string(),
            project_id: project_id.map(String::from),
            dev_session_id: None,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    /// Garbage-collect empty plain chat sessions (no messages).
    ///
    /// With lazy persistence the UI no longer creates a row until the first
    /// message is sent, so this only ever reaps:
    ///   - legacy empty "New Chat" rows created before lazy persistence, and
    ///   - the rare session whose creation succeeded but whose first message
    ///     never persisted (stream aborted).
    ///
    /// Guards:
    ///   - `dev_session_id IS NULL` — never touch dev-session chats (those are
    ///     legitimately created empty, tied to a branch/worktree).
    ///   - `datetime(created_at) < now-1min` — avoids racing a session created
    ///     moments ago whose first message is still in flight.
    ///   - `created_at` is stored as RFC3339; `datetime(...)` normalizes both
    ///     sides to UTC so the comparison is correct (plain string `<` would not
    ///     be, due to the `T`/space separator mismatch).
    ///
    /// Best-effort: returns the number of rows deleted.
    pub async fn delete_empty_sessions(&self) -> Result<u64> {
        let res = sqlx::query(
            "DELETE FROM chat_sessions
             WHERE dev_session_id IS NULL
               AND datetime(created_at) < datetime('now', '-1 minute')
               AND id NOT IN (SELECT DISTINCT session_id FROM chat_messages)"
        )
        .execute(&self.pool)
        .await
        .db_err("Failed to delete empty sessions")?;
        Ok(res.rows_affected())
    }

    /// List sessions, ordered by updated_at DESC
    pub async fn list_sessions(&self, project_id: Option<&str>) -> Result<Vec<ChatSession>> {
        // Best-effort GC of empty sessions on every load — keeps history clean
        // without a separate command/scheduler. Failure here must not block the
        // listing, so we log and continue.
        match self.delete_empty_sessions().await {
            Ok(n) if n > 0 => tracing::debug!("GC: removed {} empty chat session(s)", n),
            Ok(_) => {}
            Err(e) => tracing::warn!("GC of empty chat sessions failed: {}", e),
        }

        let rows = match project_id {
            Some(pid) => {
                sqlx::query(
                    "SELECT id, name, project_id, dev_session_id, created_at, updated_at
                     FROM chat_sessions WHERE project_id = ?
                     ORDER BY updated_at DESC"
                )
                .bind(pid)
                .fetch_all(&self.pool)
                .await
            }
            None => {
                sqlx::query(
                    "SELECT id, name, project_id, dev_session_id, created_at, updated_at
                     FROM chat_sessions ORDER BY updated_at DESC"
                )
                .fetch_all(&self.pool)
                .await
            }
        }
        .db_err("Failed to list sessions")?;

        Ok(rows.iter().map(ChatSession::from_row).collect())
    }

    /// Get a single session by ID
    pub async fn get_session(&self, id: &str) -> Result<Option<ChatSession>> {
        let row = sqlx::query(
            "SELECT id, name, project_id, dev_session_id, created_at, updated_at
             FROM chat_sessions WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .db_err("Failed to get session")?;

        Ok(row.as_ref().map(ChatSession::from_row))
    }

    /// Delete a session and all related data (FK cascade unreliable with pool)
    pub async fn delete_session(&self, id: &str) -> Result<()> {
        // Manually delete child rows since PRAGMA foreign_keys may not be active
        sqlx::query("DELETE FROM chat_tool_calls WHERE session_id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .db_err("Failed to delete tool calls")?;

        sqlx::query("DELETE FROM chat_snapshots WHERE session_id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .db_err("Failed to delete snapshots")?;

        sqlx::query("DELETE FROM chat_messages WHERE session_id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .db_err("Failed to delete messages")?;

        sqlx::query("DELETE FROM chat_sessions WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .db_err("Failed to delete session")?;

        Ok(())
    }

    /// Rename a session
    pub async fn rename_session(&self, id: &str, name: &str) -> Result<()> {
        sqlx::query("UPDATE chat_sessions SET name = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(name)
            .bind(id)
            .execute(&self.pool)
            .await
            .db_err("Failed to rename session")?;

        Ok(())
    }

    /// Touch session (update updated_at timestamp)
    pub async fn touch_session(&self, id: &str) -> Result<()> {
        sqlx::query("UPDATE chat_sessions SET updated_at = datetime('now') WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .db_err("Failed to touch session")?;

        Ok(())
    }

    // ========================================================================
    // DEV SESSION INTEGRATION
    // ========================================================================

    /// Find or create a chat session linked to a dev session
    pub async fn find_or_create_for_dev_session(
        &self,
        dev_session_id: &str,
        name: &str,
        project_id: Option<&str>,
    ) -> Result<ChatSession> {
        // Try to find existing
        if let Some(session) = self.find_by_dev_session_id(dev_session_id).await? {
            return Ok(session);
        }

        // Create new with dev_session_id
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO chat_sessions (id, name, project_id, dev_session_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(name)
        .bind(project_id)
        .bind(dev_session_id)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await
        .db_err("Failed to create dev session chat")?;

        Ok(ChatSession {
            id,
            name: name.to_string(),
            project_id: project_id.map(String::from),
            dev_session_id: Some(dev_session_id.to_string()),
            created_at: now.clone(),
            updated_at: now,
        })
    }

    /// Find a chat session by dev_session_id
    pub async fn find_by_dev_session_id(&self, dev_session_id: &str) -> Result<Option<ChatSession>> {
        let row = sqlx::query(
            "SELECT id, name, project_id, dev_session_id, created_at, updated_at
             FROM chat_sessions WHERE dev_session_id = ?"
        )
        .bind(dev_session_id)
        .fetch_optional(&self.pool)
        .await
        .db_err("Failed to find dev session chat")?;

        Ok(row.as_ref().map(ChatSession::from_row))
    }

    // ========================================================================
    // SNAPSHOTS
    // ========================================================================

    /// Save a snapshot (tool_call_id → commit_hash) for a session
    pub async fn save_snapshot(&self, session_id: &str, tool_call_id: &str, commit_hash: &str) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO chat_snapshots (tool_call_id, session_id, commit_hash, created_at)
             VALUES (?, ?, ?, strftime('%Y-%m-%d %H:%M:%f', 'now'))"
        )
        .bind(tool_call_id)
        .bind(session_id)
        .bind(commit_hash)
        .execute(&self.pool)
        .await
        .db_err("Failed to save snapshot")?;

        Ok(())
    }

    /// Get all snapshots for a session, ordered by created_at ASC.
    /// LEFT JOINs chat_tool_calls to enrich with tool_name and arguments.
    pub async fn get_snapshots(&self, session_id: &str) -> Result<Vec<SnapshotRecord>> {
        let rows = sqlx::query(
            "SELECT s.tool_call_id, s.session_id, s.commit_hash, s.created_at,
                    tc.tool_name, tc.arguments
             FROM chat_snapshots s
             LEFT JOIN chat_tool_calls tc ON tc.id = s.tool_call_id
             WHERE s.session_id = ?
             ORDER BY s.created_at ASC"
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await
        .db_err("Failed to get snapshots")?;

        Ok(rows.iter().map(|row| SnapshotRecord {
            tool_call_id: row.get("tool_call_id"),
            session_id: row.get("session_id"),
            commit_hash: row.get("commit_hash"),
            created_at: row.get("created_at"),
            tool_name: row.get("tool_name"),
            arguments: row.get("arguments"),
        }).collect())
    }

    /// Delete all snapshots in a session created after a given timestamp
    pub async fn delete_snapshots_after(&self, session_id: &str, after_created_at: &str) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM chat_snapshots WHERE session_id = ? AND created_at > ?"
        )
        .bind(session_id)
        .bind(after_created_at)
        .execute(&self.pool)
        .await
        .db_err("Failed to delete snapshots")?;

        let count = result.rows_affected();
        tracing::info!(session_id = %session_id, deleted = count, "Deleted snapshots after revert point");
        Ok(count)
    }

    // ========================================================================
    // TOOL CALLS
    // ========================================================================

    /// Save a tool call record (initial insert, before execution)
    pub async fn save_tool_call(&self, record: &ToolCallRecord) -> Result<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO chat_tool_calls (id, session_id, tool_name, arguments, success, output, commit_hash, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&record.id)
        .bind(&record.session_id)
        .bind(&record.tool_name)
        .bind(&record.arguments)
        .bind(record.success.map(|b| b as i32))
        .bind(&record.output)
        .bind(&record.commit_hash)
        .bind(&record.created_at)
        .execute(&self.pool)
        .await
        .db_err("Failed to save tool call")?;

        Ok(())
    }

    /// Update a tool call with its result (after execution)
    pub async fn update_tool_call_result(
        &self,
        id: &str,
        success: bool,
        output: &str,
        commit_hash: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE chat_tool_calls SET success = ?, output = ?, commit_hash = COALESCE(?, commit_hash)
             WHERE id = ?"
        )
        .bind(success as i32)
        .bind(output)
        .bind(commit_hash)
        .bind(id)
        .execute(&self.pool)
        .await
        .db_err("Failed to update tool call")?;

        Ok(())
    }

    /// Get all tool calls for a session, ordered by created_at ASC
    pub async fn get_tool_calls(&self, session_id: &str) -> Result<Vec<ToolCallRecord>> {
        let rows = sqlx::query(
            "SELECT id, session_id, tool_name, arguments, success, output, commit_hash, created_at
             FROM chat_tool_calls WHERE session_id = ?
             ORDER BY created_at ASC"
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await
        .db_err("Failed to get tool calls")?;

        Ok(rows.iter().map(|row| {
            let success_val: Option<i32> = row.get("success");
            ToolCallRecord {
                id: row.get("id"),
                session_id: row.get("session_id"),
                tool_name: row.get("tool_name"),
                arguments: row.get("arguments"),
                success: success_val.map(|v| v != 0),
                output: row.get("output"),
                commit_hash: row.get("commit_hash"),
                created_at: row.get("created_at"),
            }
        }).collect())
    }

    /// Get aggregated token summary for a session
    pub async fn get_session_token_summary(&self, session_id: &str) -> Result<TokenSummary> {
        let row = sqlx::query(
            "SELECT COALESCE(SUM(prompt_tokens), 0) as total_prompt,
                    COALESCE(SUM(completion_tokens), 0) as total_completion,
                    COUNT(*) as msg_count
             FROM chat_messages
             WHERE session_id = ? AND role = 'assistant'"
        )
        .bind(session_id)
        .fetch_one(&self.pool)
        .await
        .db_err("Failed to get token summary")?;

        let total_prompt: i64 = row.get("total_prompt");
        let total_completion: i64 = row.get("total_completion");
        let msg_count: i32 = row.get("msg_count");

        Ok(TokenSummary {
            total_prompt_tokens: total_prompt as u64,
            total_completion_tokens: total_completion as u64,
            message_count: msg_count as u32,
        })
    }

    /// Delete all tool calls in a session created after a given timestamp
    pub async fn delete_tool_calls_after(&self, session_id: &str, after_created_at: &str) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM chat_tool_calls WHERE session_id = ? AND created_at > ?"
        )
        .bind(session_id)
        .bind(after_created_at)
        .execute(&self.pool)
        .await
        .db_err("Failed to delete tool calls")?;

        let count = result.rows_affected();
        tracing::info!(session_id = %session_id, deleted = count, "Deleted tool calls after revert point");
        Ok(count)
    }

    // ========================================================================
    // MESSAGES
    // ========================================================================

    /// Save a message to the database
    pub async fn save_message(&self, msg: &ChatMessageRecord) -> Result<()> {
        sqlx::query(
            "INSERT INTO chat_messages (id, session_id, role, content, provider, model, prompt_tokens, completion_tokens, created_at, attachments_json)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&msg.id)
        .bind(&msg.session_id)
        .bind(&msg.role)
        .bind(&msg.content)
        .bind(&msg.provider)
        .bind(&msg.model)
        .bind(msg.prompt_tokens.map(|v| v as i64))
        .bind(msg.completion_tokens.map(|v| v as i64))
        .bind(&msg.created_at)
        .bind(&msg.attachments_json)
        .execute(&self.pool)
        .await
        .db_err("Failed to save message")?;

        Ok(())
    }

    /// Delete all messages in a session created after a given message.
    /// Uses rowid comparison (monotonically increasing) instead of timestamp to avoid
    /// race conditions when messages share the same created_at second.
    /// Returns the count of deleted messages.
    pub async fn delete_messages_after(&self, session_id: &str, message_id: &str) -> Result<u64> {
        // Verify the reference message exists
        let exists: bool = sqlx::query(
            "SELECT COUNT(*) as cnt FROM chat_messages WHERE id = ? AND session_id = ?"
        )
        .bind(message_id)
        .bind(session_id)
        .fetch_one(&self.pool)
        .await
        .map(|row| row.get::<i64, _>("cnt") > 0)
        .db_err("Failed to find message")?;

        if !exists {
            return Err(VenoreError::NotFound(format!("Message not found: {}", message_id)));
        }

        // Delete all messages with a higher rowid than the reference message
        let result = sqlx::query(
            "DELETE FROM chat_messages WHERE session_id = ? AND rowid > (SELECT rowid FROM chat_messages WHERE id = ?)"
        )
        .bind(session_id)
        .bind(message_id)
        .execute(&self.pool)
        .await
        .db_err("Failed to delete messages")?;

        let count = result.rows_affected();
        tracing::info!(session_id = %session_id, after_message = %message_id, deleted = count, "Deleted messages after revert point");
        Ok(count)
    }

    /// Delete all messages in a session created after a given timestamp.
    /// Used by Activity Tab revert (no message_id available, uses snapshot timestamp).
    pub async fn delete_messages_after_timestamp(&self, session_id: &str, after_ts: &str) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM chat_messages WHERE session_id = ? AND created_at > ?"
        )
        .bind(session_id)
        .bind(after_ts)
        .execute(&self.pool)
        .await
        .db_err("Failed to delete messages by timestamp")?;

        let count = result.rows_affected();
        tracing::info!(session_id = %session_id, after_ts = %after_ts, deleted = count, "Deleted messages after timestamp");
        Ok(count)
    }

    /// Get messages for a session, ordered by created_at ASC
    pub async fn get_messages(&self, session_id: &str, limit: Option<u32>) -> Result<Vec<ChatMessageRecord>> {
        let rows = match limit {
            Some(lim) => {
                sqlx::query(
                    "SELECT id, session_id, role, content, provider, model, prompt_tokens, completion_tokens, created_at, attachments_json
                     FROM chat_messages WHERE session_id = ?
                     ORDER BY created_at ASC LIMIT ?"
                )
                .bind(session_id)
                .bind(lim as i64)
                .fetch_all(&self.pool)
                .await
            }
            None => {
                sqlx::query(
                    "SELECT id, session_id, role, content, provider, model, prompt_tokens, completion_tokens, created_at, attachments_json
                     FROM chat_messages WHERE session_id = ?
                     ORDER BY created_at ASC"
                )
                .bind(session_id)
                .fetch_all(&self.pool)
                .await
            }
        }
        .db_err("Failed to get messages")?;

        let messages = rows
            .iter()
            .map(|row| {
                let pt: Option<i64> = row.get("prompt_tokens");
                let ct: Option<i64> = row.get("completion_tokens");
                ChatMessageRecord {
                    id: row.get("id"),
                    session_id: row.get("session_id"),
                    role: row.get("role"),
                    content: row.get("content"),
                    provider: row.get("provider"),
                    model: row.get("model"),
                    prompt_tokens: pt.map(|v| v as u32),
                    completion_tokens: ct.map(|v| v as u32),
                    created_at: row.get("created_at"),
                    attachments_json: row.get("attachments_json"),
                }
            })
            .collect();

        Ok(messages)
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    async fn create_test_pool() -> SqlitePool {
        let opts = SqliteConnectOptions::from_str("sqlite::memory:")
            .unwrap()
            .pragma("foreign_keys", "ON");
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap()
    }

    async fn create_test_repo() -> ChatRepository {
        let pool = create_test_pool().await;
        let repo = ChatRepository::new(pool);
        repo.initialize().await.unwrap();
        repo
    }

    #[tokio::test]
    async fn test_create_and_list_sessions() {
        let repo = create_test_repo().await;

        let s1 = repo.create_session("Chat 1", Some("proj-id-a")).await.unwrap();
        let s2 = repo.create_session("Chat 2", Some("proj-id-a")).await.unwrap();
        let _s3 = repo.create_session("Chat 3", Some("proj-id-b")).await.unwrap();

        assert_eq!(s1.name, "Chat 1");
        assert_eq!(s2.name, "Chat 2");

        // List all
        let all = repo.list_sessions(None).await.unwrap();
        assert_eq!(all.len(), 3);

        // List by project_id
        let project_a = repo.list_sessions(Some("proj-id-a")).await.unwrap();
        assert_eq!(project_a.len(), 2);
    }

    #[tokio::test]
    async fn test_get_session() {
        let repo = create_test_repo().await;
        let created = repo.create_session("Test", None).await.unwrap();

        let found = repo.get_session(&created.id).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "Test");

        let missing = repo.get_session("nonexistent").await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_delete_session_cascades() {
        let repo = create_test_repo().await;
        let session = repo.create_session("ToDelete", None).await.unwrap();

        // Add a message
        let msg = ChatMessageRecord {
            id: "msg-1".to_string(),
            session_id: session.id.clone(),
            role: "user".to_string(),
            content: "Hello".to_string(),
            provider: None,
            model: None,
            prompt_tokens: None,
            completion_tokens: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            attachments_json: None,
        };
        repo.save_message(&msg).await.unwrap();

        // Add a tool call
        let tc = ToolCallRecord {
            id: "tc-1".to_string(),
            session_id: session.id.clone(),
            tool_name: "read_file".to_string(),
            arguments: "{}".to_string(),
            success: None,
            output: None,
            commit_hash: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        repo.save_tool_call(&tc).await.unwrap();

        // Add a snapshot
        repo.save_snapshot(&session.id, "tc-1", "abc123").await.unwrap();

        // Delete session
        repo.delete_session(&session.id).await.unwrap();

        // Session gone
        assert!(repo.get_session(&session.id).await.unwrap().is_none());

        // Messages gone
        let msgs = repo.get_messages(&session.id, None).await.unwrap();
        assert!(msgs.is_empty());

        // Tool calls gone
        let tcs = repo.get_tool_calls(&session.id).await.unwrap();
        assert!(tcs.is_empty());

        // Snapshots gone
        let snaps = repo.get_snapshots(&session.id).await.unwrap();
        assert!(snaps.is_empty());
    }

    #[tokio::test]
    async fn test_rename_session() {
        let repo = create_test_repo().await;
        let session = repo.create_session("Original", None).await.unwrap();

        repo.rename_session(&session.id, "Renamed").await.unwrap();

        let updated = repo.get_session(&session.id).await.unwrap().unwrap();
        assert_eq!(updated.name, "Renamed");
    }

    #[tokio::test]
    async fn test_save_and_get_messages() {
        let repo = create_test_repo().await;
        let session = repo.create_session("MsgTest", None).await.unwrap();

        let msg1 = ChatMessageRecord {
            id: "msg-1".to_string(),
            session_id: session.id.clone(),
            role: "user".to_string(),
            content: "Hello".to_string(),
            provider: None,
            model: None,
            prompt_tokens: None,
            completion_tokens: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            attachments_json: None,
        };

        let msg2 = ChatMessageRecord {
            id: "msg-2".to_string(),
            session_id: session.id.clone(),
            role: "assistant".to_string(),
            content: "Hi there!".to_string(),
            provider: Some("anthropic".to_string()),
            model: Some("claude-sonnet-4-5".to_string()),
            prompt_tokens: Some(10),
            completion_tokens: Some(20),
            created_at: "2026-01-01T00:00:01Z".to_string(),
            attachments_json: None,
        };

        repo.save_message(&msg1).await.unwrap();
        repo.save_message(&msg2).await.unwrap();

        let messages = repo.get_messages(&session.id, None).await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].prompt_tokens, Some(10));
    }

    #[tokio::test]
    async fn test_get_messages_with_limit() {
        let repo = create_test_repo().await;
        let session = repo.create_session("LimitTest", None).await.unwrap();

        for i in 0..5 {
            let msg = ChatMessageRecord {
                id: format!("msg-{}", i),
                session_id: session.id.clone(),
                role: "user".to_string(),
                content: format!("Message {}", i),
                provider: None,
                model: None,
                prompt_tokens: None,
                completion_tokens: None,
                created_at: format!("2026-01-01T00:00:0{}Z", i),
                attachments_json: None,
            };
            repo.save_message(&msg).await.unwrap();
        }

        let messages = repo.get_messages(&session.id, Some(3)).await.unwrap();
        assert_eq!(messages.len(), 3);
    }

    #[tokio::test]
    async fn test_touch_session() {
        let repo = create_test_repo().await;
        let session = repo.create_session("TouchTest", None).await.unwrap();
        let _original_updated = session.updated_at.clone();

        // Small delay to ensure timestamp difference
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        repo.touch_session(&session.id).await.unwrap();

        let updated = repo.get_session(&session.id).await.unwrap().unwrap();
        // updated_at should be different (or at least not panic)
        assert!(!updated.updated_at.is_empty());
    }

    #[tokio::test]
    async fn test_migration_from_legacy_schema() {
        let pool = create_test_pool().await;

        // Create legacy schema with project_path
        sqlx::query(
            "CREATE TABLE chat_sessions (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                project_path TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )"
        )
        .execute(&pool)
        .await
        .unwrap();

        // Create projects table (required for migration join)
        sqlx::query(
            "CREATE TABLE projects (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                path TEXT NOT NULL,
                created_at TEXT NOT NULL,
                last_opened_at TEXT NOT NULL DEFAULT (datetime('now'))
            )"
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert a project
        sqlx::query(
            "INSERT INTO projects (id, name, path, created_at) VALUES ('proj-uuid-1', 'MyProject', '/path/to/project', '2026-01-01T00:00:00Z')"
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert legacy sessions
        sqlx::query(
            "INSERT INTO chat_sessions (id, name, project_path, created_at, updated_at) VALUES ('sess-1', 'Chat A', '/path/to/project', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO chat_sessions (id, name, project_path, created_at, updated_at) VALUES ('sess-2', 'Chat B', NULL, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
        )
        .execute(&pool)
        .await
        .unwrap();

        // Initialize repo (should trigger migration)
        let repo = ChatRepository::new(pool.clone());
        repo.initialize().await.unwrap();

        // Give sess-1 a message so the empty-session GC in list_sessions()
        // doesn't reap it — real legacy sessions carry message history.
        sqlx::query(
            "INSERT INTO chat_messages (id, session_id, role, content, created_at) \
             VALUES ('msg-1', 'sess-1', 'user', 'hello', '2026-01-01T00:00:00Z')"
        )
        .execute(&pool)
        .await
        .unwrap();

        // Verify migration: sess-1 should have project_id = 'proj-uuid-1'
        let session = repo.get_session("sess-1").await.unwrap().unwrap();
        assert_eq!(session.project_id, Some("proj-uuid-1".to_string()));

        // sess-2 should have project_id = None
        let session2 = repo.get_session("sess-2").await.unwrap().unwrap();
        assert_eq!(session2.project_id, None);

        // Should be able to list by project_id
        let sessions = repo.list_sessions(Some("proj-uuid-1")).await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].name, "Chat A");
    }

    #[tokio::test]
    async fn test_save_and_get_snapshots() {
        let repo = create_test_repo().await;
        let session = repo.create_session("SnapTest", None).await.unwrap();

        repo.save_snapshot(&session.id, "tc-1", "abc123").await.unwrap();
        repo.save_snapshot(&session.id, "tc-2", "def456").await.unwrap();

        let snapshots = repo.get_snapshots(&session.id).await.unwrap();
        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].tool_call_id, "tc-1");
        assert_eq!(snapshots[0].commit_hash, "abc123");
        assert_eq!(snapshots[1].tool_call_id, "tc-2");
        assert_eq!(snapshots[1].commit_hash, "def456");
    }

    #[tokio::test]
    async fn test_delete_snapshots_after() {
        let repo = create_test_repo().await;
        let session = repo.create_session("SnapDeleteTest", None).await.unwrap();

        repo.save_snapshot(&session.id, "tc-1", "aaa").await.unwrap();
        // Small delay to ensure different timestamps
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        repo.save_snapshot(&session.id, "tc-2", "bbb").await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        repo.save_snapshot(&session.id, "tc-3", "ccc").await.unwrap();

        // Get the created_at of tc-1 to use as the cutoff
        let snapshots = repo.get_snapshots(&session.id).await.unwrap();
        let cutoff = &snapshots[0].created_at;

        let deleted = repo.delete_snapshots_after(&session.id, cutoff).await.unwrap();
        assert_eq!(deleted, 2);

        let remaining = repo.get_snapshots(&session.id).await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].tool_call_id, "tc-1");
    }

    #[tokio::test]
    async fn test_snapshot_upsert() {
        let repo = create_test_repo().await;
        let session = repo.create_session("UpsertTest", None).await.unwrap();

        repo.save_snapshot(&session.id, "tc-1", "hash_v1").await.unwrap();
        repo.save_snapshot(&session.id, "tc-1", "hash_v2").await.unwrap();

        let snapshots = repo.get_snapshots(&session.id).await.unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].commit_hash, "hash_v2");
    }

    #[tokio::test]
    async fn test_save_and_get_tool_calls() {
        let repo = create_test_repo().await;
        let session = repo.create_session("ToolCallTest", None).await.unwrap();

        let record = ToolCallRecord {
            id: "tc-1".to_string(),
            session_id: session.id.clone(),
            tool_name: "read_file".to_string(),
            arguments: r#"{"file_path":"/src/main.rs"}"#.to_string(),
            success: None,
            output: None,
            commit_hash: None,
            created_at: "2026-01-01T00:00:00.000".to_string(),
        };
        repo.save_tool_call(&record).await.unwrap();

        let calls = repo.get_tool_calls(&session.id).await.unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "read_file");
        assert!(calls[0].success.is_none());
    }

    #[tokio::test]
    async fn test_update_tool_call_result() {
        let repo = create_test_repo().await;
        let session = repo.create_session("ToolCallUpdateTest", None).await.unwrap();

        let record = ToolCallRecord {
            id: "tc-2".to_string(),
            session_id: session.id.clone(),
            tool_name: "edit_file".to_string(),
            arguments: "{}".to_string(),
            success: None,
            output: None,
            commit_hash: None,
            created_at: "2026-01-01T00:00:00.000".to_string(),
        };
        repo.save_tool_call(&record).await.unwrap();

        repo.update_tool_call_result("tc-2", true, "File edited OK", Some("abc123")).await.unwrap();

        let calls = repo.get_tool_calls(&session.id).await.unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].success, Some(true));
        assert_eq!(calls[0].output.as_deref(), Some("File edited OK"));
        assert_eq!(calls[0].commit_hash.as_deref(), Some("abc123"));
    }

    #[tokio::test]
    async fn test_delete_tool_calls_after() {
        let repo = create_test_repo().await;
        let session = repo.create_session("ToolCallDeleteTest", None).await.unwrap();

        for i in 0..3 {
            let record = ToolCallRecord {
                id: format!("tc-{}", i),
                session_id: session.id.clone(),
                tool_name: "read_file".to_string(),
                arguments: "{}".to_string(),
                success: Some(true),
                output: Some("ok".to_string()),
                commit_hash: None,
                created_at: format!("2026-01-01T00:00:0{}.000", i),
            };
            repo.save_tool_call(&record).await.unwrap();
        }

        let deleted = repo.delete_tool_calls_after(&session.id, "2026-01-01T00:00:00.000").await.unwrap();
        assert_eq!(deleted, 2);

        let remaining = repo.get_tool_calls(&session.id).await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, "tc-0");
    }

    #[tokio::test]
    async fn test_get_session_token_summary() {
        let repo = create_test_repo().await;
        let session = repo.create_session("TokenTest", None).await.unwrap();

        // Save assistant messages with token counts
        for i in 0..3 {
            let msg = ChatMessageRecord {
                id: format!("msg-{}", i),
                session_id: session.id.clone(),
                role: "assistant".to_string(),
                content: format!("Response {}", i),
                provider: Some("anthropic".to_string()),
                model: Some("claude-sonnet".to_string()),
                prompt_tokens: Some(100),
                completion_tokens: Some(50),
                created_at: format!("2026-01-01T00:00:0{}Z", i),
                attachments_json: None,
            };
            repo.save_message(&msg).await.unwrap();
        }

        let summary = repo.get_session_token_summary(&session.id).await.unwrap();
        assert_eq!(summary.total_prompt_tokens, 300);
        assert_eq!(summary.total_completion_tokens, 150);
        assert_eq!(summary.message_count, 3);
    }
}
