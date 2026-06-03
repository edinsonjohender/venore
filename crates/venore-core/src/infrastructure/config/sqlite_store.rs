//! SQLite Task Configuration Store
//!
//! Persistent storage of task settings using SQLite database.

use crate::core::config::{TaskSettings, TaskDefaults};
use crate::traits::{LlmTask, TaskConfigStore};
use crate::{Result, VenoreError};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use std::str::FromStr;

/// SQLite-based task configuration store
///
/// Stores task settings in a SQLite database for persistent configuration.
/// Falls back to defaults when no custom settings exist.
///
/// # Database Schema
///
/// Uses the `task_settings` table created by migration:
/// ```sql
/// CREATE TABLE task_settings (
///     task TEXT PRIMARY KEY,
///     provider TEXT NOT NULL,
///     model TEXT NOT NULL,
///     temperature REAL,
///     max_tokens INTEGER,
///     timeout_ms INTEGER,
///     streaming INTEGER,
///     created_at TEXT,
///     updated_at TEXT
/// );
/// ```
///
/// # Examples
///
/// ```ignore
/// use venore_core::infrastructure::config::SqliteTaskConfigStore;
///
/// let store = SqliteTaskConfigStore::new("sqlite:~/.venore/config.db").await?;
/// store.initialize().await?;
///
/// let settings = store.get_task_settings(LlmTask::Chat).await?;
/// ```
pub struct SqliteTaskConfigStore {
    pool: SqlitePool,
}

impl SqliteTaskConfigStore {
    /// Create a new SQLite store
    ///
    /// # Arguments
    ///
    /// * `database_url` - SQLite connection string (e.g., "sqlite:config.db")
    ///
    /// # Returns
    ///
    /// * `Ok(SqliteTaskConfigStore)` - Store ready to use
    /// * `Err(VenoreError)` - If connection fails
    pub async fn new(database_url: &str) -> Result<Self> {
        // Pragmas applied to every pooled connection:
        // - `foreign_keys=ON` — enforce FK constraints (off by default in SQLite).
        // - `journal_mode=WAL` — concurrent readers + one writer per file, with
        //   readers never blocking the writer and vice versa. Lets two Venore
        //   processes share `config.db` without the default DELETE journal's
        //   "database is locked" collisions when they write at the same time.
        // - `busy_timeout=5000` — if a write still hits the file lock (writer
        //   contention across processes), wait up to 5s before erroring
        //   instead of returning SQLITE_BUSY immediately.
        let connect_options = SqliteConnectOptions::from_str(database_url)
            .map_err(|e| VenoreError::Unknown(format!("Invalid database URL: {}", e)))?
            .pragma("foreign_keys", "ON")
            .pragma("journal_mode", "WAL")
            .pragma("busy_timeout", "5000")
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(connect_options)
            .await
            .map_err(|e| VenoreError::Unknown(format!("Failed to connect to database: {}", e)))?;

        Ok(Self { pool })
    }

    /// Initialize the database (run migrations)
    ///
    /// Creates tables and indexes if they don't exist.
    pub async fn initialize(&self) -> Result<()> {
        // Read migration file
        let migration = include_str!("../../../migrations/20260120_001_create_task_settings.sql");

        // Execute migration
        sqlx::query(migration)
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::Unknown(format!("Failed to run migration: {}", e)))?;

        // Create app_settings table for generic key-value settings
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL
            )"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::Unknown(format!("Failed to create app_settings table: {}", e)))?;

        tracing::info!("Database initialized successfully");

        Ok(())
    }

    /// Get a generic app setting by key
    pub async fn get_app_setting(&self, key: &str) -> Result<Option<String>> {
        let row = sqlx::query("SELECT value FROM app_settings WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| VenoreError::Unknown(format!("Failed to get app setting: {}", e)))?;

        Ok(row.map(|r| r.get::<String, _>("value")))
    }

    /// Set a generic app setting (INSERT OR REPLACE)
    pub async fn set_app_setting(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query("INSERT OR REPLACE INTO app_settings (key, value) VALUES (?, ?)")
            .bind(key)
            .bind(value)
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::Unknown(format!("Failed to set app setting: {}", e)))?;

        tracing::info!(key = %key, "App setting updated");
        Ok(())
    }

    /// Get a clone of the connection pool (for sharing with other repositories)
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Convert LlmTask to database string
    fn task_to_string(task: LlmTask) -> &'static str {
        match task {
            LlmTask::Onboarding => "onboarding",
            LlmTask::Chat => "chat",
            LlmTask::Analysis => "analysis",
            LlmTask::Embeddings => "embeddings",
        }
    }

    /// Convert database boolean (INTEGER 0/1) to Option<bool>
    fn db_bool_to_option(value: Option<i64>) -> Option<bool> {
        value.map(|v| v != 0)
    }

    /// Convert Option<bool> to database INTEGER
    fn option_to_db_bool(value: Option<bool>) -> Option<i64> {
        value.map(|v| if v { 1 } else { 0 })
    }
}

#[async_trait::async_trait]
impl TaskConfigStore for SqliteTaskConfigStore {
    async fn get_task_settings(&self, task: LlmTask) -> Result<TaskSettings> {
        let task_str = Self::task_to_string(task);

        // Query database
        let row_result = sqlx::query(
            "SELECT provider, model, temperature, max_tokens, timeout_ms, streaming
             FROM task_settings WHERE task = ?",
        )
        .bind(task_str)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| VenoreError::Unknown(format!("Failed to query task settings: {}", e)))?;

        // If no custom settings, return defaults
        let row = match row_result {
            Some(r) => r,
            None => {
                tracing::debug!("No custom settings for {:?}, using defaults", task);
                return Ok(TaskDefaults::get(task));
            }
        };

        // Parse provider
        let provider_str: String = row
            .try_get("provider")
            .map_err(|e| VenoreError::Unknown(format!("Failed to get provider: {}", e)))?;
        let provider = std::str::FromStr::from_str(&provider_str)?;

        // Parse other fields
        let model: String = row
            .try_get("model")
            .map_err(|e| VenoreError::Unknown(format!("Failed to get model: {}", e)))?;

        let temperature: Option<f64> = row.try_get::<Option<f64>, _>("temperature")
            .unwrap_or(None);
        let max_tokens: Option<i64> = row.try_get::<Option<i64>, _>("max_tokens")
            .unwrap_or(None);
        let timeout_ms: Option<i64> = row.try_get::<Option<i64>, _>("timeout_ms")
            .unwrap_or(None);
        let streaming: Option<i64> = row.try_get::<Option<i64>, _>("streaming")
            .unwrap_or(None);

        Ok(TaskSettings {
            provider,
            model,
            temperature: temperature.map(|t| t as f32),
            max_tokens: max_tokens.map(|t| t as u32),
            timeout_ms: timeout_ms.map(|t| t as u64),
            streaming: Self::db_bool_to_option(streaming),
        })
    }

    async fn set_task_settings(&self, task: LlmTask, settings: TaskSettings) -> Result<()> {
        // Validate settings first
        settings.validate()?;

        let task_str = Self::task_to_string(task);

        // Insert or replace
        sqlx::query(
            "INSERT OR REPLACE INTO task_settings
             (task, provider, model, temperature, max_tokens, timeout_ms, streaming, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, datetime('now'), datetime('now'))",
        )
        .bind(task_str)
        .bind(settings.provider.as_str())
        .bind(&settings.model)
        .bind(settings.temperature.map(|t| t as f64))
        .bind(settings.max_tokens.map(|t| t as i64))
        .bind(settings.timeout_ms.map(|t| t as i64))
        .bind(Self::option_to_db_bool(settings.streaming))
        .execute(&self.pool)
        .await
        .map_err(|e| VenoreError::Unknown(format!("Failed to save task settings: {}", e)))?;

        tracing::info!("Saved settings for task: {:?}", task);

        Ok(())
    }

    async fn reset_task_settings(&self, task: LlmTask) -> Result<()> {
        let task_str = Self::task_to_string(task);

        sqlx::query("DELETE FROM task_settings WHERE task = ?")
            .bind(task_str)
            .execute(&self.pool)
            .await
            .map_err(|e| VenoreError::Unknown(format!("Failed to reset task settings: {}", e)))?;

        tracing::info!("Reset settings for task: {:?}", task);

        Ok(())
    }

    async fn has_custom_settings(&self, task: LlmTask) -> Result<bool> {
        let task_str = Self::task_to_string(task);

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task_settings WHERE task = ?")
            .bind(task_str)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| VenoreError::Unknown(format!("Failed to check task settings: {}", e)))?;

        Ok(count > 0)
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::LlmProviderType;

    async fn create_test_store() -> SqliteTaskConfigStore {
        // Use in-memory database for tests
        let store = SqliteTaskConfigStore::new("sqlite::memory:")
            .await
            .unwrap();
        store.initialize().await.unwrap();
        store
    }

    #[tokio::test]
    async fn test_get_default_settings() {
        let store = create_test_store().await;

        // Should return defaults when no custom settings
        let settings = store.get_task_settings(LlmTask::Chat).await.unwrap();
        assert_eq!(settings, TaskDefaults::chat());
        assert!(!store.has_custom_settings(LlmTask::Chat).await.unwrap());
    }

    #[tokio::test]
    async fn test_set_and_get_settings() {
        let store = create_test_store().await;

        let custom = TaskSettings {
            provider: LlmProviderType::Anthropic,
            model: "custom-model".into(),
            temperature: Some(0.5),
            max_tokens: Some(1000),
            timeout_ms: Some(30000),
            streaming: Some(true),
        };

        // Set custom settings
        store
            .set_task_settings(LlmTask::Chat, custom.clone())
            .await
            .unwrap();

        // Should have custom settings now
        assert!(store.has_custom_settings(LlmTask::Chat).await.unwrap());

        // Should retrieve the same settings
        let retrieved = store.get_task_settings(LlmTask::Chat).await.unwrap();
        assert_eq!(retrieved, custom);
    }

    #[tokio::test]
    async fn test_reset_settings() {
        let store = create_test_store().await;

        // Set custom settings
        let custom = TaskSettings {
            provider: LlmProviderType::Anthropic,
            model: "test-model".into(),
            temperature: Some(0.8),
            max_tokens: Some(500),
            timeout_ms: Some(20000),
            streaming: Some(false),
        };

        store
            .set_task_settings(LlmTask::Onboarding, custom)
            .await
            .unwrap();

        // Verify it exists
        assert!(store
            .has_custom_settings(LlmTask::Onboarding)
            .await
            .unwrap());

        // Reset
        store.reset_task_settings(LlmTask::Onboarding).await.unwrap();

        // Should not have custom settings anymore
        assert!(!store
            .has_custom_settings(LlmTask::Onboarding)
            .await
            .unwrap());

        // Should return defaults
        let settings = store.get_task_settings(LlmTask::Onboarding).await.unwrap();
        assert_eq!(settings, TaskDefaults::onboarding());
    }

    #[tokio::test]
    async fn test_update_settings() {
        let store = create_test_store().await;

        // Set initial settings
        let initial = TaskSettings {
            provider: LlmProviderType::Anthropic,
            model: "model-v1".into(),
            temperature: Some(0.5),
            max_tokens: Some(1000),
            timeout_ms: Some(30000),
            streaming: Some(false),
        };

        store
            .set_task_settings(LlmTask::Analysis, initial)
            .await
            .unwrap();

        // Update settings
        let updated = TaskSettings {
            provider: LlmProviderType::Anthropic,
            model: "model-v2".into(),
            temperature: Some(0.3),
            max_tokens: Some(2000),
            timeout_ms: Some(60000),
            streaming: Some(true),
        };

        store
            .set_task_settings(LlmTask::Analysis, updated.clone())
            .await
            .unwrap();

        // Should have the updated settings
        let retrieved = store.get_task_settings(LlmTask::Analysis).await.unwrap();
        assert_eq!(retrieved, updated);
        assert_eq!(retrieved.model, "model-v2");
    }

    #[tokio::test]
    async fn test_multiple_tasks() {
        let store = create_test_store().await;

        // Set different settings for each task
        let chat_settings = TaskSettings {
            provider: LlmProviderType::Anthropic,
            model: "chat-model".into(),
            temperature: Some(0.7),
            max_tokens: Some(1000),
            timeout_ms: Some(30000),
            streaming: Some(true),
        };

        let analysis_settings = TaskSettings {
            provider: LlmProviderType::Anthropic,
            model: "analysis-model".into(),
            temperature: Some(0.2),
            max_tokens: Some(2000),
            timeout_ms: Some(60000),
            streaming: Some(false),
        };

        store
            .set_task_settings(LlmTask::Chat, chat_settings.clone())
            .await
            .unwrap();
        store
            .set_task_settings(LlmTask::Analysis, analysis_settings.clone())
            .await
            .unwrap();

        // Each task should have its own settings
        let chat = store.get_task_settings(LlmTask::Chat).await.unwrap();
        let analysis = store.get_task_settings(LlmTask::Analysis).await.unwrap();

        assert_eq!(chat, chat_settings);
        assert_eq!(analysis, analysis_settings);
        assert_ne!(chat.model, analysis.model);

        // Onboarding should still return defaults
        let onboarding = store.get_task_settings(LlmTask::Onboarding).await.unwrap();
        assert_eq!(onboarding, TaskDefaults::onboarding());
    }

    #[tokio::test]
    async fn test_validation() {
        let store = create_test_store().await;

        // Invalid temperature
        let invalid = TaskSettings {
            provider: LlmProviderType::Anthropic,
            model: "test-model".into(),
            temperature: Some(5.0), // Invalid
            max_tokens: Some(1000),
            timeout_ms: Some(30000),
            streaming: None,
        };

        let result = store.set_task_settings(LlmTask::Chat, invalid).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_none_values() {
        let store = create_test_store().await;

        // Settings with None values
        let settings = TaskSettings {
            provider: LlmProviderType::Anthropic,
            model: "test-model".into(),
            temperature: None,
            max_tokens: None,
            timeout_ms: None,
            streaming: None,
        };

        store
            .set_task_settings(LlmTask::Chat, settings.clone())
            .await
            .unwrap();

        let retrieved = store.get_task_settings(LlmTask::Chat).await.unwrap();
        assert_eq!(retrieved, settings);
        assert!(retrieved.temperature.is_none());
        assert!(retrieved.max_tokens.is_none());
    }
}
