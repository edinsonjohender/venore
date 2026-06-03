//! Mock Configuration Store
//!
//! In-memory implementation for testing. Does not persist data.

use crate::core::config::{TaskSettings, TaskDefaults};
use crate::traits::{ApiKeyStore, ConfigStore, LlmProviderType, LlmTask, TaskConfigStore};
use crate::Result;
use std::collections::HashMap;
use std::sync::RwLock;

/// Mock configuration store for testing
///
/// Stores everything in memory using HashMaps. Useful for unit tests
/// where you don't want to interact with the actual OS keychain or database.
pub struct MockConfigStore {
    api_keys: RwLock<HashMap<LlmProviderType, String>>,
    task_settings: RwLock<HashMap<LlmTask, TaskSettings>>,
}

impl MockConfigStore {
    /// Create a new empty mock store
    pub fn new() -> Self {
        Self {
            api_keys: RwLock::new(HashMap::new()),
            task_settings: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new mock store with predefined API keys
    pub fn with_keys(keys: Vec<(LlmProviderType, &str)>) -> Self {
        let mut map = HashMap::new();
        for (provider, key) in keys {
            map.insert(provider, key.to_string());
        }

        Self {
            api_keys: RwLock::new(map),
            task_settings: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new mock store with predefined settings
    pub fn with_settings(settings: Vec<(LlmTask, TaskSettings)>) -> Self {
        let mut map = HashMap::new();
        for (task, setting) in settings {
            map.insert(task, setting);
        }

        Self {
            api_keys: RwLock::new(HashMap::new()),
            task_settings: RwLock::new(map),
        }
    }
}

impl Default for MockConfigStore {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// API KEY STORE IMPLEMENTATION
// ============================================================================

#[async_trait::async_trait]
impl ApiKeyStore for MockConfigStore {
    async fn store_api_key(&self, provider: LlmProviderType, key: String) -> Result<()> {
        self.api_keys.write().unwrap().insert(provider, key);
        Ok(())
    }

    async fn get_api_key(&self, provider: LlmProviderType) -> Result<Option<String>> {
        Ok(self.api_keys.read().unwrap().get(&provider).cloned())
    }

    async fn has_api_key(&self, provider: LlmProviderType) -> Result<bool> {
        Ok(self.api_keys.read().unwrap().contains_key(&provider))
    }

    async fn remove_api_key(&self, provider: LlmProviderType) -> Result<()> {
        self.api_keys.write().unwrap().remove(&provider);
        Ok(())
    }

    async fn list_configured_providers(&self) -> Result<Vec<LlmProviderType>> {
        Ok(self.api_keys.read().unwrap().keys().copied().collect())
    }
}

// ============================================================================
// TASK CONFIG STORE IMPLEMENTATION
// ============================================================================

#[async_trait::async_trait]
impl TaskConfigStore for MockConfigStore {
    async fn get_task_settings(&self, task: LlmTask) -> Result<TaskSettings> {
        Ok(self
            .task_settings
            .read()
            .unwrap()
            .get(&task)
            .cloned()
            .unwrap_or_else(|| TaskDefaults::get(task)))
    }

    async fn set_task_settings(&self, task: LlmTask, settings: TaskSettings) -> Result<()> {
        settings.validate()?;
        self.task_settings.write().unwrap().insert(task, settings);
        Ok(())
    }

    async fn reset_task_settings(&self, task: LlmTask) -> Result<()> {
        self.task_settings.write().unwrap().remove(&task);
        Ok(())
    }

    async fn has_custom_settings(&self, task: LlmTask) -> Result<bool> {
        Ok(self.task_settings.read().unwrap().contains_key(&task))
    }
}

// ============================================================================
// CONFIG STORE IMPLEMENTATION
// ============================================================================

#[async_trait::async_trait]
impl ConfigStore for MockConfigStore {
    async fn initialize(&self) -> Result<()> {
        // Nothing to initialize for in-memory store
        Ok(())
    }

    async fn validate(&self) -> Result<()> {
        // Validate all stored task settings
        for settings in self.task_settings.read().unwrap().values() {
            settings.validate()?;
        }

        Ok(())
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_api_key_storage() {
        let store = MockConfigStore::new();
        let provider = LlmProviderType::Anthropic;
        let key = "test-key".to_string();

        // Should not exist initially
        assert!(!store.has_api_key(provider).await.unwrap());

        // Store key
        store.store_api_key(provider, key.clone()).await.unwrap();

        // Should exist now
        assert!(store.has_api_key(provider).await.unwrap());

        // Should be able to retrieve it
        let retrieved = store.get_api_key(provider).await.unwrap().unwrap();
        assert_eq!(retrieved, key);

        // Remove key
        store.remove_api_key(provider).await.unwrap();

        // Should not exist anymore
        assert!(!store.has_api_key(provider).await.unwrap());
    }

    #[tokio::test]
    async fn test_list_configured_providers() {
        let store = MockConfigStore::new();

        // Empty initially
        assert_eq!(store.list_configured_providers().await.unwrap().len(), 0);

        // Add some keys
        store
            .store_api_key(LlmProviderType::Anthropic, "key1".to_string())
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
    }

    #[tokio::test]
    async fn test_task_settings() {
        let store = MockConfigStore::new();
        let task = LlmTask::Chat;

        // Should return defaults initially
        let settings = store.get_task_settings(task).await.unwrap();
        assert_eq!(settings, TaskDefaults::chat());
        assert!(!store.has_custom_settings(task).await.unwrap());

        // Set custom settings
        let custom = TaskSettings {
            provider: LlmProviderType::Anthropic,
            model: "custom-model".into(),
            temperature: Some(0.5),
            max_tokens: Some(1000),
            timeout_ms: Some(30000),
            streaming: Some(false),
        };

        store.set_task_settings(task, custom.clone()).await.unwrap();

        // Should have custom settings now
        assert!(store.has_custom_settings(task).await.unwrap());

        let retrieved = store.get_task_settings(task).await.unwrap();
        assert_eq!(retrieved, custom);

        // Reset to defaults
        store.reset_task_settings(task).await.unwrap();
        assert!(!store.has_custom_settings(task).await.unwrap());

        let after_reset = store.get_task_settings(task).await.unwrap();
        assert_eq!(after_reset, TaskDefaults::chat());
    }

    #[tokio::test]
    async fn test_with_keys_constructor() {
        let store = MockConfigStore::with_keys(vec![
            (LlmProviderType::Anthropic, "key1"),
            (LlmProviderType::OpenAI, "key2"),
        ]);

        assert!(store.has_api_key(LlmProviderType::Anthropic).await.unwrap());
        assert!(store.has_api_key(LlmProviderType::OpenAI).await.unwrap());
        assert!(!store.has_api_key(LlmProviderType::Gemini).await.unwrap());
    }

    #[tokio::test]
    async fn test_with_settings_constructor() {
        let custom_settings = TaskSettings {
            provider: LlmProviderType::Anthropic,
            model: "test-model".into(),
            temperature: Some(0.8),
            max_tokens: Some(500),
            timeout_ms: Some(20000),
            streaming: Some(true),
        };

        let store =
            MockConfigStore::with_settings(vec![(LlmTask::Chat, custom_settings.clone())]);

        let settings = store.get_task_settings(LlmTask::Chat).await.unwrap();
        assert_eq!(settings, custom_settings);

        // Other tasks should still return defaults
        let onboarding = store.get_task_settings(LlmTask::Onboarding).await.unwrap();
        assert_eq!(onboarding, TaskDefaults::onboarding());
    }

    #[tokio::test]
    async fn test_initialize_and_validate() {
        let store = MockConfigStore::new();

        // Initialize should succeed
        store.initialize().await.unwrap();

        // Validate should succeed for empty store
        store.validate().await.unwrap();

        // Add valid settings
        let valid = TaskSettings {
            provider: LlmProviderType::Anthropic,
            model: "model".into(),
            temperature: Some(0.7),
            max_tokens: Some(100),
            timeout_ms: Some(30000),
            streaming: None,
        };

        store.set_task_settings(LlmTask::Chat, valid).await.unwrap();

        // Validate should still succeed
        store.validate().await.unwrap();
    }
}
