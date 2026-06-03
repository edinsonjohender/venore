//! Tool Executor
//!
//! Dispatches tool calls to the appropriate handler and returns results.
//! The caller provides a `ToolExecutionContext` with the resolved terminal_id
//! so the AI never needs to know about IDs.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use once_cell::sync::Lazy;
use regex::{Regex, RegexBuilder};
use walkdir::WalkDir;

// ============================================================================
// COMPILED REGEX — shared across all tool calls (compiled once)
// ============================================================================

/// Docker -p HOST:CONTAINER or --publish HOST:CONTAINER → captures HOST(1), CONTAINER(2)
static RE_DOCKER_PORT_MAP: Lazy<Regex> = Lazy::new(||
    Regex::new(r"(?:-p|--publish)\s+(\d{2,5}):(\d{2,5})").unwrap()
);

/// --port NUM or --port=NUM → captures PORT(1)
static RE_PORT_FLAG: Lazy<Regex> = Lazy::new(||
    Regex::new(r"--port[=\s]+(\d{2,5})").unwrap()
);

/// -p NUM (non-docker, no colon) → captures PORT(1)
static RE_SHORT_P_FLAG: Lazy<Regex> = Lazy::new(||
    Regex::new(r"\s-p\s+(\d{2,5})(?:\s|$)").unwrap()
);

/// Listening port in logs: localhost:PORT, 0.0.0.0:PORT, Local: http://...:PORT
static RE_LISTEN_PORT: Lazy<Regex> = Lazy::new(||
    Regex::new(r"(?:localhost|127\.0\.0\.1|0\.0\.0\.0|Local:\s+https?://[^:]+):(\d{2,5})").unwrap()
);

use crate::terminal::TerminalSessionManager;
use crate::rag::RagRepository;
use crate::traits::EmbeddingProvider;
use crate::error::{Result, VenoreError};
use crate::llm::LlmGateway;
use crate::mesh::{MeshTransport, MeshDiscovery, get_or_create_conversation_id};
use crate::mesh::CallerMessage;
use super::fuzzy_match;
use super::names as N;

/// Directories to skip during file traversal (shared by list_files and search_text).
const SKIP_DIRS: &[&str] = &[
    ".git", "node_modules", "target", "__pycache__", ".next", "dist", "build", ".venv",
];

/// Handle for mesh follow-up communication (Phase 4b).
///
/// Allows the handler sub-agent's `ask_caller` tool to send a follow-up
/// question back to the caller and wait for the answer.
#[derive(Clone)]
pub struct MeshFollowUpHandle {
    /// Stream ID of the active request.
    pub stream_id: String,
    /// Channel to write serialized messages back to the caller via WebSocket.
    pub write_tx: tokio::sync::mpsc::UnboundedSender<String>,
    /// Shared map of (stream_id, round) → oneshot sender for answers.
    pub answer_channels: Arc<std::sync::Mutex<HashMap<(String, u32), tokio::sync::oneshot::Sender<String>>>>,
    /// Atomic counter for follow-up rounds (capped at MAX_FOLLOW_UPS).
    pub follow_up_count: Arc<AtomicU32>,
}

/// Context provided by the caller (chat command) with pre-resolved resources.
#[derive(Clone)]
pub struct ToolExecutionContext {
    /// The terminal ID resolved by the caller (auto-spawned or existing).
    pub terminal_id: Option<String>,
    /// Project root path — used to resolve relative file paths.
    pub project_path: Option<String>,
    /// RAG repository for code search (None if not indexed).
    pub rag_repository: Option<Arc<RagRepository>>,
    /// Logbook repository for knowledge hybrid search (None = grep fallback).
    pub logbook_repository: Option<Arc<crate::rag::LogbookRepository>>,
    /// Project ID for RAG scoping.
    pub project_id: Option<String>,
    /// Embedding provider for hybrid search (None = FTS5 only).
    pub embedding_provider: Option<Arc<dyn EmbeddingProvider>>,
    /// API key for embedding provider.
    pub embedding_api_key: Option<String>,
    /// API key for Tavily web search (None = web_search unavailable).
    pub web_search_api_key: Option<String>,
    /// LLM gateway for inline follow-up answers (Phase 4b).
    pub llm_gateway: Option<Arc<LlmGateway>>,
    /// Mesh follow-up handle — set only inside mesh handler tasks (Phase 4b).
    pub mesh_follow_up: Option<MeshFollowUpHandle>,
    /// Knowledge repository for research tools.
    pub knowledge_repo: Option<Arc<crate::knowledge::KnowledgeRepository>>,
    /// Active knowledge feature ID for research tools.
    pub knowledge_feature_id: Option<String>,
    /// Identifier of the LLM model that requested this tool call. Recorded
    /// alongside AI-generated logbook sections so they can be regenerated
    /// later. None when the caller didn't pre-fill it.
    pub model: Option<String>,
    /// Chat session id, used to correlate tool calls to their conversation
    /// in the chat-debug.jsonl log. None for callers that don't run inside
    /// a chat session (mesh handler, research worker, tests).
    pub session_id: Option<String>,
    /// Allowlist of tool names the LLM was actually offered for this turn.
    /// When Some, `execute_tool` rejects calls to any tool not in the list
    /// — protects against models hallucinating tools that exist in the
    /// codebase but were intentionally hidden from this mode (e.g. Gemini
    /// inventing `write_file` when a Knowledge agent only has logbook tools).
    /// None means "no allowlist enforced" (legacy callers, tests).
    pub allowed_tools: Option<Vec<String>>,
}

/// Result of executing a tool
#[derive(Debug, Clone)]
pub struct ToolExecutionResult {
    pub success: bool,
    pub output: String,
    /// Monotonic line counter baseline taken before writing a command.
    /// Used by the caller to read only output produced after this point.
    pub baseline: Option<u64>,
}

/// Execute a tool by name with the given arguments and execution context.
///
/// If `ctx.allowed_tools` is set, the call is rejected up front when `name`
/// isn't in the list. This prevents a model from invoking tools that exist
/// in the codebase but were deliberately hidden from its inventory (e.g.
/// Gemini inventing `write_file` for a Knowledge agent that only has
/// logbook tools — without this guard the dispatch would still execute it).
pub async fn execute_tool(
    name: &str,
    arguments: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    if let Some(ref allowed) = ctx.allowed_tools {
        if !allowed.iter().any(|t| t == name) {
            tracing::warn!(
                tool = %name,
                "Tool call rejected — not in this turn's allowlist (model may be hallucinating)"
            );
            return Ok(ToolExecutionResult {
                success: false,
                output: format!(
                    "Tool '{}' is not available in this mode. Available tools: {}.",
                    name,
                    allowed.join(", ")
                ),
                baseline: None,
            });
        }
    }
    match name {
        N::RUN_TERMINAL_COMMAND => execute_run_command(arguments, ctx),
        N::RUN_APP => execute_run_app(arguments, ctx).await,
        N::CHECK_HEALTH => execute_check_health(arguments).await,
        N::READ_TERMINAL_OUTPUT => execute_read_output(arguments, ctx),
        N::READ_FILE => execute_read_file(arguments, ctx),
        N::WRITE_FILE => execute_write_file(arguments, ctx),
        N::EDIT_FILE => execute_edit_file(arguments, ctx),
        N::MULTI_EDIT_FILE => execute_multi_edit_file(arguments, ctx),
        N::LIST_FILES => execute_list_files(arguments, ctx),
        N::SEARCH_CODE => execute_search_code(arguments, ctx).await,
        N::SEARCH_TEXT => execute_search_text(arguments, ctx),
        N::WEB_FETCH => execute_web_fetch(arguments).await,
        N::WEB_SEARCH => execute_web_search(arguments, ctx).await,
        N::ASK_PROJECT => execute_ask_project(arguments, ctx).await,
        N::ASK_CALLER => execute_ask_caller(arguments, ctx).await,
        // Knowledge tools
        N::PLAN_HEXAGONS => execute_plan_hexagons(arguments, ctx).await,
        N::UPDATE_HEXAGON => execute_update_hexagon(arguments, ctx).await,
        N::ADD_EVIDENCE => execute_add_evidence(arguments, ctx).await,
        N::MARK_DEAD_END => execute_mark_dead_end(arguments, ctx).await,
        N::GENERATE_REPORT => execute_generate_report(arguments, ctx).await,
        // Logbook tools
        N::LIST_LOGBOOKS => execute_list_logbooks(arguments, ctx),
        N::READ_LOGBOOK => execute_read_logbook(arguments, ctx),
        N::SEARCH_LOGBOOK => execute_search_logbook(arguments, ctx).await,
        N::LIST_CONNECTIONS => execute_list_connections(arguments, ctx),
        N::LIST_ISLANDS => execute_list_islands(arguments, ctx),
        N::QUERY_NEIGHBORHOOD => execute_query_neighborhood(arguments, ctx),
        N::PROPOSE_LOGBOOK_WRITE => execute_propose_logbook_write(arguments, ctx),
        // Structure tools — manipulate the Ocean Canvas graph itself.
        N::CREATE_LIGHTHOUSE => execute_create_lighthouse(arguments, ctx),
        N::CREATE_KNOWLEDGE_NODE => execute_create_knowledge_node(arguments, ctx),
        N::CREATE_CONNECTION => execute_create_connection(arguments, ctx),
        N::PROMOTE_TO_LIGHTHOUSE => execute_promote_to_lighthouse(arguments, ctx),
        N::SET_NODE_LIGHTHOUSE => execute_set_node_lighthouse(arguments, ctx),
        N::RENAME_NODE => execute_rename_node(arguments, ctx),
        _ => Err(VenoreError::ToolNotFound(name.to_string())),
    }
}

// ============================================================================
// PATH RESOLUTION
// ============================================================================

/// Resolve a file path: if relative, prepend project_path.
fn resolve_path(file_path: &str, ctx: &ToolExecutionContext) -> PathBuf {
    let path = Path::new(file_path);
    if path.is_absolute() {
        path.to_path_buf()
    } else if let Some(ref project) = ctx.project_path {
        Path::new(project).join(file_path)
    } else {
        path.to_path_buf()
    }
}

// ============================================================================
// TERMINAL TOOLS
// ============================================================================

fn resolve_terminal_id(ctx: &ToolExecutionContext) -> Result<&str> {
    ctx.terminal_id
        .as_deref()
        .ok_or_else(|| VenoreError::ToolExecutionFailed("No terminal available".into()))
}

/// Extract host-side port numbers from a command that might bind to ports.
pub(crate) fn extract_ports_from_command(command: &str) -> Vec<u16> {
    let mut ports = Vec::new();

    // Docker: -p 5173:5173, --publish 8080:80/tcp → host port (group 1)
    for cap in RE_DOCKER_PORT_MAP.captures_iter(command) {
        if let Some(m) = cap.get(1) {
            if let Ok(p) = m.as_str().parse::<u16>() {
                ports.push(p);
            }
        }
    }

    // --port NUMBER or --port=NUMBER (vite, next, etc.)
    for cap in RE_PORT_FLAG.captures_iter(command) {
        if let Some(m) = cap.get(1) {
            if let Ok(p) = m.as_str().parse::<u16>() {
                ports.push(p);
            }
        }
    }

    // -p NUMBER when NOT followed by colon (non-docker, e.g. next dev -p 3000)
    for cap in RE_SHORT_P_FLAG.captures_iter(command) {
        if let Some(m) = cap.get(1) {
            if let Ok(p) = m.as_str().parse::<u16>() {
                if !ports.contains(&p) {
                    ports.push(p);
                }
            }
        }
    }

    ports.sort();
    ports.dedup();
    ports
}

/// Extract container ID from terminal output of `docker run -d`.
/// The output is typically just a hex hash (64 chars) on its own line.
fn extract_container_id(terminal_output: &str) -> Option<&str> {
    terminal_output.lines()
        .rev()
        .find(|l| {
            let trimmed = l.trim();
            !trimmed.is_empty() && trimmed.len() >= 12 && trimmed.chars().all(|c| c.is_ascii_hexdigit())
        })
        .map(|l| l.trim())
}

/// Check if a detached docker container is still running after `docker run -d`.
/// Returns a warning with logs if the container exited/crashed.
pub async fn check_docker_container_health(command: &str, terminal_output: &str) -> Option<String> {
    if !command.contains("docker") || !command.contains("run") || !command.contains("-d") {
        return None;
    }

    let container_id = extract_container_id(terminal_output)?;

    // Check if container is still running
    let inspect = crate::utils::quiet_tokio_command("docker")
        .args(["inspect", "--format", "{{.State.Running}}", container_id])
        .output()
        .await
        .ok()?;

    let is_running = String::from_utf8_lossy(&inspect.stdout).trim() == "true";
    if is_running {
        return None; // Container is healthy
    }

    // Container crashed — fetch logs to show the error
    let logs_output = crate::utils::quiet_tokio_command("docker")
        .args(["logs", "--tail", "50", container_id])
        .output()
        .await
        .ok()?;

    let logs = String::from_utf8_lossy(&logs_output.stdout);
    let stderr = String::from_utf8_lossy(&logs_output.stderr);
    let combined = format!("{}\n{}", logs, stderr);
    let short_id = &container_id[..12.min(container_id.len())];

    Some(format!(
        "\n\nWARNING: Container {} exited immediately after starting! The app is NOT running.\n\
         Container logs:\n{}\n\n\
         Investigate the error above, fix the Dockerfile or configuration, then re-run the container.",
        short_id,
        combined.trim()
    ))
}

/// For detached docker containers, fetch logs to check port info.
/// Returns the container logs or None.
pub(crate) async fn fetch_docker_logs_if_detached(command: &str, terminal_output: &str) -> Option<String> {
    // Only for docker run -d with port mapping
    if !command.contains("docker") || !command.contains("run") || !command.contains("-d") {
        return None;
    }
    if extract_ports_from_command(command).is_empty() {
        return None;
    }

    let container_id = extract_container_id(terminal_output)?;

    // Wait a moment for container to start, then fetch logs
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let output = crate::utils::quiet_tokio_command("docker")
        .args(["logs", "--tail", "30", container_id])
        .output()
        .await
        .ok()?;

    let logs = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}\n{}", logs, stderr);

    if combined.trim().is_empty() {
        None
    } else {
        Some(combined)
    }
}

/// Detect Docker containers listening on the given ports.
/// Returns a hint with container IDs and names so the AI can offer to stop them.
async fn detect_docker_on_ports(ports: &[u16]) -> Option<String> {
    // docker ps --format with port filtering
    let output = crate::utils::quiet_tokio_command("docker")
        .args(["ps", "--format", "{{.ID}}\t{{.Names}}\t{{.Ports}}\t{{.Status}}"])
        .output()
        .await
        .ok()?;

    let ps_output = String::from_utf8_lossy(&output.stdout);
    if ps_output.trim().is_empty() {
        return None;
    }

    let mut matches = Vec::new();
    for line in ps_output.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 4 {
            continue;
        }
        let (id, name, ports_col, status) = (parts[0], parts[1], parts[2], parts[3]);
        for &port in ports {
            let port_str = format!(":{}", port);
            if ports_col.contains(&port_str) {
                matches.push(format!(
                    "  - Container {} ({}) on port {} — {}",
                    &id[..12.min(id.len())], name, port, status
                ));
            }
        }
    }

    if matches.is_empty() {
        return None;
    }

    Some(format!(
        "Docker containers using these ports:\n{}",
        matches.join("\n")
    ))
}

/// Check terminal output for port mismatch after a docker run command.
/// Compares container-side ports in the command with actual listening ports in the output.
/// Returns a warning if they don't match.
pub(crate) fn check_port_mismatch(command: &str, terminal_output: &str) -> Option<String> {
    if !command.contains("docker") || !command.contains("run") {
        return None;
    }

    // Extract container-side ports: -p HOST:CONTAINER → group 2 (container)
    let mapped_container_ports: Vec<u16> = RE_DOCKER_PORT_MAP.captures_iter(command)
        .filter_map(|cap| cap.get(2)?.as_str().parse().ok())
        .collect();
    if mapped_container_ports.is_empty() {
        return None;
    }

    // Extract actual listening ports from logs
    let actual_ports: Vec<u16> = RE_LISTEN_PORT.captures_iter(terminal_output)
        .filter_map(|cap| cap.get(1)?.as_str().parse().ok())
        .collect();
    if actual_ports.is_empty() {
        return None;
    }

    // No mismatch if any mapped container port matches
    if mapped_container_ports.iter().any(|mp| actual_ports.contains(mp)) {
        return None;
    }

    let actual = actual_ports.first()?;
    let mapped = mapped_container_ports.first()?;

    // Host port from group 1
    let host_port = RE_DOCKER_PORT_MAP.captures(command)
        .and_then(|cap| cap.get(1)?.as_str().parse::<u16>().ok())
        .unwrap_or(*actual);

    Some(format!(
        "\n\nWARNING: Port mismatch detected! \
         You mapped host port {} to container port {}, but the app inside is listening on port {}. \
         The correct command is: docker run -p {}:{} <image>. \
         Stop the current container and re-run with the correct port mapping.",
        host_port, mapped, actual, host_port, actual
    ))
}

/// Check if a TCP port is available on localhost.
/// Checks both IPv4 (127.0.0.1) and IPv6 ([::1]) because Node.js/Vite
/// on Windows often binds to IPv6 only.
fn is_port_available(port: u16) -> bool {
    use std::net::{TcpStream, SocketAddr, IpAddr, Ipv4Addr, Ipv6Addr};
    use std::time::Duration;

    let timeout = Duration::from_millis(300);

    // IPv4
    let v4 = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    if TcpStream::connect_timeout(&v4, timeout).is_ok() {
        return false;
    }

    // IPv6 — needed on Windows where Node.js/Vite binds to [::1]
    let v6 = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), port);
    if TcpStream::connect_timeout(&v6, timeout).is_ok() {
        return false;
    }

    true
}

/// Find available ports near the requested one.
fn suggest_available_ports(near: u16, count: usize) -> Vec<u16> {
    let mut found = Vec::new();
    let mut port = near.saturating_add(1);
    while found.len() < count && port < 65535 {
        if is_port_available(port) {
            found.push(port);
        }
        port += 1;
    }
    found
}

/// Pre-flight port availability check. Returns a BLOCKED result if any ports are busy.
async fn preflight_port_check(command: &str) -> Option<ToolExecutionResult> {
    let ports = extract_ports_from_command(command);
    if ports.is_empty() {
        return None;
    }

    let mut busy: Vec<u16> = ports.iter().copied()
        .filter(|p| !is_port_available(*p))
        .collect();
    busy.sort();
    busy.dedup();

    if busy.is_empty() {
        return None;
    }

    // Detect Docker containers occupying the busy ports
    let docker_hint = detect_docker_on_ports(&busy).await;

    let suggestions: Vec<u16> = busy.iter()
        .flat_map(|&p| suggest_available_ports(p, 3))
        .collect();
    tracing::warn!(
        command = %command,
        busy_ports = ?busy,
        "Blocked command — port(s) already in use"
    );

    let mut msg = format!(
        "BLOCKED: port(s) {} already in use. Command was NOT executed.",
        busy.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", "),
    );

    if let Some(hint) = docker_hint {
        msg.push_str(&format!("\n\n{}", hint));
        msg.push_str("\n\nAction required: Stop the old container(s) first with `run_terminal_command`, \
            then retry with `run_app`. Example: `docker rm -f <container_id>`");
    } else {
        msg.push_str(&format!(
            "\nAvailable alternatives: {}.\n\
             You MUST use `ask_user` to ask the user which port they prefer before retrying.",
            suggestions.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", "),
        ));
    }

    Some(ToolExecutionResult {
        success: false,
        output: msg,
        baseline: None,
    })
}

fn execute_run_command(
    args: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let command = args["command"]
        .as_str()
        .ok_or_else(|| VenoreError::ToolExecutionFailed("missing 'command' argument".into()))?;
    let terminal_id = resolve_terminal_id(ctx)?;

    let manager = TerminalSessionManager::global();
    let mgr = manager.lock()
        .map_err(|_| VenoreError::TerminalError("Terminal is unavailable due to an internal error. Try restarting the app.".into()))?;

    tracing::info!(terminal_id = %terminal_id, command = %command, "AI executing terminal command");

    // Snapshot the line counter BEFORE writing — used as baseline for reading new output
    let baseline = mgr.line_counter(terminal_id);

    // Write command + Enter to terminal (visible in xterm)
    // PTY on Windows expects \r for Enter (same as xterm.js sends)
    let cmd_with_enter = format!("{}\r", command);
    mgr.write(terminal_id, cmd_with_enter.as_bytes())?;

    Ok(ToolExecutionResult {
        success: true,
        output: format!("Command sent to terminal: {}", command),
        baseline: Some(baseline),
    })
}

/// Wait for a port to become busy (i.e. an app is listening), polling every 500ms.
async fn wait_for_port_listen(port: u16, timeout_secs: u64) -> bool {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    loop {
        if !is_port_available(port) {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}

/// Wait for terminal output to stabilize (no new lines for 500ms), up to max_secs.
pub async fn wait_for_output(terminal_id: &str, baseline: u64, max_secs: u64) {
    let poll = std::time::Duration::from_millis(200);
    let stability = std::time::Duration::from_millis(500);
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(max_secs);

    let mut last_lines = {
        let mgr = TerminalSessionManager::global();
        mgr.lock().map(|m| m.line_counter(terminal_id).saturating_sub(baseline)).unwrap_or(0)
    };
    let mut stable_since = tokio::time::Instant::now();

    loop {
        tokio::time::sleep(poll).await;
        if tokio::time::Instant::now() >= deadline {
            break;
        }
        let current = {
            let mgr = TerminalSessionManager::global();
            mgr.lock().map(|m| m.line_counter(terminal_id).saturating_sub(baseline)).unwrap_or(0)
        };
        if current != last_lines {
            last_lines = current;
            stable_since = tokio::time::Instant::now();
        } else if stable_since.elapsed() >= stability {
            break;
        }
    }
}

async fn execute_run_app(
    args: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    // 1. Parse arguments
    let command = args["command"]
        .as_str()
        .ok_or_else(|| VenoreError::ToolExecutionFailed("missing 'command' argument".into()))?;
    let terminal_id = resolve_terminal_id(ctx)?;

    let auto_ports = extract_ports_from_command(command);
    let port: Option<u16> = args["port"]
        .as_u64()
        .map(|p| p as u16)
        .or_else(|| auto_ports.first().copied());
    let timeout_secs = args["wait_timeout_secs"]
        .as_u64()
        .unwrap_or(15)
        .min(60);

    let is_docker_detached = command.contains("docker")
        && command.contains("run")
        && command.contains("-d");

    // 2. Pre-flight port check
    if let Some(blocked) = preflight_port_check(command).await {
        return Ok(blocked);
    }

    // 3. Execute command in PTY
    let baseline = {
        let manager = TerminalSessionManager::global();
        let mgr = manager.lock()
            .map_err(|_| VenoreError::TerminalError("Terminal is unavailable".into()))?;
        let bl = mgr.line_counter(terminal_id);
        let cmd_with_enter = format!("{}\r", command);
        mgr.write(terminal_id, cmd_with_enter.as_bytes())?;
        tracing::info!(terminal_id = %terminal_id, command = %command, port = ?port, "run_app: command sent");
        bl
    };

    // 4. Wait for startup
    if is_docker_detached {
        // Docker detached: short wait for container ID output
        wait_for_output(terminal_id, baseline, 3).await;
    } else if let Some(p) = port {
        // Foreground process: wait for port to accept connections
        let listening = wait_for_port_listen(p, timeout_secs).await;
        if !listening {
            // Fallback: read whatever output we got
            wait_for_output(terminal_id, baseline, 2).await;
        }
    } else {
        // No port: fallback to output stability
        wait_for_output(terminal_id, baseline, timeout_secs).await;
    }

    // Read terminal output
    let terminal_output = {
        let manager = TerminalSessionManager::global();
        let mgr = manager.lock()
            .map_err(|_| VenoreError::TerminalError("Terminal is unavailable".into()))?;
        mgr.get_output_after(terminal_id, baseline, 30).unwrap_or_default()
    };

    // 5. Docker health check
    let mut warnings = Vec::new();
    let mut status = "RUNNING";

    if is_docker_detached {
        if let Some(health_warning) = check_docker_container_health(command, &terminal_output).await {
            status = "FAILED";
            warnings.push(health_warning);
        } else {
            // Container alive — check port mismatch
            let logs = fetch_docker_logs_if_detached(command, &terminal_output).await
                .unwrap_or_else(|| terminal_output.clone());
            if let Some(mismatch) = check_port_mismatch(command, &logs) {
                warnings.push(mismatch);
            }
        }
    }

    // 6. Check if port is actually listening (for non-docker or post-docker)
    if status == "RUNNING" {
        if let Some(p) = port {
            if is_port_available(p) {
                status = "FAILED";
                warnings.push(format!(
                    "Port {} is not listening after {}s — the app may have failed to start.",
                    p, timeout_secs
                ));
            }
        }
    }

    // 7. Build structured output
    let mut output = format!("Status: {}\nCommand: {}", status, command);
    if let Some(p) = port {
        output.push_str(&format!("\nURL: http://localhost:{}", p));
    }
    for w in &warnings {
        output.push_str(&format!("\n{}", w.trim()));
    }
    if status == "RUNNING" {
        output.push_str("\n\nIMPORTANT: The process is listening but NOT verified. \
            You MUST call check_health now to confirm the app responds correctly. \
            Do NOT tell the user the app is ready yet.");
    }
    // For failed docker containers, include warnings (logs already fetched in health check)
    if status == "FAILED" && is_docker_detached && warnings.is_empty() {
        output.push_str("\n\nContainer failed — check terminal output below for details.");
    }
    output.push_str(&format!("\n\nTerminal output (last 30 lines):\n{}", terminal_output));

    tracing::info!(
        command = %command,
        status,
        port = ?port,
        "run_app completed"
    );

    Ok(ToolExecutionResult {
        success: status == "RUNNING",
        output,
        baseline: None,
    })
}

fn execute_read_output(
    args: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let terminal_id = resolve_terminal_id(ctx)?;
    let lines = args["lines"].as_u64().unwrap_or(50) as usize;

    let manager = TerminalSessionManager::global();
    let mgr = manager.lock()
        .map_err(|_| VenoreError::TerminalError("Terminal is unavailable due to an internal error. Try restarting the app.".into()))?;
    let output = mgr.get_recent_output(terminal_id, lines)?;

    Ok(ToolExecutionResult {
        success: true,
        output,
        baseline: None,
    })
}

// ============================================================================
// HEALTH CHECK TOOL
// ============================================================================

async fn execute_check_health(
    args: &serde_json::Value,
) -> Result<ToolExecutionResult> {
    let url = args["url"]
        .as_str()
        .ok_or_else(|| VenoreError::ToolExecutionFailed("missing 'url' argument".into()))?;
    let expected_status = args["expected_status"].as_u64().map(|s| s as u16);
    let expected_content = args["expected_content"].as_str();
    let retries = args["retries"].as_u64().unwrap_or(3) as u32;
    let timeout_secs = args["timeout_secs"].as_u64().unwrap_or(5);

    // Validate URL
    url::Url::parse(url).map_err(|e| {
        VenoreError::ToolExecutionFailed(format!("Invalid URL '{}': {}", url, e))
    })?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| VenoreError::ToolExecutionFailed(format!("HTTP client error: {}", e)))?;

    // Retry loop
    let mut last_error = String::new();
    for attempt in 0..retries {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }

        let resp = match client.get(url).send().await {
            Ok(r) => r,
            Err(e) => {
                last_error = if e.is_timeout() {
                    format!("Timeout after {}s", timeout_secs)
                } else if e.is_connect() {
                    format!("Connection refused ({})", e)
                } else {
                    format!("{}", e)
                };
                continue;
            }
        };

        let status_code = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();

        // Truncated preview for the AI
        let preview = if body.len() > 500 {
            format!("{}...", &body[..body.floor_char_boundary(500)])
        } else {
            body.clone()
        };

        // Check status code
        let status_ok = match expected_status {
            Some(expected) => status_code == expected,
            None => status_code < 500,
        };

        // Check content
        let content_ok = match expected_content {
            Some(expected) => body.contains(expected),
            None => true,
        };

        let healthy = status_ok && content_ok;

        let mut output = format!(
            "Status: {}\nURL: {}\nHTTP: {}",
            if healthy { "HEALTHY" } else { "UNHEALTHY" },
            url,
            status_code,
        );

        if let Some(expected) = expected_status {
            if !status_ok {
                output.push_str(&format!("\nExpected HTTP {}, got {}", expected, status_code));
            }
        }

        if let Some(expected) = expected_content {
            if content_ok {
                output.push_str(&format!("\nContent check: PASS (\"{}\" found)", expected));
            } else {
                output.push_str(&format!("\nContent check: FAIL (\"{}\" not found in response)", expected));
            }
        }

        output.push_str(&format!("\n\nResponse preview:\n{}", preview));

        if !healthy {
            output.push_str("\n\nThe app is responding but NOT correctly. \
                Read the response preview above to understand what's wrong. \
                If the response is empty or connection was reset, the app may be binding to 127.0.0.1 instead of 0.0.0.0 \
                (common in Docker — fix with --host 0.0.0.0 in Vite/Next.js).");
        }

        tracing::info!(
            url = %url,
            status_code,
            healthy,
            "check_health completed"
        );

        return Ok(ToolExecutionResult {
            success: healthy,
            output,
            baseline: None,
        });
    }

    // All retries exhausted — never got a response
    let output = format!(
        "Status: UNHEALTHY\nURL: {}\nHTTP: no response after {} retries\nError: {}\n\n\
         The app is NOT responding at all. Possible causes:\n\
         - The process crashed (check terminal output with read_terminal_output)\n\
         - Wrong port (the app may be listening on a different port)\n\
         - Docker: the app binds to 127.0.0.1 instead of 0.0.0.0 (fix with --host 0.0.0.0)",
        url, retries, last_error,
    );

    tracing::warn!(url = %url, error = %last_error, "check_health: all retries failed");

    Ok(ToolExecutionResult {
        success: false,
        output,
        baseline: None,
    })
}

// ============================================================================
// FILE TOOLS
// ============================================================================

fn execute_read_file(
    args: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let file_path = args["file_path"]
        .as_str()
        .ok_or_else(|| VenoreError::ToolExecutionFailed("missing 'file_path' argument".into()))?;

    let offset = args["offset"].as_u64().unwrap_or(0) as usize;
    let limit = args["limit"].as_u64().unwrap_or(2000) as usize;

    let path = resolve_path(file_path, ctx);
    let display_path = path.display().to_string();

    if !path.exists() {
        return Err(VenoreError::FileNotFound(display_path));
    }
    if !path.is_file() {
        return Err(VenoreError::ToolExecutionFailed(format!(
            "'{}' is not a file. Use list_files for directories.", display_path
        )));
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|e| VenoreError::FileReadError(format!("{}: {}", display_path, e)))?;

    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    let start = offset.min(total_lines);
    let end = (start + limit).min(total_lines);
    let selected = &lines[start..end];

    let mut output = String::new();
    for (i, line) in selected.iter().enumerate() {
        let line_num = start + i + 1; // 1-based
        output.push_str(&format!("{:>4}| {}\n", line_num, line));
    }

    // Cap output at 100k chars
    if output.len() > 100_000 {
        output.truncate(output.floor_char_boundary(100_000));
        output.push_str("\n... (output truncated at 100,000 characters)");
    }

    if end < total_lines {
        output.push_str(&format!(
            "\n(showing lines {}-{} of {}. Use offset/limit to see more.)",
            start + 1, end, total_lines
        ));
    }

    tracing::info!(path = %display_path, lines = selected.len(), "AI read file");

    Ok(ToolExecutionResult {
        success: true,
        output,
        baseline: None,
    })
}

fn execute_write_file(
    args: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let file_path = args["file_path"]
        .as_str()
        .ok_or_else(|| VenoreError::ToolExecutionFailed("missing 'file_path' argument".into()))?;
    let content = args["content"]
        .as_str()
        .ok_or_else(|| VenoreError::ToolExecutionFailed("missing 'content' argument".into()))?;

    let path = resolve_path(file_path, ctx);
    let display_path = path.display().to_string();

    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .map_err(|e| VenoreError::FileWriteError(format!(
                    "failed to create directories for {}: {}", display_path, e
                )))?;
        }
    }

    let bytes = content.len();
    std::fs::write(&path, content)
        .map_err(|e| VenoreError::FileWriteError(format!("{}: {}", display_path, e)))?;

    tracing::info!(path = %display_path, bytes, "AI wrote file");

    Ok(ToolExecutionResult {
        success: true,
        output: format!("Wrote {} bytes to {}", bytes, display_path),
        baseline: None,
    })
}

fn execute_edit_file(
    args: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let file_path = args["file_path"]
        .as_str()
        .ok_or_else(|| VenoreError::ToolExecutionFailed("missing 'file_path' argument".into()))?;
    let old_string = args["old_string"]
        .as_str()
        .ok_or_else(|| VenoreError::ToolExecutionFailed("missing 'old_string' argument".into()))?;
    let new_string = args["new_string"]
        .as_str()
        .ok_or_else(|| VenoreError::ToolExecutionFailed("missing 'new_string' argument".into()))?;
    let replace_all = args["replace_all"].as_bool().unwrap_or(false);

    let path = resolve_path(file_path, ctx);
    let display_path = path.display().to_string();

    if !path.exists() {
        return Err(VenoreError::FileNotFound(display_path));
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|e| VenoreError::FileReadError(format!("{}: {}", display_path, e)))?;

    let (new_content, strategy) = if replace_all {
        let matches = fuzzy_match::fuzzy_find_all(&content, old_string);
        if matches.is_empty() {
            return Err(VenoreError::ToolExecutionFailed(format!(
                "old_string not found in {}", display_path
            )));
        }
        let strategy = matches[0].strategy;
        // Apply replacements from end to start to preserve byte offsets
        let mut result = content.clone();
        for m in matches.iter().rev() {
            result.replace_range(m.start..m.end, new_string);
        }
        (result, strategy)
    } else {
        // Use fuzzy matching
        match fuzzy_match::fuzzy_find(&content, old_string) {
            Some(m) => {
                let mut new = String::with_capacity(content.len());
                new.push_str(&content[..m.start]);
                new.push_str(new_string);
                new.push_str(&content[m.end..]);
                (new, m.strategy)
            }
            None => {
                return Err(VenoreError::ToolExecutionFailed(format!(
                    "Could not find the specified text in {}. The file contents may have changed — try reading it again.",
                    display_path
                )));
            }
        }
    };

    // Count changed lines
    let old_line_count = content.lines().count();
    let new_line_count = new_content.lines().count();
    let diff = new_line_count.abs_diff(old_line_count);

    std::fs::write(&path, &new_content)
        .map_err(|e| VenoreError::FileWriteError(format!("{}: {}", display_path, e)))?;

    tracing::info!(
        path = %display_path,
        strategy,
        line_diff = diff,
        "AI edited file"
    );

    Ok(ToolExecutionResult {
        success: true,
        output: format!(
            "Edited {} (matched via {}). {} lines changed.",
            display_path, strategy, diff
        ),
        baseline: None,
    })
}

fn execute_multi_edit_file(
    args: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let file_path = args["file_path"]
        .as_str()
        .ok_or_else(|| VenoreError::ToolExecutionFailed("missing 'file_path' argument".into()))?;
    let edits = args["edits"]
        .as_array()
        .ok_or_else(|| VenoreError::ToolExecutionFailed("missing 'edits' argument (expected array)".into()))?;

    if edits.is_empty() {
        return Err(VenoreError::ToolExecutionFailed("'edits' array is empty".into()));
    }

    let path = resolve_path(file_path, ctx);
    let display_path = path.display().to_string();

    if !path.exists() {
        return Err(VenoreError::FileNotFound(display_path));
    }

    let mut content = std::fs::read_to_string(&path)
        .map_err(|e| VenoreError::FileReadError(format!("{}: {}", display_path, e)))?;

    let total = edits.len();
    let mut applied = 0usize;
    let mut failed = 0usize;
    let mut details = Vec::new();

    for (i, edit) in edits.iter().enumerate() {
        let old_string = match edit["old_string"].as_str() {
            Some(s) => s,
            None => {
                failed += 1;
                details.push(format!("Edit {}: FAILED — missing 'old_string'", i + 1));
                continue;
            }
        };
        let new_string = match edit["new_string"].as_str() {
            Some(s) => s,
            None => {
                failed += 1;
                details.push(format!("Edit {}: FAILED — missing 'new_string'", i + 1));
                continue;
            }
        };

        match fuzzy_match::fuzzy_find(&content, old_string) {
            Some(m) => {
                let mut new_content = String::with_capacity(content.len());
                new_content.push_str(&content[..m.start]);
                new_content.push_str(new_string);
                new_content.push_str(&content[m.end..]);
                content = new_content;
                applied += 1;
                details.push(format!("Edit {}: OK ({})", i + 1, m.strategy));
            }
            None => {
                failed += 1;
                let preview = if old_string.len() > 40 {
                    format!("{}...", &old_string[..old_string.floor_char_boundary(40)])
                } else {
                    old_string.to_string()
                };
                details.push(format!("Edit {}: FAILED — not found: \"{}\"", i + 1, preview));
            }
        }
    }

    if applied == 0 {
        return Err(VenoreError::ToolExecutionFailed(format!(
            "All {} edits failed in {}:\n{}",
            total, display_path, details.join("\n")
        )));
    }

    std::fs::write(&path, &content)
        .map_err(|e| VenoreError::FileWriteError(format!("{}: {}", display_path, e)))?;

    tracing::info!(
        path = %display_path,
        applied,
        failed,
        total,
        "AI multi-edited file"
    );

    let output = format!(
        "Multi-edit {}: {}/{} edits applied.\n{}",
        display_path, applied, total, details.join("\n")
    );

    Ok(ToolExecutionResult {
        success: failed == 0,
        output,
        baseline: None,
    })
}

fn execute_list_files(
    args: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let dir_path = args["path"]
        .as_str()
        .ok_or_else(|| VenoreError::ToolExecutionFailed("missing 'path' argument".into()))?;
    let pattern = args["pattern"].as_str();

    let base = resolve_path(dir_path, ctx);
    let display_base = base.display().to_string();

    if !base.exists() {
        return Err(VenoreError::DirectoryNotFound(display_base));
    }
    if !base.is_dir() {
        return Err(VenoreError::ToolExecutionFailed(format!(
            "'{}' is not a directory", display_base
        )));
    }

    const MAX_ENTRIES: usize = 500;

    let mut files: Vec<String> = Vec::new();

    if let Some(pat) = pattern {
        // Use glob pattern relative to base directory
        let glob_pattern = base.join(pat).display().to_string();
        // Normalize path separators for glob (it expects forward slashes)
        let glob_pattern = glob_pattern.replace('\\', "/");

        match glob::glob(&glob_pattern) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    if files.len() >= MAX_ENTRIES {
                        break;
                    }
                    // Skip entries inside ignored directories
                    let entry_str = entry.display().to_string();
                    if SKIP_DIRS.iter().any(|d| entry_str.contains(&format!("{}{}",  std::path::MAIN_SEPARATOR, d)) || entry_str.contains(&format!("{}/", d))) {
                        continue;
                    }
                    if let Ok(rel) = entry.strip_prefix(&base) {
                        files.push(rel.display().to_string());
                    } else {
                        files.push(entry.display().to_string());
                    }
                }
            }
            Err(e) => {
                return Err(VenoreError::ToolExecutionFailed(format!(
                    "invalid glob pattern '{}': {}", pat, e
                )));
            }
        }
    } else {
        // Recursive walk up to depth 3
        walk_dir(&base, &base, 0, 3, SKIP_DIRS, &mut files, MAX_ENTRIES);
    }

    files.sort();

    let total = files.len();
    let output = if files.is_empty() {
        format!("No files found in {}", display_base)
    } else {
        let mut out = files.join("\n");
        if total >= MAX_ENTRIES {
            out.push_str(&format!("\n\n(showing first {} entries. Use a glob pattern to filter.)", MAX_ENTRIES));
        }
        out
    };

    tracing::info!(path = %display_base, count = total, "AI listed files");

    Ok(ToolExecutionResult {
        success: true,
        output,
        baseline: None,
    })
}

// ============================================================================
// SEARCH TOOLS
// ============================================================================

async fn execute_search_code(
    args: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let query = args["query"]
        .as_str()
        .ok_or_else(|| VenoreError::ToolExecutionFailed("missing 'query' argument".into()))?;
    let max_results = args["max_results"].as_u64().unwrap_or(10) as u32;

    let rag_repo = ctx.rag_repository.as_ref()
        .ok_or_else(|| VenoreError::ToolExecutionFailed(
            "Code search is not available — the project has not been indexed yet. Run indexing first.".into()
        ))?;
    let project_id = ctx.project_id.as_deref()
        .ok_or_else(|| VenoreError::ToolExecutionFailed(
            "No project context available for code search.".into()
        ))?;

    let results = crate::rag::search_code_hybrid(
        rag_repo,
        project_id,
        query,
        max_results,
        8000,
        ctx.embedding_provider.as_deref(),
        ctx.embedding_api_key.as_deref(),
    ).await?;

    if results.is_empty() {
        return Ok(ToolExecutionResult {
            success: true,
            output: format!("No results found for '{}'", query),
            baseline: None,
        });
    }

    let mut output = format!("Found {} results for '{}':\n\n", results.len(), query);
    for (i, result) in results.iter().enumerate() {
        let chunk = &result.chunk;
        output.push_str(&format!(
            "--- Result {} ---\n{} ({})\n{} · lines {}-{}\n{}\n\n",
            i + 1,
            chunk.name,
            chunk.chunk_type,
            chunk.relative_path,
            chunk.line_start,
            chunk.line_end,
            chunk.content,
        ));
    }

    tracing::info!(query = %query, results = results.len(), "AI searched code");

    Ok(ToolExecutionResult {
        success: true,
        output,
        baseline: None,
    })
}

fn execute_search_text(
    args: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let pattern = args["pattern"]
        .as_str()
        .ok_or_else(|| VenoreError::ToolExecutionFailed("missing 'pattern' argument".into()))?;
    let sub_path = args["path"].as_str();
    let file_pattern = args["file_pattern"].as_str();
    let case_sensitive = args["case_sensitive"].as_bool().unwrap_or(false);
    let max_results = args["max_results"].as_u64().unwrap_or(50) as usize;

    // Resolve search root
    let search_root = if let Some(p) = sub_path {
        resolve_path(p, ctx)
    } else if let Some(ref proj) = ctx.project_path {
        PathBuf::from(proj)
    } else {
        return Err(VenoreError::ToolExecutionFailed(
            "No project path available — cannot search.".into(),
        ));
    };

    if !search_root.exists() || !search_root.is_dir() {
        return Err(VenoreError::DirectoryNotFound(
            search_root.display().to_string(),
        ));
    }

    // Compile regex
    let regex = RegexBuilder::new(pattern)
        .case_insensitive(!case_sensitive)
        .build()
        .map_err(|e| {
            VenoreError::ToolExecutionFailed(format!("Invalid search pattern '{}': {}", pattern, e))
        })?;

    // Compile optional file glob filter
    let file_glob = match file_pattern {
        Some(fp) => Some(glob::Pattern::new(fp).map_err(|e| {
            VenoreError::ToolExecutionFailed(format!("Invalid file_pattern '{}': {}", fp, e))
        })?),
        None => None,
    };

    const MAX_FILE_SIZE: u64 = 500_000; // 500KB
    const MAX_OUTPUT_CHARS: usize = 100_000;

    let mut matches: Vec<String> = Vec::new();
    let mut files_with_matches: usize = 0;
    let mut current_file_has_match: bool;

    let project_root = ctx
        .project_path
        .as_deref()
        .map(Path::new)
        .unwrap_or(&search_root);

    for entry in WalkDir::new(&search_root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            // Skip hidden dirs/files and SKIP_DIRS
            if name.starts_with('.') {
                return false;
            }
            if e.file_type().is_dir() && SKIP_DIRS.contains(&name.as_ref()) {
                return false;
            }
            true
        })
    {
        if matches.len() >= max_results {
            break;
        }

        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().is_file() {
            continue;
        }

        // Skip files exceeding size limit
        if let Ok(meta) = entry.metadata() {
            if meta.len() > MAX_FILE_SIZE {
                continue;
            }
        }

        let path = entry.path();

        // Apply file glob filter
        if let Some(ref fg) = file_glob {
            let file_name = path
                .file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default();
            if !fg.matches(&file_name) {
                continue;
            }
        }

        // Try to read as text — skip binary files
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue, // binary or unreadable
        };

        current_file_has_match = false;

        let rel_path = path
            .strip_prefix(project_root)
            .unwrap_or(path)
            .display()
            .to_string()
            .replace('\\', "/");

        for (line_idx, line) in content.lines().enumerate() {
            if matches.len() >= max_results {
                break;
            }
            if regex.is_match(line) {
                if !current_file_has_match {
                    files_with_matches += 1;
                    current_file_has_match = true;
                }
                // Truncate very long lines
                let display_line = if line.len() > 200 {
                    format!("{}...", &line[..line.floor_char_boundary(200)])
                } else {
                    line.to_string()
                };
                matches.push(format!(
                    "{}:{}:{}",
                    rel_path,
                    line_idx + 1,
                    display_line
                ));
            }
        }
    }

    let output = if matches.is_empty() {
        format!("No matches found for '{}'", pattern)
    } else {
        let mut out = format!(
            "Found {} matches for '{}' in {} files:\n\n",
            matches.len(),
            pattern,
            files_with_matches
        );
        for m in &matches {
            out.push_str(m);
            out.push('\n');
        }
        if out.len() > MAX_OUTPUT_CHARS {
            out.truncate(MAX_OUTPUT_CHARS);
            out.push_str("\n... (output truncated)");
        }
        if matches.len() >= max_results {
            out.push_str(&format!(
                "\n(results capped at {}. Use path or file_pattern to narrow scope.)",
                max_results
            ));
        }
        out
    };

    tracing::info!(
        pattern = %pattern,
        matches = matches.len(),
        files = files_with_matches,
        "AI searched text"
    );

    Ok(ToolExecutionResult {
        success: true,
        output,
        baseline: None,
    })
}

// ============================================================================
// WEB TOOLS
// ============================================================================

async fn execute_web_fetch(
    args: &serde_json::Value,
) -> Result<ToolExecutionResult> {
    let url = args["url"]
        .as_str()
        .ok_or_else(|| VenoreError::ToolExecutionFailed("missing 'url' argument".into()))?;
    let max_chars = args["max_chars"].as_u64().unwrap_or(50_000) as usize;

    // Validate URL
    let parsed_url = url::Url::parse(url).map_err(|e| {
        VenoreError::ToolExecutionFailed(format!("Invalid URL '{}': {}", url, e))
    })?;

    let scheme = parsed_url.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(VenoreError::ToolExecutionFailed(format!(
            "Only http/https URLs are supported, got '{}'", scheme
        )));
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (compatible; VenoreBot/1.0)")
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|e| VenoreError::ToolExecutionFailed(format!("HTTP client error: {}", e)))?;

    let response = client.get(url).send().await.map_err(|e| {
        if e.is_timeout() {
            VenoreError::ToolExecutionFailed(format!("Request timed out after 15s: {}", url))
        } else if e.is_connect() {
            VenoreError::ToolExecutionFailed(format!("Could not connect to {}: {}", url, e))
        } else {
            VenoreError::ToolExecutionFailed(format!("HTTP request failed: {}", e))
        }
    })?;

    let status = response.status();
    if !status.is_success() {
        return Ok(ToolExecutionResult {
            success: false,
            output: format!("HTTP {} for {}", status.as_u16(), url),
            baseline: None,
        });
    }

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();

    let body = response.text().await.map_err(|e| {
        VenoreError::ToolExecutionFailed(format!("Failed to read response body: {}", e))
    })?;

    let mut output = if content_type.contains("text/html") {
        html2text::from_read(body.as_bytes(), 80)
    } else {
        body
    };

    if output.len() > max_chars {
        output.truncate(output.floor_char_boundary(max_chars));
        output.push_str("\n\n... (content truncated)");
    }

    tracing::info!(url = %url, chars = output.len(), "AI fetched web page");

    Ok(ToolExecutionResult {
        success: true,
        output,
        baseline: None,
    })
}

async fn execute_web_search(
    args: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let query = args["query"]
        .as_str()
        .ok_or_else(|| VenoreError::ToolExecutionFailed("missing 'query' argument".into()))?;
    let max_results = args["max_results"].as_u64().unwrap_or(5) as usize;

    // Backend selection.
    //
    // Gemini ships native grounding (Google Search) on the server side and
    // it costs nothing extra for the user — no API key, no third-party.
    // When the active chat provider is Gemini we route through a
    // grounding-only sub-call (no function tools, just `web_search=true`)
    // so the mixed-tools restriction on 2.5 never bites us.
    //
    // Tavily stays only as a fallback for providers without native search
    // (Ollama and friends), and only if the user explicitly configured a
    // key. Other native search backends slot in here later.
    if let Some(gateway) = ctx.llm_gateway.clone() {
        use crate::traits::{LlmProviderType, LlmTask};
        let options = crate::llm::gateway::GatewayOptions::for_task(LlmTask::Chat);
        let (provider, model) = gateway.resolve_model(&options).await;
        if matches!(provider, LlmProviderType::Gemini) {
            return execute_web_search_gemini_grounding(&gateway, &model, query, options).await;
        }
    }

    let api_key = match &ctx.web_search_api_key {
        Some(key) => key.clone(),
        None => {
            return Ok(ToolExecutionResult {
                success: false,
                output: "Web search is not available — switch to a Gemini model (native grounding, no setup) or configure a Tavily API key in AI Configuration.".into(),
                baseline: None,
            });
        }
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| VenoreError::ToolExecutionFailed(format!("HTTP client error: {}", e)))?;

    let request_body = serde_json::json!({
        "api_key": api_key,
        "query": query,
        "max_results": max_results,
    });

    let response = client
        .post("https://api.tavily.com/search")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                VenoreError::ToolExecutionFailed("Tavily search timed out after 15s".into())
            } else {
                VenoreError::ToolExecutionFailed(format!("Tavily search failed: {}", e))
            }
        })?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        let body_truncated = if body.len() > 200 { &body[..body.floor_char_boundary(200)] } else { &body };
        return Ok(ToolExecutionResult {
            success: false,
            output: format!("Tavily API error ({}): {}", status.as_u16(), body_truncated),
            baseline: None,
        });
    }

    let data: serde_json::Value = response.json().await.map_err(|e| {
        VenoreError::ToolExecutionFailed(format!("Failed to parse Tavily response: {}", e))
    })?;

    let results = data["results"].as_array();
    let mut output = format!("Web search results for '{}':\n\n", query);

    if let Some(results) = results {
        if results.is_empty() {
            output.push_str("No results found.");
        } else {
            for (i, result) in results.iter().enumerate() {
                let title = result["title"].as_str().unwrap_or("(no title)");
                let url = result["url"].as_str().unwrap_or("");
                let content = result["content"].as_str().unwrap_or("(no snippet)");

                output.push_str(&format!(
                    "[{}] {}\n{}\n{}\n\n",
                    i + 1, title, url, content
                ));
            }
        }
    } else {
        output.push_str("No results returned.");
    }

    tracing::info!(query = %query, "AI searched web");

    Ok(ToolExecutionResult {
        success: true,
        output,
        baseline: None,
    })
}

/// Backend for `web_search` when the active chat provider is Gemini.
///
/// Fires a grounding-only sub-call: `web_search=true`, no function tools.
/// Because there are no `function_declarations`, the mixed-tools
/// restriction on Gemini 2.5 never kicks in, and 3.x is happy too. The
/// model returns a grounded answer; `sources` come from the response's
/// `groundingMetadata` and are appended to the tool output so the calling
/// agent can cite them.
async fn execute_web_search_gemini_grounding(
    gateway: &LlmGateway,
    model: &str,
    query: &str,
    options: crate::llm::gateway::GatewayOptions,
) -> Result<ToolExecutionResult> {
    use crate::llm::types::{LlmMessage, LlmRequest, MessageRole};

    let request = LlmRequest {
        model: model.to_string(),
        messages: vec![LlmMessage {
            role: MessageRole::User,
            content: query.to_string(),
            tool_call_id: None,
            tool_calls: None,
            content_parts: None,
        }],
        temperature: Some(0.3),
        max_tokens: Some(2000),
        tools: None,
        json_schema: None,
        timeout_secs: Some(30),
        web_search: true,
    };

    match gateway.complete(request, options).await {
        Ok(resp) => {
            let mut output = format!("Web search results for '{}':\n\n", query);
            output.push_str(&resp.content);
            if !resp.sources.is_empty() {
                output.push_str("\n\nSources:\n");
                for (i, s) in resp.sources.iter().enumerate() {
                    output.push_str(&format!("[{}] {} — {}\n", i + 1, s.title, s.uri));
                }
            }
            tracing::info!(
                query = %query,
                source_count = resp.sources.len(),
                "AI searched web (Gemini grounding)"
            );
            Ok(ToolExecutionResult {
                success: true,
                output,
                baseline: None,
            })
        }
        Err(e) => {
            tracing::warn!(error = %e, "Gemini grounding sub-call failed");
            Ok(ToolExecutionResult {
                success: false,
                output: format!("Web search failed: {}", e),
                baseline: None,
            })
        }
    }
}

// ============================================================================
// MESH TOOLS
// ============================================================================

/// Maximum number of follow-up questions a handler sub-agent can ask per request.
const MAX_FOLLOW_UPS: u32 = 5;

/// Timeout waiting for the caller to answer a follow-up question.
const FOLLOW_UP_ANSWER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

async fn execute_ask_project(
    arguments: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let project_param = arguments["project"]
        .as_str()
        .ok_or_else(|| VenoreError::InvalidParams("Missing 'project' parameter".into()))?;
    let question = arguments["question"]
        .as_str()
        .ok_or_else(|| VenoreError::InvalidParams("Missing 'question' parameter".into()))?;
    let context_hint = arguments["context_hint"].as_str();

    // Resolve project parameter: could be a project_id (UUID) or project_name.
    // Try exact match first, then fall back to name-based lookup.
    //
    // `from_project` (the caller name shown to the remote handler) is
    // resolved against the local mesh registrations. We try the caller's
    // `project_id` first; if that misses (e.g. knowledge projects whose
    // chat-side identity is derived differently than their mesh-side one)
    // we fall back to matching by `project_path` so the log says the
    // project name instead of "unknown".
    let (resolved_id, from_project) = {
        let mesh = MeshDiscovery::global();
        let guard = mesh.lock().map_err(|e| {
            VenoreError::MeshError(format!("Discovery mutex poisoned: {}", e))
        })?;

        let by_id = ctx
            .project_id
            .as_deref()
            .and_then(|id| guard.get_local_registration(id));
        let by_path = by_id.or_else(|| {
            ctx.project_path.as_deref().and_then(|p| {
                guard
                    .iter_local_registrations()
                    .find(|r| r.project_path == p)
            })
        });
        let from = by_path
            .map(|r| r.project_name.clone())
            .unwrap_or_else(|| "unknown".to_string());

        // If exact project_id match exists, use it directly
        let resolved = if guard.get_peer_registration(project_param).is_ok() {
            project_param.to_string()
        } else {
            // Fall back: search discovered peers by project_name (case-insensitive)
            let peers = guard.discover_peers().unwrap_or_default();
            let param_lower = project_param.to_lowercase();
            peers
                .iter()
                .find(|p| p.project_name.to_lowercase() == param_lower)
                .map(|p| p.project_id.clone())
                .ok_or_else(|| {
                    VenoreError::MeshPeerNotFound(format!(
                        "No peer found matching '{}'. Available: {}",
                        project_param,
                        peers.iter().map(|p| p.project_name.as_str()).collect::<Vec<_>>().join(", ")
                    ))
                })?
        };

        (resolved, from)
    };

    let project_id = &resolved_id;

    // Get or reuse a conversation ID for multi-turn context (Phase 4a)
    let conversation_id = get_or_create_conversation_id(project_id);

    // Phase 1: Lock transport → auto-connect + send request → release lock
    let (stream_id, mut rx) = {
        let transport = MeshTransport::global();
        let mut t = transport.lock().await;

        if !t.is_running() {
            return Err(VenoreError::MeshTransportNotRunning);
        }

        // Auto-connect if not already connected
        if !t.connected_peers().contains(&project_id.to_string()) {
            t.connect_to_peer(project_id).await?;
        }

        t.send_request(project_id, question, &from_project, context_hint, Some(&conversation_id))?
    }; // Transport lock released here

    tracing::info!(
        project = %project_id,
        question = %question,
        stream_id = %stream_id,
        "ask_project: waiting for response"
    );

    // Phase 2: Wait for messages (no lock held) — 240s total budget.
    // Raised from 120s to give the remote sub-agent room to actually use its
    // (now parity-sized) iteration budget on a deep question: a 120s ceiling
    // capped it at ~8 turns regardless of MAX_ITERATIONS. The remote peer may
    // also send follow-up questions (Phase 4b) before the final response.
    let total_timeout = std::time::Duration::from_secs(240);
    let started = tokio::time::Instant::now();

    loop {
        let remaining = total_timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            crate::mesh::remove_pending_response(&stream_id);
            return Ok(ToolExecutionResult {
                success: false,
                output: format!("Request to project '{}' timed out after 240s", project_id),
                baseline: None,
            });
        }

        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(CallerMessage::Response(result))) => {
                // Terminal message — return the response
                let content = result?;
                return Ok(ToolExecutionResult {
                    success: true,
                    output: content,
                    baseline: None,
                });
            }
            Ok(Some(CallerMessage::FollowUp { question: follow_up_q, round, stream_id: fu_stream_id })) => {
                // Non-terminal: the handler sub-agent needs clarification (Phase 4b)
                tracing::info!(
                    stream_id = %fu_stream_id,
                    round,
                    follow_up = %follow_up_q,
                    "ask_project: received follow-up question, generating answer"
                );

                let answer = answer_follow_up(ctx, &follow_up_q, question, project_id).await;

                // Send the answer back to the handler
                let answer_msg = crate::mesh::MeshMessage::AgentFollowUpAnswer {
                    stream_id: fu_stream_id,
                    answer,
                    round,
                };
                // Brief lock to send the answer
                let transport = MeshTransport::global();
                let t = transport.lock().await;
                if let Err(e) = t.send_to_peer(project_id, answer_msg) {
                    tracing::warn!(error = %e, "Failed to send follow-up answer");
                }
                // Continue loop — wait for the next message (more follow-ups or final response)
            }
            Ok(None) => {
                // Channel closed — peer disconnected
                return Ok(ToolExecutionResult {
                    success: false,
                    output: "Mesh response channel closed unexpectedly".to_string(),
                    baseline: None,
                });
            }
            Err(_) => {
                // Timeout
                crate::mesh::remove_pending_response(&stream_id);
                return Ok(ToolExecutionResult {
                    success: false,
                    output: format!("Request to project '{}' timed out after 240s", project_id),
                    baseline: None,
                });
            }
        }
    }
}

/// Generate an inline LLM answer for a follow-up question from a remote handler (Phase 4b).
///
/// Uses the caller's LLM gateway to produce a concise answer. Falls back to echoing
/// the original question if no gateway is available.
async fn answer_follow_up(
    ctx: &ToolExecutionContext,
    follow_up_question: &str,
    original_question: &str,
    target_project: &str,
) -> String {
    let gateway = match &ctx.llm_gateway {
        Some(gw) => gw,
        None => {
            // Fallback: no LLM configured, return the original question as context
            return format!(
                "I cannot generate a detailed answer (no LLM configured). My original question was: {}",
                original_question
            );
        }
    };

    use crate::llm::gateway::GatewayOptions;
    use crate::llm::types::{LlmMessage, LlmRequest, MessageRole};
    use crate::traits::LlmTask;

    let request = LlmRequest {
        model: String::new(), // resolved by gateway
        messages: vec![
            LlmMessage {
                role: MessageRole::System,
                content: format!(
                    "You are answering a clarifying question from an AI agent in the project \"{}\". \
                     Be concise and specific. Answer in 1-3 sentences.",
                    target_project
                ),
                tool_call_id: None,
                tool_calls: None,
                content_parts: None,
            },
            LlmMessage {
                role: MessageRole::User,
                content: format!(
                    "My original question to the project was: \"{}\"\n\nThe agent asks: \"{}\"",
                    original_question, follow_up_question
                ),
                tool_call_id: None,
                tool_calls: None,
                content_parts: None,
            },
        ],
        temperature: Some(0.3),
        max_tokens: Some(500),
        tools: None,
        json_schema: None,
        timeout_secs: Some(30),
        web_search: false,
    };

    let options = GatewayOptions::for_task(LlmTask::Chat);

    match gateway.complete(request, options).await {
        Ok(response) => response.content,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to generate follow-up answer via LLM");
            format!(
                "Could not generate a detailed answer. Original question: {}",
                original_question
            )
        }
    }
}

/// Execute `ask_caller` — handler sub-agent asks the requesting agent for clarification (Phase 4b).
async fn execute_ask_caller(
    arguments: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let question = arguments["question"]
        .as_str()
        .ok_or_else(|| VenoreError::InvalidParams("Missing 'question' parameter".into()))?;

    let handle = ctx.mesh_follow_up.as_ref().ok_or_else(|| {
        VenoreError::ToolExecutionFailed(
            "ask_caller is only available inside a mesh handler (responding to a remote request)".into(),
        )
    })?;

    // Check follow-up budget
    let round = handle.follow_up_count.fetch_add(1, Ordering::SeqCst) + 1;
    if round > MAX_FOLLOW_UPS {
        return Ok(ToolExecutionResult {
            success: false,
            output: format!(
                "Follow-up limit reached ({} questions). Answer with the information you already have.",
                MAX_FOLLOW_UPS
            ),
            baseline: None,
        });
    }

    // Create oneshot channel for the answer
    let (answer_tx, answer_rx) = tokio::sync::oneshot::channel::<String>();
    {
        let mut channels = handle.answer_channels.lock().unwrap_or_else(|e| e.into_inner());
        channels.insert((handle.stream_id.clone(), round), answer_tx);
    }

    // Send AgentFollowUp message to the caller via the write channel
    let follow_up_msg = crate::mesh::MeshMessage::AgentFollowUp {
        stream_id: handle.stream_id.clone(),
        question: question.to_string(),
        round,
    };
    let json = follow_up_msg.to_json()?;
    handle.write_tx.send(json).map_err(|e| {
        VenoreError::MeshConnectionFailed(format!("Failed to send follow-up: {}", e))
    })?;

    tracing::info!(
        stream_id = %handle.stream_id,
        round,
        question = %question,
        "ask_caller: sent follow-up, waiting for answer"
    );

    // Wait for the answer with timeout
    match tokio::time::timeout(FOLLOW_UP_ANSWER_TIMEOUT, answer_rx).await {
        Ok(Ok(answer)) => {
            tracing::info!(
                stream_id = %handle.stream_id,
                round,
                answer_len = answer.len(),
                "ask_caller: received answer"
            );
            Ok(ToolExecutionResult {
                success: true,
                output: answer,
                baseline: None,
            })
        }
        Ok(Err(_)) => {
            // Oneshot sender dropped — caller disconnected
            Ok(ToolExecutionResult {
                success: false,
                output: "The requesting agent disconnected before answering. Continue with the information you have.".to_string(),
                baseline: None,
            })
        }
        Err(_) => {
            // Timeout
            // Clean up the pending channel
            {
                let mut channels = handle.answer_channels.lock().unwrap_or_else(|e| e.into_inner());
                channels.remove(&(handle.stream_id.clone(), round));
            }
            Ok(ToolExecutionResult {
                success: false,
                output: format!(
                    "Follow-up answer timed out after {}s. Continue with the information you have.",
                    FOLLOW_UP_ANSWER_TIMEOUT.as_secs()
                ),
                baseline: None,
            })
        }
    }
}

// ============================================================================
// KNOWLEDGE TOOL HANDLERS
// ============================================================================

async fn execute_plan_hexagons(
    arguments: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let repo = ctx.knowledge_repo.as_ref()
        .ok_or_else(|| VenoreError::ToolNotFound("plan_hexagons requires knowledge context".into()))?;
    let feature_id = ctx.knowledge_feature_id.as_ref()
        .ok_or_else(|| VenoreError::ToolNotFound("plan_hexagons requires knowledge_feature_id".into()))?;

    let seed = arguments["seed"].as_str().unwrap_or("");
    let objective = arguments["objective"].as_str().unwrap_or("explore");
    let count = arguments["count"].as_i64().unwrap_or(5).min(12).max(1) as usize;

    if seed.is_empty() {
        return Ok(ToolExecutionResult {
            success: false,
            output: "Error: 'seed' parameter is required".to_string(),
            baseline: None,
        });
    }

    // Use LLM to decompose seed into research points
    let gateway = ctx.llm_gateway.as_ref()
        .ok_or_else(|| VenoreError::ToolNotFound("plan_hexagons requires LLM gateway".into()))?;

    let decompose_prompt = format!(
        "Decompose this research topic into exactly {} specific research points.\n\
        Topic: {}\n\
        Objective: {}\n\n\
        Return ONLY a JSON array of objects with \"title\" and \"description\" fields.\n\
        Example: [{{\"title\": \"Performance benchmarks\", \"description\": \"Compare throughput under load\"}}]\n\
        No markdown, no explanation, just the JSON array.",
        count, seed, objective
    );

    let request = crate::llm::types::LlmRequest {
        model: String::new(), // resolved by gateway
        messages: vec![crate::llm::types::LlmMessage {
            role: crate::llm::prelude::MessageRole::User,
            content: decompose_prompt,
            tool_call_id: None,
            tool_calls: None,
            content_parts: None,
        }],
        temperature: Some(0.7),
        max_tokens: Some(2000),
        tools: None,
        json_schema: None,
        timeout_secs: None,
        web_search: false,
    };

    let options = crate::llm::prelude::GatewayOptions::for_task(crate::llm::prelude::LlmTask::Chat);
    let response = gateway.complete(request, options).await
        .map_err(|e| VenoreError::LlmProviderError(format!("Failed to decompose seed: {}", e)))?;

    // Parse JSON array from response
    let text = response.content.trim();
    // Try to extract JSON from the response (might be wrapped in ```json ... ```)
    let json_text = if let Some(start) = text.find('[') {
        if let Some(end) = text.rfind(']') {
            &text[start..=end]
        } else {
            text
        }
    } else {
        text
    };

    let points: Vec<serde_json::Value> = serde_json::from_str(json_text)
        .unwrap_or_else(|_| {
            // Fallback: create generic hexagons
            (0..count).map(|i| serde_json::json!({
                "title": format!("Research point {}", i + 1),
                "description": format!("Investigate aspect {} of: {}", i + 1, seed)
            })).collect()
        });

    let now = chrono::Utc::now().to_rfc3339();
    let mut created = Vec::new();

    for point in points.iter().take(count) {
        let title = point["title"].as_str().unwrap_or("Untitled");
        let description = point["description"].as_str().unwrap_or("");
        let hex_id = uuid::Uuid::new_v4().to_string();

        let hexagon = crate::knowledge::KnowledgeHexagon {
            id: hex_id.clone(),
            feature_id: feature_id.clone(),
            title: title.to_string(),
            description: description.to_string(),
            phase: "discover".to_string(),
            percentage: 0,
            confidence: "low".to_string(),
            risk: "unknown".to_string(),
            priority: "medium".to_string(),
            is_dead_end: false,
            blocked_by: "[]".to_string(),
            notes_user: String::new(),
            agent_status: "idle".to_string(),
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        if let Err(e) = repo.create_hexagon(&hexagon).await {
            tracing::warn!("Failed to create hexagon '{}': {}", title, e);
            continue;
        }

        created.push(serde_json::json!({
            "id": hex_id,
            "title": title,
            "description": description,
        }));
    }

    Ok(ToolExecutionResult {
        success: true,
        output: serde_json::json!({
            "created": created.len(),
            "hexagons": created,
        }).to_string(),
        baseline: None,
    })
}

async fn execute_update_hexagon(
    arguments: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let repo = ctx.knowledge_repo.as_ref()
        .ok_or_else(|| VenoreError::ToolNotFound("update_hexagon requires knowledge context".into()))?;

    let hexagon_id = arguments["hexagon_id"].as_str().unwrap_or("");
    if hexagon_id.is_empty() {
        return Ok(ToolExecutionResult {
            success: false,
            output: "Error: 'hexagon_id' is required".to_string(),
            baseline: None,
        });
    }

    let mut hex = repo.get_hexagon(hexagon_id).await?
        .ok_or_else(|| VenoreError::NotFound(format!("Hexagon '{}' not found", hexagon_id)))?;

    if let Some(phase) = arguments["phase"].as_str() {
        hex.phase = phase.to_string();
    }
    if let Some(pct) = arguments["percentage"].as_i64() {
        hex.percentage = pct.clamp(0, 100) as i32;
    }
    if let Some(conf) = arguments["confidence"].as_str() {
        hex.confidence = conf.to_string();
    }
    if let Some(risk) = arguments["risk"].as_str() {
        hex.risk = risk.to_string();
    }
    if let Some(notes) = arguments["notes"].as_str() {
        if hex.notes_user.is_empty() {
            hex.notes_user = notes.to_string();
        } else {
            hex.notes_user = format!("{}\n\n{}", hex.notes_user, notes);
        }
    }
    hex.updated_at = chrono::Utc::now().to_rfc3339();

    repo.update_hexagon(&hex).await?;

    Ok(ToolExecutionResult {
        success: true,
        output: format!("Updated hexagon '{}': phase={}, {}%, confidence={}, risk={}",
            hex.title, hex.phase, hex.percentage, hex.confidence, hex.risk),
        baseline: None,
    })
}

async fn execute_add_evidence(
    arguments: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let repo = ctx.knowledge_repo.as_ref()
        .ok_or_else(|| VenoreError::ToolNotFound("add_evidence requires knowledge context".into()))?;

    let hexagon_id = arguments["hexagon_id"].as_str().unwrap_or("");
    let content = arguments["content"].as_str().unwrap_or("");

    if hexagon_id.is_empty() || content.is_empty() {
        return Ok(ToolExecutionResult {
            success: false,
            output: "Error: 'hexagon_id' and 'content' are required".to_string(),
            baseline: None,
        });
    }

    let evidence = crate::knowledge::KnowledgeEvidence {
        id: uuid::Uuid::new_v4().to_string(),
        hexagon_id: hexagon_id.to_string(),
        content: content.to_string(),
        source_url: arguments["source_url"].as_str().unwrap_or("").to_string(),
        source_type: arguments["source_type"].as_str().unwrap_or("manual").to_string(),
        confidence: arguments["confidence"].as_str().unwrap_or("medium").to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    repo.create_evidence(&evidence).await?;

    Ok(ToolExecutionResult {
        success: true,
        output: format!("Evidence added to hexagon '{}' (type: {}, confidence: {})",
            hexagon_id, evidence.source_type, evidence.confidence),
        baseline: None,
    })
}

async fn execute_mark_dead_end(
    arguments: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let repo = ctx.knowledge_repo.as_ref()
        .ok_or_else(|| VenoreError::ToolNotFound("mark_dead_end requires knowledge context".into()))?;

    let hexagon_id = arguments["hexagon_id"].as_str().unwrap_or("");
    let reason = arguments["reason"].as_str().unwrap_or("");

    if hexagon_id.is_empty() || reason.is_empty() {
        return Ok(ToolExecutionResult {
            success: false,
            output: "Error: 'hexagon_id' and 'reason' are required".to_string(),
            baseline: None,
        });
    }

    let mut hex = repo.get_hexagon(hexagon_id).await?
        .ok_or_else(|| VenoreError::NotFound(format!("Hexagon '{}' not found", hexagon_id)))?;

    hex.is_dead_end = true;
    hex.agent_status = "completed".to_string();
    if hex.notes_user.is_empty() {
        hex.notes_user = format!("DEAD END: {}", reason);
    } else {
        hex.notes_user = format!("{}\n\nDEAD END: {}", hex.notes_user, reason);
    }
    hex.updated_at = chrono::Utc::now().to_rfc3339();

    repo.update_hexagon(&hex).await?;

    Ok(ToolExecutionResult {
        success: true,
        output: format!("Hexagon '{}' marked as dead end: {}", hex.title, reason),
        baseline: None,
    })
}

async fn execute_generate_report(
    arguments: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let _ = arguments; // no params
    let repo = ctx.knowledge_repo.as_ref()
        .ok_or_else(|| VenoreError::ToolNotFound("generate_report requires knowledge context".into()))?;
    let feature_id = ctx.knowledge_feature_id.as_ref()
        .ok_or_else(|| VenoreError::ToolNotFound("generate_report requires knowledge_feature_id".into()))?;
    let gateway = ctx.llm_gateway.as_ref()
        .ok_or_else(|| VenoreError::ToolNotFound("generate_report requires LLM gateway".into()))?;

    let feature = repo.get_feature(feature_id).await?
        .ok_or_else(|| VenoreError::NotFound(format!("Feature '{}' not found", feature_id)))?;

    let hexagons = repo.list_hexagons_by_feature(feature_id).await?;
    if hexagons.is_empty() {
        return Ok(ToolExecutionResult {
            success: false,
            output: "No hexagons to report on. Use plan_hexagons first.".to_string(),
            baseline: None,
        });
    }

    // Build report prompt with all hexagons and evidence
    let mut hex_sections = String::new();
    for hex in &hexagons {
        hex_sections.push_str(&format!(
            "\n## {} ({})\n- Phase: {} | Progress: {}% | Confidence: {} | Risk: {}\n- Dead end: {}\n",
            hex.title, hex.id, hex.phase, hex.percentage, hex.confidence, hex.risk, hex.is_dead_end
        ));
        if !hex.notes_user.is_empty() {
            hex_sections.push_str(&format!("- Notes: {}\n", hex.notes_user));
        }
        let evidence = repo.list_evidence_by_hexagon(&hex.id).await.unwrap_or_default();
        if !evidence.is_empty() {
            hex_sections.push_str("### Evidence:\n");
            for ev in &evidence {
                hex_sections.push_str(&format!(
                    "- [{}] {} (source: {}, confidence: {})\n",
                    ev.source_type, ev.content, ev.source_url, ev.confidence
                ));
            }
        }
    }

    let report_prompt = format!(
        "Generate a comprehensive research report based on the following investigation.\n\n\
        # Research: {}\n\
        Description: {}\n\
        Objective: {}\n\
        Intensity: {}\n\
        {}\n\n\
        Write a structured report with:\n\
        1. Executive Summary\n\
        2. Key Findings (organized by hexagon)\n\
        3. Confidence Assessment\n\
        4. Dead Ends and What We Learned\n\
        5. Recommendations and Next Steps\n\n\
        Be factual and cite the evidence provided.",
        feature.name, feature.description, feature.objective, feature.intensity, hex_sections
    );

    let request = crate::llm::types::LlmRequest {
        model: String::new(),
        messages: vec![crate::llm::types::LlmMessage {
            role: crate::llm::prelude::MessageRole::User,
            content: report_prompt,
            tool_call_id: None,
            tool_calls: None,
            content_parts: None,
        }],
        temperature: Some(0.5),
        max_tokens: Some(4000),
        tools: None,
        json_schema: None,
        timeout_secs: None,
        web_search: false,
    };

    let options = crate::llm::prelude::GatewayOptions::for_task(crate::llm::prelude::LlmTask::Chat);
    let response = gateway.complete(request, options).await
        .map_err(|e| VenoreError::LlmProviderError(format!("Failed to generate report: {}", e)))?;

    Ok(ToolExecutionResult {
        success: true,
        output: response.content,
        baseline: None,
    })
}

/// Recursive directory walk with depth limit and skip list.
fn walk_dir(
    current: &Path,
    base: &Path,
    depth: usize,
    max_depth: usize,
    skip: &[&str],
    files: &mut Vec<String>,
    max_entries: usize,
) {
    if depth > max_depth || files.len() >= max_entries {
        return;
    }

    let entries = match std::fs::read_dir(current) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut sorted: Vec<_> = entries.flatten().collect();
    sorted.sort_by_key(|e| e.file_name());

    for entry in sorted {
        if files.len() >= max_entries {
            return;
        }

        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip hidden files/dirs and ignored directories
        if name_str.starts_with('.') {
            continue;
        }
        if path.is_dir() && skip.contains(&name_str.as_ref()) {
            continue;
        }

        if let Ok(rel) = path.strip_prefix(base) {
            files.push(rel.display().to_string());
        }

        if path.is_dir() {
            walk_dir(&path, base, depth + 1, max_depth, skip, files, max_entries);
        }
    }
}

// ============================================================================
// LOGBOOK TOOLS — read-only access to logbooks of the current project
// ============================================================================

/// `list_logbooks` — enumerate every knowledge_node + lighthouse so the AI
/// can answer "what logbooks do we have" without abusing search_logbook.
fn execute_list_logbooks(
    _arguments: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let project_path = match &ctx.project_path {
        Some(p) => p.clone(),
        None => {
            return Ok(ToolExecutionResult {
                success: false,
                output: "list_logbooks requires an active project context".to_string(),
                baseline: None,
            });
        }
    };

    use crate::ocean::NodeVariant;
    let layout = match crate::ocean::service::with_service(&project_path, |service| {
        service.get_layout()
    }) {
        Ok(l) => l,
        Err(e) => {
            return Ok(ToolExecutionResult {
                success: false,
                output: format!("Failed to read ocean state: {}", e),
                baseline: None,
            });
        }
    };

    // Build a name lookup for resolving lighthouse_id → name and counting
    // connections per node by iterating manual_connections once.
    let id_to_name: std::collections::HashMap<String, String> = layout
        .positions
        .iter()
        .map(|(id, e)| (id.clone(), e.module_name.clone()))
        .collect();
    let mut conn_in: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    let mut conn_out: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for c in &layout.manual_connections {
        *conn_out.entry(c.from_id.clone()).or_insert(0) += 1;
        *conn_in.entry(c.to_id.clone()).or_insert(0) += 1;
    }

    // (id, name, variant, isla_label, sections, conn_in, conn_out)
    type Row = (String, String, &'static str, String, usize, u32, u32);
    let mut rows: Vec<Row> = Vec::new();
    for (node_id, entry) in &layout.positions {
        let variant = match entry.node_variant {
            NodeVariant::KnowledgeNode => "knowledge_node",
            NodeVariant::Lighthouse => "lighthouse",
            // Module / Buoy / Cylinder represent code, not knowledge — skip
            // them when listing the project's knowledge graph.
            NodeVariant::Module | NodeVariant::Buoy | NodeVariant::Cylinder => continue,
        };
        let isla = match (&entry.lighthouse_id, &entry.node_variant) {
            (_, NodeVariant::Lighthouse) => "(is lighthouse)".to_string(),
            (Some(lh_id), _) => id_to_name
                .get(lh_id)
                .cloned()
                .unwrap_or_else(|| format!("(lighthouse {})", &lh_id[..8.min(lh_id.len())])),
            (None, _) => "(no island)".to_string(),
        };
        let section_count = layout
            .knowledge_data
            .get(node_id)
            .map(|d| d.sections.len())
            .unwrap_or(0);
        let cin = conn_in.get(node_id).copied().unwrap_or(0);
        let cout = conn_out.get(node_id).copied().unwrap_or(0);
        rows.push((
            node_id.clone(),
            entry.module_name.clone(),
            variant,
            isla,
            section_count,
            cin,
            cout,
        ));
    }
    // Stable order: lighthouses first, then alphabetical by name.
    rows.sort_by(|a, b| {
        let a_lh = a.2 == "lighthouse";
        let b_lh = b.2 == "lighthouse";
        b_lh.cmp(&a_lh).then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase()))
    });

    if rows.is_empty() {
        return Ok(ToolExecutionResult {
            success: true,
            output: "This project has no logbooks (knowledge nodes) yet.".to_string(),
            baseline: None,
        });
    }

    // Identify true orphans up front so the AI doesn't have to derive them:
    // a node is "orphan" iff it has no lighthouse AND no incoming or
    // outgoing manual connections.
    let orphan_ids: Vec<&String> = rows
        .iter()
        .filter(|(_, _, variant, isla, _, cin, cout)| {
            *variant != "lighthouse" && isla == "(no island)" && *cin == 0 && *cout == 0
        })
        .map(|(id, ..)| id)
        .collect();

    let mut out = format!("{} logbook(s) in the project:\n\n", rows.len());
    out.push_str("| node_id | name | variant | island | sections | conn_in | conn_out |\n");
    out.push_str("|---------|------|---------|--------|----------|---------|----------|\n");
    for (id, name, variant, isla, sections, cin, cout) in &rows {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            id, name, variant, isla, sections, cin, cout,
        ));
    }
    if !orphan_ids.is_empty() {
        out.push_str(&format!(
            "\n**Orphan nodes** (no island AND no connections, {} in total):\n",
            orphan_ids.len()
        ));
        for id in &orphan_ids {
            out.push_str(&format!("- {}\n", id));
        }
    }

    Ok(ToolExecutionResult {
        success: true,
        output: out,
        baseline: None,
    })
}

/// `read_logbook` — return one node's logbook (subtype + sections).
fn execute_read_logbook(
    arguments: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let project_path = match &ctx.project_path {
        Some(p) => p.clone(),
        None => {
            return Ok(ToolExecutionResult {
                success: false,
                output: "read_logbook requires an active project context".to_string(),
                baseline: None,
            });
        }
    };
    // Accept either `node_id` (canonical) or `logbook_id` (alias) — some
    // models prefer one over the other and we'd rather succeed than nag.
    let node_id_arg = arguments
        .get("node_id")
        .and_then(|v| v.as_str())
        .or_else(|| arguments.get("logbook_id").and_then(|v| v.as_str()));
    let node_id = match node_id_arg {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => {
            return Ok(ToolExecutionResult {
                success: false,
                output: "read_logbook missing required argument: pass 'node_id' (preferred) or 'logbook_id'. Use list_logbooks to find ids.".to_string(),
                baseline: None,
            });
        }
    };

    // Pull both the layout entry (for name + variant) and the knowledge data.
    let snapshot = crate::ocean::service::with_service(&project_path, |service| {
        let layout = service.get_layout();
        let entry = layout.positions.get(&node_id).cloned();
        let data = service.get_knowledge_data(&node_id);
        (entry, data)
    });

    let (entry, data) = match snapshot {
        Ok(s) => s,
        Err(e) => {
            return Ok(ToolExecutionResult {
                success: false,
                output: format!("Failed to read ocean state: {}", e),
                baseline: None,
            });
        }
    };

    let entry = match entry {
        Some(e) => e,
        None => {
            return Ok(ToolExecutionResult {
                success: false,
                output: format!("Node '{}' not found in this project", node_id),
                baseline: None,
            });
        }
    };

    use crate::ocean::NodeVariant;
    if entry.node_variant == NodeVariant::Module {
        return Ok(ToolExecutionResult {
            success: false,
            output: format!(
                "Node '{}' is a code module — modules don't have logbooks. Use read_file for source files.",
                entry.module_name
            ),
            baseline: None,
        });
    }

    let data = match data {
        Some(d) => d,
        None => {
            return Ok(ToolExecutionResult {
                success: true,
                output: format!(
                    "Logbook for '{}' is empty (no sections yet).",
                    entry.module_name
                ),
                baseline: None,
            });
        }
    };

    let mut out = String::new();
    out.push_str(&format!(
        "Logbook: {} (variant: {}, subtype: {:?})\n",
        entry.module_name,
        match entry.node_variant {
            NodeVariant::Module => "module",
            NodeVariant::KnowledgeNode => "knowledge_node",
            NodeVariant::Lighthouse => "lighthouse",
            NodeVariant::Buoy => "buoy",
            NodeVariant::Cylinder => "cylinder",
        },
        data.subtype,
    ));
    out.push_str(&format!("Sections: {}\n\n", data.sections.len()));

    if data.sections.is_empty() {
        out.push_str("(no sections)");
    } else {
        for (i, section) in data.sections.iter().enumerate() {
            let source_label = match &section.source {
                crate::ocean::SourceAttribution::User => "user".to_string(),
                crate::ocean::SourceAttribution::Ai { model, .. } => format!("ai · {}", model),
            };
            // Section id is the handle the AI needs to call
            // `propose_logbook_write` with `edit_section_id=…` instead of
            // creating duplicate sections. Without exposing it here the
            // model has no way to refer back to a section by identity.
            out.push_str(&format!(
                "## {}. {} [id: {} · {}]\n{}\n\n",
                i + 1,
                section.name,
                section.id,
                source_label,
                if section.content_markdown.trim().is_empty() {
                    "(empty)"
                } else {
                    &section.content_markdown
                },
            ));
        }
        out.push_str(
            "Tip: to add to or rewrite an existing section, call propose_logbook_write with `edit_section_id=<id>` (and pass the FULL new content, not just an addition). Only omit edit_section_id when creating a brand-new section.\n",
        );
    }

    // Pending writes — AI proposals on this node that the user hasn't yet
    // accepted/discarded. They are NOT real sections (they don't appear
    // above) and are NOT visible to anyone else. Including them here lets
    // the AI:
    //   - avoid re-proposing the exact same content,
    //   - reuse the dedupe key when iterating (Create with same name
    //     replaces the prior pending; Edit with same edit_section_id
    //     replaces the prior pending),
    //   - reference baseline content for an Edit it already proposed.
    let pendings = crate::chat::pending_writes::list_for_node(&project_path, &node_id);
    if !pendings.is_empty() {
        out.push_str("\n### Pending writes (awaiting user approval — not real sections yet)\n\n");
        for w in &pendings {
            let (kind_label, target) = match &w.kind {
                crate::chat::pending_writes::PendingKind::Create => {
                    ("create".to_string(), String::new())
                }
                crate::chat::pending_writes::PendingKind::Edit { section_id, .. } => {
                    ("edit".to_string(), format!(", edit_section_id: {}", section_id))
                }
            };
            let preview: String = w
                .content_markdown
                .lines()
                .take(2)
                .map(|l| if l.len() > 120 { &l[..120] } else { l })
                .collect::<Vec<_>>()
                .join(" / ");
            out.push_str(&format!(
                "- [{}] write_id: {} · name: \"{}\"{} · +{} -{}\n  preview: {}\n",
                kind_label,
                w.write_id,
                w.name,
                target,
                w.additions,
                w.deletions,
                if preview.is_empty() { "(empty)" } else { &preview },
            ));
        }
        out.push_str(
            "\nRules for pending writes:\n\
             - If the user wants to *iterate* on a pending Create, call propose_logbook_write again with the SAME `name` — it replaces the previous pending automatically.\n\
             - If the user wants to *iterate* on a pending Edit, call propose_logbook_write with the SAME `edit_section_id` — same dedupe behaviour.\n\
             - Don't re-propose identical content; if the user just asks \"is it ready?\", answer in chat (the user accepts or discards from the panel).\n",
        );
    }

    Ok(ToolExecutionResult {
        success: true,
        output: out,
        baseline: None,
    })
}

/// `search_logbook` — semantic + keyword search across all logbooks of the
/// current project. Uses hybrid search (FTS5 + embeddings) when the logbook
/// index is available, falling back to a case-insensitive substring grep so
/// the tool never hard-fails before the first index lands. Returns at most
/// `limit` hits with snippets.
async fn execute_search_logbook(
    arguments: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let project_path = match &ctx.project_path {
        Some(p) => p.clone(),
        None => {
            return Ok(ToolExecutionResult {
                success: false,
                output: "search_logbook requires an active project context".to_string(),
                baseline: None,
            });
        }
    };
    let query = match arguments.get("query").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => {
            return Ok(ToolExecutionResult {
                success: false,
                output: "search_logbook missing required string argument 'query'".to_string(),
                baseline: None,
            });
        }
    };
    let limit = arguments
        .get("limit")
        .and_then(|v| v.as_i64())
        .unwrap_or(10)
        .clamp(1, 50) as usize;

    // Hybrid path: requires the logbook repo + a resolved project_id.
    if let (Some(repo), Some(project_id)) = (ctx.logbook_repository.as_ref(), ctx.project_id.as_deref()) {
        match crate::rag::search_logbook_hybrid(
            repo,
            project_id,
            &query,
            limit as u32,
            8000,
            ctx.embedding_provider.as_deref(),
            ctx.embedding_api_key.as_deref(),
        ).await {
            Ok(results) if !results.is_empty() => {
                // Resolve node display names from the layout (best-effort).
                let names = crate::ocean::service::with_service(&project_path, |service| {
                    let layout = service.get_layout();
                    layout.positions.iter()
                        .map(|(id, e)| (id.clone(), e.module_name.clone()))
                        .collect::<std::collections::HashMap<String, String>>()
                }).unwrap_or_default();

                let needle = query.to_lowercase();
                let mut hits: Vec<String> = Vec::new();
                for r in &results {
                    let node_id = &r.chunk.file_id; // logbook projection stores node_id here
                    let node_name = names.get(node_id).cloned().unwrap_or_else(|| node_id.clone());
                    let snippet = build_logbook_snippet(&r.chunk.content, &needle, false);
                    hits.push(format!(
                        "- node_id: {}\n  node: {}\n  section: {}\n  snippet: {}\n",
                        node_id, node_name, r.chunk.name, snippet,
                    ));
                }
                let header = format!("{} matches for '{}':\n\n", hits.len(), query);
                return Ok(ToolExecutionResult {
                    success: true,
                    output: format!("{}{}", header, hits.join("\n")),
                    baseline: None,
                });
            }
            Ok(_) => {
                // Index present but no hits — fall through to grep (catches
                // sections not yet swept by the Index Current).
            }
            Err(e) => {
                tracing::warn!("Logbook hybrid search failed (falling back to grep): {}", e);
            }
        }
    }

    // Fallback: case-insensitive substring grep over the live ocean layout.
    search_logbook_grep(&project_path, &query, limit)
}

/// Substring-grep fallback for `search_logbook` (pre-index / zero-hit path).
fn search_logbook_grep(
    project_path: &str,
    query: &str,
    limit: usize,
) -> Result<ToolExecutionResult> {
    let layout = crate::ocean::service::with_service(project_path, |service| {
        service.get_layout()
    });
    let layout = match layout {
        Ok(l) => l,
        Err(e) => {
            return Ok(ToolExecutionResult {
                success: false,
                output: format!("Failed to read ocean state: {}", e),
                baseline: None,
            });
        }
    };

    let needle = query.to_lowercase();
    let mut hits: Vec<String> = Vec::new();

    for (node_id, entry) in &layout.positions {
        let data = match layout.knowledge_data.get(node_id) {
            Some(d) => d,
            None => continue,
        };
        for section in &data.sections {
            let name_match = section.name.to_lowercase().contains(&needle);
            let content_match = section.content_markdown.to_lowercase().contains(&needle);
            if !name_match && !content_match {
                continue;
            }
            let snippet = build_logbook_snippet(&section.content_markdown, &needle, name_match);
            hits.push(format!(
                "- node_id: {}\n  node: {}\n  section: {}\n  snippet: {}\n",
                node_id, entry.module_name, section.name, snippet,
            ));
            if hits.len() >= limit {
                break;
            }
        }
        if hits.len() >= limit {
            break;
        }
    }

    let header = if hits.is_empty() {
        format!("No matches for '{}' in any logbook.", query)
    } else {
        format!("{} matches for '{}':\n\n", hits.len(), query)
    };

    Ok(ToolExecutionResult {
        success: true,
        output: format!("{}{}", header, hits.join("\n")),
        baseline: None,
    })
}

/// `list_connections` — enumerate every directed manual connection
/// (`from_id → to_id`) currently drawn in the project. Read-only; resolves
/// both ends to their human-readable names so the LLM doesn't need a
/// separate `read_logbook` to interpret the rows.
fn execute_list_connections(
    _arguments: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let project_path = match &ctx.project_path {
        Some(p) => p.clone(),
        None => {
            return Ok(ToolExecutionResult {
                success: false,
                output: "list_connections requires an active project context".to_string(),
                baseline: None,
            });
        }
    };

    let layout = match crate::ocean::service::with_service(&project_path, |service| {
        service.get_layout()
    }) {
        Ok(l) => l,
        Err(e) => {
            return Ok(ToolExecutionResult {
                success: false,
                output: format!("Failed to read ocean state: {}", e),
                baseline: None,
            });
        }
    };

    if layout.manual_connections.is_empty() {
        return Ok(ToolExecutionResult {
            success: true,
            output: "No manual connections in the project.".to_string(),
            baseline: None,
        });
    }

    let id_to_name: std::collections::HashMap<&String, &String> = layout
        .positions
        .iter()
        .map(|(id, e)| (id, &e.module_name))
        .collect();

    let mut out = format!(
        "{} connection(s) in the project:\n\n",
        layout.manual_connections.len()
    );
    out.push_str("| from_id | from_name | → | to_id | to_name |\n");
    out.push_str("|---------|-----------|---|-------|---------|\n");
    for c in &layout.manual_connections {
        let from_name = id_to_name
            .get(&c.from_id)
            .map(|n| n.as_str())
            .unwrap_or("(unknown)");
        let to_name = id_to_name
            .get(&c.to_id)
            .map(|n| n.as_str())
            .unwrap_or("(unknown)");
        out.push_str(&format!(
            "| {} | {} | → | {} | {} |\n",
            c.from_id, from_name, c.to_id, to_name,
        ));
    }

    Ok(ToolExecutionResult {
        success: true,
        output: out,
        baseline: None,
    })
}

/// `list_islands` — group nodes by lighthouse and show themes.
/// Read-only. Reuses `manual_connections` to count internal vs external
/// edges per island so the LLM can judge how interconnected each island is.
fn execute_list_islands(
    _arguments: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let project_path = match &ctx.project_path {
        Some(p) => p.clone(),
        None => {
            return Ok(ToolExecutionResult {
                success: false,
                output: "list_islands requires an active project context".to_string(),
                baseline: None,
            });
        }
    };

    use crate::ocean::NodeVariant;
    let layout = match crate::ocean::service::with_service(&project_path, |service| {
        service.get_layout()
    }) {
        Ok(l) => l,
        Err(e) => {
            return Ok(ToolExecutionResult {
                success: false,
                output: format!("Failed to read ocean state: {}", e),
                baseline: None,
            });
        }
    };

    // Collect lighthouses and their children. We intentionally compute this
    // here from `positions` rather than ask the service so the tool stays a
    // thin read-only wrapper.
    struct Isla<'a> {
        faro_id: &'a String,
        faro_name: &'a String,
        children: Vec<(&'a String, &'a String)>, // (id, name)
        sections_total: usize,
    }
    let mut islas: std::collections::HashMap<&String, Isla> = std::collections::HashMap::new();
    let mut floating: Vec<(&String, &String)> = Vec::new(); // (id, name)
    let mut node_to_isla: std::collections::HashMap<&String, Option<&String>> =
        std::collections::HashMap::new();

    for (id, e) in &layout.positions {
        match e.node_variant {
            NodeVariant::Lighthouse => {
                islas.insert(
                    id,
                    Isla {
                        faro_id: id,
                        faro_name: &e.module_name,
                        children: Vec::new(),
                        sections_total: layout
                            .knowledge_data
                            .get(id)
                            .map(|d| d.sections.len())
                            .unwrap_or(0),
                    },
                );
                node_to_isla.insert(id, Some(id));
            }
            NodeVariant::KnowledgeNode => {
                node_to_isla.insert(id, e.lighthouse_id.as_ref());
            }
            // Code-representational variants are ignored when building the
            // knowledge-graph view (islas + floating knowledge nodes).
            NodeVariant::Module | NodeVariant::Buoy | NodeVariant::Cylinder => {}
        }
    }
    // Now attach children to islas (and collect floating nodes).
    for (id, e) in &layout.positions {
        if !matches!(e.node_variant, NodeVariant::KnowledgeNode) {
            continue;
        }
        let sections = layout
            .knowledge_data
            .get(id)
            .map(|d| d.sections.len())
            .unwrap_or(0);
        match &e.lighthouse_id {
            Some(lh_id) if islas.contains_key(lh_id) => {
                let isla = islas.get_mut(lh_id).expect("checked");
                isla.children.push((id, &e.module_name));
                isla.sections_total += sections;
            }
            _ => {
                floating.push((id, &e.module_name));
            }
        }
    }

    // Connection accounting: per island, count edges where both endpoints
    // belong to the same island (internal) vs cross-island (external in/out).
    let mut conn_internal: std::collections::HashMap<&String, u32> = std::collections::HashMap::new();
    let mut conn_in_ext: std::collections::HashMap<&String, u32> = std::collections::HashMap::new();
    let mut conn_out_ext: std::collections::HashMap<&String, u32> = std::collections::HashMap::new();
    for c in &layout.manual_connections {
        let from_isla = node_to_isla.get(&c.from_id).copied().flatten();
        let to_isla = node_to_isla.get(&c.to_id).copied().flatten();
        match (from_isla, to_isla) {
            (Some(a), Some(b)) if a == b => *conn_internal.entry(a).or_insert(0) += 1,
            (Some(a), Some(b)) => {
                *conn_out_ext.entry(a).or_insert(0) += 1;
                *conn_in_ext.entry(b).or_insert(0) += 1;
            }
            (Some(a), None) => *conn_out_ext.entry(a).or_insert(0) += 1,
            (None, Some(b)) => *conn_in_ext.entry(b).or_insert(0) += 1,
            (None, None) => {}
        }
    }

    if islas.is_empty() && floating.is_empty() {
        return Ok(ToolExecutionResult {
            success: true,
            output: "This project has no islands or nodes yet.".to_string(),
            baseline: None,
        });
    }

    // Sort islas alphabetically by name for stable output.
    let mut sorted: Vec<&Isla> = islas.values().collect();
    sorted.sort_by(|a, b| a.faro_name.to_lowercase().cmp(&b.faro_name.to_lowercase()));

    let mut out = format!("{} island(s) in the project:\n\n", sorted.len());
    out.push_str("| island_name | lighthouse_id | child_count | child_names | sections_total | conn_internal | conn_in_ext | conn_out_ext |\n");
    out.push_str("|-------------|---------------|-------------|-------------|----------------|---------------|-------------|--------------|\n");
    for isla in &sorted {
        let names = if isla.children.is_empty() {
            "(no children)".to_string()
        } else {
            isla.children
                .iter()
                .map(|(_, n)| n.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
            isla.faro_name,
            isla.faro_id,
            isla.children.len(),
            names,
            isla.sections_total,
            conn_internal.get(isla.faro_id).copied().unwrap_or(0),
            conn_in_ext.get(isla.faro_id).copied().unwrap_or(0),
            conn_out_ext.get(isla.faro_id).copied().unwrap_or(0),
        ));
    }
    if !floating.is_empty() {
        out.push_str(&format!(
            "\n**Standalone nodes** ({} in total):\n",
            floating.len()
        ));
        let mut floating = floating.clone();
        floating.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
        for (id, name) in &floating {
            out.push_str(&format!("- {} ({})\n", name, id));
        }
    }

    Ok(ToolExecutionResult {
        success: true,
        output: out,
        baseline: None,
    })
}

/// `query_neighborhood` — list nodes within `radius` Manhattan cells of
/// the given node. Read-only; spatial query over `positions`.
fn execute_query_neighborhood(
    arguments: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let project_path = match &ctx.project_path {
        Some(p) => p.clone(),
        None => {
            return Ok(ToolExecutionResult {
                success: false,
                output: "query_neighborhood requires an active project context".to_string(),
                baseline: None,
            });
        }
    };
    let node_id = match arguments.get("node_id").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => {
            return Ok(ToolExecutionResult {
                success: false,
                output: "query_neighborhood missing required argument: 'node_id' (UUID).".to_string(),
                baseline: None,
            });
        }
    };
    let radius = arguments
        .get("radius")
        .and_then(|v| v.as_i64())
        .unwrap_or(3)
        .clamp(1, 10) as i32;

    use crate::ocean::NodeVariant;
    let layout = match crate::ocean::service::with_service(&project_path, |service| {
        service.get_layout()
    }) {
        Ok(l) => l,
        Err(e) => {
            return Ok(ToolExecutionResult {
                success: false,
                output: format!("Failed to read ocean state: {}", e),
                baseline: None,
            });
        }
    };

    let center_entry = match layout.positions.get(&node_id) {
        Some(e) => e,
        None => {
            return Ok(ToolExecutionResult {
                success: false,
                output: format!(
                    "query_neighborhood: node '{}' not found. Use list_logbooks to find valid ids.",
                    node_id
                ),
                baseline: None,
            });
        }
    };
    let center = center_entry.cell;

    // Resolve island name lookups.
    let id_to_name: std::collections::HashMap<&String, &String> = layout
        .positions
        .iter()
        .map(|(id, e)| (id, &e.module_name))
        .collect();

    // Collect neighbours — exclude the center node itself.
    let mut rows: Vec<(String, String, &'static str, String, i32)> = Vec::new();
    for (id, e) in &layout.positions {
        if id == &node_id {
            continue;
        }
        // Code-representational variants (module / buoy / cylinder) aren't
        // relevant in Knowledge mode.
        if matches!(
            e.node_variant,
            NodeVariant::Module | NodeVariant::Buoy | NodeVariant::Cylinder
        ) {
            continue;
        }
        let dist = e.cell.manhattan_distance(&center);
        if dist > radius {
            continue;
        }
        let variant = match e.node_variant {
            NodeVariant::KnowledgeNode => "knowledge_node",
            NodeVariant::Lighthouse => "lighthouse",
            NodeVariant::Module | NodeVariant::Buoy | NodeVariant::Cylinder => continue,
        };
        let isla = match (&e.lighthouse_id, &e.node_variant) {
            (_, NodeVariant::Lighthouse) => "(is lighthouse)".to_string(),
            (Some(lh_id), _) => id_to_name
                .get(lh_id)
                .map(|n| n.to_string())
                .unwrap_or_else(|| format!("(lighthouse {})", &lh_id[..8.min(lh_id.len())])),
            (None, _) => "(no island)".to_string(),
        };
        rows.push((id.clone(), e.module_name.clone(), variant, isla, dist));
    }
    // Sort by distance asc, then name.
    rows.sort_by(|a, b| {
        a.4.cmp(&b.4)
            .then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase()))
    });

    let center_name = &center_entry.module_name;
    if rows.is_empty() {
        return Ok(ToolExecutionResult {
            success: true,
            output: format!(
                "No nodes within ≤{} Manhattan cells of \"{}\" ({}, {}).",
                radius, center_name, center.col, center.row,
            ),
            baseline: None,
        });
    }

    let mut out = format!(
        "{} neighbour(s) of \"{}\" ({}, {}) within Manhattan radius {}:\n\n",
        rows.len(),
        center_name,
        center.col,
        center.row,
        radius,
    );
    out.push_str("| node_id | name | variant | island | distance |\n");
    out.push_str("|---------|------|---------|--------|----------|\n");
    for (id, name, variant, isla, dist) in &rows {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            id, name, variant, isla, dist,
        ));
    }
    Ok(ToolExecutionResult {
        success: true,
        output: out,
        baseline: None,
    })
}

/// Build a short snippet around the first occurrence of `needle` in
/// `content` (case-insensitive). If the match is in the section name only,
/// returns the first 80 chars of content as preview instead.
fn build_logbook_snippet(content: &str, needle: &str, name_only_match: bool) -> String {
    const RADIUS: usize = 60;
    let content_lower = content.to_lowercase();

    if let Some(pos) = content_lower.find(needle) {
        let start = pos.saturating_sub(RADIUS);
        let end = (pos + needle.len() + RADIUS).min(content.len());
        // Snap to char boundaries to avoid panics inside multi-byte sequences.
        let safe_start = (0..=start).rev().find(|&i| content.is_char_boundary(i)).unwrap_or(0);
        let safe_end = (end..=content.len()).find(|&i| content.is_char_boundary(i)).unwrap_or(content.len());
        let mut snippet = content[safe_start..safe_end].replace('\n', " ");
        if safe_start > 0 {
            snippet.insert_str(0, "...");
        }
        if safe_end < content.len() {
            snippet.push_str("...");
        }
        return snippet;
    }

    if name_only_match {
        let preview_end = content.len().min(80);
        let safe_end = (preview_end..=content.len())
            .find(|&i| content.is_char_boundary(i))
            .unwrap_or(content.len());
        let mut snippet = content[..safe_end].replace('\n', " ");
        if safe_end < content.len() {
            snippet.push_str("...");
        }
        return snippet;
    }

    String::new()
}

/// `propose_logbook_write` — append a new section or replace an existing one
/// in a knowledge node, attributed to the AI. Phase 5 (Slice A) auto-applies
/// without a pending preview; Slice B will gate this behind an approval flow.
fn execute_propose_logbook_write(
    arguments: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let project_path = match &ctx.project_path {
        Some(p) => p.clone(),
        None => {
            return Ok(ToolExecutionResult {
                success: false,
                output: "propose_logbook_write requires an active project context".to_string(),
                baseline: None,
            });
        }
    };
    let node_id = match arguments.get("node_id").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => {
            return Ok(ToolExecutionResult {
                success: false,
                output: "propose_logbook_write missing required argument: 'node_id' (UUID from list_logbooks).".to_string(),
                baseline: None,
            });
        }
    };
    let section_name = match arguments.get("name").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => {
            return Ok(ToolExecutionResult {
                success: false,
                output: "propose_logbook_write missing required argument: 'name' (non-empty section title).".to_string(),
                baseline: None,
            });
        }
    };
    let content = match arguments.get("content_markdown").and_then(|v| v.as_str()) {
        Some(s) => s.to_string(),
        None => {
            return Ok(ToolExecutionResult {
                success: false,
                output: "propose_logbook_write missing required argument: 'content_markdown'.".to_string(),
                baseline: None,
            });
        }
    };
    let prompt_intent = arguments
        .get("prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("(no prompt provided)")
        .to_string();
    let edit_section_id = arguments
        .get("edit_section_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let model = ctx.model.clone().unwrap_or_else(|| "ai".to_string());

    // Build the pending write (resolving baseline + diff inside the same
    // service lock for Edit). Does NOT mutate node sections — that happens
    // later when the user accepts the pending from the panel.
    let outcome: Result<crate::chat::pending_writes::PendingSectionWrite> =
        crate::ocean::service::with_service(&project_path, |service| {
            let kind = if let Some(ref sec_id) = edit_section_id {
                let data = service.get_knowledge_data(&node_id).ok_or_else(|| {
                    VenoreError::NotFound(format!("node '{}'", node_id))
                })?;
                let sec = data.sections.iter().find(|s| &s.id == sec_id).ok_or_else(|| {
                    VenoreError::NotFound(format!("section '{}' in node '{}'", sec_id, node_id))
                })?;
                crate::chat::pending_writes::PendingKind::Edit {
                    section_id: sec.id.clone(),
                    baseline_name: sec.name.clone(),
                    baseline_content: sec.content_markdown.clone(),
                }
            } else {
                if service.get_knowledge_data(&node_id).is_none() {
                    return Err(VenoreError::NotFound(format!("node '{}'", node_id)));
                }
                crate::chat::pending_writes::PendingKind::Create
            };

            let (diff_patch, additions, deletions) = match &kind {
                crate::chat::pending_writes::PendingKind::Edit {
                    baseline_content, ..
                } => {
                    let (patch, adds, dels) = crate::chat::pending_writes::compute_diff_patch(
                        &section_name,
                        baseline_content,
                        &content,
                    );
                    (Some(patch), adds, dels)
                }
                crate::chat::pending_writes::PendingKind::Create => {
                    // No baseline → counting non-empty new lines as additions.
                    let adds = content.lines().filter(|l| !l.is_empty()).count() as u32;
                    (None, adds, 0)
                }
            };

            Ok(crate::chat::pending_writes::PendingSectionWrite {
                write_id: crate::chat::pending_writes::new_write_id(),
                project_path: project_path.clone(),
                node_id: node_id.clone(),
                session_id: ctx.session_id.clone(),
                kind,
                name: section_name.clone(),
                content_markdown: content.clone(),
                ai_prompt: prompt_intent.clone(),
                ai_model: model.clone(),
                diff_patch,
                additions,
                deletions,
                created_at: chrono::Utc::now().timestamp(),
            })
        })
        .unwrap_or_else(|e| Err(VenoreError::ToolExecutionFailed(format!("ocean service: {}", e))));

    match outcome {
        Ok(write) => {
            let kind_tag = match &write.kind {
                crate::chat::pending_writes::PendingKind::Create => "create",
                crate::chat::pending_writes::PendingKind::Edit { .. } => "edit",
            };
            let write_id = crate::chat::pending_writes::insert(write);
            // Trailing machine-readable marker is parsed by the desktop
            // dispatch layer to emit `ai-write-proposed`. The leading
            // sentence is what the LLM reads in its tool result.
            Ok(ToolExecutionResult {
                success: true,
                output: format!(
                    "Pending {} write created for node {} (name: \"{}\"). Awaiting user approval in the node panel — do not retry; the user will accept, discard, or regenerate.\n[ai-write-pending write_id={};node_id={};kind={}]",
                    kind_tag, node_id, section_name, write_id, node_id, kind_tag,
                ),
                baseline: None,
            })
        }
        Err(e) => Ok(ToolExecutionResult {
            success: false,
            output: format!("propose_logbook_write failed: {}", e),
            baseline: None,
        }),
    }
}

// ============================================================================
// STRUCTURE TOOLS — manipulate the Ocean Canvas graph (lighthouses, nodes, connections)
// ============================================================================

/// `create_lighthouse` — anchor a new island on the canvas.
fn execute_create_lighthouse(
    arguments: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let project_path = match &ctx.project_path {
        Some(p) => p.clone(),
        None => {
            return Ok(ToolExecutionResult {
                success: false,
                output: "create_lighthouse requires an active project context".to_string(),
                baseline: None,
            });
        }
    };
    let name = match arguments.get("name").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => {
            return Ok(ToolExecutionResult {
                success: false,
                output: "create_lighthouse missing required argument: 'name'".to_string(),
                baseline: None,
            });
        }
    };
    let near = arguments
        .get("near_node_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    // Inter-island spacing: a new lighthouse must land at least MIN_LIGHTHOUSE_GAP
    // cells (Manhattan) from every existing node. With centroid-based
    // child placement, an island of ~12 nodes occupies a 5×5 area; gap=5
    // gives ~3 cells of empty buffer between islands. If the user supplied
    // an explicit `near_node_id`, honour it (legacy behaviour) — they're
    // overriding the auto-spacing on purpose.
    const MIN_LIGHTHOUSE_GAP: i32 = 5;
    let outcome = crate::ocean::service::with_service(&project_path, |service| {
        let target = if let Some(ref near_id) = near {
            let anchor = service
                .get_layout()
                .positions
                .get(near_id)
                .map(|e| e.cell)
                .unwrap_or_else(|| crate::ocean::GridCell::new(0, 0));
            service.find_free_cell_near(anchor, 8).unwrap_or(anchor)
        } else {
            service
                .find_free_cell_min_distance(MIN_LIGHTHOUSE_GAP, 32)
                .unwrap_or_else(|| crate::ocean::GridCell::new(0, 0))
        };
        service.create_lighthouse(name.clone(), target)
    });
    match outcome {
        Ok(crate::ocean::MoveResult::Accepted { node_id, cell }) => Ok(ToolExecutionResult {
            success: true,
            output: format!(
                "Lighthouse \"{}\" created (id: {}) at cell ({}, {}).",
                name, node_id, cell.col, cell.row,
            ),
            baseline: None,
        }),
        Ok(crate::ocean::MoveResult::Rejected { reason, .. }) => Ok(ToolExecutionResult {
            success: false,
            output: format!("create_lighthouse rejected: {}", reason),
            baseline: None,
        }),
        Err(e) => Ok(ToolExecutionResult {
            success: false,
            output: format!("create_lighthouse failed: {}", e),
            baseline: None,
        }),
    }
}

/// `create_knowledge_node` — add a new sub-topic node, optionally attached
/// to a lighthouse.
fn execute_create_knowledge_node(
    arguments: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let project_path = match &ctx.project_path {
        Some(p) => p.clone(),
        None => {
            return Ok(ToolExecutionResult {
                success: false,
                output: "create_knowledge_node requires an active project context".to_string(),
                baseline: None,
            });
        }
    };
    let name = match arguments.get("name").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => {
            return Ok(ToolExecutionResult {
                success: false,
                output: "create_knowledge_node missing required argument: 'name'".to_string(),
                baseline: None,
            });
        }
    };
    let lighthouse_id = arguments
        .get("lighthouse_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let explicit_near = arguments
        .get("near_node_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    // Centroid-aware placement: collect anchor cells from the new node's
    // semantic neighbourhood — the lighthouse it'll belong to plus all of
    // its existing children (siblings of the new node). The free-cell
    // search starts at the centroid of those anchors so a 5-node island
    // ends up distributed around the lighthouse instead of stacked beside it.
    // Also collect cells of OTHER lighthouses as `forbidden` so the new node
    // can't drift into a neighbouring island's territory.
    // If no lighthouse, fall back to the explicit `near_node_id` or origin.
    const MIN_INTER_ISLA_GAP: i32 = 3;
    let create_outcome = crate::ocean::service::with_service(&project_path, |service| {
        let layout = service.get_layout();
        let mut anchors: Vec<crate::ocean::GridCell> = Vec::new();
        let mut forbidden: Vec<crate::ocean::GridCell> = Vec::new();
        if let Some(ref lh_id) = lighthouse_id {
            if let Some(lh_entry) = layout.positions.get(lh_id) {
                anchors.push(lh_entry.cell);
            }
            for entry in layout.positions.values() {
                let same_isla = entry
                    .lighthouse_id
                    .as_deref()
                    .map(|id| id == lh_id.as_str())
                    .unwrap_or(false);
                if same_isla {
                    // Sibling knowledge_node — pulls the centroid.
                    anchors.push(entry.cell);
                } else if matches!(entry.node_variant, crate::ocean::NodeVariant::Lighthouse) {
                    // A different island's lighthouse — push the new node away.
                    forbidden.push(entry.cell);
                }
            }
        }
        if anchors.is_empty() {
            if let Some(ref near_id) = explicit_near {
                if let Some(entry) = layout.positions.get(near_id) {
                    anchors.push(entry.cell);
                }
            }
        }

        let target = service.find_free_cell_centroid(&anchors, &forbidden, MIN_INTER_ISLA_GAP);
        service.create_knowledge_node(name.clone(), target)
    });
    let (node_id, cell) = match create_outcome {
        Ok(crate::ocean::MoveResult::Accepted { node_id, cell }) => (node_id, cell),
        Ok(crate::ocean::MoveResult::Rejected { reason, .. }) => {
            return Ok(ToolExecutionResult {
                success: false,
                output: format!("create_knowledge_node rejected: {}", reason),
                baseline: None,
            });
        }
        Err(e) => {
            return Ok(ToolExecutionResult {
                success: false,
                output: format!("create_knowledge_node failed: {}", e),
                baseline: None,
            });
        }
    };

    // Attach to lighthouse if requested. We do this in a separate service
    // call to keep error handling clean and to surface assignment failures
    // separately from creation failures.
    if let Some(lh) = &lighthouse_id {
        let lh_clone = lh.clone();
        let attach_outcome =
            crate::ocean::service::with_service(&project_path, |service| {
                service.set_node_lighthouse(&node_id, Some(lh_clone))
            });
        match attach_outcome {
            Ok(Ok(())) => {}
            Ok(Err(reason)) => {
                return Ok(ToolExecutionResult {
                    success: false,
                    output: format!(
                        "Knowledge node \"{}\" created (id: {}) but assignment to lighthouse {} failed: {}. The node exists but is currently floating.",
                        name, node_id, lh, reason,
                    ),
                    baseline: None,
                });
            }
            Err(e) => {
                return Ok(ToolExecutionResult {
                    success: false,
                    output: format!(
                        "Knowledge node \"{}\" created (id: {}) but lighthouse assignment errored: {}",
                        name, node_id, e,
                    ),
                    baseline: None,
                });
            }
        }
    }

    let isla = match &lighthouse_id {
        Some(lh) => format!(" attached to lighthouse {}", lh),
        None => " (floating, no lighthouse)".to_string(),
    };
    Ok(ToolExecutionResult {
        success: true,
        output: format!(
            "Knowledge node \"{}\" created (id: {}) at cell ({}, {}){}.",
            name, node_id, cell.col, cell.row, isla,
        ),
        baseline: None,
    })
}

/// `create_connection` — directed manual edge between two nodes.
fn execute_create_connection(
    arguments: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let project_path = match &ctx.project_path {
        Some(p) => p.clone(),
        None => {
            return Ok(ToolExecutionResult {
                success: false,
                output: "create_connection requires an active project context".to_string(),
                baseline: None,
            });
        }
    };
    let from = match arguments.get("from_node_id").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => {
            return Ok(ToolExecutionResult {
                success: false,
                output: "create_connection missing required argument: 'from_node_id'".to_string(),
                baseline: None,
            });
        }
    };
    let to = match arguments.get("to_node_id").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => {
            return Ok(ToolExecutionResult {
                success: false,
                output: "create_connection missing required argument: 'to_node_id'".to_string(),
                baseline: None,
            });
        }
    };
    let outcome = crate::ocean::service::with_service(&project_path, |service| {
        service.create_connection(&from, &to)
    });
    match outcome {
        Ok(Ok(conn)) => Ok(ToolExecutionResult {
            success: true,
            output: format!(
                "Connection created (id: {}): {} → {}.",
                conn.id, conn.from_id, conn.to_id,
            ),
            baseline: None,
        }),
        Ok(Err(reason)) => Ok(ToolExecutionResult {
            success: false,
            output: format!("create_connection rejected: {}", reason),
            baseline: None,
        }),
        Err(e) => Ok(ToolExecutionResult {
            success: false,
            output: format!("create_connection failed: {}", e),
            baseline: None,
        }),
    }
}

/// `promote_to_lighthouse` — turn a knowledge_node into a lighthouse.
fn execute_promote_to_lighthouse(
    arguments: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let project_path = match &ctx.project_path {
        Some(p) => p.clone(),
        None => {
            return Ok(ToolExecutionResult {
                success: false,
                output: "promote_to_lighthouse requires an active project context".to_string(),
                baseline: None,
            });
        }
    };
    let node_id = match arguments.get("node_id").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => {
            return Ok(ToolExecutionResult {
                success: false,
                output: "promote_to_lighthouse missing required argument: 'node_id'".to_string(),
                baseline: None,
            });
        }
    };
    let outcome = crate::ocean::service::with_service(&project_path, |service| {
        service.promote_to_lighthouse(&node_id)
    });
    match outcome {
        Ok(Ok(())) => Ok(ToolExecutionResult {
            success: true,
            output: format!("Node {} promoted to lighthouse.", node_id),
            baseline: None,
        }),
        Ok(Err(reason)) => Ok(ToolExecutionResult {
            success: false,
            output: format!("promote_to_lighthouse rejected: {}", reason),
            baseline: None,
        }),
        Err(e) => Ok(ToolExecutionResult {
            success: false,
            output: format!("promote_to_lighthouse failed: {}", e),
            baseline: None,
        }),
    }
}

/// `rename_node` — change the human-readable name of a node in place.
fn execute_rename_node(
    arguments: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let project_path = match &ctx.project_path {
        Some(p) => p.clone(),
        None => {
            return Ok(ToolExecutionResult {
                success: false,
                output: "rename_node requires an active project context".to_string(),
                baseline: None,
            });
        }
    };
    let node_id = match arguments.get("node_id").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => {
            return Ok(ToolExecutionResult {
                success: false,
                output: "rename_node missing required argument: 'node_id'".to_string(),
                baseline: None,
            });
        }
    };
    let new_name = match arguments.get("new_name").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => {
            return Ok(ToolExecutionResult {
                success: false,
                output: "rename_node missing required argument: 'new_name' (non-empty)".to_string(),
                baseline: None,
            });
        }
    };

    let outcome = crate::ocean::service::with_service(&project_path, |service| {
        service.rename_node(&node_id, new_name.clone())
    });
    match outcome {
        Ok(true) => Ok(ToolExecutionResult {
            success: true,
            output: format!("Node {} renamed to \"{}\".", node_id, new_name),
            baseline: None,
        }),
        Ok(false) => Ok(ToolExecutionResult {
            success: false,
            output: format!("rename_node failed: node '{}' not found", node_id),
            baseline: None,
        }),
        Err(e) => Ok(ToolExecutionResult {
            success: false,
            output: format!("rename_node failed: {}", e),
            baseline: None,
        }),
    }
}

/// `set_node_lighthouse` — re-assign a knowledge_node to a different lighthouse,
/// or clear its assignment.
fn execute_set_node_lighthouse(
    arguments: &serde_json::Value,
    ctx: &ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let project_path = match &ctx.project_path {
        Some(p) => p.clone(),
        None => {
            return Ok(ToolExecutionResult {
                success: false,
                output: "set_node_lighthouse requires an active project context".to_string(),
                baseline: None,
            });
        }
    };
    let node_id = match arguments.get("node_id").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => {
            return Ok(ToolExecutionResult {
                success: false,
                output: "set_node_lighthouse missing required argument: 'node_id'".to_string(),
                baseline: None,
            });
        }
    };
    // Empty string or omitted → detach the node from any lighthouse.
    let lighthouse_id = arguments
        .get("lighthouse_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let outcome = crate::ocean::service::with_service(&project_path, |service| {
        service.set_node_lighthouse(&node_id, lighthouse_id.clone())
    });
    match outcome {
        Ok(Ok(())) => Ok(ToolExecutionResult {
            success: true,
            output: match &lighthouse_id {
                Some(lh) => format!("Node {} re-assigned to lighthouse {}.", node_id, lh),
                None => format!("Node {} detached (now floating, no island).", node_id),
            },
            baseline: None,
        }),
        Ok(Err(reason)) => Ok(ToolExecutionResult {
            success: false,
            output: format!("set_node_lighthouse rejected: {}", reason),
            baseline: None,
        }),
        Err(e) => Ok(ToolExecutionResult {
            success: false,
            output: format!("set_node_lighthouse failed: {}", e),
            baseline: None,
        }),
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- Port extraction tests -----------------------------------------------

    #[test]
    fn test_extract_docker_port() {
        let ports = extract_ports_from_command("docker run -d -p 5173:5173 my-app");
        assert_eq!(ports, vec![5173]);
    }

    #[test]
    fn test_extract_docker_publish() {
        let ports = extract_ports_from_command("docker run --publish 8080:80 nginx");
        assert_eq!(ports, vec![8080]);
    }

    #[test]
    fn test_extract_docker_multiple_ports() {
        let ports = extract_ports_from_command("docker run -p 3000:3000 -p 5432:5432 app");
        assert_eq!(ports, vec![3000, 5432]);
    }

    #[test]
    fn test_extract_port_flag() {
        let ports = extract_ports_from_command("vite --port 3000");
        assert_eq!(ports, vec![3000]);
    }

    #[test]
    fn test_extract_port_flag_equals() {
        let ports = extract_ports_from_command("next dev --port=4000");
        assert_eq!(ports, vec![4000]);
    }

    #[test]
    fn test_extract_short_p_flag() {
        let ports = extract_ports_from_command("next dev -p 3001");
        assert_eq!(ports, vec![3001]);
    }

    #[test]
    fn test_extract_no_ports() {
        let ports = extract_ports_from_command("cargo build --release");
        assert!(ports.is_empty());
    }

    #[test]
    fn test_extract_ignores_small_numbers() {
        // Single-digit numbers should not be treated as ports
        let ports = extract_ports_from_command("echo -p 5 test");
        assert!(ports.is_empty());
    }

    // -- Port availability tests ---------------------------------------------

    #[test]
    fn test_is_port_available_detects_busy() {
        // Bind a port and start listening so connect() succeeds
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        // While listener is alive, port should be busy (connect succeeds)
        assert!(!is_port_available(port));
        // After dropping, should be free (connect fails)
        drop(listener);
        assert!(is_port_available(port));
    }

    #[test]
    fn test_suggest_available_ports_returns_requested_count() {
        let suggestions = suggest_available_ports(49000, 3);
        assert_eq!(suggestions.len(), 3);
        for &p in &suggestions {
            assert!(p > 49000);
        }
    }

    // -- Port mismatch detection tests ---------------------------------------

    #[test]
    fn test_mismatch_detected() {
        let cmd = "docker run -p 5178:5173 virtualization-app";
        let output = "VITE v6.4.1  ready in 117ms\n  → Local: http://localhost:3333/";
        let warning = check_port_mismatch(cmd, output);
        assert!(warning.is_some());
        let w = warning.unwrap();
        assert!(w.contains("3333"));
        assert!(w.contains("5173"));
    }

    #[test]
    fn test_mismatch_no_warning_when_ports_match() {
        let cmd = "docker run -p 5178:3333 my-app";
        let output = "Listening on http://localhost:3333/";
        let warning = check_port_mismatch(cmd, output);
        assert!(warning.is_none());
    }

    #[test]
    fn test_mismatch_ignores_non_docker() {
        let cmd = "npm start --port 3000";
        let output = "Server running on localhost:3000";
        let warning = check_port_mismatch(cmd, output);
        assert!(warning.is_none());
    }

    #[test]
    fn test_mismatch_no_ports_in_output() {
        let cmd = "docker run -p 8080:80 nginx";
        let output = "container started abc123";
        let warning = check_port_mismatch(cmd, output);
        assert!(warning.is_none());
    }

    // -- ask_caller tests -----------------------------------------------------

    #[tokio::test]
    async fn test_ask_caller_without_handle_returns_error() {
        let ctx = ToolExecutionContext {
            terminal_id: None,
            project_path: None,
            rag_repository: None,
            logbook_repository: None,
            project_id: None,
            embedding_provider: None,
            embedding_api_key: None,
            web_search_api_key: None,
            llm_gateway: None,
            mesh_follow_up: None,
            knowledge_repo: None,
            knowledge_feature_id: None,
            model: None,
            session_id: None,
            allowed_tools: None,
        };

        let args = serde_json::json!({ "question": "What auth method?" });
        let result = execute_ask_caller(&args, &ctx).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("only available inside a mesh handler"));
    }

    #[tokio::test]
    async fn test_ask_caller_over_budget_returns_error() {
        let (write_tx, _write_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let handle = MeshFollowUpHandle {
            stream_id: "test-stream".to_string(),
            write_tx,
            answer_channels: Arc::new(std::sync::Mutex::new(HashMap::new())),
            follow_up_count: Arc::new(AtomicU32::new(MAX_FOLLOW_UPS)), // already at limit
        };

        let ctx = ToolExecutionContext {
            terminal_id: None,
            project_path: None,
            rag_repository: None,
            logbook_repository: None,
            project_id: None,
            embedding_provider: None,
            embedding_api_key: None,
            web_search_api_key: None,
            llm_gateway: None,
            mesh_follow_up: Some(handle),
            knowledge_repo: None,
            knowledge_feature_id: None,
            model: None,
            session_id: None,
            allowed_tools: None,
        };

        let args = serde_json::json!({ "question": "One more question?" });
        let result = execute_ask_caller(&args, &ctx).await.unwrap();
        assert!(!result.success);
        assert!(result.output.contains("Follow-up limit reached"));
    }

    // -- Logbook tools tests --------------------------------------------------

    use crate::ocean::{GridCell, MoveResult, SourceAttribution};

    /// Build a ToolExecutionContext that points at the given project_path
    /// and leaves every other resource None.
    fn ctx_for_project(project_path: &str) -> ToolExecutionContext {
        ToolExecutionContext {
            terminal_id: None,
            project_path: Some(project_path.to_string()),
            rag_repository: None,
            logbook_repository: None,
            project_id: None,
            embedding_provider: None,
            embedding_api_key: None,
            web_search_api_key: None,
            llm_gateway: None,
            mesh_follow_up: None,
            knowledge_repo: None,
            knowledge_feature_id: None,
            model: None,
            session_id: None,
            allowed_tools: None,
        }
    }

    /// Create a knowledge node at (col,row) with the given sections and
    /// return its id. Each section is appended via the public service API.
    fn make_knowledge_node(
        project_path: &str,
        name: &str,
        col: i32,
        row: i32,
        sections: &[(&str, &str, SourceAttribution)],
    ) -> String {
        let move_result = crate::ocean::service::with_service(project_path, |service| {
            service.create_knowledge_node(name.to_string(), GridCell::new(col, row))
        })
        .expect("with_service create_knowledge_node");
        let node_id = match move_result {
            MoveResult::Accepted { node_id, .. } => node_id,
            MoveResult::Rejected { reason, .. } => panic!("create rejected: {}", reason),
        };
        // The default with_now() creates an empty knowledge data entry now —
        // we add sections via add_node_section so they go through the real path.
        for (section_name, content, source) in sections {
            crate::ocean::service::with_service(project_path, |service| {
                service.add_node_section(
                    &node_id,
                    section_name.to_string(),
                    content.to_string(),
                    source.clone(),
                    None,
                    None,
                )
            })
            .expect("with_service add_node_section");
        }
        node_id
    }

    #[test]
    fn test_read_logbook_missing_project_path() {
        let ctx = ctx_for_project("");
        // Empty project path triggers the with_service to use a path of "" —
        // we test the explicit None case here instead.
        let ctx = ToolExecutionContext { project_path: None, ..ctx };
        let args = serde_json::json!({ "node_id": "some-id" });
        let result = execute_read_logbook(&args, &ctx).unwrap();
        assert!(!result.success);
        assert!(result.output.contains("requires an active project context"));
    }

    #[test]
    fn test_read_logbook_missing_node_id() {
        let dir = tempfile::TempDir::new().unwrap();
        let project = dir.path().to_string_lossy().to_string();
        let ctx = ctx_for_project(&project);
        let result = execute_read_logbook(&serde_json::json!({}), &ctx).unwrap();
        assert!(!result.success);
        assert!(result.output.contains("'node_id'"));
        assert!(result.output.contains("'logbook_id'"));
    }

    #[test]
    fn test_read_logbook_node_not_found() {
        let dir = tempfile::TempDir::new().unwrap();
        let project = dir.path().to_string_lossy().to_string();
        let ctx = ctx_for_project(&project);
        let args = serde_json::json!({ "node_id": "unknown-node" });
        let result = execute_read_logbook(&args, &ctx).unwrap();
        assert!(!result.success);
        assert!(result.output.contains("not found"));
    }

    #[test]
    fn test_read_logbook_returns_sections() {
        let dir = tempfile::TempDir::new().unwrap();
        let project = dir.path().to_string_lossy().to_string();
        let ctx = ctx_for_project(&project);
        let node_id = make_knowledge_node(
            &project,
            "Auth",
            0,
            0,
            &[
                ("Visión", "Authentication subsystem", SourceAttribution::User),
                ("Decisiones", "Use JWT", SourceAttribution::User),
            ],
        );
        let args = serde_json::json!({ "node_id": node_id });
        let result = execute_read_logbook(&args, &ctx).unwrap();
        assert!(result.success);
        assert!(result.output.contains("Auth"));
        assert!(result.output.contains("Visión"));
        assert!(result.output.contains("Authentication subsystem"));
        assert!(result.output.contains("Decisiones"));
        assert!(result.output.contains("Use JWT"));
        // Section header format is "## N. NAME [id: <uuid> · user]" — we look
        // for the source-label segment, not the brackets.
        assert!(result.output.contains("· user]"));
    }

    #[test]
    fn test_read_logbook_marks_ai_source() {
        let dir = tempfile::TempDir::new().unwrap();
        let project = dir.path().to_string_lossy().to_string();
        let ctx = ctx_for_project(&project);
        let ai_source = SourceAttribution::Ai {
            model: "gpt-5".to_string(),
            timestamp: 0,
        };
        let node_id = make_knowledge_node(
            &project,
            "Concept",
            0,
            0,
            &[("AI section", "generated", ai_source)],
        );
        let args = serde_json::json!({ "node_id": node_id });
        let result = execute_read_logbook(&args, &ctx).unwrap();
        assert!(result.success);
        assert!(result.output.contains("ai · gpt-5"));
    }

    #[tokio::test]
    async fn test_search_logbook_missing_query() {
        let dir = tempfile::TempDir::new().unwrap();
        let project = dir.path().to_string_lossy().to_string();
        let ctx = ctx_for_project(&project);
        let result = execute_search_logbook(&serde_json::json!({}), &ctx).await.unwrap();
        assert!(!result.success);
        assert!(result.output.contains("missing required string argument 'query'"));
    }

    #[tokio::test]
    async fn test_search_logbook_no_matches() {
        let dir = tempfile::TempDir::new().unwrap();
        let project = dir.path().to_string_lossy().to_string();
        let ctx = ctx_for_project(&project);
        make_knowledge_node(
            &project,
            "Auth",
            0,
            0,
            &[("Visión", "JWT tokens here", SourceAttribution::User)],
        );
        let args = serde_json::json!({ "query": "nothingmatches" });
        let result = execute_search_logbook(&args, &ctx).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("No matches"));
    }

    #[tokio::test]
    async fn test_search_logbook_finds_in_content() {
        let dir = tempfile::TempDir::new().unwrap();
        let project = dir.path().to_string_lossy().to_string();
        let ctx = ctx_for_project(&project);
        let node_id = make_knowledge_node(
            &project,
            "Auth",
            0,
            0,
            &[("Visión", "We use JWT tokens for sessions", SourceAttribution::User)],
        );
        let args = serde_json::json!({ "query": "JWT" });
        let result = execute_search_logbook(&args, &ctx).await.unwrap();
        assert!(result.success, "expected success, output: {}", result.output);
        assert!(result.output.contains(&node_id));
        assert!(result.output.contains("Auth"));
        assert!(result.output.contains("Visión"));
        assert!(result.output.to_lowercase().contains("jwt"));
    }

    #[tokio::test]
    async fn test_search_logbook_finds_in_section_name() {
        let dir = tempfile::TempDir::new().unwrap();
        let project = dir.path().to_string_lossy().to_string();
        let ctx = ctx_for_project(&project);
        make_knowledge_node(
            &project,
            "Idea",
            0,
            0,
            &[("Authentication notes", "anything", SourceAttribution::User)],
        );
        let args = serde_json::json!({ "query": "authentication" });
        let result = execute_search_logbook(&args, &ctx).await.unwrap();
        assert!(result.success);
        assert!(result.output.to_lowercase().contains("authentication notes"));
    }

    #[tokio::test]
    async fn test_search_logbook_case_insensitive() {
        let dir = tempfile::TempDir::new().unwrap();
        let project = dir.path().to_string_lossy().to_string();
        let ctx = ctx_for_project(&project);
        make_knowledge_node(
            &project,
            "x",
            0,
            0,
            &[("s", "JsonWebToken", SourceAttribution::User)],
        );
        let args = serde_json::json!({ "query": "JSONWEBTOKEN" });
        let result = execute_search_logbook(&args, &ctx).await.unwrap();
        assert!(result.success);
        assert!(result.output.to_lowercase().contains("jsonwebtoken"));
    }

    #[tokio::test]
    async fn test_search_logbook_respects_limit() {
        let dir = tempfile::TempDir::new().unwrap();
        let project = dir.path().to_string_lossy().to_string();
        let ctx = ctx_for_project(&project);
        // Five nodes each with a single matching section.
        for i in 0..5 {
            make_knowledge_node(
                &project,
                &format!("Node{}", i),
                i,
                0,
                &[("s", "common-needle", SourceAttribution::User)],
            );
        }
        let args = serde_json::json!({ "query": "common-needle", "limit": 2 });
        let result = execute_search_logbook(&args, &ctx).await.unwrap();
        assert!(result.success);
        // The header reports the count of hits, capped at 2.
        assert!(result.output.contains("2 matches"));
    }

    #[test]
    fn test_list_logbooks_empty() {
        let dir = tempfile::TempDir::new().unwrap();
        let project = dir.path().to_string_lossy().to_string();
        let ctx = ctx_for_project(&project);
        let result = execute_list_logbooks(&serde_json::json!({}), &ctx).unwrap();
        assert!(result.success);
        assert!(result.output.contains("no logbooks"));
    }

    #[test]
    fn test_list_logbooks_returns_nodes() {
        let dir = tempfile::TempDir::new().unwrap();
        let project = dir.path().to_string_lossy().to_string();
        let ctx = ctx_for_project(&project);
        let auth_id = make_knowledge_node(
            &project,
            "Auth",
            0,
            0,
            &[("s1", "x", SourceAttribution::User), ("s2", "y", SourceAttribution::User)],
        );
        let payments_id = make_knowledge_node(
            &project,
            "Payments",
            1,
            0,
            &[("s1", "x", SourceAttribution::User)],
        );

        let result = execute_list_logbooks(&serde_json::json!({}), &ctx).unwrap();
        assert!(result.success);
        // Both ids present in the table.
        assert!(result.output.contains(&auth_id));
        assert!(result.output.contains(&payments_id));
        // Names present.
        assert!(result.output.contains("Auth"));
        assert!(result.output.contains("Payments"));
        // Section counts present (Auth=2, Payments=1).
        assert!(result.output.contains("| 2 |"));
        assert!(result.output.contains("| 1 |"));
        // Header announces total.
        assert!(result.output.contains("2 logbook(s)"));
    }

    #[test]
    fn test_read_logbook_accepts_logbook_id_alias() {
        let dir = tempfile::TempDir::new().unwrap();
        let project = dir.path().to_string_lossy().to_string();
        let ctx = ctx_for_project(&project);
        let id = make_knowledge_node(
            &project,
            "Auth",
            0,
            0,
            &[("Visión", "JWT tokens", SourceAttribution::User)],
        );
        // Models that hallucinate `logbook_id` should still get a valid result.
        let args = serde_json::json!({ "logbook_id": id });
        let result = execute_read_logbook(&args, &ctx).unwrap();
        assert!(result.success, "expected success, got: {}", result.output);
        assert!(result.output.contains("JWT tokens"));
    }

    #[tokio::test]
    async fn test_search_logbook_multiple_nodes() {
        let dir = tempfile::TempDir::new().unwrap();
        let project = dir.path().to_string_lossy().to_string();
        let ctx = ctx_for_project(&project);
        make_knowledge_node(
            &project,
            "Auth",
            0,
            0,
            &[("Visión", "uses a JWT token for sessions", SourceAttribution::User)],
        );
        make_knowledge_node(
            &project,
            "Payments",
            1,
            0,
            &[("Notas", "Stripe webhook signature is a token", SourceAttribution::User)],
        );
        // Both should appear when searching for "token" (case-insensitive).
        let args = serde_json::json!({ "query": "token" });
        let result = execute_search_logbook(&args, &ctx).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("Auth"));
        assert!(result.output.contains("Payments"));
    }

    // -- propose_logbook_write tests -----------------------------------------

    #[test]
    fn test_propose_logbook_write_creates_section() {
        let dir = tempfile::TempDir::new().unwrap();
        let project = dir.path().to_string_lossy().to_string();
        let mut ctx = ctx_for_project(&project);
        ctx.model = Some("claude-test".to_string());
        let node_id = make_knowledge_node(&project, "Auth", 0, 0, &[]);

        let args = serde_json::json!({
            "node_id": node_id,
            "name": "Consideraciones de seguridad",
            "content_markdown": "## Riesgos\n- Token leak\n- CSRF",
            "prompt": "agrega consideraciones de seguridad para este nodo",
        });
        let result = execute_propose_logbook_write(&args, &ctx).unwrap();
        assert!(result.success, "expected success, got: {}", result.output);
        assert!(result.output.contains("Pending create write created"));

        // propose_logbook_write doesn't mutate sections directly anymore — it
        // stashes a pending proposal that the user reviews and accepts.
        // Verify the proposal landed in the pending-writes registry with AI
        // source + ai_prompt + correct kind.
        let pending = crate::chat::pending_writes::list_for_node(&project, &node_id);
        let proposal = pending
            .iter()
            .find(|w| w.name == "Consideraciones de seguridad")
            .expect("new pending write");
        assert!(matches!(
            proposal.kind,
            crate::chat::pending_writes::PendingKind::Create
        ));
        assert_eq!(
            proposal.ai_prompt,
            "agrega consideraciones de seguridad para este nodo"
        );
        assert_eq!(proposal.ai_model, "claude-test");
    }

    #[test]
    fn test_propose_logbook_write_edits_section() {
        let dir = tempfile::TempDir::new().unwrap();
        let project = dir.path().to_string_lossy().to_string();
        let mut ctx = ctx_for_project(&project);
        ctx.model = Some("claude-test".to_string());
        let node_id = make_knowledge_node(
            &project,
            "Auth",
            0,
            0,
            &[("Decisiones", "Use JWT", SourceAttribution::User)],
        );

        // Read the existing section id from the layout.
        let section_id = {
            let layout = crate::ocean::service::with_service(&project, |s| s.get_layout()).unwrap();
            layout
                .knowledge_data
                .get(&node_id)
                .unwrap()
                .sections
                .iter()
                .find(|s| s.name == "Decisiones")
                .unwrap()
                .id
                .clone()
        };

        let args = serde_json::json!({
            "node_id": node_id,
            "name": "Decisiones",
            "content_markdown": "Use JWT + refresh tokens, rotated weekly.",
            "prompt": "actualiza decisiones con la rotación semanal",
            "edit_section_id": section_id.clone(),
        });
        let result = execute_propose_logbook_write(&args, &ctx).unwrap();
        assert!(result.success, "expected success, got: {}", result.output);
        assert!(result.output.contains("Pending edit write created"));

        // Edit proposals also land in pending-writes; the original section
        // stays untouched until the user accepts.
        let pending = crate::chat::pending_writes::list_for_node(&project, &node_id);
        let proposal = pending
            .iter()
            .find(|w| matches!(
                &w.kind,
                crate::chat::pending_writes::PendingKind::Edit { section_id: sid, .. } if sid == &section_id
            ))
            .expect("edit pending write");
        assert_eq!(proposal.content_markdown, "Use JWT + refresh tokens, rotated weekly.");
        assert_eq!(proposal.ai_model, "claude-test");
    }

    #[test]
    fn test_propose_logbook_write_missing_node_id() {
        let dir = tempfile::TempDir::new().unwrap();
        let project = dir.path().to_string_lossy().to_string();
        let ctx = ctx_for_project(&project);
        let args = serde_json::json!({
            "name": "x",
            "content_markdown": "y",
            "prompt": "z",
        });
        let result = execute_propose_logbook_write(&args, &ctx).unwrap();
        assert!(!result.success);
        assert!(result.output.contains("node_id"));
    }
}
