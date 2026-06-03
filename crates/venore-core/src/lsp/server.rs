//! LSP server lifecycle management
//!
//! Spawns an LSP server child process, performs the initialize handshake,
//! tracks open files, and stores diagnostics from publishDiagnostics notifications.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tokio::process::Child;
use tokio::sync::{Mutex, Notify, RwLock};

use crate::error::{Result, VenoreError};
use crate::lsp::client::{self, JsonRpcClient};
use crate::lsp::config::LspServerConfig;
use crate::lsp::diagnostics::DiagnosticEntry;

/// State of an LSP server process.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ServerState {
    Starting,
    Running,
    Failed,
    Stopped,
}

/// An active LSP server connected over stdio.
pub struct LspServer {
    pub config: LspServerConfig,
    pub project_root: String,
    client: Arc<Mutex<JsonRpcClient>>,
    diagnostics: Arc<RwLock<HashMap<String, Vec<DiagnosticEntry>>>>,
    diagnostic_signals: Arc<Mutex<HashMap<String, Arc<Notify>>>>,
    opened_files: Arc<Mutex<HashSet<String>>>,
    child: Arc<Mutex<Option<Child>>>,
    reader_handle: Option<tokio::task::JoinHandle<()>>,
    state: ServerState,
}

impl LspServer {
    /// Spawn an LSP server process, perform the initialize handshake, and start
    /// the background reader loop.
    pub async fn start(config: LspServerConfig, project_root: &str) -> Result<Self> {
        // Check that the binary exists
        let check = crate::utils::quiet_tokio_command(&config.command)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await;

        if check.is_err() {
            return Err(VenoreError::LspSpawnFailed(format!(
                "'{}' not found in PATH",
                config.command
            )));
        }

        // Spawn the LSP server process
        let mut child = crate::utils::quiet_tokio_command(&config.command)
            .args(&config.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                VenoreError::LspSpawnFailed(format!("Failed to spawn '{}': {}", config.command, e))
            })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            VenoreError::LspSpawnFailed("Failed to capture stdin".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            VenoreError::LspSpawnFailed("Failed to capture stdout".to_string())
        })?;

        let pending = Arc::new(Mutex::new(HashMap::new()));
        let json_client = JsonRpcClient::new(stdin, pending.clone());
        let client = Arc::new(Mutex::new(json_client));

        let diagnostics: Arc<RwLock<HashMap<String, Vec<DiagnosticEntry>>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let diagnostic_signals: Arc<Mutex<HashMap<String, Arc<Notify>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Start background reader
        let diag_store = diagnostics.clone();
        let diag_signals = diagnostic_signals.clone();
        let reader_handle = tokio::spawn(async move {
            client::reader_loop(stdout, pending, move |method, params| {
                if method == "textDocument/publishDiagnostics" {
                    handle_publish_diagnostics(&diag_store, &diag_signals, params);
                }
            })
            .await;
        });

        let mut server = Self {
            config,
            project_root: project_root.to_string(),
            client,
            diagnostics,
            diagnostic_signals,
            opened_files: Arc::new(Mutex::new(HashSet::new())),
            child: Arc::new(Mutex::new(Some(child))),
            reader_handle: Some(reader_handle),
            state: ServerState::Starting,
        };

        // Perform initialize handshake
        match server.initialize().await {
            Ok(_) => {
                server.state = ServerState::Running;
                tracing::info!("LSP server '{}' started for {}", server.config.name, project_root);
                Ok(server)
            }
            Err(e) => {
                server.state = ServerState::Failed;
                tracing::error!("LSP initialize failed: {}", e);
                Err(e)
            }
        }
    }

    /// Send the `initialize` request and `initialized` notification.
    async fn initialize(&self) -> Result<()> {
        let root_uri = path_to_uri(&self.project_root);

        let init_params = serde_json::json!({
            "processId": std::process::id(),
            "rootUri": root_uri,
            "capabilities": {
                "textDocument": {
                    "publishDiagnostics": {
                        "relatedInformation": false
                    },
                    "synchronization": {
                        "didOpen": true,
                        "didChange": true
                    }
                }
            }
        });

        let client = self.client.lock().await;
        let _result = client.send_request("initialize", init_params).await?;
        client
            .send_notification("initialized", serde_json::json!({}))
            .await?;

        Ok(())
    }

    /// Notify the server that a file has been opened or changed.
    ///
    /// On first call for a file, sends `textDocument/didOpen`.
    /// On subsequent calls, sends `textDocument/didChange` with full content.
    pub async fn notify_file_changed(&self, file_path: &str, content: &str) -> Result<()> {
        let uri = path_to_uri(file_path);
        let language_id = self.detect_language_id(file_path);

        // Clear existing diagnostics for this file so we know when fresh ones arrive
        {
            let mut diag = self.diagnostics.write().await;
            diag.remove(&uri);
        }

        let mut opened = self.opened_files.lock().await;
        let client = self.client.lock().await;

        if opened.contains(&uri) {
            // didChange (full sync)
            client
                .send_notification(
                    "textDocument/didChange",
                    serde_json::json!({
                        "textDocument": { "uri": uri, "version": 2 },
                        "contentChanges": [{ "text": content }]
                    }),
                )
                .await?;
        } else {
            // didOpen
            client
                .send_notification(
                    "textDocument/didOpen",
                    serde_json::json!({
                        "textDocument": {
                            "uri": uri,
                            "languageId": language_id,
                            "version": 1,
                            "text": content
                        }
                    }),
                )
                .await?;
            opened.insert(uri);
        }

        Ok(())
    }

    /// Wait for fresh diagnostics for a file, with a timeout.
    ///
    /// Returns the diagnostics collected so far (may be empty if timeout fires first).
    pub async fn wait_for_diagnostics(
        &self,
        file_path: &str,
        timeout_ms: u64,
    ) -> Vec<DiagnosticEntry> {
        let uri = path_to_uri(file_path);

        // Get or create the signal for this URI
        let notify = {
            let mut signals = self.diagnostic_signals.lock().await;
            signals
                .entry(uri.clone())
                .or_insert_with(|| Arc::new(Notify::new()))
                .clone()
        };

        // Wait for the signal or timeout
        let _ = tokio::time::timeout(
            std::time::Duration::from_millis(timeout_ms),
            notify.notified(),
        )
        .await;

        // Return whatever diagnostics we have
        let diag = self.diagnostics.read().await;
        diag.get(&uri).cloned().unwrap_or_default()
    }

    /// Stop the server gracefully: shutdown → exit → kill.
    pub async fn stop(&mut self) {
        if self.state == ServerState::Stopped {
            return;
        }

        tracing::info!("Stopping LSP server '{}'", self.config.name);

        // Try shutdown request (ignore errors — server may already be dead)
        {
            let client = self.client.lock().await;
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(2),
                client.send_request("shutdown", serde_json::Value::Null),
            )
            .await;

            let _ = client
                .send_notification("exit", serde_json::Value::Null)
                .await;
        }

        // Kill child process
        {
            let mut child_guard = self.child.lock().await;
            if let Some(ref mut child) = *child_guard {
                let _ = child.kill().await;
            }
            *child_guard = None;
        }

        // Abort reader task
        if let Some(handle) = self.reader_handle.take() {
            handle.abort();
        }

        self.state = ServerState::Stopped;
    }

    /// Detect language ID from file extension.
    fn detect_language_id(&self, file_path: &str) -> &'static str {
        let ext = std::path::Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        crate::lsp::config::extension_to_language_id(ext).unwrap_or("plaintext")
    }
}

/// Handle a `textDocument/publishDiagnostics` notification.
///
/// Parses the diagnostics and stores them, then signals any waiters.
fn handle_publish_diagnostics(
    store: &Arc<RwLock<HashMap<String, Vec<DiagnosticEntry>>>>,
    signals: &Arc<Mutex<HashMap<String, Arc<Notify>>>>,
    params: serde_json::Value,
) {
    let uri = match params["uri"].as_str() {
        Some(u) => u.to_string(),
        None => return,
    };

    let diagnostics_array = match params["diagnostics"].as_array() {
        Some(arr) => arr,
        None => return,
    };

    let mut entries = Vec::new();
    for diag in diagnostics_array {
        let severity = match diag["severity"].as_u64() {
            Some(1) => crate::lsp::diagnostics::DiagnosticSeverity::Error,
            Some(2) => crate::lsp::diagnostics::DiagnosticSeverity::Warning,
            Some(3) => crate::lsp::diagnostics::DiagnosticSeverity::Information,
            Some(4) => crate::lsp::diagnostics::DiagnosticSeverity::Hint,
            _ => crate::lsp::diagnostics::DiagnosticSeverity::Error,
        };

        let message = diag["message"].as_str().unwrap_or("").to_string();
        let line = diag["range"]["start"]["line"].as_u64().unwrap_or(0) as u32 + 1; // 0-based → 1-based
        let column = diag["range"]["start"]["character"].as_u64().unwrap_or(0) as u32 + 1;
        let source = diag["source"].as_str().map(String::from);

        entries.push(DiagnosticEntry {
            severity,
            message,
            line,
            column,
            source,
        });
    }

    // Store diagnostics (blocking on async lock via tokio::task::block_in_place would be complex,
    // so we use try_write / spawn approach)
    let store = store.clone();
    let signals = signals.clone();
    let uri_clone = uri.clone();

    // We're called from the reader_loop closure which is sync — use a spawned task
    tokio::spawn(async move {
        {
            let mut diag = store.write().await;
            diag.insert(uri_clone.clone(), entries);
        }
        {
            let sigs = signals.lock().await;
            if let Some(notify) = sigs.get(&uri_clone) {
                notify.notify_waiters();
            }
        }
    });
}

/// Convert a file system path to a `file://` URI.
fn path_to_uri(path: &str) -> String {
    // Normalize backslashes to forward slashes
    let normalized = path.replace('\\', "/");

    if normalized.starts_with('/') {
        format!("file://{}", normalized)
    } else {
        // Windows: D:/foo → file:///D:/foo
        format!("file:///{}", normalized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_to_uri_unix() {
        assert_eq!(path_to_uri("/home/user/file.ts"), "file:///home/user/file.ts");
    }

    #[test]
    fn test_path_to_uri_windows() {
        assert_eq!(
            path_to_uri("D:\\project\\src\\main.rs"),
            "file:///D:/project/src/main.rs"
        );
    }

    #[test]
    fn test_path_to_uri_windows_forward_slash() {
        assert_eq!(
            path_to_uri("C:/Users/dev/app.ts"),
            "file:///C:/Users/dev/app.ts"
        );
    }
}
