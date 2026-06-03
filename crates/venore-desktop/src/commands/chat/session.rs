//! Chat session CRUD commands.

use venore_core::error::VenoreError;
use venore_core::traits::LlmTask;

use crate::state::LazyAppState;
use crate::utils::{CommandResult, IntoStateCommandResult, StateCommandResult};

use super::dto::{
    ChatContextOptionDto, ChatMessageDto, ChatSessionDto, CreateChatSessionRequest,
    SessionActivityDto, SnapshotDto, TokenSummaryDto, ToolCallRecordDto,
};
use super::helpers::get_chat_repo;

/// Get or create a chat session linked to a dev session.
#[tauri::command]
pub async fn get_or_create_dev_session_chat(
    lazy_state: tauri::State<'_, LazyAppState>,
    dev_session_id: String,
    session_name: String,
    project_id: Option<String>,
) -> StateCommandResult<ChatSessionDto> {
    let repo = get_chat_repo(&lazy_state);
    let result: Result<ChatSessionDto, VenoreError> = async {
        let repo = repo?;
        let session = repo
            .find_or_create_for_dev_session(
                &dev_session_id,
                &session_name,
                project_id.as_deref(),
            )
            .await?;
        Ok(session.into())
    }
    .await;
    result.into_state()
}

/// Create a new chat session.
#[tauri::command]
pub async fn create_chat_session(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: CreateChatSessionRequest,
) -> StateCommandResult<ChatSessionDto> {
    let repo = get_chat_repo(&lazy_state);
    let result: Result<ChatSessionDto, VenoreError> = async {
        let repo = repo?;
        let name = request.name.unwrap_or_else(|| "New Chat".to_string());
        let session = repo
            .create_session(&name, request.project_id.as_deref())
            .await?;
        Ok(session.into())
    }
    .await;
    result.into_state()
}

/// List chat sessions.
#[tauri::command]
pub async fn list_chat_sessions(
    lazy_state: tauri::State<'_, LazyAppState>,
    project_id: Option<String>,
) -> StateCommandResult<Vec<ChatSessionDto>> {
    let repo = get_chat_repo(&lazy_state);
    let result: Result<Vec<ChatSessionDto>, VenoreError> = async {
        let repo = repo?;
        let sessions = repo.list_sessions(project_id.as_deref()).await?;
        Ok(sessions.into_iter().map(|s| s.into()).collect())
    }
    .await;
    result.into_state()
}

/// Delete a chat session.
#[tauri::command]
pub async fn delete_chat_session(
    lazy_state: tauri::State<'_, LazyAppState>,
    session_id: String,
) -> StateCommandResult<()> {
    let repo = get_chat_repo(&lazy_state);
    let result: Result<(), VenoreError> = async {
        let repo = repo?;
        repo.delete_session(&session_id).await?;
        Ok(())
    }
    .await;
    result.into_state()
}

/// Rename a chat session.
#[tauri::command]
pub async fn rename_chat_session(
    lazy_state: tauri::State<'_, LazyAppState>,
    session_id: String,
    name: String,
) -> StateCommandResult<()> {
    let repo = get_chat_repo(&lazy_state);
    let result: Result<(), VenoreError> = async {
        let repo = repo?;
        repo.rename_session(&session_id, &name).await?;
        Ok(())
    }
    .await;
    result.into_state()
}

/// Get messages for a chat session.
#[tauri::command]
pub async fn get_chat_messages(
    lazy_state: tauri::State<'_, LazyAppState>,
    session_id: String,
    limit: Option<u32>,
) -> StateCommandResult<Vec<ChatMessageDto>> {
    let repo = get_chat_repo(&lazy_state);
    let result: Result<Vec<ChatMessageDto>, VenoreError> = async {
        let repo = repo?;
        let messages = repo.get_messages(&session_id, limit).await?;
        Ok(messages.into_iter().map(|m| m.into()).collect())
    }
    .await;
    result.into_state()
}

/// Get all snapshots for a chat session.
#[tauri::command]
pub async fn get_chat_snapshots(
    lazy_state: tauri::State<'_, LazyAppState>,
    session_id: String,
) -> StateCommandResult<Vec<SnapshotDto>> {
    let repo = get_chat_repo(&lazy_state);
    let result: Result<Vec<SnapshotDto>, VenoreError> = async {
        let repo = repo?;
        let snapshots = repo.get_snapshots(&session_id).await?;
        Ok(snapshots
            .into_iter()
            .map(|s| {
                let file_path = s.arguments.as_ref().and_then(|args_str| {
                    serde_json::from_str::<serde_json::Value>(args_str).ok()
                        .and_then(|v| v["file_path"].as_str().map(|s| s.to_string()))
                });
                SnapshotDto {
                    tool_call_id: s.tool_call_id,
                    commit_hash: s.commit_hash,
                    created_at: s.created_at,
                    tool_name: s.tool_name,
                    file_path,
                }
            })
            .collect())
    }
    .await;
    result.into_state()
}

/// Get available context options (modules with .context.md) for a project.
#[tauri::command]
pub async fn get_chat_context_options(
    project_path: String,
) -> CommandResult<Vec<ChatContextOptionDto>> {
    let result: Result<Vec<ChatContextOptionDto>, VenoreError> = (|| {
        let modules =
            venore_core::chat::context::scan_available_modules(std::path::Path::new(&project_path))?;
        Ok(modules
            .into_iter()
            .map(|m| ChatContextOptionDto {
                name: m.name,
                path: m.path,
                has_context: m.has_context,
            })
            .collect())
    })();
    result.into()
}

/// Get session activity data: tool calls, snapshots, and token usage.
#[tauri::command]
pub async fn get_session_activity(
    lazy_state: tauri::State<'_, LazyAppState>,
    session_id: String,
) -> StateCommandResult<SessionActivityDto> {
    let repo = get_chat_repo(&lazy_state);
    let result: Result<SessionActivityDto, VenoreError> = async {
        let repo = repo?;
        let (tool_calls, snapshots, token_summary) = tokio::join!(
            repo.get_tool_calls(&session_id),
            repo.get_snapshots(&session_id),
            repo.get_session_token_summary(&session_id),
        );

        let tool_calls = tool_calls?;
        let snapshots = snapshots?;
        let token_summary = token_summary?;

        Ok(SessionActivityDto {
            tool_calls: tool_calls.into_iter().map(|tc| {
                let args: serde_json::Value = serde_json::from_str(&tc.arguments)
                    .unwrap_or(serde_json::Value::Object(Default::default()));
                ToolCallRecordDto {
                    id: tc.id,
                    tool_name: tc.tool_name,
                    arguments: args,
                    success: tc.success,
                    output: tc.output,
                    commit_hash: tc.commit_hash,
                    created_at: tc.created_at,
                }
            }).collect(),
            snapshots: snapshots.into_iter().map(|s| {
                let file_path = s.arguments.as_ref().and_then(|args_str| {
                    serde_json::from_str::<serde_json::Value>(args_str).ok()
                        .and_then(|v| v["file_path"].as_str().map(|fp| fp.to_string()))
                });
                SnapshotDto {
                    tool_call_id: s.tool_call_id,
                    commit_hash: s.commit_hash,
                    created_at: s.created_at,
                    tool_name: s.tool_name,
                    file_path,
                }
            }).collect(),
            token_summary: TokenSummaryDto {
                total_prompt_tokens: token_summary.total_prompt_tokens,
                total_completion_tokens: token_summary.total_completion_tokens,
                message_count: token_summary.message_count,
            },
        })
    }
    .await;
    result.into_state()
}

/// Generate a short LLM-based title for a chat session.
#[tauri::command]
pub async fn generate_chat_title(
    lazy_state: tauri::State<'_, LazyAppState>,
    user_message: String,
) -> StateCommandResult<String> {
    use crate::commands::llm::get_services;
    use venore_core::traits::TaskConfigStore;

    let services = get_services(&lazy_state);
    let result: Result<String, VenoreError> = async {
        let (config_store, llm_gateway) = services?;
        let task_settings = config_store.get_task_settings(LlmTask::Chat).await?;

        venore_core::chat::generate_session_title(
            &llm_gateway,
            &user_message,
            task_settings.provider,
            &task_settings.model,
        )
        .await
    }
    .await;
    result.into_state()
}
