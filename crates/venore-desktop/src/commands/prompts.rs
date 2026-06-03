//! Prompt registry Tauri commands
//!
//! CRUD operations for the centralized LLM prompt registry.

use std::sync::Arc;

use venore_core::error::VenoreError;
use venore_core::prompts::PromptRepository;

use crate::state::LazyAppState;
use crate::utils::{IntoStateCommandResult, StateCommandResult};

use super::dto::prompts::*;

// =============================================================================
// Helpers
// =============================================================================

fn get_prompt_repo(lazy: &LazyAppState) -> Result<Arc<PromptRepository>, VenoreError> {
    let guard = lazy.get();
    match guard.as_ref() {
        Some(state) => Ok(Arc::clone(&state.prompt_repository)),
        None => Err(VenoreError::NotFound("Backend not initialized".into())),
    }
}

// =============================================================================
// Commands
// =============================================================================

#[tauri::command]
pub async fn list_prompts(
    lazy_state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<Vec<PromptDto>> {
    let repo = get_prompt_repo(&lazy_state);
    let result: Result<Vec<PromptDto>, VenoreError> = async {
        let repo = repo?;
        let prompts = repo.list_prompts().await?;
        Ok(prompts.into_iter().map(|p| p.into()).collect())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn get_prompt(
    lazy_state: tauri::State<'_, LazyAppState>,
    id: String,
) -> StateCommandResult<PromptDto> {
    let repo = get_prompt_repo(&lazy_state);
    let result: Result<PromptDto, VenoreError> = async {
        let repo = repo?;
        let prompt = repo.get_prompt(&id).await?;
        Ok(prompt.into())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn update_prompt(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: UpdatePromptRequest,
) -> StateCommandResult<PromptDto> {
    let repo = get_prompt_repo(&lazy_state);
    let result: Result<PromptDto, VenoreError> = async {
        let repo = repo?;
        let prompt = repo.update_prompt(&request.id, &request.content).await?;
        Ok(prompt.into())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn reset_prompt(
    lazy_state: tauri::State<'_, LazyAppState>,
    id: String,
) -> StateCommandResult<PromptDto> {
    let repo = get_prompt_repo(&lazy_state);
    let result: Result<PromptDto, VenoreError> = async {
        let repo = repo?;
        let prompt = repo.reset_prompt(&id).await?;
        Ok(prompt.into())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn list_prompt_versions(
    lazy_state: tauri::State<'_, LazyAppState>,
    prompt_id: String,
) -> StateCommandResult<Vec<PromptVersionDto>> {
    let repo = get_prompt_repo(&lazy_state);
    let result: Result<Vec<PromptVersionDto>, VenoreError> = async {
        let repo = repo?;
        let versions = repo.list_versions(&prompt_id).await?;
        Ok(versions.into_iter().map(|v| v.into()).collect())
    }
    .await;
    result.into_state()
}

// =============================================================================
// Task-based commands (PromptsView redesign)
// =============================================================================

#[tauri::command]
pub async fn list_prompt_tasks(
    lazy_state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<Vec<String>> {
    let repo = get_prompt_repo(&lazy_state);
    let result: Result<Vec<String>, VenoreError> = async {
        let repo = repo?;
        repo.list_tasks().await
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn get_task_prompts(
    lazy_state: tauri::State<'_, LazyAppState>,
    category: String,
) -> StateCommandResult<Vec<PromptDto>> {
    let repo = get_prompt_repo(&lazy_state);
    let result: Result<Vec<PromptDto>, VenoreError> = async {
        let repo = repo?;
        let prompts = repo.get_prompts_for_task(&category).await?;
        Ok(prompts.into_iter().map(|p| p.into()).collect())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn save_task_prompt(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: SaveTaskPromptRequest,
) -> StateCommandResult<PromptDto> {
    let repo = get_prompt_repo(&lazy_state);
    let result: Result<PromptDto, VenoreError> = async {
        let repo = repo?;
        let name = format!(
            "{} — {} override",
            capitalize_first(&request.category),
            capitalize_first(&request.provider),
        );
        let prompt = repo
            .upsert_task_prompt(
                &request.category,
                &request.provider,
                &name,
                &request.content,
                "[]",
            )
            .await?;
        Ok(prompt.into())
    }
    .await;
    result.into_state()
}

fn capitalize_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

// =============================================================================
// Chat fragment commands (Phase 5 — system prompt blocks as templates)
// =============================================================================

#[tauri::command]
pub async fn list_chat_fragments(
    lazy_state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<Vec<PromptDto>> {
    let repo = get_prompt_repo(&lazy_state);
    let result: Result<Vec<PromptDto>, VenoreError> = async {
        let repo = repo?;
        let prompts = repo.list_chat_fragments().await?;
        Ok(prompts.into_iter().map(|p| p.into()).collect())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn set_prompt_enabled(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: SetPromptEnabledRequest,
) -> StateCommandResult<PromptDto> {
    let repo = get_prompt_repo(&lazy_state);
    let result: Result<PromptDto, VenoreError> = async {
        let repo = repo?;
        let prompt = repo.set_prompt_enabled(&request.id, request.enabled).await?;
        Ok(prompt.into())
    }
    .await;
    result.into_state()
}
