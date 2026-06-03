//! Default Configuration Store
//!
//! Production-ready store combining keyring and SQLite storage.

use super::{KeyringApiKeyStore, SqliteTaskConfigStore};
use crate::core::config::TaskSettings;
use crate::traits::{ApiKeyStore, ConfigStore, LlmProviderType, LlmTask, TaskConfigStore};
use crate::{Result, VenoreError};

/// Production configuration store
///
/// Combines two storage backends:
/// - **KeyringApiKeyStore**: Secure OS keychain for API keys
/// - **SqliteTaskConfigStore**: SQLite database for task settings
///
/// This provides the best of both worlds:
/// - API keys are encrypted by the OS and require authentication
/// - Task settings are persisted in a local database
///
/// # Examples
///
/// ```ignore
/// use venore_core::infrastructure::config::DefaultConfigStore;
///
/// let store = DefaultConfigStore::new("sqlite:~/.venore/config.db").await?;
/// store.initialize().await?;
///
/// // Store API key (goes to OS keychain)
/// store.store_api_key(
///     LlmProviderType::Anthropic,
///     "sk-ant-...".to_string()
/// ).await?;
///
/// // Store task settings (goes to SQLite)
/// store.set_task_settings(LlmTask::Chat, settings).await?;
/// ```
pub struct DefaultConfigStore {
    keyring_store: KeyringApiKeyStore,
    sqlite_store: SqliteTaskConfigStore,
}

impl DefaultConfigStore {
    /// Create a new default store
    ///
    /// # Arguments
    ///
    /// * `database_url` - SQLite connection string for task settings
    ///
    /// # Returns
    ///
    /// * `Ok(DefaultConfigStore)` - Store ready to use
    /// * `Err(VenoreError)` - If SQLite connection fails
    pub async fn new(database_url: &str) -> Result<Self> {
        let keyring_store = KeyringApiKeyStore::new();
        let sqlite_store = SqliteTaskConfigStore::new(database_url).await?;

        Ok(Self {
            keyring_store,
            sqlite_store,
        })
    }

    /// Get reference to keyring store
    pub fn keyring_store(&self) -> &KeyringApiKeyStore {
        &self.keyring_store
    }

    /// Get reference to SQLite store
    pub fn sqlite_store(&self) -> &SqliteTaskConfigStore {
        &self.sqlite_store
    }

    /// Get the SQLite connection pool (for sharing with other repositories)
    pub fn pool(&self) -> &sqlx::sqlite::SqlitePool {
        self.sqlite_store.pool()
    }

    /// Get a generic app setting by key
    pub async fn get_app_setting(&self, key: &str) -> crate::Result<Option<String>> {
        self.sqlite_store.get_app_setting(key).await
    }

    /// Set a generic app setting
    pub async fn set_app_setting(&self, key: &str, value: &str) -> crate::Result<()> {
        self.sqlite_store.set_app_setting(key, value).await
    }
}

// ============================================================================
// API KEY STORE IMPLEMENTATION (delegates to keyring)
// ============================================================================

#[async_trait::async_trait]
impl ApiKeyStore for DefaultConfigStore {
    async fn store_api_key(&self, provider: LlmProviderType, key: String) -> Result<()> {
        self.keyring_store.store_api_key(provider, key).await
    }

    async fn get_api_key(&self, provider: LlmProviderType) -> Result<Option<String>> {
        self.keyring_store.get_api_key(provider).await
    }

    async fn has_api_key(&self, provider: LlmProviderType) -> Result<bool> {
        self.keyring_store.has_api_key(provider).await
    }

    async fn remove_api_key(&self, provider: LlmProviderType) -> Result<()> {
        self.keyring_store.remove_api_key(provider).await
    }

    async fn list_configured_providers(&self) -> Result<Vec<LlmProviderType>> {
        self.keyring_store.list_configured_providers().await
    }
}

// ============================================================================
// TASK CONFIG STORE IMPLEMENTATION (delegates to SQLite)
// ============================================================================

#[async_trait::async_trait]
impl TaskConfigStore for DefaultConfigStore {
    async fn get_task_settings(&self, task: LlmTask) -> Result<TaskSettings> {
        self.sqlite_store.get_task_settings(task).await
    }

    async fn set_task_settings(&self, task: LlmTask, settings: TaskSettings) -> Result<()> {
        self.sqlite_store.set_task_settings(task, settings).await
    }

    async fn reset_task_settings(&self, task: LlmTask) -> Result<()> {
        self.sqlite_store.reset_task_settings(task).await
    }

    async fn has_custom_settings(&self, task: LlmTask) -> Result<bool> {
        self.sqlite_store.has_custom_settings(task).await
    }
}

// ============================================================================
// CONFIG STORE IMPLEMENTATION
// ============================================================================

#[async_trait::async_trait]
impl ConfigStore for DefaultConfigStore {
    async fn initialize(&self) -> Result<()> {
        // Initialize SQLite database (keyring doesn't need initialization)
        self.sqlite_store.initialize().await?;

        tracing::info!("DefaultConfigStore initialized successfully");

        Ok(())
    }

    async fn validate(&self) -> Result<()> {
        // Check that at least one provider has an API key
        let configured_providers = self.list_configured_providers().await?;

        if configured_providers.is_empty() {
            return Err(VenoreError::LlmNoApiKey(
                "No API keys configured. Please configure at least one provider.".into(),
            ));
        }

        tracing::info!(
            "Configuration valid: {} provider(s) configured",
            configured_providers.len()
        );

        Ok(())
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::Mutex;

    /// Mutex to serialize tests that use the OS keyring (shared global resource)
    static KEYRING_LOCK: std::sync::LazyLock<Mutex<()>> = std::sync::LazyLock::new(|| Mutex::new(()));

    async fn create_test_store() -> DefaultConfigStore {
        DefaultConfigStore::new("sqlite::memory:")
            .await
            .unwrap()
    }

    /// Remove all test keys from OS keyring to avoid cross-test pollution
    async fn cleanup_keyring(store: &DefaultConfigStore) {
        let providers = [
            LlmProviderType::Anthropic,
            LlmProviderType::OpenAI,
            LlmProviderType::Gemini,
        ];
        for p in providers {
            let _ = store.remove_api_key(p).await;
        }
    }

    #[tokio::test]
    async fn test_initialize() {
        let store = create_test_store().await;
        assert!(store.initialize().await.is_ok());
    }

    #[tokio::test]
    #[ignore] // Touches the real OS keychain (shared across processes) — non-hermetic; run explicitly with --ignored
    async fn test_api_key_operations() {
        let _lock = KEYRING_LOCK.lock().await;
        let store = create_test_store().await;
        store.initialize().await.unwrap();
        cleanup_keyring(&store).await;

        let provider = LlmProviderType::Anthropic;
        let key = "test-key-123".to_string();

        // Should not exist initially
        assert!(!store.has_api_key(provider).await.unwrap());

        // Store key
        store.store_api_key(provider, key.clone()).await.unwrap();

        // Should exist now
        assert!(store.has_api_key(provider).await.unwrap());

        // Should retrieve the same key
        let retrieved = store.get_api_key(provider).await.unwrap().unwrap();
        assert_eq!(retrieved, key);

        // Cleanup
        cleanup_keyring(&store).await;
    }

    #[tokio::test]
    async fn test_task_settings_operations() {
        let store = create_test_store().await;
        store.initialize().await.unwrap();

        let task = LlmTask::Chat;
        let custom = TaskSettings {
            provider: LlmProviderType::Anthropic,
            model: "custom-model".into(),
            temperature: Some(0.5),
            max_tokens: Some(1000),
            timeout_ms: Some(30000),
            streaming: Some(true),
        };

        // Should not have custom settings initially
        assert!(!store.has_custom_settings(task).await.unwrap());

        // Set custom settings
        store.set_task_settings(task, custom.clone()).await.unwrap();

        // Should have custom settings now
        assert!(store.has_custom_settings(task).await.unwrap());

        // Should retrieve the same settings
        let retrieved = store.get_task_settings(task).await.unwrap();
        assert_eq!(retrieved, custom);

        // Reset
        store.reset_task_settings(task).await.unwrap();
        assert!(!store.has_custom_settings(task).await.unwrap());
    }

    #[tokio::test]
    #[ignore] // Touches the real OS keychain (shared across processes) — non-hermetic; run explicitly with --ignored
    async fn test_validate_no_api_keys() {
        let _lock = KEYRING_LOCK.lock().await;
        let store = create_test_store().await;
        store.initialize().await.unwrap();
        cleanup_keyring(&store).await;

        // Should fail validation with no API keys
        let result = store.validate().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), VenoreError::LlmNoApiKey(_)));
    }

    #[tokio::test]
    #[ignore] // Touches the real OS keychain (shared across processes) — non-hermetic; run explicitly with --ignored
    async fn test_validate_with_api_key() {
        let _lock = KEYRING_LOCK.lock().await;
        let store = create_test_store().await;
        store.initialize().await.unwrap();
        cleanup_keyring(&store).await;

        // Add an API key
        store
            .store_api_key(
                LlmProviderType::Anthropic,
                "test-key".to_string(),
            )
            .await
            .unwrap();

        // Should pass validation now
        assert!(store.validate().await.is_ok());

        // Cleanup
        cleanup_keyring(&store).await;
    }

    #[tokio::test]
    #[ignore] // Touches the real OS keychain (shared across processes) — non-hermetic; run explicitly with --ignored
    async fn test_list_configured_providers() {
        let _lock = KEYRING_LOCK.lock().await;
        let store = create_test_store().await;
        store.initialize().await.unwrap();
        cleanup_keyring(&store).await;

        // Empty initially
        assert_eq!(store.list_configured_providers().await.unwrap().len(), 0);

        // Add providers
        store
            .store_api_key(
                LlmProviderType::Anthropic,
                "key1".to_string(),
            )
            .await
            .unwrap();
        store
            .store_api_key(LlmProviderType::OpenAI, "key2".to_string())
            .await
            .unwrap();

        let providers = store.list_configured_providers().await.unwrap();
        assert_eq!(providers.len(), 2);
        assert!(providers.contains(&LlmProviderType::Anthropic));
        assert!(providers.contains(&LlmProviderType::OpenAI));

        // Cleanup
        cleanup_keyring(&store).await;
    }

    #[tokio::test]
    #[ignore] // Touches the real OS keychain (shared across processes) — non-hermetic; run explicitly with --ignored
    async fn test_separate_storage() {
        let _lock = KEYRING_LOCK.lock().await;
        let store = create_test_store().await;
        store.initialize().await.unwrap();
        cleanup_keyring(&store).await;

        // API keys and task settings are stored separately
        // Changing one should not affect the other

        // Set API key
        store
            .store_api_key(
                LlmProviderType::Anthropic,
                "api-key".to_string(),
            )
            .await
            .unwrap();

        // Set task settings
        let settings = TaskSettings {
            provider: LlmProviderType::Anthropic,
            model: "test-model".into(),
            temperature: Some(0.7),
            max_tokens: Some(100),
            timeout_ms: Some(30000),
            streaming: None,
        };
        store
            .set_task_settings(LlmTask::Chat, settings.clone())
            .await
            .unwrap();

        // Both should be retrievable
        assert!(store.has_api_key(LlmProviderType::Anthropic).await.unwrap());
        assert!(store.has_custom_settings(LlmTask::Chat).await.unwrap());

        // Removing API key should not affect task settings
        store
            .remove_api_key(LlmProviderType::Anthropic)
            .await
            .unwrap();
        assert!(store.has_custom_settings(LlmTask::Chat).await.unwrap());

        // Resetting task settings should not affect API keys
        // (but we removed it already, so this just tests the separation)
        store.reset_task_settings(LlmTask::Chat).await.unwrap();
        assert!(!store.has_custom_settings(LlmTask::Chat).await.unwrap());
    }
}
