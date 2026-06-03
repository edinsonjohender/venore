//! LSP server manager — singleton that owns all running LSP servers
//!
//! Lazily starts servers on first request for a given language+project,
//! and provides `stop_all()` for cleanup on app exit.

use std::collections::HashMap;
use std::sync::Arc;

use once_cell::sync::Lazy;
use tokio::sync::Mutex;

use crate::lsp::config;
use crate::lsp::server::LspServer;

/// Global singleton for the LSP manager.
static GLOBAL: Lazy<Arc<Mutex<LspManager>>> =
    Lazy::new(|| Arc::new(Mutex::new(LspManager::new())));

/// Manages active LSP server instances, keyed by `"{language_id}:{project_root}"`.
pub struct LspManager {
    servers: HashMap<String, LspServer>,
}

impl LspManager {
    fn new() -> Self {
        Self {
            servers: HashMap::new(),
        }
    }

    /// Get the global LSP manager instance.
    pub fn global() -> Arc<Mutex<Self>> {
        GLOBAL.clone()
    }

    /// Get an existing server or start a new one for the given file.
    ///
    /// Returns `None` if the language has no known LSP config or the server
    /// fails to start (graceful degradation).
    pub async fn get_or_start(
        &mut self,
        file_path: &str,
        project_root: &str,
    ) -> Option<&LspServer> {
        let ext = std::path::Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let language_id = config::extension_to_language_id(ext)?;
        let server_config = config::default_config_for_language(language_id)?;

        let key = format!("{}:{}", language_id, project_root);

        // Return existing server if running
        if self.servers.contains_key(&key) {
            return self.servers.get(&key);
        }

        // Start new server
        tracing::info!(
            language_id,
            project_root,
            "Starting LSP server for language"
        );

        match LspServer::start(server_config, project_root).await {
            Ok(server) => {
                self.servers.insert(key.clone(), server);
                self.servers.get(&key)
            }
            Err(e) => {
                tracing::warn!("LSP server failed to start: {}", e);
                None
            }
        }
    }

    /// Stop all running LSP servers.
    pub async fn stop_all(&mut self) {
        let keys: Vec<String> = self.servers.keys().cloned().collect();
        for key in keys {
            if let Some(mut server) = self.servers.remove(&key) {
                server.stop().await;
            }
        }
        tracing::info!("All LSP servers stopped");
    }
}
