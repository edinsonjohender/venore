//! Configuration Loader
//!
//! Loads configuration with precedence:
//! 1. Environment variables (highest priority)
//! 2. SQLite database (persistent user settings)
//! 3. Default values (fallback)

use super::models::TaskSettings;
use super::defaults::TaskDefaults;
use crate::traits::{LlmTask, LlmProviderType, TaskConfigStore};
use crate::Result;
use std::str::FromStr;

// ============================================================================
// ENVIRONMENT VARIABLE LOADING
// ============================================================================

/// Load task settings from environment variables
///
/// Environment variables follow the pattern:
/// `VENORE_TASK_{TASK}_{SETTING}`
///
/// Examples:
/// - `VENORE_TASK_ONBOARDING_PROVIDER=anthropic`
/// - `VENORE_TASK_ONBOARDING_MODEL=claude-haiku-4-5`
/// - `VENORE_TASK_ONBOARDING_TEMPERATURE=0.5`
/// - `VENORE_TASK_CHAT_MAX_TOKENS=2000`
///
/// # Arguments
///
/// * `task` - Task to load settings for
///
/// # Returns
///
/// * `Ok(Some(TaskSettings))` - Settings loaded from env
/// * `Ok(None)` - No env vars found
/// * `Err(VenoreError)` - Invalid env var values
fn load_from_env(task: LlmTask) -> Result<Option<TaskSettings>> {
    let task_name = match task {
        LlmTask::Onboarding => "ONBOARDING",
        LlmTask::Chat => "CHAT",
        LlmTask::Analysis => "ANALYSIS",
        LlmTask::Embeddings => "EMBEDDINGS",
    };

    // Check if any env vars exist for this task
    let provider_key = format!("VENORE_TASK_{}_PROVIDER", task_name);
    let model_key = format!("VENORE_TASK_{}_MODEL", task_name);

    let provider_env = std::env::var(&provider_key).ok();
    let model_env = std::env::var(&model_key).ok();

    // If no provider or model specified, no env config exists
    if provider_env.is_none() && model_env.is_none() {
        return Ok(None);
    }

    // Load base settings from defaults
    let mut settings = TaskDefaults::get(task);

    // Override provider if specified
    if let Some(provider_str) = provider_env {
        settings.provider = LlmProviderType::from_str(&provider_str)?;
    }

    // Override model if specified
    if let Some(model) = model_env {
        settings.model = model;
    }

    // Override temperature if specified
    let temp_key = format!("VENORE_TASK_{}_TEMPERATURE", task_name);
    if let Ok(temp_str) = std::env::var(&temp_key) {
        let temp: f32 = temp_str.parse().map_err(|_| {
            crate::VenoreError::LlmInvalidRequest(format!(
                "Invalid temperature in {}: {}",
                temp_key, temp_str
            ))
        })?;
        settings.temperature = Some(temp);
    }

    // Override max_tokens if specified
    let tokens_key = format!("VENORE_TASK_{}_MAX_TOKENS", task_name);
    if let Ok(tokens_str) = std::env::var(&tokens_key) {
        let tokens: u32 = tokens_str.parse().map_err(|_| {
            crate::VenoreError::LlmInvalidRequest(format!(
                "Invalid max_tokens in {}: {}",
                tokens_key, tokens_str
            ))
        })?;
        settings.max_tokens = Some(tokens);
    }

    // Override timeout_ms if specified
    let timeout_key = format!("VENORE_TASK_{}_TIMEOUT_MS", task_name);
    if let Ok(timeout_str) = std::env::var(&timeout_key) {
        let timeout: u64 = timeout_str.parse().map_err(|_| {
            crate::VenoreError::LlmInvalidRequest(format!(
                "Invalid timeout_ms in {}: {}",
                timeout_key, timeout_str
            ))
        })?;
        settings.timeout_ms = Some(timeout);
    }

    // Override streaming if specified
    let streaming_key = format!("VENORE_TASK_{}_STREAMING", task_name);
    if let Ok(streaming_str) = std::env::var(&streaming_key) {
        let streaming: bool = streaming_str.parse().map_err(|_| {
            crate::VenoreError::LlmInvalidRequest(format!(
                "Invalid streaming in {}: {}",
                streaming_key, streaming_str
            ))
        })?;
        settings.streaming = Some(streaming);
    }

    // Validate before returning
    settings.validate()?;

    Ok(Some(settings))
}

// ============================================================================
// MAIN LOADER
// ============================================================================

/// Load task settings with precedence:
/// 1. Environment variables (highest)
/// 2. SQLite database
/// 3. Defaults (lowest)
///
/// # Arguments
///
/// * `task` - Task to load settings for
/// * `store` - Config store to load from
///
/// # Returns
///
/// * `Ok(TaskSettings)` - Loaded settings
/// * `Err(VenoreError)` - If loading fails
///
/// # Examples
///
/// ```ignore
/// use venore_core::core::config::load_task_settings;
/// use venore_core::traits::LlmTask;
///
/// let settings = load_task_settings(LlmTask::Chat, &store).await?;
/// ```
pub async fn load_task_settings(
    task: LlmTask,
    store: &dyn TaskConfigStore,
) -> Result<TaskSettings> {
    // 1. Try environment variables first
    if let Some(settings) = load_from_env(task)? {
        tracing::debug!("Loaded {} settings from environment", format!("{:?}", task));
        return Ok(settings);
    }

    // 2. Try SQLite database
    match store.get_task_settings(task).await {
        Ok(settings) => {
            tracing::debug!("Loaded {} settings from database", format!("{:?}", task));
            return Ok(settings);
        }
        Err(_) => {
            // Database may not be initialized yet, fall through to defaults
            tracing::debug!("Database not available, using defaults for {:?}", task);
        }
    }

    // 3. Fall back to defaults
    tracing::debug!("Using default settings for {:?}", task);
    Ok(TaskDefaults::get(task))
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::Mutex;

    /// Mutex to serialize env var tests (they share global process state)
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn clear_chat_env_vars() {
        env::remove_var("VENORE_TASK_CHAT_PROVIDER");
        env::remove_var("VENORE_TASK_CHAT_MODEL");
        env::remove_var("VENORE_TASK_CHAT_TEMPERATURE");
        env::remove_var("VENORE_TASK_CHAT_MAX_TOKENS");
        env::remove_var("VENORE_TASK_CHAT_TIMEOUT_MS");
        env::remove_var("VENORE_TASK_CHAT_STREAMING");
    }

    #[test]
    fn test_load_from_env_none() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_chat_env_vars();

        let result = load_from_env(LlmTask::Chat).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_load_from_env_provider_only() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_chat_env_vars();

        env::set_var("VENORE_TASK_CHAT_PROVIDER", "anthropic");

        let result = load_from_env(LlmTask::Chat).unwrap();
        assert!(result.is_some());

        let settings = result.unwrap();
        assert_eq!(settings.provider, LlmProviderType::Anthropic);

        clear_chat_env_vars();
    }

    #[test]
    fn test_load_from_env_full() {
        env::set_var("VENORE_TASK_ONBOARDING_PROVIDER", "anthropic");
        env::set_var("VENORE_TASK_ONBOARDING_MODEL", "custom-model");
        env::set_var("VENORE_TASK_ONBOARDING_TEMPERATURE", "0.5");
        env::set_var("VENORE_TASK_ONBOARDING_MAX_TOKENS", "2000");
        env::set_var("VENORE_TASK_ONBOARDING_TIMEOUT_MS", "45000");
        env::set_var("VENORE_TASK_ONBOARDING_STREAMING", "true");

        let result = load_from_env(LlmTask::Onboarding).unwrap();
        assert!(result.is_some());

        let settings = result.unwrap();
        assert_eq!(settings.provider, LlmProviderType::Anthropic);
        assert_eq!(settings.model, "custom-model");
        assert_eq!(settings.temperature, Some(0.5));
        assert_eq!(settings.max_tokens, Some(2000));
        assert_eq!(settings.timeout_ms, Some(45000));
        assert_eq!(settings.streaming, Some(true));

        // Cleanup
        env::remove_var("VENORE_TASK_ONBOARDING_PROVIDER");
        env::remove_var("VENORE_TASK_ONBOARDING_MODEL");
        env::remove_var("VENORE_TASK_ONBOARDING_TEMPERATURE");
        env::remove_var("VENORE_TASK_ONBOARDING_MAX_TOKENS");
        env::remove_var("VENORE_TASK_ONBOARDING_TIMEOUT_MS");
        env::remove_var("VENORE_TASK_ONBOARDING_STREAMING");
    }

    #[test]
    fn test_load_from_env_invalid_provider() {
        env::set_var("VENORE_TASK_ANALYSIS_PROVIDER", "invalid-provider");

        let result = load_from_env(LlmTask::Analysis);
        assert!(result.is_err());

        // Cleanup
        env::remove_var("VENORE_TASK_ANALYSIS_PROVIDER");
    }

    #[test]
    fn test_load_from_env_invalid_temperature() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_chat_env_vars();

        env::set_var("VENORE_TASK_CHAT_PROVIDER", "anthropic");
        env::set_var("VENORE_TASK_CHAT_TEMPERATURE", "not-a-number");

        let result = load_from_env(LlmTask::Chat);
        assert!(result.is_err());

        clear_chat_env_vars();
    }
}
