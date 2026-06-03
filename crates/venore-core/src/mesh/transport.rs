//! Mesh Transport — WebSocket server/client for peer communication
//!
//! Binds a local WebSocket server on an ephemeral port, accepts inbound
//! connections from peers, and provides `connect_to_peer()` to establish
//! outbound connections. Each outbound peer connection uses an mpsc channel
//! so `send_to_peer()` is non-blocking.

use std::collections::HashMap;
use std::sync::Arc;

use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use once_cell::sync::Lazy;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

use crate::error::{Result, VenoreError};
use crate::mesh::discovery::MeshDiscovery;
use crate::mesh::handler::MeshRequestHandler;
use crate::mesh::protocol::MeshMessage;
use crate::tools::MeshFollowUpHandle;

/// Lock a std::sync::Mutex, recovering from poison if needed.
fn lock_or_recover<'a, T>(mutex: &'a std::sync::Mutex<T>, _name: &str) -> std::sync::MutexGuard<'a, T> {
    mutex.lock().unwrap_or_else(|e| {
        tracing::warn!("Recovered from poisoned mutex");
        e.into_inner()
    })
}

/// Global singleton for the mesh transport.
static GLOBAL: Lazy<Arc<Mutex<MeshTransport>>> =
    Lazy::new(|| Arc::new(Mutex::new(MeshTransport::new())));

/// Messages received by the caller from the remote handler (Phase 4b).
///
/// A single request can produce multiple messages: follow-up questions
/// before the final response. The mpsc channel stays open until the
/// terminal `Response` message arrives.
#[derive(Debug)]
pub enum CallerMessage {
    /// Final response from the handler (terminal — no more messages).
    Response(Result<String>),
    /// Follow-up question from the handler sub-agent (non-terminal).
    FollowUp { question: String, round: u32, stream_id: String },
}

/// Pending responses for outbound requests, keyed by stream_id.
/// Changed from oneshot to mpsc to support follow-up questions (Phase 4b).
static PENDING_RESPONSES: Lazy<std::sync::Mutex<HashMap<String, mpsc::UnboundedSender<CallerMessage>>>> =
    Lazy::new(|| std::sync::Mutex::new(HashMap::new()));

/// Request handlers for inbound queries, keyed by the receiving peer's
/// `project_id`. One handler per local peer — the inbound dispatch reads
/// the `to_project` field on each `AgentRequest` to pick the right one.
static REQUEST_HANDLERS: Lazy<tokio::sync::RwLock<HashMap<String, Arc<dyn MeshRequestHandler>>>> =
    Lazy::new(|| tokio::sync::RwLock::new(HashMap::new()));

/// Caller-side conversation tracker: maps peer_project_id → conversation_id.
/// Used by `get_or_create_conversation_id` to reuse conversation IDs for
/// consecutive calls to the same peer (Phase 4a multi-turn support).
static MESH_CONVERSATIONS: Lazy<std::sync::Mutex<HashMap<String, MeshConversationTracker>>> =
    Lazy::new(|| std::sync::Mutex::new(HashMap::new()));

use super::MESH_CONVERSATION_TTL_SECS;

/// Tracks a conversation with a specific peer.
struct MeshConversationTracker {
    conversation_id: String,
    last_used: std::time::Instant,
}

/// Get or create a conversation ID for a peer project.
///
/// If the caller has talked to this peer recently (within TTL), the same
/// conversation_id is reused so the remote handler can load prior context.
/// Otherwise, a new UUID is generated.
pub fn get_or_create_conversation_id(peer_project_id: &str) -> String {
    let mut convs = lock_or_recover(&MESH_CONVERSATIONS, "MESH_CONVERSATIONS");
    let now = std::time::Instant::now();
    let ttl = std::time::Duration::from_secs(MESH_CONVERSATION_TTL_SECS);

    // Lazy TTL cleanup
    convs.retain(|_, tracker| now.duration_since(tracker.last_used) < ttl);

    // Reuse or create
    let tracker = convs.entry(peer_project_id.to_string()).or_insert_with(|| {
        MeshConversationTracker {
            conversation_id: uuid::Uuid::new_v4().to_string(),
            last_used: now,
        }
    });
    tracker.last_used = now;
    tracker.conversation_id.clone()
}

/// Clear all conversation trackers (called on shutdown).
fn clear_conversations() {
    let mut convs = lock_or_recover(&MESH_CONVERSATIONS, "MESH_CONVERSATIONS");
    convs.clear();
}

/// Set the request handler for a specific local peer. Called from the
/// Tauri layer once per project opened in this process.
pub async fn set_request_handler(project_id: &str, handler: Arc<dyn MeshRequestHandler>) {
    let mut guard = REQUEST_HANDLERS.write().await;
    guard.insert(project_id.to_string(), handler);
    tracing::info!(project_id = %project_id, "Mesh request handler configured");
}

/// Remove a local peer's request handler. Called when the project is
/// closed or the mesh is shutting down.
pub async fn unset_request_handler(project_id: &str) {
    let mut guard = REQUEST_HANDLERS.write().await;
    if guard.remove(project_id).is_some() {
        tracing::info!(project_id = %project_id, "Mesh request handler removed");
    }
}

/// Clear all handlers — used at app exit.
pub async fn clear_request_handlers() {
    let mut guard = REQUEST_HANDLERS.write().await;
    guard.clear();
}

/// Remove a pending response by stream_id (used for timeout cleanup).
pub fn remove_pending_response(stream_id: &str) {
    let mut pending = lock_or_recover(&PENDING_RESPONSES, "PENDING_RESPONSES");
    pending.remove(stream_id);
}

/// An outbound peer connection — messages are sent via mpsc channel,
/// and a background task handles the actual WebSocket I/O.
struct PeerConnection {
    tx: mpsc::UnboundedSender<MeshMessage>,
    task_handle: JoinHandle<()>,
}

/// WebSocket transport for mesh peer-to-peer communication.
///
/// Follows the `LspManager` async singleton pattern (`tokio::sync::Mutex`).
pub struct MeshTransport {
    port: u16,
    accept_handle: Option<JoinHandle<()>>,
    connections: HashMap<String, PeerConnection>,
    running: bool,
}

impl MeshTransport {
    /// Private constructor for the global singleton.
    fn new() -> Self {
        Self {
            port: 0,
            accept_handle: None,
            connections: HashMap::new(),
            running: false,
        }
    }

    /// Get the global transport instance.
    pub fn global() -> Arc<Mutex<Self>> {
        GLOBAL.clone()
    }

    /// Start the WebSocket server on an ephemeral port.
    ///
    /// Binds `127.0.0.1:0`, spawns an accept loop, and updates the
    /// mesh discovery registration with the real port.
    pub async fn start(&mut self) -> Result<u16> {
        if self.running {
            return Ok(self.port);
        }

        let listener = TcpListener::bind("127.0.0.1:0").await.map_err(|e| {
            VenoreError::MeshConnectionFailed(format!("Failed to bind: {}", e))
        })?;

        let port = listener.local_addr().map_err(|e| {
            VenoreError::MeshConnectionFailed(format!("Failed to get local addr: {}", e))
        })?.port();

        self.port = port;
        self.running = true;

        // Update discovery registration with the real port.
        // Hold the std::sync::Mutex briefly — never across .await.
        {
            let mesh = MeshDiscovery::global();
            let mut guard = mesh.lock().map_err(|e| {
                VenoreError::MeshError(format!("Discovery mutex poisoned: {}", e))
            })?;
            guard.update_port_all(port)?;
        }

        // Spawn the accept loop
        let handle = tokio::spawn(async move {
            Self::accept_loop(listener).await;
        });
        self.accept_handle = Some(handle);

        tracing::info!(port = port, "Mesh transport started");
        Ok(port)
    }

    /// Connect to a peer by project_id.
    ///
    /// Reads the peer's registration to get its port, then establishes
    /// a WebSocket connection and spawns read/write tasks.
    pub async fn connect_to_peer(&mut self, project_id: &str) -> Result<()> {
        if !self.running {
            return Err(VenoreError::MeshTransportNotRunning);
        }

        if self.connections.contains_key(project_id) {
            tracing::debug!(project_id = project_id, "Already connected to peer");
            return Ok(());
        }

        // Read peer registration — hold std Mutex briefly
        let peer_port = {
            let mesh = MeshDiscovery::global();
            let guard = mesh.lock().map_err(|e| {
                VenoreError::MeshError(format!("Discovery mutex poisoned: {}", e))
            })?;
            let reg = guard.get_peer_registration(project_id)?;
            if reg.port == 0 {
                return Err(VenoreError::MeshConnectionFailed(format!(
                    "Peer {} has no transport running (port=0)",
                    project_id
                )));
            }
            reg.port
        };

        let url = format!("ws://127.0.0.1:{}", peer_port);
        let (ws_stream, _) =
            tokio_tungstenite::connect_async(&url).await.map_err(|e| {
                VenoreError::MeshConnectionFailed(format!(
                    "WebSocket connect to {} failed: {}",
                    url, e
                ))
            })?;

        let (write, read) = ws_stream.split();
        let (tx, rx) = mpsc::unbounded_channel::<MeshMessage>();
        let pid = project_id.to_string();

        // Spawn a task that:
        // 1. Forwards messages from mpsc channel → WebSocket
        // 2. Reads incoming messages and handles Ping → Pong
        let task_handle = tokio::spawn(async move {
            Self::peer_io_loop(pid, write, read, rx).await;
        });

        self.connections.insert(
            project_id.to_string(),
            PeerConnection {
                tx,
                task_handle,
            },
        );

        tracing::info!(project_id = project_id, port = peer_port, "Connected to peer");
        Ok(())
    }

    /// Send a message to a connected peer.
    pub fn send_to_peer(&self, project_id: &str, msg: MeshMessage) -> Result<()> {
        if !self.running {
            return Err(VenoreError::MeshTransportNotRunning);
        }

        let conn = self.connections.get(project_id).ok_or_else(|| {
            VenoreError::MeshPeerNotFound(format!("Not connected to peer: {}", project_id))
        })?;

        conn.tx.send(msg).map_err(|e| {
            VenoreError::MeshConnectionFailed(format!("Channel send failed: {}", e))
        })?;

        Ok(())
    }

    /// Disconnect from a specific peer.
    pub async fn disconnect_peer(&mut self, project_id: &str) {
        if let Some(conn) = self.connections.remove(project_id) {
            // Best-effort send Disconnect message
            let _ = conn.tx.send(MeshMessage::Disconnect {
                reason: "disconnect requested".to_string(),
            });
            // Give the task a moment to flush, then abort
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            conn.task_handle.abort();
            tracing::info!(project_id = project_id, "Disconnected from peer");
        }
    }

    /// Shutdown the transport: disconnect all peers and stop the server.
    pub async fn shutdown(&mut self) {
        if !self.running {
            return;
        }

        // Disconnect all peers
        let peer_ids: Vec<String> = self.connections.keys().cloned().collect();
        for pid in peer_ids {
            self.disconnect_peer(&pid).await;
        }

        // Abort accept loop
        if let Some(handle) = self.accept_handle.take() {
            handle.abort();
        }

        // Drain pending responses so waiters get RecvError
        {
            let mut pending = lock_or_recover(&PENDING_RESPONSES, "PENDING_RESPONSES");
            pending.clear();
        }

        // Clear caller-side conversation trackers (Phase 4a)
        clear_conversations();

        self.running = false;
        self.port = 0;
        tracing::info!("Mesh transport shut down");
    }

    /// Auto-connect to discovered peers owned by OTHER processes.
    ///
    /// Skips peers that are already connected, have port 0 (no transport),
    /// or are locally-registered in this same process. Local siblings are
    /// reachable directly by their handler — no need to open a WebSocket
    /// loop back to ourselves.
    pub async fn auto_connect(&mut self) -> Result<Vec<String>> {
        if !self.running {
            return Ok(vec![]);
        }

        let (peers, local_ids) = {
            let mesh = MeshDiscovery::global();
            let guard = mesh.lock().map_err(|e| {
                VenoreError::MeshError(format!("Discovery mutex poisoned: {}", e))
            })?;
            let peers = guard.discover_peers()?;
            let locals: std::collections::HashSet<String> = guard
                .iter_local_registrations()
                .map(|r| r.project_id.clone())
                .collect();
            (peers, locals)
        };

        let mut newly_connected = Vec::new();
        for peer in peers {
            if local_ids.contains(&peer.project_id) {
                continue;
            }
            if peer.port > 0 && !self.connections.contains_key(&peer.project_id) {
                match self.connect_to_peer(&peer.project_id).await {
                    Ok(()) => newly_connected.push(peer.project_id),
                    Err(e) => tracing::debug!(peer = %peer.project_name, error = %e, "Auto-connect failed"),
                }
            }
        }

        if !newly_connected.is_empty() {
            tracing::info!(count = newly_connected.len(), "Auto-connected to peers");
        }

        Ok(newly_connected)
    }

    /// Current listening port (0 if not running).
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Whether the server is listening.
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// List of connected peer project_ids.
    pub fn connected_peers(&self) -> Vec<String> {
        self.connections.keys().cloned().collect()
    }

    /// Send an agent request and get an mpsc Receiver for responses.
    ///
    /// The receiver yields `CallerMessage` variants: follow-up questions
    /// (Phase 4b) and the final response. Caller MUST drop the transport
    /// lock before reading from the receiver.
    /// `conversation_id` enables multi-turn context on the remote handler (Phase 4a).
    pub fn send_request(
        &mut self,
        project_id: &str,
        question: &str,
        from_project: &str,
        context_hint: Option<&str>,
        conversation_id: Option<&str>,
    ) -> Result<(String, mpsc::UnboundedReceiver<CallerMessage>)> {
        let stream_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = mpsc::unbounded_channel();

        // Insert sender into pending responses
        {
            let mut pending = lock_or_recover(&PENDING_RESPONSES, "PENDING_RESPONSES");
            pending.insert(stream_id.clone(), tx);
        }

        // Spawn a timeout cleanup task: if the response hasn't been consumed
        // after 5 minutes, send a Timeout error and remove from pending.
        {
            let timeout_stream_id = stream_id.clone();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(300)).await;
                let mut pending = PENDING_RESPONSES.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(tx) = pending.remove(&timeout_stream_id) {
                    let _ = tx.send(CallerMessage::Response(Err(
                        VenoreError::Timeout(300_000),
                    )));
                    tracing::warn!(
                        stream_id = %timeout_stream_id,
                        "Mesh request timed out after 5 minutes"
                    );
                }
            });
        }

        // Send the request message. `to_project` matches the peer connection
        // key so the receiving process can route to the right local handler.
        let msg = MeshMessage::AgentRequest {
            stream_id: stream_id.clone(),
            from_project: from_project.to_string(),
            to_project: project_id.to_string(),
            question: question.to_string(),
            context_hint: context_hint.map(|s| s.to_string()),
            conversation_id: conversation_id.map(|s| s.to_string()),
        };

        if let Err(e) = self.send_to_peer(project_id, msg) {
            // Clean up on send failure
            let mut pending = lock_or_recover(&PENDING_RESPONSES, "PENDING_RESPONSES");
            pending.remove(&stream_id);
            return Err(e);
        }

        Ok((stream_id, rx))
    }

    // =========================================================================
    // Private — accept loop
    // =========================================================================

    /// Accept loop: listens for inbound WebSocket connections.
    async fn accept_loop(listener: TcpListener) {
        loop {
            let stream = match listener.accept().await {
                Ok((stream, addr)) => {
                    tracing::debug!(addr = %addr, "Accepted inbound mesh connection");
                    stream
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Accept error in mesh transport");
                    continue;
                }
            };

            // Each inbound connection is handled in its own task
            tokio::spawn(async move {
                Self::handle_inbound(stream).await;
            });
        }
    }

    /// Handle a single inbound connection (fire-and-forget).
    ///
    /// Uses an mpsc channel for writing: the read loop handles Ping/Pong inline
    /// (fast) while AgentRequest handlers are spawned as separate tasks that send
    /// responses through the channel. This prevents long-running LLM agent loops
    /// from blocking the WebSocket connection (ping/pong keeps flowing).
    async fn handle_inbound(stream: TcpStream) {
        let ws_stream = match tokio_tungstenite::accept_async(stream).await {
            Ok(ws) => ws,
            Err(e) => {
                tracing::warn!(error = %e, "WebSocket handshake failed");
                return;
            }
        };

        let (mut write, mut read) = ws_stream.split();
        let (write_tx, mut write_rx) = mpsc::unbounded_channel::<String>();

        // Shared answer channels for follow-up routing (Phase 4b).
        // Maps (stream_id, round) → oneshot sender that the handler's
        // `ask_caller` tool is awaiting.
        let follow_up_answers: Arc<std::sync::Mutex<
            std::collections::HashMap<(String, u32), tokio::sync::oneshot::Sender<String>>
        >> = Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));

        // Spawn a write task that drains the channel → WebSocket
        let write_task = tokio::spawn(async move {
            while let Some(json) = write_rx.recv().await {
                if write.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
        });

        // Read loop: handle fast messages inline, spawn agent requests
        while let Some(msg_result) = read.next().await {
            let msg = match msg_result {
                Ok(Message::Text(text)) => match MeshMessage::from_json(&text) {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::warn!("Failed to deserialize inbound mesh message: {}", e);
                        continue;
                    }
                },
                Ok(Message::Close(_)) => break,
                Err(e) => {
                    tracing::debug!("Inbound WebSocket read error: {}", e);
                    break;
                }
                _ => continue,
            };

            match msg {
                MeshMessage::Ping => {
                    let pong = match MeshMessage::Pong.to_json() {
                        Ok(j) => j,
                        Err(_) => continue,
                    };
                    if write_tx.send(pong).is_err() {
                        break;
                    }
                }
                MeshMessage::Pong => {
                    // Received pong
                }
                MeshMessage::Disconnect { reason } => {
                    tracing::debug!(reason = %reason, "Peer sent Disconnect");
                    break;
                }
                MeshMessage::AgentRequest { stream_id, from_project, to_project, question, context_hint, conversation_id } => {
                    tracing::info!(from = %from_project, to = %to_project, conversation_id = ?conversation_id, "Incoming mesh query");
                    let tx = write_tx.clone();

                    // Create MeshFollowUpHandle for Phase 4b
                    let follow_up_handle = MeshFollowUpHandle {
                        stream_id: stream_id.clone(),
                        write_tx: tx.clone(),
                        answer_channels: Arc::clone(&follow_up_answers),
                        follow_up_count: Arc::new(std::sync::atomic::AtomicU32::new(0)),
                    };

                    // Resolve the target handler by `to_project`. If the
                    // sender omitted it (older peer) or it points to a
                    // project that isn't loaded in this process, respond
                    // with AgentError instead of silently picking another
                    // local handler.
                    tokio::spawn(async move {
                        let handler = {
                            let guard = REQUEST_HANDLERS.read().await;
                            guard.get(&to_project).cloned()
                        };
                        let response_msg = match handler {
                            Some(h) => {
                                match h.handle_request(
                                    &question, &from_project,
                                    context_hint.as_deref(), conversation_id.as_deref(),
                                    Some(follow_up_handle),
                                ).await {
                                    Ok(content) => MeshMessage::AgentResponse { stream_id, content, conversation_id },
                                    Err(e) => MeshMessage::AgentError { stream_id, error: e.to_string() },
                                }
                            }
                            None => MeshMessage::AgentError {
                                stream_id,
                                error: if to_project.is_empty() {
                                    "No request handler configured (request missing to_project)".to_string()
                                } else {
                                    format!("No request handler configured for project '{}'", to_project)
                                },
                            },
                        };
                        if let Ok(json) = response_msg.to_json() {
                            let _ = tx.send(json);
                        }
                    });
                }
                MeshMessage::AgentFollowUpAnswer { stream_id, answer, round } => {
                    // Route the answer to the handler's ask_caller oneshot (Phase 4b)
                    let mut channels = follow_up_answers.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(sender) = channels.remove(&(stream_id.clone(), round)) {
                        let _ = sender.send(answer);
                    } else {
                        tracing::debug!(
                            stream_id = %stream_id, round,
                            "Received AgentFollowUpAnswer but no channel waiting"
                        );
                    }
                }
                MeshMessage::AgentResponse { .. } | MeshMessage::AgentError { .. } => {
                    // Inbound connections shouldn't receive responses — ignore
                    tracing::warn!("Received unexpected response on inbound connection");
                }
                MeshMessage::AgentFollowUp { .. } => {
                    // Inbound connections shouldn't receive follow-ups — ignore
                    tracing::warn!("Received unexpected AgentFollowUp on inbound connection");
                }
            }
        }

        // Drop the sender to signal the write task to finish
        drop(write_tx);
        let _ = write_task.await;
    }

    // =========================================================================
    // Private — outbound peer I/O loop
    // =========================================================================

    /// I/O loop for an outbound peer connection.
    ///
    /// Reads from both the mpsc channel (outbound) and the WebSocket (inbound).
    async fn peer_io_loop(
        project_id: String,
        mut write: SplitSink<WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>, Message>,
        mut read: futures::stream::SplitStream<WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>>,
        mut rx: mpsc::UnboundedReceiver<MeshMessage>,
    ) {
        loop {
            tokio::select! {
                // Outbound: message from channel → WebSocket
                Some(msg) = rx.recv() => {
                    let json = match msg.to_json() {
                        Ok(j) => j,
                        Err(_) => continue,
                    };
                    if write.send(Message::Text(json.into())).await.is_err() {
                        tracing::debug!(peer = %project_id, "WebSocket write failed, closing");
                        break;
                    }
                }
                // Inbound: message from WebSocket
                Some(result) = read.next() => {
                    match result {
                        Ok(Message::Text(text)) => {
                            let text_str: &str = &text;
                            match MeshMessage::from_json(text_str) {
                                Ok(MeshMessage::Ping) => {
                                    // Respond with Pong
                                    if let Ok(pong) = MeshMessage::Pong.to_json() {
                                        let _ = write.send(Message::Text(pong.into())).await;
                                    }
                                }
                                Ok(MeshMessage::Pong) => {
                                    tracing::debug!(peer = %project_id, "Received Pong");
                                }
                                Ok(MeshMessage::Disconnect { reason }) => {
                                    tracing::debug!(peer = %project_id, reason = %reason, "Peer disconnected");
                                    break;
                                }
                                Ok(MeshMessage::AgentResponse { stream_id, content, .. }) => {
                                    // Terminal message — send Response and remove from pending
                                    let mut pending = PENDING_RESPONSES.lock().unwrap_or_else(|e| e.into_inner());
                                    if let Some(tx) = pending.remove(&stream_id) {
                                        let _ = tx.send(CallerMessage::Response(Ok(content)));
                                    }
                                }
                                Ok(MeshMessage::AgentError { stream_id, error }) => {
                                    // Terminal message — send Response(Err) and remove from pending
                                    let mut pending = PENDING_RESPONSES.lock().unwrap_or_else(|e| e.into_inner());
                                    if let Some(tx) = pending.remove(&stream_id) {
                                        let _ = tx.send(CallerMessage::Response(Err(VenoreError::MeshError(error))));
                                    }
                                }
                                Ok(MeshMessage::AgentFollowUp { stream_id, question, round }) => {
                                    // Non-terminal: follow-up question from handler (Phase 4b)
                                    // Do NOT remove from pending — more messages will come
                                    let pending = PENDING_RESPONSES.lock().unwrap_or_else(|e| e.into_inner());
                                    if let Some(tx) = pending.get(&stream_id) {
                                        let _ = tx.send(CallerMessage::FollowUp {
                                            question,
                                            round,
                                            stream_id: stream_id.clone(),
                                        });
                                    }
                                }
                                Ok(MeshMessage::AgentRequest { .. }) => {
                                    // Outbound connections shouldn't receive requests — ignore
                                    tracing::warn!(peer = %project_id, "Received unexpected request on outbound connection");
                                }
                                Ok(MeshMessage::AgentFollowUpAnswer { .. }) => {
                                    // Outbound connections shouldn't receive follow-up answers — ignore
                                    tracing::warn!(peer = %project_id, "Received unexpected AgentFollowUpAnswer on outbound connection");
                                }
                                Err(e) => {
                                    tracing::debug!(peer = %project_id, "Failed to deserialize peer message: {}", e);
                                }
                            }
                        }
                        Ok(Message::Close(_)) | Err(_) => break,
                        _ => {}
                    }
                }
                else => break,
            }
        }

        tracing::debug!(peer = %project_id, "Peer I/O loop ended");
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::types::PeerRegistration;
    use chrono::Utc;

    /// Helper: write a fake peer registration file
    fn write_peer_file(mesh_dir: &std::path::Path, project_id: &str, port: u16) {
        std::fs::create_dir_all(mesh_dir).unwrap();
        let reg = PeerRegistration {
            project_id: project_id.to_string(),
            project_name: format!("Project {}", project_id),
            project_path: format!("/path/{}", project_id),
            pid: std::process::id(),
            port,
            registered_at: Utc::now(),
            last_seen: Utc::now(),
            profile: None,
        };
        let content = serde_json::to_string_pretty(&reg).unwrap();
        std::fs::write(
            mesh_dir.join(format!("{}.json", project_id)),
            &content,
        )
        .unwrap();
    }

    #[tokio::test]
    async fn test_start_binds_port() {
        // Use the global singleton for this test — start and verify port
        let transport = MeshTransport::global();
        let mut t = transport.lock().await;

        // If already running from another test, shutdown first
        if t.is_running() {
            t.shutdown().await;
        }

        // We need to be registered in discovery first
        {
            let mesh = MeshDiscovery::global();
            let mut m = mesh.lock().unwrap();
            m.register("transport-test", "Transport Test", "/test/path")
                .unwrap();
        }

        let port = t.start().await.unwrap();
        assert!(port > 0, "Should bind to a real port");
        assert!(t.is_running());
        assert_eq!(t.port(), port);

        t.shutdown().await;
        assert!(!t.is_running());
        assert_eq!(t.port(), 0);

        // Clean up discovery
        {
            let mesh = MeshDiscovery::global();
            let mut m = mesh.lock().unwrap();
            let _ = m.unregister_all();
        }
    }

    #[tokio::test]
    async fn test_start_updates_discovery_port() {
        let transport = MeshTransport::global();
        let mut t = transport.lock().await;
        if t.is_running() {
            t.shutdown().await;
        }

        {
            let mesh = MeshDiscovery::global();
            let mut m = mesh.lock().unwrap();
            m.register("port-update-test", "Port Update", "/test/port")
                .unwrap();
        }

        let port = t.start().await.unwrap();

        // Verify discovery file has the updated port
        {
            let mesh = MeshDiscovery::global();
            let m = mesh.lock().unwrap();
            let reg = m.get_peer_registration("port-update-test").unwrap();
            assert_eq!(reg.port, port);
        }

        t.shutdown().await;

        {
            let mesh = MeshDiscovery::global();
            let mut m = mesh.lock().unwrap();
            let _ = m.unregister_all();
        }
    }

    #[tokio::test]
    async fn test_peer_connection() {
        // Start two "transports" — we use the global for one and a raw
        // TcpListener for the simulated peer.
        let transport = MeshTransport::global();
        let mut t = transport.lock().await;
        if t.is_running() {
            t.shutdown().await;
        }

        {
            let mesh = MeshDiscovery::global();
            let mut m = mesh.lock().unwrap();
            m.register("conn-test-main", "Main", "/main").unwrap();
        }

        t.start().await.unwrap();

        // Start a fake peer WebSocket server
        let peer_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let peer_port = peer_listener.local_addr().unwrap().port();

        // Write peer registration file so connect_to_peer can find it
        {
            let mesh = MeshDiscovery::global();
            let m = mesh.lock().unwrap();
            let mesh_dir = m.mesh_dir();
            write_peer_file(mesh_dir, "conn-test-peer", peer_port);
        }

        // Spawn fake peer accept
        let peer_handle = tokio::spawn(async move {
            let (stream, _) = peer_listener.accept().await.unwrap();
            let ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            let (_write, _read) = ws.split();
            // Keep alive briefly
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        });

        // Connect
        t.connect_to_peer("conn-test-peer").await.unwrap();
        assert!(t.connected_peers().contains(&"conn-test-peer".to_string()));

        // Disconnect
        t.disconnect_peer("conn-test-peer").await;
        assert!(!t.connected_peers().contains(&"conn-test-peer".to_string()));

        peer_handle.abort();
        t.shutdown().await;

        {
            let mesh = MeshDiscovery::global();
            let mut m = mesh.lock().unwrap();
            let _ = m.unregister_all();
        }
    }

    #[tokio::test]
    async fn test_ping_pong() {
        let transport = MeshTransport::global();
        let mut t = transport.lock().await;
        if t.is_running() {
            t.shutdown().await;
        }

        {
            let mesh = MeshDiscovery::global();
            let mut m = mesh.lock().unwrap();
            m.register("ping-test-main", "Main", "/main").unwrap();
        }

        let main_port = t.start().await.unwrap();

        // Create a "peer" that connects to our transport and sends a Ping,
        // then reads the Pong response.
        let peer_task = tokio::spawn(async move {
            let url = format!("ws://127.0.0.1:{}", main_port);
            let (ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
            let (mut write, mut read) = ws.split();

            // Send Ping
            let ping_json = MeshMessage::Ping.to_json().unwrap();
            write.send(Message::Text(ping_json.into())).await.unwrap();

            // Read Pong
            if let Some(Ok(Message::Text(text))) = read.next().await {
                let msg = MeshMessage::from_json(&text).unwrap();
                assert!(matches!(msg, MeshMessage::Pong));
                true
            } else {
                false
            }
        });

        let got_pong = peer_task.await.unwrap();
        assert!(got_pong, "Should have received Pong in response to Ping");

        t.shutdown().await;

        {
            let mesh = MeshDiscovery::global();
            let mut m = mesh.lock().unwrap();
            let _ = m.unregister_all();
        }
    }

    #[tokio::test]
    async fn test_connect_nonexistent_peer() {
        let transport = MeshTransport::global();
        let mut t = transport.lock().await;
        if t.is_running() {
            t.shutdown().await;
        }

        {
            let mesh = MeshDiscovery::global();
            let mut m = mesh.lock().unwrap();
            m.register("noconn-test", "NoConn", "/noconn").unwrap();
        }

        t.start().await.unwrap();

        let result = t.connect_to_peer("nonexistent-peer-xyz").await;
        assert!(result.is_err());

        t.shutdown().await;

        {
            let mesh = MeshDiscovery::global();
            let mut m = mesh.lock().unwrap();
            let _ = m.unregister_all();
        }
    }

    #[tokio::test]
    async fn test_request_response_roundtrip() {
        // Instance A (our transport) connects to a fake peer (Instance B)
        // that reads AgentRequest and sends back AgentResponse.
        let transport = MeshTransport::global();
        let mut t = transport.lock().await;
        if t.is_running() {
            t.shutdown().await;
        }

        {
            let mesh = MeshDiscovery::global();
            let mut m = mesh.lock().unwrap();
            m.register("reqresp-main", "Main", "/main").unwrap();
        }

        t.start().await.unwrap();

        // Fake peer: accepts connection, reads AgentRequest, sends AgentResponse
        let peer_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let peer_port = peer_listener.local_addr().unwrap().port();

        {
            let mesh = MeshDiscovery::global();
            let m = mesh.lock().unwrap();
            write_peer_file(m.mesh_dir(), "reqresp-peer", peer_port);
        }

        let peer_handle = tokio::spawn(async move {
            let (stream, _) = peer_listener.accept().await.unwrap();
            let ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            let (mut write, mut read) = ws.split();

            // Read until we get an AgentRequest
            while let Some(Ok(Message::Text(text))) = read.next().await {
                if let Ok(MeshMessage::AgentRequest { stream_id, question, .. }) =
                    MeshMessage::from_json(&text)
                {
                    // Echo the question back as the response content
                    let resp = MeshMessage::AgentResponse {
                        stream_id,
                        content: format!("Answer to: {}", question),
                        conversation_id: None,
                    };
                    let json = resp.to_json().unwrap();
                    let _ = write.send(Message::Text(json.into())).await;
                    break;
                }
            }
            // Keep alive briefly so the response can be read
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        });

        // Connect and send request
        t.connect_to_peer("reqresp-peer").await.unwrap();
        let (_stream_id, mut rx) = t
            .send_request("reqresp-peer", "What is the API?", "main-project", None, None)
            .unwrap();

        // Drop lock before awaiting
        drop(t);

        // Wait for response (short timeout for test)
        let msg = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
            .await
            .expect("Should not timeout")
            .expect("Channel should not be closed");

        match msg {
            CallerMessage::Response(result) => {
                let content = result.unwrap();
                assert_eq!(content, "Answer to: What is the API?");
            }
            other => panic!("Expected CallerMessage::Response, got {:?}", other),
        }

        peer_handle.abort();

        // Cleanup
        let mut t = transport.lock().await;
        t.shutdown().await;
        {
            let mesh = MeshDiscovery::global();
            let mut m = mesh.lock().unwrap();
            let _ = m.unregister_all();
        }
    }

    #[tokio::test]
    async fn test_request_timeout() {
        // Verify the timeout pattern used by the mesh executor:
        // when nobody sends on the mpsc, the caller can time out.
        //
        // Uses a local mpsc (not the global transport or PENDING_RESPONSES)
        // to avoid interference from concurrent tests calling shutdown().
        let (_tx, mut rx) = mpsc::unbounded_channel::<CallerMessage>();

        // Nobody will send on _tx, so rx.recv() blocks until timeout
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(200),
            rx.recv(),
        )
        .await;

        assert!(result.is_err(), "Should timeout waiting for response");
    }

    #[tokio::test]
    async fn test_request_response_e2e() {
        // Exercises handle_inbound wire-up end-to-end without depending on
        // the production AgentHandler (which needs an LlmGateway + API
        // key). A trivial stub handler echoes the project id and a
        // synthetic content string — the test verifies the WebSocket
        // request/response plumbing, not the handler logic itself.
        //
        // Uses a local TcpListener (not the global transport) to avoid
        // race conditions with concurrent tests.

        use async_trait::async_trait;
        use std::sync::Arc;

        use crate::error::Result;
        use crate::mesh::handler::MeshRequestHandler;
        use crate::tools::MeshFollowUpHandle;

        struct StubHandler {
            project_id: String,
        }

        #[async_trait]
        impl MeshRequestHandler for StubHandler {
            async fn handle_request(
                &self,
                _question: &str,
                _from_project: &str,
                _context_hint: Option<&str>,
                _conversation_id: Option<&str>,
                _follow_up_handle: Option<MeshFollowUpHandle>,
            ) -> Result<String> {
                Ok(format!(
                    "# Project: {}\n\nThis project has a REST API with user endpoints.",
                    self.project_id
                ))
            }
        }

        let handler = Arc::new(StubHandler {
            project_id: "e2e-project".to_string(),
        });
        super::set_request_handler("e2e-project", handler).await;

        // Local WS server — isolated from the global transport
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let server = tokio::spawn(async move {
            if let Ok((stream, _)) = listener.accept().await {
                MeshTransport::handle_inbound(stream).await;
            }
        });

        let url = format!("ws://127.0.0.1:{}", port);
        let (ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
        let (mut write, mut read) = ws.split();

        let req = MeshMessage::AgentRequest {
            stream_id: "e2e-stream-1".to_string(),
            from_project: "other-project".to_string(),
            to_project: "e2e-project".to_string(),
            question: "What does this project do?".to_string(),
            context_hint: None,
            conversation_id: None,
        };
        let json = req.to_json().unwrap();
        write.send(Message::Text(json.into())).await.unwrap();

        let resp = tokio::time::timeout(std::time::Duration::from_secs(5), read.next())
            .await
            .expect("Should not timeout");

        if let Some(Ok(Message::Text(text))) = resp {
            let msg = MeshMessage::from_json(&text).unwrap();
            match msg {
                MeshMessage::AgentResponse { stream_id, content, .. } => {
                    assert_eq!(stream_id, "e2e-stream-1");
                    assert!(
                        content.contains("REST API"),
                        "Response should contain the stub handler payload, got: {}",
                        &content[..200.min(content.len())]
                    );
                    assert!(content.contains("e2e-project"));
                }
                other => panic!("Expected AgentResponse, got {:?}", other),
            }
        } else {
            panic!("Expected text message from server");
        }

        server.abort();
        // Only remove the handler we installed — avoid clearing handlers
        // that parallel tests may have set up.
        super::unset_request_handler("e2e-project").await;
    }

    #[tokio::test]
    async fn test_request_no_handler() {
        // Test handle_inbound without any matching REQUEST_HANDLER:
        // send AgentRequest with `to_project` pointing at a project_id
        // that was never registered → handle_inbound should respond with
        // AgentError. Uses a unique project_id so this is safe to run
        // concurrently with tests that register their own handlers.

        // Local WS server — isolated from the global transport
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let server = tokio::spawn(async move {
            if let Ok((stream, _)) = listener.accept().await {
                MeshTransport::handle_inbound(stream).await;
            }
        });

        let url = format!("ws://127.0.0.1:{}", port);
        let (ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
        let (mut write, mut read) = ws.split();

        let req = MeshMessage::AgentRequest {
            stream_id: "no-handler-test".to_string(),
            from_project: "other-project".to_string(),
            to_project: "nonexistent-target".to_string(),
            question: "How does auth work?".to_string(),
            context_hint: None,
            conversation_id: None,
        };
        let json = req.to_json().unwrap();
        write.send(Message::Text(json.into())).await.unwrap();

        let resp = tokio::time::timeout(std::time::Duration::from_secs(5), read.next())
            .await
            .expect("Should not timeout");

        if let Some(Ok(Message::Text(text))) = resp {
            let msg = MeshMessage::from_json(&text).unwrap();
            match msg {
                MeshMessage::AgentError { stream_id, error } => {
                    assert_eq!(stream_id, "no-handler-test");
                    assert!(error.contains("No request handler configured"));
                    assert!(error.contains("nonexistent-target"));
                }
                other => panic!("Expected AgentError, got {:?}", other),
            }
        } else {
            panic!("Expected text message from server");
        }

        server.abort();
    }

    #[test]
    fn test_conversation_tracker_creates_new_id() {
        // Clear any prior state
        clear_conversations();

        let id1 = get_or_create_conversation_id("project-alpha");
        assert!(!id1.is_empty());

        // Same peer → same id
        let id2 = get_or_create_conversation_id("project-alpha");
        assert_eq!(id1, id2);

        // Different peer → different id
        let id3 = get_or_create_conversation_id("project-beta");
        assert_ne!(id1, id3);

        clear_conversations();
    }

    #[test]
    fn test_conversation_tracker_ttl_cleanup() {
        clear_conversations();

        // Insert an entry with old timestamp by manually manipulating the map
        {
            let mut convs = MESH_CONVERSATIONS.lock().unwrap();
            convs.insert("stale-peer".to_string(), MeshConversationTracker {
                conversation_id: "old-conv-id".to_string(),
                last_used: std::time::Instant::now() - std::time::Duration::from_secs(MESH_CONVERSATION_TTL_SECS + 10),
            });
        }

        // Next call should evict the stale entry and create a new one
        let new_id = get_or_create_conversation_id("stale-peer");
        assert_ne!(new_id, "old-conv-id");

        clear_conversations();
    }
}
