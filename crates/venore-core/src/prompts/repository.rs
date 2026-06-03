//! Prompt repository — SQLite CRUD for prompts and versions

use sqlx::sqlite::SqlitePool;
use sqlx::Row;

use crate::{Result, VenoreError};
use super::models::{Prompt, PromptVersion};

pub struct PromptRepository {
    pool: SqlitePool,
}

impl PromptRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Access the underlying pool (used by seed.rs)
    pub(crate) fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    // =========================================================================
    // Initialize
    // =========================================================================

    pub async fn initialize(&self) -> Result<()> {
        self.create_tables().await?;
        // Migration: add compatible_providers column (ignore if already exists)
        let _ = sqlx::query(
            "ALTER TABLE prompts ADD COLUMN compatible_providers TEXT NOT NULL DEFAULT '[]'"
        )
        .execute(&self.pool)
        .await;
        // Migration: add provider column (ignore if already exists)
        let _ = sqlx::query(
            "ALTER TABLE prompts ADD COLUMN provider TEXT NOT NULL DEFAULT 'base'"
        )
        .execute(&self.pool)
        .await;
        // Migration: add is_enabled column (Phase 5 — chat-fragment toggle)
        let _ = sqlx::query(
            "ALTER TABLE prompts ADD COLUMN is_enabled INTEGER NOT NULL DEFAULT 1"
        )
        .execute(&self.pool)
        .await;
        // Migration: rename `chat-fragment-bitacoras-hint` → `chat-fragment-logbook-hint`
        // (project terminology cleanup; idempotent — UPDATE WHERE matches once only).
        let _ = sqlx::query(
            "UPDATE prompts SET id = 'chat-fragment-logbook-hint' WHERE id = 'chat-fragment-bitacoras-hint'"
        )
        .execute(&self.pool)
        .await;
        let _ = sqlx::query(
            "UPDATE prompt_versions SET prompt_id = 'chat-fragment-logbook-hint' WHERE prompt_id = 'chat-fragment-bitacoras-hint'"
        )
        .execute(&self.pool)
        .await;
        tracing::info!("Prompt repository initialized");
        Ok(())
    }

    async fn create_tables(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS prompts (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                category TEXT NOT NULL,
                provider TEXT NOT NULL DEFAULT 'base',
                content TEXT NOT NULL,
                variables TEXT NOT NULL DEFAULT '[]',
                is_template INTEGER NOT NULL DEFAULT 0,
                is_enabled INTEGER NOT NULL DEFAULT 1,
                version INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create prompts table: {}", e)))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS prompt_versions (
                id TEXT PRIMARY KEY,
                prompt_id TEXT NOT NULL REFERENCES prompts(id),
                version INTEGER NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create prompt_versions table: {}", e)))?;

        Ok(())
    }

    // =========================================================================
    // CRUD
    // =========================================================================

    pub async fn list_prompts(&self) -> Result<Vec<Prompt>> {
        let rows = sqlx::query(
            "SELECT id, name, category, provider, content, variables, is_template, is_enabled, version,
                    created_at, updated_at
             FROM prompts ORDER BY category ASC, name ASC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to list prompts: {}", e)))?;

        Ok(rows.iter().map(row_to_prompt).collect())
    }

    pub async fn list_by_category(&self, category: &str) -> Result<Vec<Prompt>> {
        let rows = sqlx::query(
            "SELECT id, name, category, provider, content, variables, is_template, is_enabled, version,
                    created_at, updated_at
             FROM prompts WHERE category = ? ORDER BY name ASC"
        )
        .bind(category)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to list prompts by category: {}", e)))?;

        Ok(rows.iter().map(row_to_prompt).collect())
    }

    pub async fn get_prompt(&self, id: &str) -> Result<Prompt> {
        let row = sqlx::query(
            "SELECT id, name, category, provider, content, variables, is_template, is_enabled, version,
                    created_at, updated_at
             FROM prompts WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to get prompt: {}", e)))?;

        match row {
            Some(r) => Ok(row_to_prompt(&r)),
            None => Err(VenoreError::NotFound(format!("Prompt '{}'", id))),
        }
    }

    pub async fn create_prompt(&self, prompt: &Prompt) -> Result<()> {
        sqlx::query(
            "INSERT INTO prompts (id, name, category, provider, content, variables, is_template, is_enabled, version, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&prompt.id)
        .bind(&prompt.name)
        .bind(&prompt.category)
        .bind(&prompt.provider)
        .bind(&prompt.content)
        .bind(&prompt.variables)
        .bind(prompt.is_template)
        .bind(prompt.is_enabled)
        .bind(prompt.version as i32)
        .bind(&prompt.created_at)
        .bind(&prompt.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to create prompt: {}", e)))?;

        tracing::debug!(id = %prompt.id, name = %prompt.name, "Prompt created");
        Ok(())
    }

    /// Update a prompt's content: saves old version to prompt_versions, bumps version.
    pub async fn update_prompt(&self, id: &str, new_content: &str) -> Result<Prompt> {
        let existing = self.get_prompt(id).await?;
        let now = chrono::Utc::now().to_rfc3339();

        // Save current content as a version snapshot
        let version_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO prompt_versions (id, prompt_id, version, content, created_at)
             VALUES (?, ?, ?, ?, ?)"
        )
        .bind(&version_id)
        .bind(id)
        .bind(existing.version as i32)
        .bind(&existing.content)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to save prompt version: {}", e)))?;

        // Bump version and update content
        let new_version = existing.version + 1;
        sqlx::query(
            "UPDATE prompts SET content = ?, version = ?, updated_at = ? WHERE id = ?"
        )
        .bind(new_content)
        .bind(new_version as i32)
        .bind(&now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to update prompt: {}", e)))?;

        tracing::info!(id = %id, version = new_version, "Prompt updated");
        self.get_prompt(id).await
    }

    /// Reset a prompt to its original content (version 1 from prompt_versions).
    pub async fn reset_prompt(&self, id: &str) -> Result<Prompt> {
        let original = sqlx::query(
            "SELECT content FROM prompt_versions WHERE prompt_id = ? AND version = 1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to get original version: {}", e)))?;

        let original_content: String = match original {
            Some(row) => row.get("content"),
            None => return Err(VenoreError::NotFound(format!("Original version for prompt '{}'", id))),
        };

        self.update_prompt(id, &original_content).await
    }

    pub async fn list_versions(&self, prompt_id: &str) -> Result<Vec<PromptVersion>> {
        let rows = sqlx::query(
            "SELECT id, prompt_id, version, content, created_at
             FROM prompt_versions WHERE prompt_id = ? ORDER BY version DESC"
        )
        .bind(prompt_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to list versions: {}", e)))?;

        Ok(rows.iter().map(row_to_version).collect())
    }

    pub async fn count_prompts(&self) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) as cnt FROM prompts")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to count prompts: {}", e)))?;

        Ok(row.get::<i64, _>("cnt"))
    }

    // =========================================================================
    // Prompt Resolution (used by chat, github, etc.)
    // =========================================================================

    /// Resolve the best prompt for a (category, provider) pair.
    /// Tries provider-specific override first, falls back to base.
    pub async fn resolve_prompt(&self, category: &str, provider: &str) -> Result<Prompt> {
        // 1. Try provider-specific override
        if provider != "base" {
            if let Ok(prompt) = self.find_by_category_provider(category, provider).await {
                return Ok(prompt);
            }
        }
        // 2. Fall back to base
        self.find_by_category_provider(category, "base").await
    }

    async fn find_by_category_provider(&self, category: &str, provider: &str) -> Result<Prompt> {
        let row = sqlx::query(
            "SELECT id, name, category, provider, content, variables, is_template, is_enabled, version,
                    created_at, updated_at
             FROM prompts WHERE category = ? AND provider = ? LIMIT 1"
        )
        .bind(category)
        .bind(provider)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to find prompt by category/provider: {}", e)))?;

        match row {
            Some(r) => Ok(row_to_prompt(&r)),
            None => Err(VenoreError::NotFound(format!("Prompt for category='{}' provider='{}'", category, provider))),
        }
    }

    // =========================================================================
    // Task-based queries (PromptsView redesign)
    // =========================================================================

    /// List distinct task categories (from base prompts only).
    pub async fn list_tasks(&self) -> Result<Vec<String>> {
        let rows = sqlx::query(
            "SELECT DISTINCT category FROM prompts WHERE provider = 'base' ORDER BY category"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to list tasks: {}", e)))?;

        Ok(rows.iter().map(|r| r.get::<String, _>("category")).collect())
    }

    /// Get all prompts for a task category (base + provider overrides), ordered base-first.
    pub async fn get_prompts_for_task(&self, category: &str) -> Result<Vec<Prompt>> {
        let rows = sqlx::query(
            "SELECT id, name, category, provider, content, variables, is_template, is_enabled, version,
                    created_at, updated_at
             FROM prompts WHERE category = ?
             ORDER BY CASE provider
                WHEN 'base' THEN 0
                WHEN 'anthropic' THEN 1
                WHEN 'openai' THEN 2
                WHEN 'gemini' THEN 3
                WHEN 'ollama' THEN 4
             END"
        )
        .bind(category)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to get prompts for task: {}", e)))?;

        Ok(rows.iter().map(row_to_prompt).collect())
    }

    /// Upsert a task prompt: creates a provider override or updates an existing one.
    pub async fn upsert_task_prompt(
        &self,
        category: &str,
        provider: &str,
        name: &str,
        content: &str,
        variables: &str,
    ) -> Result<Prompt> {
        let now = chrono::Utc::now().to_rfc3339();
        let id = format!("{}-{}", category, provider);

        // Check if exists
        let existing = sqlx::query("SELECT id FROM prompts WHERE id = ?")
            .bind(&id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to check prompt existence: {}", e)))?;

        if existing.is_some() {
            // Update existing — use update_prompt for versioning
            return self.update_prompt(&id, content).await;
        }

        // Create new
        let prompt = Prompt {
            id: id.clone(),
            name: name.to_string(),
            category: category.to_string(),
            provider: provider.to_string(),
            content: content.to_string(),
            variables: variables.to_string(),
            is_template: true,
            is_enabled: true,
            version: 1,
            created_at: now.clone(),
            updated_at: now,
        };

        self.create_prompt(&prompt).await?;

        // Save version 1 snapshot for reset
        let version_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO prompt_versions (id, prompt_id, version, content, created_at)
             VALUES (?, ?, 1, ?, ?)"
        )
        .bind(&version_id)
        .bind(&id)
        .bind(content)
        .bind(&prompt.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::DatabaseError(format!("Failed to save seed version: {}", e)))?;

        self.get_prompt(&id).await
    }

    // =========================================================================
    // Chat fragments (Phase 5 — system prompt blocks as templates)
    // =========================================================================

    /// List all chat-fragment prompts (the editable blocks of the system prompt).
    pub async fn list_chat_fragments(&self) -> Result<Vec<Prompt>> {
        self.list_by_category(super::fragments::CATEGORY_CHAT_FRAGMENT).await
    }

    /// Toggle the `is_enabled` flag on any prompt. Used by the UI to disable
    /// chat-fragment blocks without deleting them.
    pub async fn set_prompt_enabled(&self, id: &str, enabled: bool) -> Result<Prompt> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE prompts SET is_enabled = ?, updated_at = ? WHERE id = ?")
            .bind(enabled)
            .bind(&now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::DatabaseError(format!("Failed to set prompt enabled: {}", e)))?;
        self.get_prompt(id).await
    }
}

// =============================================================================
// Row mapping helpers
// =============================================================================

fn row_to_prompt(row: &sqlx::sqlite::SqliteRow) -> Prompt {
    // is_enabled may be missing on legacy rows: try_get + default true
    let is_enabled = row
        .try_get::<bool, _>("is_enabled")
        .unwrap_or(true);
    Prompt {
        id: row.get("id"),
        name: row.get("name"),
        category: row.get("category"),
        provider: row.get("provider"),
        content: row.get("content"),
        variables: row.get("variables"),
        is_template: row.get::<bool, _>("is_template"),
        is_enabled,
        version: row.get::<i32, _>("version") as u32,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn row_to_version(row: &sqlx::sqlite::SqliteRow) -> PromptVersion {
    PromptVersion {
        id: row.get("id"),
        prompt_id: row.get("prompt_id"),
        version: row.get::<i32, _>("version") as u32,
        content: row.get("content"),
        created_at: row.get("created_at"),
    }
}
