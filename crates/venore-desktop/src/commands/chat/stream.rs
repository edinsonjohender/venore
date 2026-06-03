//! Chat streaming — send_chat_message orchestrator + context building.

use std::sync::Arc;

use tauri::{AppHandle, Emitter};

use venore_core::chat::{ChatContextDeps, ContextModule};
use venore_core::chat::connection_resolver::ConnectionTarget;
use crate::ai_connections::AiConnectionTarget;
use venore_core::error::VenoreError;
use venore_core::llm::prelude::*;
use venore_core::tools;

use crate::state::LazyAppState;
use crate::utils::{IntoStateCommandResult, StateCommandResult};

use super::agentic_loop::{self, AgenticLoopCtx};
use super::dto::{SendChatMessageRequest, SendChatMessageResponse};
use super::helpers::{
    get_agent_repo_for_chat, get_chat_repo, get_prompt_repo,
    get_rag_repo, resolve_worktree_path,
};
use super::state::ACTIVE_STREAMS;

use crate::commands::llm::get_services;

/// Send a chat message and receive streaming response via Tauri events.
/// Supports an agentic tool loop: if the AI calls tools, they are executed
/// and the loop continues until no more tool calls are made (max 5 iterations).
#[tauri::command]
pub async fn send_chat_message(
    app: AppHandle,
    lazy_state: tauri::State<'_, LazyAppState>,
    request: SendChatMessageRequest,
) -> StateCommandResult<SendChatMessageResponse> {
    let services = get_services(&lazy_state);
    let chat_repo = get_chat_repo(&lazy_state);

    let result: Result<SendChatMessageResponse, VenoreError> = async {
        let (config_store, llm_gateway) = services?;
        let chat_repo = chat_repo.ok();

        let stream_id = request.stream_id.clone();
        let session_id = request.session_id.clone();
        let messages = request.messages.clone();

        // Append the new user input to the chat debug log. messages always
        // contains the full conversation; the last role=user entry is the
        // turn the user just submitted.
        if let Some(last_user) = messages.iter().rev().find(|m| m.role == "user") {
            venore_core::chat::log_chat_event(venore_core::chat::ChatDebugEvent::UserMessage {
                session_id: session_id.clone().unwrap_or_default(),
                content: last_user.content.clone(),
                ts: venore_core::chat::chat_event_now(),
            });
        }

        // Read user's task settings
        use venore_core::traits::TaskConfigStore;
        let task_settings = config_store.get_task_settings(LlmTask::Chat).await?;
        let configured_provider = task_settings.provider;
        let configured_model = task_settings.model.clone();

        tracing::info!(
            provider = configured_provider.as_str(),
            model = %configured_model,
            attachments = request.attachments.as_ref().map(|a| a.len()).unwrap_or(0),
            "Chat request received"
        );

        // Load main agent tools from DB (fallback to hardcoded)
        // If knowledge_feature_id is present, use knowledge research tools instead
        let agent_repo_for_tools = get_agent_repo_for_chat(&lazy_state);
        let knowledge_repo: Option<std::sync::Arc<venore_core::knowledge::KnowledgeRepository>> = {
            let guard = lazy_state.get();
            guard.as_ref().map(|s| std::sync::Arc::clone(&s.knowledge_repository))
        };
        // Resolve the project kind so we can pick the right default mode.
        // None or unknown kind → treat as "code" (the historical default).
        // Scope the MutexGuard tightly so we don't hold it across the await.
        let project_repo = {
            let guard = lazy_state.get();
            guard
                .as_ref()
                .map(|s| std::sync::Arc::clone(&s.project_repository))
        };
        // Resolve project kind. Order:
        //   1. Look up by exact path in DB
        //   2. Fall back to a path heuristic — knowledge projects live under
        //      `<config_dir>/knowledge/<uuid>/`, so the substring "/knowledge/"
        //      (or "\knowledge\") in the path is a strong signal even when
        //      the DB row was registered with a slightly different separator
        //      style.
        //   3. Default to "code" only when neither helped.
        let project_kind: String = match (&request.project_path, project_repo) {
            (Some(path), Some(repo)) => match repo.find_by_path(path).await {
                Ok(Some(p)) => p.project_type,
                _ => {
                    if path.contains("/knowledge/") || path.contains("\\knowledge\\") {
                        "knowledge".to_string()
                    } else {
                        "code".to_string()
                    }
                }
            },
            (Some(path), None) => {
                if path.contains("/knowledge/") || path.contains("\\knowledge\\") {
                    "knowledge".to_string()
                } else {
                    "code".to_string()
                }
            }
            _ => "code".to_string(),
        };

        let (llm_tools, tool_source): (Option<Vec<venore_core::llm::types::LlmTool>>, String) =
            if request.knowledge_feature_id.is_some() {
                // Hexagon research session — keep the dedicated tool set, ignore mode.
                tracing::info!("Knowledge research mode — using knowledge_research_tools");
                (
                    Some(tools::knowledge_research_tools()),
                    "knowledge_research_tools".to_string(),
                )
            } else {
                match &agent_repo_for_tools {
                    Some(repo) => match repo.load_llm_tools_for_kind(&project_kind).await {
                        Ok(t) if !t.is_empty() => {
                            tracing::info!(
                                count = t.len(),
                                kind = %project_kind,
                                "Loaded tools from DB via mode"
                            );
                            (Some(t), format!("mode-{}", project_kind))
                        }
                        _ => {
                            // Mode came back empty. DO NOT widen to "all enabled" —
                            // that would silently put file/terminal tools in front
                            // of a Knowledge agent. Fall back to a kind-specific
                            // hardcoded set so the AI never escapes its mode.
                            tracing::warn!(
                                kind = %project_kind,
                                "Mode-filtered DB tools empty, falling back to kind-specific hardcoded set"
                            );
                            let t = match project_kind.as_str() {
                                "knowledge" => tools::knowledge_mode_tools(),
                                _ => tools::main_agent_tools(),
                            };
                            (
                                Some(t),
                                format!("fallback-hardcoded-{}", project_kind),
                            )
                        }
                    },
                    None => {
                        let t = match project_kind.as_str() {
                            "knowledge" => tools::knowledge_mode_tools(),
                            _ => tools::main_agent_tools(),
                        };
                        (Some(t), format!("hardcoded-no-repo-{}", project_kind))
                    }
                }
            };

        // Query routing: for clearly project-level informational questions
        // ("what is this", "de qué va", "what's the architecture"),
        // suppress the codebase-investigation tools so the model answers
        // from Project Memory instead of burning a turn re-discovering it
        // with list_files / read_file. Conservative — anything actionable
        // or code-specific keeps the full toolset (see query_router docs).
        let query_class = messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .map(|m| venore_core::chat::classify_query(&m.content))
            .unwrap_or(venore_core::chat::QueryClass::CodeTask);

        let llm_tools = if query_class == venore_core::chat::QueryClass::ProjectQuestion {
            let before = llm_tools.as_ref().map(|t| t.len()).unwrap_or(0);
            let filtered = llm_tools.map(|ts| {
                ts.into_iter()
                    .filter(|t| {
                        !venore_core::chat::INVESTIGATION_TOOLS.contains(&t.name.as_str())
                    })
                    .collect::<Vec<_>>()
            });
            let after = filtered.as_ref().map(|t| t.len()).unwrap_or(0);
            tracing::info!(
                before,
                after,
                "Query routed as ProjectQuestion — suppressed investigation tools (answer from memory)"
            );
            filtered
        } else {
            llm_tools
        };

        // Audit trail: one line per chat turn telling us what the model is
        // actually seeing. Cheap to write, priceless when something feels off.
        venore_core::chat::log_chat_event(venore_core::chat::ChatDebugEvent::SessionInit {
            session_id: session_id.clone().unwrap_or_default(),
            project_path: request.project_path.clone(),
            project_kind: project_kind.clone(),
            tool_count: llm_tools.as_ref().map(|t| t.len()).unwrap_or(0),
            tool_source: tool_source.clone(),
            tool_names: llm_tools
                .as_ref()
                .map(|t| t.iter().map(|tool| tool.name.clone()).collect())
                .unwrap_or_default(),
            ts: venore_core::chat::chat_event_now(),
        });

        // Resolve Tavily API key for web_search
        let tavily_api_key: Option<String> = {
            use venore_core::traits::ApiKeyStore;
            config_store
                .get_api_key(venore_core::traits::LlmProviderType::Tavily)
                .await
                .ok()
                .flatten()
        };

        let rag_repo_for_tools = get_rag_repo(&lazy_state).ok();
        let logbook_repo_for_tools = super::helpers::get_logbook_repo(&lazy_state).ok();
        let (embedding_provider, embedding_api_key) =
            super::helpers::resolve_embedding_provider(&config_store).await;
        let project_id_for_tools: Option<String> = request.project_path.as_ref().and_then(|pp| {
            venore_core::project::ProjectService::read_or_create_identity(
                std::path::Path::new(pp),
            )
            .ok()
            .map(|identity| identity.id.to_string())
        });

        // Build system prompt via core orchestration
        let session_repo = {
            let guard = lazy_state.get();
            guard.as_ref().map(|s| Arc::clone(&s.session_repository))
        };
        let memory_repo = {
            let guard = lazy_state.get();
            guard.as_ref().map(|s| Arc::clone(&s.memory_repository))
        };
        let context_deps = ChatContextDeps {
            prompt_repo: get_prompt_repo(&lazy_state).ok(),
            rag_repo: rag_repo_for_tools.clone(),
            session_repo: session_repo.clone(),
            memory_repo,
            knowledge_repo: knowledge_repo.clone(),
            context_repo: {
                let guard = lazy_state.get();
                guard.as_ref().map(|s| Arc::clone(&s.context_repository))
            },
        };
        let context_modules: Option<Vec<ContextModule>> = request.context_modules.as_ref().map(|mods| {
            mods.iter().map(|m| ContextModule { name: m.name.clone(), path: m.path.clone() }).collect()
        });

        // Pull active AI-connection attachments from the cross-window
        // registry. They become the "📎 attached entities" of this turn —
        // resolver inlines their content (sections / .context.md / hex
        // evidence) into the system prompt so the AI doesn't need to
        // re-fetch what the user explicitly pinned.
        let connection_inputs: Vec<(String, ConnectionTarget)> = lazy_state
            .ai_connections
            .snapshot_active()
            .into_iter()
            .map(|r| {
                let id = r.id.clone();
                let target = match r.target {
                    AiConnectionTarget::KnowledgeNode {
                        project_path,
                        node_id,
                        display_name,
                    } => ConnectionTarget::KnowledgeNode {
                        project_path,
                        node_id,
                        display_name,
                    },
                    AiConnectionTarget::CodeModule {
                        project_path,
                        module_name,
                        module_path,
                    } => ConnectionTarget::CodeModule {
                        project_path,
                        module_name,
                        module_path,
                    },
                    AiConnectionTarget::Hexagon {
                        project_path,
                        feature_id,
                        hexagon_id,
                        display_name,
                    } => ConnectionTarget::Hexagon {
                        project_path,
                        feature_id,
                        hexagon_id,
                        display_name,
                    },
                };
                (id, target)
            })
            .collect();

        let connection_blocks = if connection_inputs.is_empty() {
            None
        } else {
            let result = venore_core::chat::connection_resolver::resolve_connections(
                &connection_inputs,
                context_deps.knowledge_repo.as_ref(),
            )
            .await;
            // Evict registry entries whose entity is gone for good. Frontend
            // mirror catches up via the broadcast in the `unregister` command
            // path — but we're calling the registry directly here and there's
            // no AppHandle in scope, so emit a narrow event ourselves.
            for stale_id in &result.stale_ids {
                lazy_state.ai_connections.unregister(stale_id);
                tracing::info!(
                    stale_id = %stale_id,
                    "Evicted stale AI connection (target gone)"
                );
            }
            if !result.stale_ids.is_empty() {
                // Re-broadcast so all windows drop the dead entry from
                // their local mirror without waiting for the next
                // mutation.
                let snap: Vec<crate::commands::ai_connections::AiConnectionDto> = lazy_state
                    .ai_connections
                    .snapshot()
                    .into_iter()
                    .map(|r| crate::commands::ai_connections::AiConnectionDto {
                        id: r.id,
                        active: r.active,
                        window_label: r.window_label,
                        target: r.target,
                    })
                    .collect();
                let _ = app.emit("ai-connection:update", &snap);
            }
            if result.blocks.is_empty() {
                None
            } else {
                Some(result.blocks)
            }
        };

        let (system_prompt, dev_session_name) = venore_core::chat::build_full_chat_context(
            &context_deps,
            request.project_path.as_deref(),
            context_modules.as_deref(),
            &messages,
            configured_provider,
            project_id_for_tools.as_deref(),
            request.dev_session_id.as_deref(),
            llm_tools.as_deref(),
            request.knowledge_feature_id.as_deref(),
            Some(project_kind.as_str()),
            connection_blocks,
        ).await;

        // Create stream — gateway resolves provider/model from DB internally
        let options = GatewayOptions::for_task(LlmTask::Chat);

        // Convert attachments to content_parts for multimodal support
        let content_parts: Option<Vec<venore_core::llm::types::ContentPart>> =
            request.attachments.as_ref().and_then(|atts| {
                let parts: Vec<_> = atts.iter()
                    .filter(|a| a.mime_type.starts_with("image/"))
                    .map(|a| {
                        tracing::info!(
                            name = %a.name,
                            mime = %a.mime_type,
                            base64_len = a.data_base64.len(),
                            "Attaching image to LLM request"
                        );
                        venore_core::llm::types::ContentPart::ImageBase64 {
                            media_type: a.mime_type.clone(),
                            data: a.data_base64.clone(),
                        }
                    })
                    .collect();
                if parts.is_empty() { None } else { Some(parts) }
            });

        let (initial_stream, model) = venore_core::chat::create_chat_stream_with_attachments(
            &llm_gateway,
            messages.clone(),
            &system_prompt,
            options.clone(),
            llm_tools.clone(),
            content_parts,
        )
        .await?;

        // Resolve tool project path (worktree for dev sessions)
        let project_path = request.project_path.clone();
        let tool_project_path = if let Some(ref dev_session_id) = request.dev_session_id {
            resolve_worktree_path(&lazy_state, dev_session_id).await
                .or(project_path.clone())
        } else {
            project_path.clone()
        };

        // Resolve dev session base branch
        let dev_session_base_branch: Option<String> = if let Some(ref dev_session_id) = request.dev_session_id {
            let session_repo = {
                let guard = lazy_state.get();
                guard.as_ref().map(|s| Arc::clone(&s.session_repository))
            };
            if let Some(repo) = session_repo {
                repo.get(dev_session_id).await.ok().flatten().map(|s| s.base_branch)
            } else {
                None
            }
        } else {
            None
        };

        let provider_name = configured_provider.as_str().to_string();
        let provider_type = configured_provider;
        let stream_id_clone = stream_id.clone();
        let app_clone = app.clone();

        // Build attachment metadata JSON (name + mime only, no base64 data)
        let attachments_json: Option<String> = request.attachments.as_ref().and_then(|atts| {
            if atts.is_empty() { return None; }
            let meta: Vec<serde_json::Value> = atts.iter().map(|a| {
                serde_json::json!({ "name": a.name, "mimeType": a.mime_type })
            }).collect();
            serde_json::to_string(&meta).ok()
        });

        // Clone session_id before it's moved into loop_ctx (needed by watchdog)
        let watchdog_session_id = session_id.clone();

        // Build loop context
        let loop_ctx = AgenticLoopCtx {
            app: app_clone,
            stream_id: stream_id_clone,
            dev_session_id: request.dev_session_id.clone(),
            dev_session_base_branch,
            session_id,
            tool_project_path,
            dev_session_name,
            llm_gateway: llm_gateway.clone(),
            options,
            llm_tools,
            agent_repo: agent_repo_for_tools,
            rag_repo: rag_repo_for_tools,
            logbook_repo: logbook_repo_for_tools,
            project_id: project_id_for_tools,
            tavily_api_key,
            embedding_provider,
            embedding_api_key,
            chat_repo,
            provider_name,
            model,
            provider_type,
            messages,
            attachments_json,
            knowledge_feature_id: request.knowledge_feature_id.clone(),
            knowledge_repo,
        };

        // Spawn the agentic loop — lock ACTIVE_STREAMS first so the
        // AbortHandle is stored before the task can finish (avoids race
        // where stop_chat_stream finds nothing to abort).
        {
            let mut streams = ACTIVE_STREAMS.lock().unwrap_or_else(|e| {
                tracing::warn!("ACTIVE_STREAMS lock poisoned, recovering");
                e.into_inner()
            });
            let join_handle = tokio::spawn(async move {
                agentic_loop::run_main_loop(loop_ctx, initial_stream, &system_prompt).await;
            });
            streams.insert(stream_id.clone(), join_handle.abort_handle());

            // Track session → stream mapping for pop-out window reconnection
            if let Some(ref sid) = request.session_id {
                if let Ok(mut session_streams) = super::state::SESSION_STREAMS.lock() {
                    session_streams.insert(sid.clone(), stream_id.clone());
                }
            }

            // Watchdog: ensure frontend is always notified if the task panics
            let watchdog_app = app.clone();
            let watchdog_stream_id = stream_id.clone();
            tokio::spawn(async move {
                match join_handle.await {
                    Ok(()) => {
                        // Normal completion — done event already emitted by run_main_loop
                    }
                    Err(e) if e.is_cancelled() => {
                        // User cancelled via stop_chat_stream — UI handles this
                        tracing::info!("[stream:{}] Task was cancelled", watchdog_stream_id);
                    }
                    Err(e) => {
                        // PANIC — task died without emitting done
                        tracing::error!(
                            "[stream:{}] Task panicked: {}",
                            watchdog_stream_id, e
                        );
                        let _ = watchdog_app.emit(
                            "chat-stream-error",
                            super::dto::ChatStreamErrorPayload {
                                stream_id: watchdog_stream_id.clone(),
                                session_id: watchdog_session_id.clone(),
                                message: "Internal error — the AI processing task crashed. Please try again.".into(),
                                code: "INTERNAL_PANIC".into(),
                            },
                        );
                        let _ = watchdog_app.emit(
                            "chat-stream-done",
                            super::dto::ChatStreamDonePayload {
                                stream_id: watchdog_stream_id,
                                session_id: watchdog_session_id,
                                prompt_tokens: 0,
                                completion_tokens: 0,
                                total_tokens: 0,
                                provider: String::new(),
                                model: String::new(),
                            },
                        );
                    }
                }
            });
        }

        Ok(SendChatMessageResponse { stream_id })
    }
    .await;

    result.into_state()
}

