//! JSON-RPC client over stdio for LSP communication
//!
//! Implements the LSP base protocol with Content-Length framing.
//! Handles sending requests/notifications and dispatching responses.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout};
use tokio::sync::{Mutex, oneshot};

use crate::error::{Result, VenoreError};

/// JSON-RPC client that writes to a child process's stdin.
pub struct JsonRpcClient {
    writer: Arc<Mutex<tokio::io::BufWriter<ChildStdin>>>,
    next_id: AtomicI64,
    pending: Arc<Mutex<HashMap<i64, oneshot::Sender<serde_json::Value>>>>,
}

impl JsonRpcClient {
    /// Create a new client wrapping the child's stdin.
    pub fn new(
        stdin: ChildStdin,
        pending: Arc<Mutex<HashMap<i64, oneshot::Sender<serde_json::Value>>>>,
    ) -> Self {
        Self {
            writer: Arc::new(Mutex::new(tokio::io::BufWriter::new(stdin))),
            next_id: AtomicI64::new(1),
            pending,
        }
    }

    /// Send a JSON-RPC request and wait for the response.
    pub async fn send_request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(id, tx);
        }

        self.write_message(&msg).await?;

        rx.await.map_err(|_| {
            VenoreError::LspError(format!("Response channel closed for request {}", id))
        })
    }

    /// Send a JSON-RPC notification (no response expected).
    pub async fn send_notification(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<()> {
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });

        self.write_message(&msg).await
    }

    /// Write a JSON-RPC message with Content-Length framing.
    async fn write_message(&self, msg: &serde_json::Value) -> Result<()> {
        let body = serde_json::to_string(msg)?;
        let header = format!("Content-Length: {}\r\n\r\n", body.len());

        let mut writer = self.writer.lock().await;
        writer
            .write_all(header.as_bytes())
            .await
            .map_err(|e| VenoreError::LspError(format!("Failed to write header: {}", e)))?;
        writer
            .write_all(body.as_bytes())
            .await
            .map_err(|e| VenoreError::LspError(format!("Failed to write body: {}", e)))?;
        writer
            .flush()
            .await
            .map_err(|e| VenoreError::LspError(format!("Failed to flush: {}", e)))?;

        Ok(())
    }
}

/// Read JSON-RPC messages from stdout in a loop.
///
/// Dispatches:
/// - Responses (with `id`) → oneshot channel of the matching pending request
/// - Notifications (without `id`) → `on_notification` callback
pub async fn reader_loop(
    stdout: ChildStdout,
    pending: Arc<Mutex<HashMap<i64, oneshot::Sender<serde_json::Value>>>>,
    on_notification: impl Fn(&str, serde_json::Value) + Send + 'static,
) {
    let mut reader = BufReader::new(stdout);

    loop {
        // Parse Content-Length header
        let content_length = match read_content_length(&mut reader).await {
            Some(len) => len,
            None => break, // EOF or parse error → server closed
        };

        // Read the body
        let mut body_buf = vec![0u8; content_length];
        if reader.read_exact(&mut body_buf).await.is_err() {
            break;
        }

        let body = match String::from_utf8(body_buf) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let msg: serde_json::Value = match serde_json::from_str(&body) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Dispatch based on whether message has an `id` field
        if let Some(id) = msg.get("id").and_then(|v| v.as_i64()) {
            // Response to a request
            let mut pending = pending.lock().await;
            if let Some(tx) = pending.remove(&id) {
                let result = if let Some(result) = msg.get("result") {
                    result.clone()
                } else if let Some(error) = msg.get("error") {
                    error.clone()
                } else {
                    serde_json::Value::Null
                };
                let _ = tx.send(result);
            }
        } else if let Some(method) = msg.get("method").and_then(|v| v.as_str()) {
            // Server notification
            let params = msg.get("params").cloned().unwrap_or(serde_json::Value::Null);
            on_notification(method, params);
        }
    }

    tracing::debug!("LSP reader loop ended");
}

/// Read headers until we find Content-Length, then consume the empty line.
async fn read_content_length(reader: &mut BufReader<ChildStdout>) -> Option<usize> {
    let mut content_length: Option<usize> = None;

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line).await {
            Ok(0) => return None, // EOF
            Ok(_) => {}
            Err(_) => return None,
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            // End of headers
            break;
        }

        if let Some(value) = trimmed.strip_prefix("Content-Length: ") {
            content_length = value.parse().ok();
        }
        // Ignore other headers (Content-Type, etc.)
    }

    content_length
}

/// Encode a JSON value as a Content-Length framed message (for testing).
pub fn encode_message(value: &serde_json::Value) -> Vec<u8> {
    let body = serde_json::to_string(value).unwrap();
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    let mut result = header.into_bytes();
    result.extend_from_slice(body.as_bytes());
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_message_format() {
        let msg = serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "test"});
        let encoded = encode_message(&msg);
        let text = String::from_utf8(encoded).unwrap();

        assert!(text.starts_with("Content-Length: "));
        assert!(text.contains("\r\n\r\n"));

        let parts: Vec<&str> = text.splitn(2, "\r\n\r\n").collect();
        let header = parts[0];
        let body = parts[1];

        let declared_len: usize = header
            .strip_prefix("Content-Length: ")
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(declared_len, body.len());
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let original = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 42,
            "result": {"capabilities": {}}
        });

        let encoded = encode_message(&original);
        let text = String::from_utf8(encoded).unwrap();

        let parts: Vec<&str> = text.splitn(2, "\r\n\r\n").collect();
        let body = parts[1];
        let decoded: serde_json::Value = serde_json::from_str(body).unwrap();

        assert_eq!(original, decoded);
    }

    #[test]
    fn test_encode_notification_no_id() {
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {"uri": "file:///test.ts", "diagnostics": []}
        });

        let encoded = encode_message(&notification);
        let text = String::from_utf8(encoded).unwrap();

        let parts: Vec<&str> = text.splitn(2, "\r\n\r\n").collect();
        let body = parts[1];
        let decoded: serde_json::Value = serde_json::from_str(body).unwrap();

        assert!(decoded.get("id").is_none());
        assert_eq!(
            decoded["method"].as_str().unwrap(),
            "textDocument/publishDiagnostics"
        );
    }
}
