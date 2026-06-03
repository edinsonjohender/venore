//! LLM-related Tauri commands
//!
//! Exposes LLM functionality to the frontend.

use serde::{Deserialize, Serialize};
use crate::state::LazyAppState;
use crate::utils::{CommandResult, StateCommandResult, IntoStateCommandResult};
use venore_core::error::VenoreError;
use venore_core::llm::prelude::*;
use venore_core::traits::{ApiKeyStore, TaskConfigStore};
use std::sync::Arc;
use venore_core::llm::LlmGateway;
use venore_core::infrastructure::config::DefaultConfigStore;

// Helper to extract config_store and llm_gateway from LazyAppState
// Returns cloned Arcs so they can be used in async contexts
pub fn get_services(lazy: &LazyAppState) -> Result<(Arc<DefaultConfigStore>, Arc<LlmGateway>), VenoreError> {
    let guard = lazy.get();
    match guard.as_ref() {
        Some(state) => Ok((Arc::clone(&state.config_store), Arc::clone(&state.llm_gateway))),
        None => Err(VenoreError::NotFound("Backend not initialized".into())),
    }
}

// ============================================================================
// REQUEST/RESPONSE TYPES
// ============================================================================

// --- API Key Management ---

#[derive(Deserialize)]
pub struct SetApiKeyRequest {
    pub provider: String,
    pub api_key: String,
}

#[derive(Serialize)]
pub struct ApiKeyStatusResponse {
    pub has_key: bool,
}

#[derive(Serialize)]
pub struct ConfiguredProvidersResponse {
    pub providers: Vec<String>,
}

// --- Task Configuration ---

#[derive(Deserialize)]
pub struct SetTaskSettingsRequest {
    pub task: String,
    pub provider: String,
    pub model: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub timeout_ms: Option<u64>,
    pub streaming: Option<bool>,
}

#[derive(Serialize)]
pub struct TaskSettingsResponse {
    pub provider: String,
    pub model: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub timeout_ms: Option<u64>,
    pub streaming: Option<bool>,
}

#[derive(Serialize)]
pub struct AllTaskSettingsResponse {
    pub onboarding: TaskSettingsResponse,
    pub chat: TaskSettingsResponse,
    pub analysis: TaskSettingsResponse,
    pub embeddings: TaskSettingsResponse,
}

// --- Provider Information ---

#[derive(Serialize)]
pub struct AvailableModelsResponse {
    pub provider: String,
    pub models: Vec<String>,
    pub default_model: String,
}

#[derive(Deserialize)]
pub struct TestConnectionRequest {
    pub provider: String,
    pub model: Option<String>,
}

#[derive(Serialize)]
pub struct TestConnectionResponse {
    pub success: bool,
    pub latency_ms: u64,
    pub model: String,
    pub error: Option<String>,
}

// --- LLM Generation ---

#[derive(Deserialize)]
pub struct GenerateTextRequest {
    pub task: String,
    pub messages: Vec<GenerateMessageRequest>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub provider: Option<String>,
    pub model: Option<String>,
}

#[derive(Deserialize)]
pub struct GenerateMessageRequest {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
pub struct GenerateTextResponse {
    pub content: String,
    pub provider: String,
    pub model: String,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

// ============================================================================
// API KEY MANAGEMENT COMMANDS (5)
// ============================================================================

/// Store API key for a provider
#[tauri::command]
pub async fn set_api_key(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: SetApiKeyRequest,
) -> StateCommandResult<()> {
    let services = get_services(&lazy_state);
    let result: Result<(), VenoreError> = async {
        let (config_store, _) = services?;
        let provider: LlmProviderType = request.provider.parse()?;
        config_store.store_api_key(provider, request.api_key).await?;
        Ok(())
    }.await;
    result.into_state()
}

/// Get API key for a provider (returns masked version)
#[tauri::command]
pub async fn get_api_key(
    lazy_state: tauri::State<'_, LazyAppState>,
    provider: String,
) -> StateCommandResult<Option<String>> {
    let services = get_services(&lazy_state);
    let result: Result<Option<String>, VenoreError> = async {
        let (config_store, _) = services?;
        let provider: LlmProviderType = provider.parse()?;
        let key = config_store.get_api_key(provider).await?;
        Ok(key.map(|k| {
            if k.len() > 8 {
                format!("{}...", &k[..8])
            } else {
                "***".to_string()
            }
        }))
    }.await;
    result.into_state()
}

/// Check if API key exists for a provider
#[tauri::command]
pub async fn has_api_key(
    lazy_state: tauri::State<'_, LazyAppState>,
    provider: String,
) -> StateCommandResult<ApiKeyStatusResponse> {
    let services = get_services(&lazy_state);
    let result: Result<ApiKeyStatusResponse, VenoreError> = async {
        let (config_store, _) = services?;
        let provider: LlmProviderType = provider.parse()?;
        let has_key = config_store.has_api_key(provider).await?;
        Ok(ApiKeyStatusResponse { has_key })
    }.await;
    result.into_state()
}

/// Remove API key for a provider
#[tauri::command]
pub async fn remove_api_key(
    lazy_state: tauri::State<'_, LazyAppState>,
    provider: String,
) -> StateCommandResult<()> {
    let services = get_services(&lazy_state);
    let result: Result<(), VenoreError> = async {
        let (config_store, _) = services?;
        let provider: LlmProviderType = provider.parse()?;
        config_store.remove_api_key(provider).await?;
        Ok(())
    }.await;
    result.into_state()
}

/// Get list of providers that have API keys configured
#[tauri::command]
pub async fn get_configured_providers(
    lazy_state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<ConfiguredProvidersResponse> {
    let services = get_services(&lazy_state);
    let result: Result<ConfiguredProvidersResponse, VenoreError> = async {
        let (config_store, _) = services?;
        let providers = config_store.list_configured_providers().await?;
        let provider_names: Vec<String> = providers.iter().map(|p| p.as_str().to_string()).collect();
        Ok(ConfiguredProvidersResponse { providers: provider_names })
    }.await;
    result.into_state()
}

// ============================================================================
// TASK CONFIGURATION COMMANDS (4)
// ============================================================================

/// Get settings for a specific task
#[tauri::command]
pub async fn get_task_settings(
    lazy_state: tauri::State<'_, LazyAppState>,
    task: String,
) -> StateCommandResult<TaskSettingsResponse> {
    let services = get_services(&lazy_state);
    let result: Result<TaskSettingsResponse, VenoreError> = async {
        let (config_store, _) = services?;
        let task: LlmTask = parse_task(&task)?;
        let settings = config_store.get_task_settings(task).await?;
        Ok(TaskSettingsResponse {
            provider: settings.provider.as_str().to_string(),
            model: settings.model,
            temperature: settings.temperature,
            max_tokens: settings.max_tokens,
            timeout_ms: settings.timeout_ms,
            streaming: settings.streaming,
        })
    }.await;
    result.into_state()
}

/// Set settings for a specific task
#[tauri::command]
pub async fn set_task_settings(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: SetTaskSettingsRequest,
) -> StateCommandResult<()> {
    let services = get_services(&lazy_state);
    let result: Result<(), VenoreError> = async {
        let (config_store, _) = services?;

        use venore_core::core::config::TaskSettings;

        let task: LlmTask = parse_task(&request.task)?;
        let provider: LlmProviderType = request.provider.parse()?;

        let settings = TaskSettings {
            provider,
            model: request.model,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            timeout_ms: request.timeout_ms,
            streaming: request.streaming,
        };

        config_store.set_task_settings(task, settings).await?;
        Ok(())
    }.await;
    result.into_state()
}

/// Get all task settings
#[tauri::command]
pub async fn get_all_task_settings(
    lazy_state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<AllTaskSettingsResponse> {
    let services = get_services(&lazy_state);
    let result: Result<AllTaskSettingsResponse, VenoreError> = async {
        let (config_store, _) = services?;
        let onboarding = config_store.get_task_settings(LlmTask::Onboarding).await?;
        let chat = config_store.get_task_settings(LlmTask::Chat).await?;
        let analysis = config_store.get_task_settings(LlmTask::Analysis).await?;
        let embeddings = config_store.get_task_settings(LlmTask::Embeddings).await?;
        Ok(AllTaskSettingsResponse {
            onboarding: to_response(&onboarding),
            chat: to_response(&chat),
            analysis: to_response(&analysis),
            embeddings: to_response(&embeddings),
        })
    }.await;
    result.into_state()
}

/// Reset task settings to defaults
#[tauri::command]
pub async fn reset_task_settings(
    lazy_state: tauri::State<'_, LazyAppState>,
    task: String,
) -> StateCommandResult<()> {
    let services = get_services(&lazy_state);
    let result: Result<(), VenoreError> = async {
        let (config_store, _) = services?;
        let task: LlmTask = parse_task(&task)?;
        config_store.reset_task_settings(task).await?;
        Ok(())
    }.await;
    result.into_state()
}

// ============================================================================
// PROVIDER INFORMATION COMMANDS (3)
// ============================================================================

/// List all available providers
#[tauri::command]
pub async fn list_providers(
    _state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<Vec<String>> {
    Ok(CommandResult::ok(vec![
        "anthropic".to_string(),
        "openai".to_string(),
        "gemini".to_string(),
        "ollama".to_string(),
    ]))
}

/// Get available models for a provider
#[tauri::command]
pub async fn get_available_models(
    _state: tauri::State<'_, LazyAppState>,
    provider: String,
) -> StateCommandResult<AvailableModelsResponse> {
    let result: Result<AvailableModelsResponse, VenoreError> = (|| {
        use venore_core::llm::registry;

        let provider_type: LlmProviderType = provider.parse()?;
        let models = registry::get_provider_models(provider_type);
        let default_model = registry::get_default_model(provider_type);

        Ok(AvailableModelsResponse { provider, models, default_model })
    })();
    result.into_state()
}

/// Test connection to a provider
#[tauri::command]
pub async fn test_connection(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: TestConnectionRequest,
) -> StateCommandResult<TestConnectionResponse> {
    let services = get_services(&lazy_state);
    let result: Result<TestConnectionResponse, VenoreError> = async {
        let (_, llm_gateway) = services?;
        let provider: LlmProviderType = request.provider.parse()?;
        let result = llm_gateway.test_connection(provider, request.model).await?;
        Ok(TestConnectionResponse {
            success: result.success,
            latency_ms: result.latency_ms,
            model: result.model,
            error: result.error,
        })
    }.await;
    result.into_state()
}

/// Get installed Ollama models
#[tauri::command]
pub async fn get_ollama_models(
    _state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<Vec<String>> {
    let result: Result<Vec<String>, VenoreError> = async {
        use venore_core::llm::providers::ollama::OllamaProvider;
        let provider = OllamaProvider::new();
        provider.list_models().await
    }.await;
    result.into_state()
}

// ============================================================================
// LLM GENERATION COMMANDS (1)
// ============================================================================

/// Generate text using LLM
#[tauri::command]
pub async fn generate_text(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: GenerateTextRequest,
) -> StateCommandResult<GenerateTextResponse> {
    let services = get_services(&lazy_state);
    let result: Result<GenerateTextResponse, VenoreError> = async {
        let (_, llm_gateway) = services?;

        let task: LlmTask = parse_task(&request.task)?;

        let messages: Vec<LlmMessage> = request.messages
            .iter()
            .map(|m| {
                let role = match m.role.as_str() {
                    "system" => MessageRole::System,
                    "user" => MessageRole::User,
                    "assistant" => MessageRole::Assistant,
                    _ => MessageRole::User,
                };
                LlmMessage { role, content: m.content.clone(), tool_call_id: None, tool_calls: None, content_parts: None }
            })
            .collect();

        let llm_request = LlmRequest {
            model: request.model.clone().unwrap_or_else(|| "claude-sonnet-4-5".into()),
            messages,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            tools: None,
            json_schema: None,
            timeout_secs: Some(120),
            web_search: false,
        };

        let mut options = GatewayOptions::for_task(task);

        if let Some(provider_str) = request.provider {
            let provider: LlmProviderType = provider_str.parse()?;
            options = options.with_provider(provider);
        }

        if let Some(model) = request.model {
            options = options.with_model(model);
        }

        if let Some(temp) = request.temperature {
            options = options.with_temperature(temp);
        }

        if let Some(tokens) = request.max_tokens {
            options = options.with_max_tokens(tokens);
        }

        let response = llm_gateway.complete(llm_request, options).await?;

        Ok(GenerateTextResponse {
            content: response.content,
            provider: response.provider.as_str().to_string(),
            model: response.model,
            prompt_tokens: response.usage.as_ref().map(|u| u.prompt_tokens),
            completion_tokens: response.usage.as_ref().map(|u| u.completion_tokens),
            total_tokens: response.usage.as_ref().map(|u| u.total_tokens),
        })
    }.await;
    result.into_state()
}

// ============================================================================
// BOOT DATA COMMAND (preload all AI config in a single call)
// ============================================================================

#[derive(Serialize)]
pub struct AIBootDataResponse {
    pub configured_providers: Vec<String>,
    pub ollama_available: bool,
    pub task_settings: AllTaskSettingsResponse,
    pub available_models: std::collections::HashMap<String, Vec<String>>,
}

/// Preload all AI configuration data in a single command.
/// Called once during boot to avoid 6+ sequential API calls.
#[tauri::command]
pub async fn get_ai_boot_data(
    lazy_state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<AIBootDataResponse> {
    let services = get_services(&lazy_state);
    let result: Result<AIBootDataResponse, VenoreError> = async {
        let (config_store, llm_gateway) = services?;

        // Run independent tasks concurrently
        let (providers_result, ollama_result, task_settings_result) = tokio::join!(
            config_store.list_configured_providers(),
            llm_gateway.test_connection(LlmProviderType::Ollama, None),
            async {
                let onboarding = config_store.get_task_settings(LlmTask::Onboarding).await?;
                let chat = config_store.get_task_settings(LlmTask::Chat).await?;
                let analysis = config_store.get_task_settings(LlmTask::Analysis).await?;
                let embeddings = config_store.get_task_settings(LlmTask::Embeddings).await?;
                Ok::<_, VenoreError>((onboarding, chat, analysis, embeddings))
            }
        );

        let configured_providers = providers_result?;
        let provider_names: Vec<String> = configured_providers.iter().map(|p| p.as_str().to_string()).collect();

        let ollama_available = ollama_result.map(|r| r.success).unwrap_or(false);

        let (onboarding, chat, analysis, embeddings) = task_settings_result?;
        let task_settings = AllTaskSettingsResponse {
            onboarding: to_response(&onboarding),
            chat: to_response(&chat),
            analysis: to_response(&analysis),
            embeddings: to_response(&embeddings),
        };

        // Build available models from registry (sync, no network needed)
        let mut available_models = std::collections::HashMap::new();
        for provider_type in &[LlmProviderType::Anthropic, LlmProviderType::OpenAI, LlmProviderType::Gemini, LlmProviderType::Ollama] {
            let models = venore_core::llm::registry::get_provider_models(*provider_type);
            available_models.insert(provider_type.as_str().to_string(), models);
        }

        Ok(AIBootDataResponse {
            configured_providers: provider_names,
            ollama_available,
            task_settings,
            available_models,
        })
    }.await;
    result.into_state()
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

fn parse_task(task: &str) -> Result<LlmTask, VenoreError> {
    match task.to_lowercase().as_str() {
        "onboarding" => Ok(LlmTask::Onboarding),
        "chat" => Ok(LlmTask::Chat),
        "analysis" => Ok(LlmTask::Analysis),
        "embeddings" => Ok(LlmTask::Embeddings),
        _ => Err(VenoreError::InvalidParams(format!("Invalid task: {}", task))),
    }
}

fn to_response(settings: &venore_core::core::config::TaskSettings) -> TaskSettingsResponse {
    TaskSettingsResponse {
        provider: settings.provider.as_str().to_string(),
        model: settings.model.clone(),
        temperature: settings.temperature,
        max_tokens: settings.max_tokens,
        timeout_ms: settings.timeout_ms,
        streaming: settings.streaming,
    }
}
