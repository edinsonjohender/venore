//! venore-eval — headless chat scenario harness.
//!
//! Goal: drive the same LLM + tool stack the desktop app uses, without the
//! Tauri/UI layer. Reads a JSON file of scenarios, runs each one in parallel,
//! captures the model's first-turn tool calls + assistant text, executes any
//! tools the model invoked, and writes a structured JSONL of results.
//!
//! Single-turn semantics — we feed the user prompt, drain the LLM stream once,
//! execute whatever tools came back, append the tool results, drain the
//! follow-up assistant text, and stop. No multi-iteration agentic loop. That
//! covers ~80% of the "did the model pick the right tool" question without
//! decoupling the Tauri-bound agentic loop.
//!
//! Usage:
//!   cargo run --bin venore-eval -- \
//!     --scenarios crates/venore-cli/tests/fixtures/chat_scenarios.json \
//!     --output    eval-results.jsonl

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use venore_core::agents::AgentRepository;
use venore_core::chat::{build_full_chat_context, ChatContextDeps, ChatMessageInput};
use venore_core::context::ContextRepository;
use venore_core::infrastructure::config::{DefaultConfigStore, KeyringApiKeyStore};
use venore_core::knowledge::KnowledgeRepository;
use venore_core::llm::types::{LlmStreamChunk, LlmToolCall};
use venore_core::llm::{GatewayOptions, LlmGateway};
use venore_core::traits::LlmTask;
use venore_core::memory::MemoryRepository;
use venore_core::project::ProjectRepository;
use venore_core::prompts::PromptRepository;
use venore_core::rag::RagRepository;
use venore_core::session::SessionRepository;
use venore_core::tools;
use venore_core::traits::{ConfigStore, LlmProviderType};

// -----------------------------------------------------------------------------
// CLI args
// -----------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(
    name = "venore-eval",
    about = "Run chat scenarios against the local Venore stack and record results"
)]
struct Args {
    /// Path to the scenarios JSON file.
    #[arg(long, short)]
    scenarios: PathBuf,

    /// Output JSONL file (one ScenarioResult per line). Defaults to
    /// `eval-results.jsonl` next to the scenarios file.
    #[arg(long, short)]
    output: Option<PathBuf>,

    /// Cap on how many scenarios run concurrently. Higher = faster, but
    /// burns more provider quota in parallel. Default: 8.
    #[arg(long, default_value_t = 8)]
    concurrency: usize,

    /// If set, only run the first N scenarios. Handy for smoke tests.
    #[arg(long)]
    limit: Option<usize>,
}

// -----------------------------------------------------------------------------
// Scenario + result schemas
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
struct Scenario {
    /// Stable name for the scenario (used in result rows).
    name: String,
    /// "code" or "knowledge" — determines mode and project picked.
    project_kind: String,
    /// What the user types into the chat.
    user_message: String,
    /// Optional list of tool names — if the model calls any of these,
    /// we mark the scenario as having "expected_tool_used = true".
    #[serde(default)]
    expected_tools_any_of: Vec<String>,
    /// Free-form notes shown alongside the result.
    #[serde(default)]
    notes: Option<String>,
}

#[derive(Debug, Serialize)]
struct ScenarioResult {
    name: String,
    project_kind: String,
    project_path: String,
    user_message: String,
    /// Tool names + JSON arguments captured from the LLM's first turn.
    tool_calls: Vec<ToolCallRecord>,
    /// Assistant text streamed before the tool calls (if any).
    pre_tool_text: String,
    /// Assistant text after we replied to the tool results.
    post_tool_text: String,
    /// Tools the user marked as acceptable for this scenario, if any.
    expected_tools_any_of: Vec<String>,
    /// True iff the model called at least one of the expected tools.
    expected_tool_used: bool,
    /// Names of tools that were actually executed (after permission, etc).
    tools_executed: Vec<String>,
    /// Per-tool execution outputs (truncated for log readability).
    tool_results: Vec<ToolResultRecord>,
    duration_ms: u128,
    model: String,
    /// First fatal error if anything blew up.
    error: Option<String>,
    notes: Option<String>,
    /// Names of every tool exposed to the model this turn, in alphabetical
    /// order. Lets us audit "did the AI even see the tool we expected?".
    tools_exposed: Vec<String>,
    /// First N chars of the system prompt actually sent. Truncated to keep
    /// the JSONL line size reasonable; use --full-prompt if you need it all.
    system_prompt_excerpt: String,
}

#[derive(Debug, Serialize)]
struct ToolCallRecord {
    name: String,
    arguments: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct ToolResultRecord {
    name: String,
    success: bool,
    output: String,
}

// -----------------------------------------------------------------------------
// State bootstrap
// -----------------------------------------------------------------------------

/// All the pieces of venore-core a chat turn needs. Mirrors the relevant
/// subset of `venore-desktop`'s `LazyAppState` without Tauri or terminal
/// session manager dependencies.
struct EvalState {
    agent_repo: Arc<AgentRepository>,
    prompt_repo: Arc<PromptRepository>,
    project_repo: Arc<ProjectRepository>,
    knowledge_repo: Arc<KnowledgeRepository>,
    memory_repo: Arc<MemoryRepository>,
    rag_repo: Arc<RagRepository>,
    session_repo: Arc<SessionRepository>,
    context_repo: Arc<ContextRepository>,
    llm_gateway: Arc<LlmGateway>,
}

async fn boot_state() -> Result<EvalState> {
    // Use the same config dir the desktop app uses (debug build → temp dir).
    let config_dir = if cfg!(debug_assertions) {
        std::env::temp_dir().join("venore-dev")
    } else {
        dirs::home_dir()
            .ok_or_else(|| anyhow!("no home dir"))?
            .join(".venore")
    };
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir)?;
    }
    let db_path = config_dir.join("config.db");
    if !db_path.exists() {
        std::fs::File::create(&db_path)?;
    }
    let db_url = format!("sqlite:{}", db_path.display());

    let raw_config_store = DefaultConfigStore::new(&db_url).await?;
    raw_config_store.initialize().await?;
    let config_store = Arc::new(raw_config_store);

    let llm_gateway = Arc::new(LlmGateway::with_config_store(
        Box::new(KeyringApiKeyStore::new()),
        config_store.clone() as Arc<dyn venore_core::traits::TaskConfigStore>,
    ));

    let project_repo = Arc::new(ProjectRepository::new(config_store.pool().clone()));
    project_repo.initialize().await?;

    let knowledge_repo = Arc::new(KnowledgeRepository::new(config_store.pool().clone()));
    knowledge_repo.initialize().await?;

    let agent_repo = Arc::new(AgentRepository::new(config_store.pool().clone()));
    agent_repo.initialize().await?;
    agent_repo.seed_defaults().await?;

    let prompt_repo = Arc::new(PromptRepository::new(config_store.pool().clone()));
    prompt_repo.initialize().await?;
    prompt_repo.seed_defaults().await?;
    prompt_repo.seed_provider_prompts().await.ok();
    prompt_repo.seed_gemini_v4().await.ok();
    prompt_repo.seed_gemini_v5().await.ok();
    prompt_repo.seed_chat_fragments().await.ok();
    prompt_repo.seed_mesh_fragments_v2().await.ok();
    prompt_repo.seed_knowledge_prompts().await.ok();

    let memory_repo = Arc::new(MemoryRepository::new(config_store.pool().clone()));
    memory_repo.initialize().await?;

    let rag_repo = Arc::new(RagRepository::new(config_store.pool().clone()));
    rag_repo.initialize().await?;

    let session_repo = Arc::new(SessionRepository::new(config_store.pool().clone()));
    session_repo.initialize().await?;

    let context_repo = Arc::new(ContextRepository::new(config_store.pool().clone()));
    context_repo.initialize().await?;

    Ok(EvalState {
        agent_repo,
        prompt_repo,
        project_repo,
        knowledge_repo,
        memory_repo,
        rag_repo,
        session_repo,
        context_repo,
        llm_gateway,
    })
}

// -----------------------------------------------------------------------------
// Project picking
// -----------------------------------------------------------------------------

/// Pick an existing registered project of the requested kind. If none exists
/// in the DB, fall back to the first project regardless of kind. (We do NOT
/// create new projects here — eval is meant to run against the user's real
/// data so results match what they see in the app.)
async fn pick_project_for_kind(state: &EvalState, kind: &str) -> Result<(String, String)> {
    let projects = state.project_repo.list().await?;
    if projects.is_empty() {
        return Err(anyhow!(
            "no registered projects in the DB. Open the app once and create at least one project before running eval."
        ));
    }
    let chosen = projects
        .iter()
        .find(|p| p.project_type == kind)
        .or_else(|| projects.first())
        .unwrap();
    Ok((chosen.path.clone(), chosen.project_type.clone()))
}

// -----------------------------------------------------------------------------
// Single scenario runner
// -----------------------------------------------------------------------------

async fn run_scenario(state: Arc<EvalState>, scenario: Scenario) -> ScenarioResult {
    let started = std::time::Instant::now();
    let session_id = format!("eval-{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));

    let mut result = ScenarioResult {
        name: scenario.name.clone(),
        project_kind: scenario.project_kind.clone(),
        project_path: String::new(),
        user_message: scenario.user_message.clone(),
        tool_calls: Vec::new(),
        pre_tool_text: String::new(),
        post_tool_text: String::new(),
        expected_tools_any_of: scenario.expected_tools_any_of.clone(),
        expected_tool_used: false,
        tools_executed: Vec::new(),
        tool_results: Vec::new(),
        duration_ms: 0,
        model: String::new(),
        error: None,
        notes: scenario.notes.clone(),
        tools_exposed: Vec::new(),
        system_prompt_excerpt: String::new(),
    };

    let inner = run_scenario_inner(&state, &scenario, &session_id, &mut result).await;
    if let Err(e) = inner {
        // `{:#}` walks the full anyhow context chain (top context + source),
        // so the recorded error is "create_chat_stream: <provider cause>"
        // instead of just the outermost label.
        result.error = Some(format!("{:#}", e));
    }
    result.duration_ms = started.elapsed().as_millis();
    result
}

async fn run_scenario_inner(
    state: &EvalState,
    scenario: &Scenario,
    session_id: &str,
    result: &mut ScenarioResult,
) -> Result<()> {
    // 1. Pick project of the right kind
    let (project_path, project_kind) = pick_project_for_kind(state, &scenario.project_kind).await?;
    result.project_path = project_path.clone();

    // 2. Resolve the tool inventory the way stream.rs does it.
    let tools_for_kind = state
        .agent_repo
        .load_llm_tools_for_kind(&project_kind)
        .await
        .context("loading tools for kind")?;
    let tools_vec = if tools_for_kind.is_empty() {
        match project_kind.as_str() {
            "knowledge" => tools::knowledge_mode_tools(),
            _ => tools::main_agent_tools(),
        }
    } else {
        tools_for_kind
    };

    // 3. Build system prompt via the same orchestrator the app uses.
    let provider = LlmProviderType::Gemini; // matches user's default
    let deps = ChatContextDeps {
        prompt_repo: Some(Arc::clone(&state.prompt_repo)),
        rag_repo: Some(Arc::clone(&state.rag_repo)),
        session_repo: Some(Arc::clone(&state.session_repo)),
        memory_repo: Some(Arc::clone(&state.memory_repo)),
        knowledge_repo: Some(Arc::clone(&state.knowledge_repo)),
        context_repo: Some(Arc::clone(&state.context_repo)),
    };

    let messages = vec![ChatMessageInput {
        role: "user".into(),
        content: scenario.user_message.clone(),
    }];

    let project_id_resolved: Option<String> = state
        .project_repo
        .find_by_path(&project_path)
        .await
        .ok()
        .flatten()
        .map(|p| p.id.to_string());

    let (system_prompt, _dev_session_name) = build_full_chat_context(
        &deps,
        Some(project_path.as_str()),
        None,
        &messages,
        provider,
        project_id_resolved.as_deref(),
        None,
        Some(&tools_vec),
        None,
        Some(project_kind.as_str()),
        None,
    )
    .await;

    // Snapshot the inputs the model is actually getting so we can diff
    // good vs bad runs without re-running.
    {
        let mut names: Vec<String> = tools_vec.iter().map(|t| t.name.clone()).collect();
        names.sort();
        result.tools_exposed = names;
        result.system_prompt_excerpt = truncate(&system_prompt, 12000);
    }

    // 4. Open the LLM stream.
    let options = GatewayOptions::for_task(LlmTask::Chat);
    let (stream, model) = venore_core::chat::create_chat_stream(
        &state.llm_gateway,
        messages.clone(),
        &system_prompt,
        options.clone(),
        Some(tools_vec.clone()),
    )
    .await
    .context("create_chat_stream")?;
    result.model = model.clone();

    // 5. Drain the first turn — collect text + tool calls.
    let mut stream = stream;
    let mut pre_text = String::new();
    let mut tool_calls: Vec<LlmToolCall> = Vec::new();
    while let Some(chunk_res) = stream.next().await {
        let chunk = chunk_res.context("stream chunk")?;
        match chunk {
            LlmStreamChunk::Text { content } => pre_text.push_str(&content),
            LlmStreamChunk::ToolCall { call } => tool_calls.push(call),
            LlmStreamChunk::Done { .. } => break,
            _ => {}
        }
    }
    drop(stream);
    result.pre_tool_text = pre_text;
    result.tool_calls = tool_calls
        .iter()
        .map(|tc| ToolCallRecord {
            name: tc.name.clone(),
            arguments: tc.arguments.clone(),
        })
        .collect();
    result.expected_tool_used = tool_calls.iter().any(|tc| {
        scenario
            .expected_tools_any_of
            .iter()
            .any(|t| t == &tc.name)
    });

    // 6. Execute the tool calls (single round) and feed results back so the
    // model can produce a closing message — same as a normal turn does.
    if tool_calls.is_empty() {
        return Ok(());
    }

    let tool_ctx = tools::ToolExecutionContext {
        terminal_id: None,
        project_path: Some(project_path.clone()),
        rag_repository: Some(Arc::clone(&state.rag_repo)),
        logbook_repository: None,
        project_id: project_id_resolved.clone(),
        embedding_provider: None,
        embedding_api_key: None,
        web_search_api_key: None,
        llm_gateway: Some(Arc::clone(&state.llm_gateway)),
        mesh_follow_up: None,
        knowledge_repo: Some(Arc::clone(&state.knowledge_repo)),
        knowledge_feature_id: None,
        model: Some(model.clone()),
        session_id: Some(session_id.to_string()),
        allowed_tools: Some(tools_vec.iter().map(|t| t.name.clone()).collect()),
    };

    // Execute each tool call against the real venore-core executor so we
    // see whether the model passed valid arguments and whether the tool
    // actually does what the test scenario expects. We do NOT feed tool
    // results back to a closing LLM call — that would require building
    // LlmMessages directly (ChatMessageInput doesn't carry tool_call_id),
    // which is more plumbing than v1 needs to audit tool selection.
    for tc in &tool_calls {
        let exec = tools::execute_tool(&tc.name, &tc.arguments, &tool_ctx).await;
        let (success, output) = match exec {
            Ok(r) => (r.success, r.output),
            Err(e) => (false, e.to_string()),
        };
        result.tools_executed.push(tc.name.clone());
        result.tool_results.push(ToolResultRecord {
            name: tc.name.clone(),
            success,
            output: truncate(&output, 800),
        });
    }

    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while !s.is_char_boundary(end) && end > 0 {
        end -= 1;
    }
    format!("{}…[+{} bytes]", &s[..end], s.len() - end)
}

// -----------------------------------------------------------------------------
// Main
// -----------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,sqlx=warn")),
        )
        .compact()
        .init();

    let args = Args::parse();

    let scenarios_text =
        std::fs::read_to_string(&args.scenarios).context("reading scenarios file")?;
    let mut scenarios: Vec<Scenario> =
        serde_json::from_str(&scenarios_text).context("parsing scenarios JSON")?;
    if let Some(n) = args.limit {
        scenarios.truncate(n);
    }
    println!("Loaded {} scenarios", scenarios.len());

    let output_path = args.output.clone().unwrap_or_else(|| {
        let mut p = args.scenarios.clone();
        p.set_file_name("eval-results.jsonl");
        p
    });

    let state = Arc::new(boot_state().await.context("boot_state")?);
    println!("Backend bootstrapped, running scenarios...");

    let mut all_results: Vec<ScenarioResult> = Vec::with_capacity(scenarios.len());
    for chunk in scenarios.chunks(args.concurrency) {
        let mut handles = Vec::with_capacity(chunk.len());
        for scenario in chunk {
            let s = Arc::clone(&state);
            let sc = scenario.clone();
            handles.push(tokio::spawn(async move { run_scenario(s, sc).await }));
        }
        for h in handles {
            match h.await {
                Ok(r) => all_results.push(r),
                Err(e) => {
                    eprintln!("scenario task panicked: {}", e);
                }
            }
        }
    }

    // Persist results JSONL
    let mut out = String::new();
    for r in &all_results {
        out.push_str(&serde_json::to_string(r).unwrap());
        out.push('\n');
    }
    std::fs::write(&output_path, out).context("writing eval-results.jsonl")?;

    // Console summary
    println!();
    println!("== EVAL SUMMARY ==");
    println!("Results written to: {}", output_path.display());
    println!();
    let mut ok = 0u32;
    let mut empty = 0u32;
    let mut errored = 0u32;
    let mut expected_used = 0u32;
    for r in &all_results {
        if r.error.is_some() {
            errored += 1;
        } else if r.tool_calls.is_empty() {
            empty += 1;
        } else {
            ok += 1;
        }
        if r.expected_tool_used {
            expected_used += 1;
        }
        let tag = if r.error.is_some() {
            "ERR"
        } else if r.tool_calls.is_empty() {
            "no-tool"
        } else if r.expected_tool_used {
            "ok-expected"
        } else {
            "ok-other"
        };
        let tools = r
            .tool_calls
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "[{:>11}] {:30} tools=[{}] dur={}ms",
            tag, r.name, tools, r.duration_ms
        );
    }
    println!();
    println!(
        "{} ok-with-tool, {} expected-tool-hit, {} no-tool-call, {} errors / {} total",
        ok,
        expected_used,
        empty,
        errored,
        all_results.len()
    );

    Ok(())
}
