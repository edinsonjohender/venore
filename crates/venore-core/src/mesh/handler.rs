//! Mesh Request Handler — processes inbound queries from remote peers
//!
//! `MeshRequestHandler` trait defines the interface for handling agent
//! questions. `AgentHandler` is the production implementation: an
//! LLM-powered sub-agent that reasons over the local project with
//! read-only tools.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex as TokioMutex;
use tokio::time::Instant;

use crate::error::Result;
use crate::llm::gateway::GatewayOptions;
use crate::llm::types::LlmMessage;
use crate::llm::LlmGateway;
use crate::rag::RagRepository;
use crate::tools::{self, MeshFollowUpHandle, ToolExecutionContext};
use crate::traits::LlmTask;

use super::MESH_CONVERSATION_TTL_SECS;

/// Trait for handling inbound mesh requests from remote agents.
#[async_trait]
pub trait MeshRequestHandler: Send + Sync {
    async fn handle_request(
        &self,
        question: &str,
        from_project: &str,
        context_hint: Option<&str>,
        conversation_id: Option<&str>,
        follow_up_handle: Option<MeshFollowUpHandle>,
    ) -> Result<String>;
}

// =============================================================================
// AgentHandler — LLM-powered mesh handler
// =============================================================================

/// A stored conversation: LLM message history + last access time.
struct ConversationEntry {
    messages: Vec<LlmMessage>,
    last_accessed: Instant,
}

/// Smart handler: uses an LLM sub-agent with read-only tools to answer questions.
///
/// When a remote peer asks a question, this handler:
/// 1. Builds a system prompt with project context
/// 2. Runs a mini agentic loop (LLM + read_file/list_files/search_code/search_text)
/// 3. Returns the LLM's synthesized answer
///
/// Supports multi-turn conversations (Phase 4a): when a `conversation_id` is
/// provided, previous LLM messages are loaded and prepended to give the sub-agent
/// context from prior exchanges.
///
/// Falls back gracefully if the LLM is not configured (returns an error message).
pub struct AgentHandler {
    project_id: String,
    project_path: PathBuf,
    rag_repository: Arc<RagRepository>,
    llm_gateway: Arc<LlmGateway>,
    /// In-memory conversation store keyed by conversation_id (Phase 4a).
    conversations: TokioMutex<HashMap<String, ConversationEntry>>,
}

impl AgentHandler {
    pub fn new(
        project_id: String,
        project_path: impl Into<PathBuf>,
        rag_repository: Arc<RagRepository>,
        llm_gateway: Arc<LlmGateway>,
    ) -> Self {
        Self {
            project_id,
            project_path: project_path.into(),
            rag_repository,
            llm_gateway,
            conversations: TokioMutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl MeshRequestHandler for AgentHandler {
    async fn handle_request(
        &self,
        question: &str,
        from_project: &str,
        context_hint: Option<&str>,
        conversation_id: Option<&str>,
        follow_up_handle: Option<MeshFollowUpHandle>,
    ) -> Result<String> {
        tracing::info!(
            from = %from_project,
            question = %question,
            hint = ?context_hint,
            conversation_id = ?conversation_id,
            "Handling mesh request with agent"
        );

        // Load previous messages from conversation store (if multi-turn)
        let previous_messages = if let Some(conv_id) = conversation_id {
            let mut convs = self.conversations.lock().await;
            // TTL cleanup: remove stale entries
            let cutoff = Instant::now() - std::time::Duration::from_secs(MESH_CONVERSATION_TTL_SECS);
            convs.retain(|_, entry| entry.last_accessed > cutoff);
            // Load history for this conversation
            convs.get(conv_id).map(|e| e.messages.clone()).unwrap_or_default()
        } else {
            Vec::new()
        };

        let has_history = !previous_messages.is_empty();
        if has_history {
            tracing::info!(
                conversation_id = ?conversation_id,
                previous_messages = previous_messages.len(),
                "Continuing multi-turn conversation"
            );
        }

        let project_path_str = self.project_path.display().to_string();

        // Build system prompt with project context
        let hint_section = context_hint
            .map(|h| format!("\nThe requesting agent is especially interested in the '{}' area/module.", h))
            .unwrap_or_default();

        // Load the project's compact memory block (the same map the main chat
        // agent gets via `create_full_chat_context`). This is what lets the
        // sub-agent answer with project-aware accuracy instead of guessing from
        // surface code: it carries identity, architecture, conventions, the
        // response language, and a condensed summary — ~1KB, not the codebase.
        // File-only load is enough here (the .venore/ file is the post-Phase-5
        // source of truth); the DB fallback is a legacy edge case the mesh skips.
        let memory_section = crate::memory::file_storage::load(&self.project_path)
            .ok()
            .flatten()
            .map(|m| format!("\n\n{}", crate::memory::format_project_memory(&m)))
            .unwrap_or_default();

        let system_prompt = format!(
            r#"You are an expert code assistant for the project "{project_id}" located at {project_path}.

An agent from the project "{from_project}" is asking you a question about YOUR project. You have access to read-only tools to investigate your codebase and provide an accurate answer.
{hint_section}{memory_section}

## Instructions
- Use search_code and search_text to find relevant code
- Use read_file to examine specific files in detail
- Use list_files to explore directory structure
- Be precise: include file paths, function names, and line numbers in your answer
- Keep your answer focused and concise (the requesting agent needs actionable information)
- If you cannot find the answer, say so clearly rather than guessing

## Project ID: {project_id}
## Project Path: {project_path}"#,
            project_id = self.project_id,
            project_path = project_path_str,
            from_project = from_project,
            hint_section = hint_section,
            memory_section = memory_section,
        );

        // Build tool execution context
        let tool_ctx = ToolExecutionContext {
            terminal_id: None,
            project_path: Some(project_path_str),
            rag_repository: Some(Arc::clone(&self.rag_repository)),
            logbook_repository: None,
            project_id: Some(self.project_id.clone()),
            embedding_provider: None,
            embedding_api_key: None,
            web_search_api_key: None,
            llm_gateway: Some(Arc::clone(&self.llm_gateway)),
            mesh_follow_up: follow_up_handle,
            knowledge_repo: None,
            knowledge_feature_id: None,
            model: None,
            session_id: None,
            allowed_tools: None,
        };

        let options = GatewayOptions::for_task(LlmTask::Chat);
        let tools_def = tools::mesh_agent_tools();

        match super::agent_loop::run_mesh_agent_loop(
            &self.llm_gateway,
            &system_prompt,
            question,
            tools_def,
            options,
            tool_ctx,
            previous_messages,
        )
        .await
        {
            Ok((response, final_messages)) => {
                // Store conversation history for future turns
                if let Some(conv_id) = conversation_id {
                    let mut convs = self.conversations.lock().await;
                    convs.insert(conv_id.to_string(), ConversationEntry {
                        messages: final_messages,
                        last_accessed: Instant::now(),
                    });
                }
                tracing::info!(
                    from = %from_project,
                    response_len = response.len(),
                    conversation_id = ?conversation_id,
                    "Mesh agent request handled"
                );
                Ok(response)
            }
            Err(e) => {
                tracing::error!(
                    from = %from_project,
                    error = %e,
                    "Mesh agent loop failed"
                );
                Err(e)
            }
        }
    }
}

