//! Keyring API Key Storage
//!
//! Secure storage of API keys using OS keychain:
//! - Windows: DPAPI (Data Protection API)
//! - macOS: Keychain
//! - Linux: Secret Service API

use crate::traits::{ApiKeyStore, LlmProviderType};
use crate::{Result, VenoreError};

/// Service name for keyring entries.
///
/// Debug builds use a separate namespace (`venore.ai.dev`) so the development
/// build (`cargo tauri dev`) and an installed release never share API keys —
/// mirroring the data-directory split in `venore-desktop`'s `state.rs`
/// (`%TEMP%/venore-dev` vs `~/.venore`).
const SERVICE_NAME: &str = if cfg!(debug_assertions) {
    "venore.ai.dev"
} else {
    "venore.ai"
};

/// API key storage using OS keychain
///
/// Stores API keys securely in the operating system's native credential store.
/// Keys are encrypted at rest and require OS authentication to access.
///
/// # Platform Support
///
/// - **Windows**: Uses DPAPI (Data Protection API)
/// - **macOS**: Uses Keychain
/// - **Linux**: Uses Secret Service API (requires libsecret)
///
/// # Examples
///
/// ```ignore
/// use venore_core::infrastructure::config::KeyringApiKeyStore;
/// use venore_core::traits::{ApiKeyStore, LlmProviderType};
///
/// let store = KeyringApiKeyStore::new();
///
/// // Store API key
/// store.store_api_key(
///     LlmProviderType::Anthropic,
///     "sk-ant-...".to_string()
/// ).await?;
///
/// // Retrieve API key
/// let key = store.get_api_key(LlmProviderType::Anthropic).await?;
/// ```
pub struct KeyringApiKeyStore;

impl KeyringApiKeyStore {
    /// Create a new keyring store
    pub fn new() -> Self {
        Self
    }

    /// Get keyring key name for a provider
    fn get_key_name(provider: LlmProviderType) -> String {
        format!("{}_api_key", provider.as_str())
    }
}

impl Default for KeyringApiKeyStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ApiKeyStore for KeyringApiKeyStore {
    async fn store_api_key(&self, provider: LlmProviderType, key: String) -> Result<()> {
        let key_name = Self::get_key_name(provider);

        // Create keyring entry
        let entry = keyring::Entry::new(SERVICE_NAME, &key_name)
            .map_err(|e| VenoreError::Unknown(format!("Failed to create keyring entry: {}", e)))?;

        // Store the key
        entry
            .set_password(&key)
            .map_err(|e| VenoreError::Unknown(format!("Failed to store API key: {}", e)))?;

        tracing::debug!("Stored API key for provider: {}", provider.as_str());

        Ok(())
    }

    async fn get_api_key(&self, provider: LlmProviderType) -> Result<Option<String>> {
        let key_name = Self::get_key_name(provider);

        // Create keyring entry
        let entry = keyring::Entry::new(SERVICE_NAME, &key_name)
            .map_err(|e| VenoreError::Unknown(format!("Failed to create keyring entry: {}", e)))?;

        // Get the key
        match entry.get_password() {
            Ok(password) => {
                tracing::debug!("Retrieved API key for provider: {}", provider.as_str());
                Ok(Some(password))
            }
            Err(keyring::Error::NoEntry) => {
                tracing::debug!("No API key found for provider: {}", provider.as_str());
                Ok(None)
            }
            Err(e) => Err(VenoreError::Unknown(format!(
                "Failed to retrieve API key: {}",
                e
            ))),
        }
    }

    async fn has_api_key(&self, provider: LlmProviderType) -> Result<bool> {
        Ok(self.get_api_key(provider).await?.is_some())
    }

    async fn remove_api_key(&self, provider: LlmProviderType) -> Result<()> {
        let key_name = Self::get_key_name(provider);

        // Create keyring entry
        let entry = keyring::Entry::new(SERVICE_NAME, &key_name)
            .map_err(|e| VenoreError::Unknown(format!("Failed to create keyring entry: {}", e)))?;

        // Delete the key
        match entry.delete_credential() {
            Ok(_) => {
                tracing::debug!("Removed API key for provider: {}", provider.as_str());
                Ok(())
            }
            Err(keyring::Error::NoEntry) => {
                // Key doesn't exist, that's fine
                tracing::debug!("No API key to remove for provider: {}", provider.as_str());
                Ok(())
            }
            Err(e) => Err(VenoreError::Unknown(format!(
                "Failed to remove API key: {}",
                e
            ))),
        }
    }

    async fn list_configured_providers(&self) -> Result<Vec<LlmProviderType>> {
        // Check each known provider
        let mut configured = Vec::new();

        for provider in [
            LlmProviderType::Anthropic,
            LlmProviderType::OpenAI,
            LlmProviderType::Gemini,
            LlmProviderType::Tavily,
        ] {
            if self.has_api_key(provider).await? {
                configured.push(provider);
            }
        }

        Ok(configured)
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // NOTE: These tests interact with the actual OS keychain
    // They should be run sequentially and clean up after themselves

    #[tokio::test]
    #[ignore] // Only run explicitly to avoid polluting keychain
    async fn test_store_and_retrieve() {
        let store = KeyringApiKeyStore::new();
        let provider = LlmProviderType::Anthropic;
        let test_key = "test-key-12345".to_string();

        // Store
        store
            .store_api_key(provider, test_key.clone())
            .await
            .unwrap();

        // Retrieve
        let retrieved = store.get_api_key(provider).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), test_key);

        // Cleanup
        store.remove_api_key(provider).await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn test_has_api_key() {
        let store = KeyringApiKeyStore::new();
        let provider = LlmProviderType::OpenAI;

        // Should not exist initially
        assert!(!store.has_api_key(provider).await.unwrap());

        // Store key
        store
            .store_api_key(provider, "test-key".to_string())
            .await
            .unwrap();

        // Should exist now
        assert!(store.has_api_key(provider).await.unwrap());

        // Cleanup
        store.remove_api_key(provider).await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn test_remove_api_key() {
        let store = KeyringApiKeyStore::new();
        let provider = LlmProviderType::Gemini;

        // Store key
        store
            .store_api_key(provider, "test-key".to_string())
            .await
            .unwrap();

        // Verify it exists
        assert!(store.has_api_key(provider).await.unwrap());

        // Remove
        store.remove_api_key(provider).await.unwrap();

        // Verify it's gone
        assert!(!store.has_api_key(provider).await.unwrap());

        // Removing again should not error
        store.remove_api_key(provider).await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn test_list_configured_providers() {
        let store = KeyringApiKeyStore::new();

        // Clean up first
        for provider in [
            LlmProviderType::Anthropic,
            LlmProviderType::OpenAI,
            LlmProviderType::Gemini,
        ] {
            let _ = store.remove_api_key(provider).await;
        }

        // Should be empty
        let providers = store.list_configured_providers().await.unwrap();
        assert_eq!(providers.len(), 0);

        // Add two providers
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

        // Should have 2
        let providers = store.list_configured_providers().await.unwrap();
        assert_eq!(providers.len(), 2);
        assert!(providers.contains(&LlmProviderType::Anthropic));
        assert!(providers.contains(&LlmProviderType::OpenAI));

        // Cleanup
        store
            .remove_api_key(LlmProviderType::Anthropic)
            .await
            .unwrap();
        store.remove_api_key(LlmProviderType::OpenAI).await.unwrap();
    }

    #[tokio::test]
    #[ignore] // Hits the real OS keychain (needs secret-service/DBus on Linux) — run explicitly
    async fn test_get_nonexistent_key() {
        let store = KeyringApiKeyStore::new();

        // Ensure key doesn't exist
        let _ = store.remove_api_key(LlmProviderType::Anthropic).await;

        // Getting non-existent key should return None
        let result = store.get_api_key(LlmProviderType::Anthropic).await.unwrap();
        assert!(result.is_none());
    }
}
