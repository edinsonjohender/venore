//! Sub-agent runner — mini agentic loop with limited tools and iterations.

use std::sync::Arc;

use tokio::time::{timeout, Duration};

use venore_core::error::VenoreError;
use venore_core::llm::prelude::*;
use venore_core::llm::types::{LlmMessage, LlmToolCall};
use venore_core::tools;
use venore_core::tools::names as N;

use super::helpers::is_parallelizable;
use super::state::ACTIVE_SUB_AGENTS;
use super::tool_dispatch::{execute_tool_safe, append_lsp_diagnostics, post_process_terminal};

/// Max time to wait for a single stream chunk before considering the connection stalled.
const STREAM_CHUNK_TIMEOUT: Duration = Duration::from_secs(120);

/// Decrement the active sub-agent count for a stream.
pub(super) fn decrement_active_sub_agents(stream_id: &str) {
    if let Ok(mut agents) = ACTIVE_SUB_AGENTS.lock() {
        if let Some(count) = agents.get_mut(stream_id) {
            *count = count.saturating_sub(1);
        }
    }
}

/// Hardcoded fallback prompt for sub-agents (used when no DB profile is found).
pub(super) fn hardcoded_sub_agent_prompt(agent_type: &str, task: &str) -> String {
    if agent_type == "executor" {
        format!(r#"You are an executor agent. Your job is to start and verify an application.

## Task
{}

## Mandatory Workflow — follow IN ORDER, do not skip steps

### Step 1: Analyze the project
- `list_files` to see project structure
- `read_file` on the config file:
  - Node.js: package.json (check "scripts")
  - Python: requirements.txt / pyproject.toml
  - Docker: Dockerfile / docker-compose.yml
  - Rust: Cargo.toml  |  Go: go.mod
- Identify: app type, start command, port, dependencies status

### Step 2: Install dependencies (if needed)
- Node.js: npm install (if no node_modules/)
- Python: pip install -r requirements.txt
- Docker: docker build (if Dockerfile present)
- If install fails, report the error

### Step 3: Start the app
- Use `run_app` with the correct command and port
- Do NOT use `run_terminal_command` for starting apps

### Step 4: Verify health
- After run_app returns RUNNING, IMMEDIATELY call `check_health`
- URL: http://localhost:PORT
- If UNHEALTHY: use `read_terminal_output` to check logs, report the error
- If HEALTHY: report success with the URL

## Rules
- NEVER skip the analysis step (Step 1)
- NEVER skip health check after run_app
- NEVER modify source code — read-only for files
- Report errors clearly so the main agent can fix them"#, task)
    } else {
        format!(
            "You are a {} sub-agent. Your task: {}\n\nYou have access to a limited set of tools. Complete the task and return a concise result. Max 3 iterations.",
            agent_type, task
        )
    }
}

/// Run a mini agentic loop for a sub-agent with limited tools and iterations.
pub(super) async fn run_sub_agent(
    llm_gateway: Arc<venore_core::llm::LlmGateway>,
    initial_messages: Vec<LlmMessage>,
    system_prompt: String,
    llm_tools: Option<Vec<venore_core::llm::types::LlmTool>>,
    model: String,
    options: GatewayOptions,
    ctx: tools::ToolExecutionContext,
    active_terminal_id: Option<String>,
    max_iterations_override: Option<u32>,
    max_tool_calls_override: Option<u32>,
) -> Result<String, VenoreError> {
    use futures::StreamExt;

    let max_iterations = max_iterations_override.unwrap_or(3);
    let max_tool_calls = max_tool_calls_override.unwrap_or(15);
    let mut total_tool_calls = 0u32;
    let mut llm_messages = initial_messages;

    llm_messages.insert(0, LlmMessage {
        role: MessageRole::System,
        content: system_prompt.to_string(),
        tool_call_id: None,
        tool_calls: None,
        content_parts: None,
    });

    let mut accumulated_content = String::new();

    let (initial_stream, _model) = venore_core::chat::create_chat_stream(
        &llm_gateway,
        vec![venore_core::chat::ChatMessageInput {
            role: "user".to_string(),
            content: llm_messages.last().map(|m| m.content.clone()).unwrap_or_default(),
        }],
        &system_prompt,
        options.clone(),
        llm_tools.clone(),
    )
    .await?;

    let mut current_stream = initial_stream;

    for _iteration in 0..max_iterations {
        let mut iteration_text = String::new();
        let mut iteration_tool_calls: Vec<LlmToolCall> = Vec::new();

        loop {
            let chunk_result = match timeout(STREAM_CHUNK_TIMEOUT, current_stream.next()).await {
                Ok(Some(chunk)) => chunk,
                Ok(None) => break, // Stream ended normally
                Err(_) => {
                    tracing::error!("[sub-agent] Stream chunk timeout after {}s", STREAM_CHUNK_TIMEOUT.as_secs());
                    return Err(VenoreError::Timeout(STREAM_CHUNK_TIMEOUT.as_millis() as u64));
                }
            };
            match chunk_result {
                Ok(chunk) => match chunk {
                    LlmStreamChunk::Text { content } => {
                        iteration_text.push_str(&content);
                    }
                    LlmStreamChunk::ToolCall { call } => {
                        iteration_tool_calls.push(call);
                    }
                    LlmStreamChunk::Done { .. } => break,
                    LlmStreamChunk::Error { error } => {
                        return Err(VenoreError::LlmStreamError(error));
                    }
                },
                Err(e) => return Err(e),
            }
        }

        // Strip any pasted tool-call syntax (Gemini quirk) before persist /
        // re-injection. Same rationale as the main agentic loop.
        let iteration_text =
            venore_core::chat::guardrails::strip_tool_call_syntax(&iteration_text);

        accumulated_content.push_str(&iteration_text);

        if iteration_tool_calls.is_empty() {
            break;
        }

        total_tool_calls += iteration_tool_calls.len() as u32;
        if total_tool_calls > max_tool_calls {
            break;
        }

        llm_messages.push(LlmMessage {
            role: MessageRole::Assistant,
            content: iteration_text,
            tool_call_id: None,
            tool_calls: Some(iteration_tool_calls.clone()),
            content_parts: None,
        });

        // Partition sub-agent tool calls: parallel (read-only) vs sequential
        let mut parallel_calls: Vec<&LlmToolCall> = Vec::new();
        let mut sequential_calls: Vec<&LlmToolCall> = Vec::new();

        for tc in &iteration_tool_calls {
            if is_parallelizable(&tc.name) {
                parallel_calls.push(tc);
            } else {
                sequential_calls.push(tc);
            }
        }

        // Execute parallel batch
        if !parallel_calls.is_empty() {
            let parallel_futures: Vec<_> = parallel_calls
                .iter()
                .map(|tc| {
                    let name = tc.name.clone();
                    let args = tc.arguments.clone();
                    let ctx = ctx.clone();
                    async move { execute_tool_safe(&name, &args, &ctx).await }
                })
                .collect();

            let parallel_results = futures::future::join_all(parallel_futures).await;

            for (tc, result) in parallel_calls.iter().zip(parallel_results) {
                llm_messages.push(LlmMessage {
                    role: MessageRole::Tool,
                    content: result.output,
                    tool_call_id: Some(tc.id.clone()),
                    tool_calls: None,
                    content_parts: None,
                });
            }
        }

        // Execute sequential calls
        for tool_call in &sequential_calls {
            let mut tool_result = execute_tool_safe(&tool_call.name, &tool_call.arguments, &ctx).await;

            // LSP diagnostics for file-editing tools
            if let Some(ref proj_path) = ctx.project_path {
                append_lsp_diagnostics(tool_call, &mut tool_result, proj_path).await;
            }

            // Terminal output auto-read
            if tool_call.name == N::RUN_TERMINAL_COMMAND && tool_result.success
                && post_process_terminal(tool_call, active_terminal_id.as_deref(), &tool_result, &mut llm_messages).await {
                    continue;
                }

            llm_messages.push(LlmMessage {
                role: MessageRole::Tool,
                content: tool_result.output,
                tool_call_id: Some(tool_call.id.clone()),
                tool_calls: None,
                content_parts: None,
            });
        }

        // Continue sub-agent loop
        match venore_core::chat::continue_chat_stream(
            &llm_gateway,
            llm_messages.clone(),
            llm_tools.clone(),
            &model,
            options.clone(),
        )
        .await
        {
            Ok(next_stream) => current_stream = next_stream,
            Err(e) => return Err(e),
        }
    }

    // Truncate result if too long
    if accumulated_content.len() > 10_000 {
        accumulated_content.truncate(accumulated_content.floor_char_boundary(10_000));
        accumulated_content.push_str("\n\n... (sub-agent output truncated)");
    }

    Ok(accumulated_content)
}
