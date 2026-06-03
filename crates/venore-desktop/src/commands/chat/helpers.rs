//! Chat helpers — shared utility functions for chat commands.

use std::sync::Arc;

use tauri::AppHandle;
use tauri::Emitter;

use venore_core::error::VenoreError;
use venore_core::terminal::TerminalSessionManager;

use venore_core::tools::names as N;

use crate::state::{get_state_field, LazyAppState};
use crate::commands::dto::terminal::{TerminalAiSpawnedPayload, TerminalSessionSpawnedPayload};
use crate::commands::terminal::start_terminal_read_loop;

use super::state::SESSION_APPROVALS;

// ── Repository accessors ─────────────────────────────────────────────

pub(super) fn get_chat_repo(lazy: &LazyAppState) -> Result<Arc<venore_core::chat::ChatRepository>, VenoreError> {
    get_state_field!(lazy, chat_repository)
}

pub(super) fn get_prompt_repo(lazy: &LazyAppState) -> Result<Arc<venore_core::prompts::PromptRepository>, VenoreError> {
    get_state_field!(lazy, prompt_repository)
}

pub(super) fn get_rag_repo(lazy: &LazyAppState) -> Result<Arc<venore_core::rag::RagRepository>, VenoreError> {
    get_state_field!(lazy, rag_repository)
}

pub(crate) fn get_logbook_repo(lazy: &LazyAppState) -> Result<Arc<venore_core::rag::LogbookRepository>, VenoreError> {
    get_state_field!(lazy, logbook_repository)
}

/// Resolve the configured embedding provider + API key for hybrid search.
///
/// Returns `(provider, api_key)` only when BOTH a provider is configured for
/// the Embeddings task AND its API key is present. Any missing piece yields
/// `(None, None)` so callers degrade gracefully to FTS-only search — no error.
pub(crate) async fn resolve_embedding_provider(
    config_store: &Arc<venore_core::infrastructure::config::DefaultConfigStore>,
) -> (Option<Arc<dyn venore_core::traits::EmbeddingProvider>>, Option<String>) {
    use venore_core::traits::{ApiKeyStore, TaskConfigStore, LlmTask};

    let settings = match config_store.get_task_settings(LlmTask::Embeddings).await {
        Ok(s) => s,
        Err(_) => return (None, None),
    };
    let provider_name = settings.provider.as_str();
    let provider = match venore_core::rag::create_embedding_provider(provider_name, Some(&settings.model)) {
        Ok(p) => Arc::from(p),
        Err(e) => {
            tracing::warn!("Embedding provider unavailable (FTS-only search): {}", e);
            return (None, None);
        }
    };
    let api_key = config_store.get_api_key(settings.provider).await.ok().flatten();
    if api_key.is_none() {
        // Ollama needs no key; others do. Let the search layer try and degrade.
        tracing::debug!("No embedding API key for provider '{}'", provider_name);
    }
    (Some(provider), api_key)
}

pub(super) fn get_agent_repo_for_chat(lazy: &LazyAppState) -> Option<Arc<venore_core::agents::AgentRepository>> {
    get_state_field!(lazy, agent_repository).ok()
}

// ── Tool resource extraction ─────────────────────────────────────────

/// Extract the "resource" (file path, command, URL) from tool arguments.
/// Used by the permission engine to evaluate resource-specific rules.
pub(super) fn extract_tool_resource(tool_name: &str, arguments: &serde_json::Value) -> Option<String> {
    match tool_name {
        N::EDIT_FILE | N::WRITE_FILE | N::READ_FILE | N::MULTI_EDIT_FILE => {
            arguments["file_path"].as_str().map(String::from)
        }
        N::RUN_TERMINAL_COMMAND => arguments["command"].as_str().map(String::from),
        N::WEB_FETCH => arguments["url"].as_str().map(String::from),
        N::WEB_SEARCH => arguments["query"].as_str().map(String::from),
        N::LIST_FILES | N::SEARCH_CODE | N::SEARCH_TEXT => {
            arguments["directory"].as_str().map(String::from)
        }
        _ => None,
    }
}

/// Tools that are safe to execute in parallel (read-only, no interaction).
pub(super) fn is_parallelizable(tool_name: &str) -> bool {
    N::PARALLELIZABLE_TOOLS.contains(&tool_name)
}

/// Pre-check permission action for a tool without triggering UI interaction.
///
/// Approval key priority (must match what the frontend sends to
/// `approve_tool_call`):
///   1. `dev_session_id` — when the chat is inside a code-mode dev session
///   2. `chat_session_id` — Knowledge mode and any plain chat without a dev session
///   3. `stream_id` — last-resort per-message key (approvals here die with the stream)
pub(super) fn check_permission_action(
    tool_name: &str,
    arguments: &serde_json::Value,
    stream_id: &str,
    dev_session_id: Option<&str>,
    chat_session_id: Option<&str>,
) -> venore_core::permissions::PermissionAction {
    let resource = extract_tool_resource(tool_name, arguments);
    let rules = venore_core::permissions::default_rules();
    let approval_key = dev_session_id
        .or(chat_session_id)
        .unwrap_or(stream_id);
    let session_approved = SESSION_APPROVALS
        .lock()
        .ok()
        .and_then(|s| s.get(approval_key).cloned())
        .unwrap_or_default();
    venore_core::permissions::evaluate(tool_name, resource.as_deref(), &rules, &session_approved)
}

// ── Terminal resolution ──────────────────────────────────────────────

/// Resolve an existing terminal or spawn a new one.
/// Session-aware: if `dev_session_id` is provided, uses the session's dedicated terminal.
pub(super) fn resolve_or_spawn_terminal(
    app: &AppHandle,
    project_path: Option<&str>,
    dev_session_id: Option<&str>,
    session_label: Option<&str>,
) -> Result<String, VenoreError> {
    let lock_err = || VenoreError::TerminalError(
        "Terminal is unavailable due to an internal error. Try restarting the app.".into(),
    );

    // ── Session-bound path ──────────────────────────────────────────
    if let Some(sid) = dev_session_id {
        let existing = {
            let manager = TerminalSessionManager::global();
            let guard = manager.lock().map_err(|_| lock_err())?;
            guard.get_session_terminal(sid).map(|s| s.to_string())
        };
        if let Some(tid) = existing {
            tracing::debug!(terminal_id = %tid, dev_session_id = %sid, "reusing session terminal");
            return Ok(tid);
        }

        let cwd = project_path
            .map(|p| p.to_string())
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| ".".to_string())
            });

        let label_str = session_label
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                std::path::Path::new(&cwd)
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_else(|| "terminal".to_string())
            });

        let (terminal_id, reader) = {
            let manager = TerminalSessionManager::global();
            let mut guard = manager.lock().map_err(|_| lock_err())?;
            let (tid, reader) = guard.spawn(&cwd, 80, 24, Some(&label_str))?;
            guard.bind_session(sid, &tid);
            (tid, reader)
        };

        start_terminal_read_loop(app.clone(), terminal_id.clone(), reader);

        let _ = app.emit(
            "terminal:session-spawned",
            TerminalSessionSpawnedPayload {
                terminal_id: terminal_id.clone(),
                dev_session_id: sid.to_string(),
                label: label_str,
            },
        );

        tracing::info!(terminal_id = %terminal_id, dev_session_id = %sid, "spawned session-bound terminal");
        return Ok(terminal_id);
    }

    // ── No session — only use unbound terminals ────
    let existing = {
        let manager = TerminalSessionManager::global();
        let guard = manager.lock().map_err(|_| lock_err())?;
        guard.list_unbound()
    };

    if let Some(first) = existing.into_iter().next() {
        tracing::debug!(terminal_id = %first, "reusing unbound terminal for AI");
        return Ok(first);
    }

    let cwd = project_path
        .map(|p| p.to_string())
        .unwrap_or_else(|| {
            dirs::home_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| ".".to_string())
        });

    let label: Option<String> = std::path::Path::new(&cwd)
        .file_name()
        .map(|f| f.to_string_lossy().to_string());

    let (terminal_id, reader) = {
        let manager = TerminalSessionManager::global();
        let mut guard = manager.lock().map_err(|_| lock_err())?;
        guard.spawn(&cwd, 80, 24, label.as_deref())?
    };

    start_terminal_read_loop(app.clone(), terminal_id.clone(), reader);

    let _ = app.emit(
        "terminal:ai-spawned",
        TerminalAiSpawnedPayload {
            terminal_id: terminal_id.clone(),
        },
    );

    tracing::info!(terminal_id = %terminal_id, "AI auto-spawned unbound terminal");
    Ok(terminal_id)
}

// ── Dev session helpers ──────────────────────────────────────────────

/// Resolve the worktree path for a dev session.
pub(super) async fn resolve_worktree_path(
    lazy: &LazyAppState,
    dev_session_id: &str,
) -> Option<String> {
    let session_repo = {
        let guard = lazy.get();
        guard.as_ref().map(|s| Arc::clone(&s.session_repository))
    }?;

    let session = session_repo.get(dev_session_id).await.ok()??;
    let wt = &session.worktree_path;
    if wt.is_empty() {
        None
    } else {
        Some(wt.clone())
    }
}

