//! Terminal Session Manager — singleton managing multiple PTY sessions
//!
//! Pattern: `once_cell::Lazy<Arc<Mutex<Self>>>` (same as WizardSessionManager).

use std::collections::{HashMap, VecDeque};
use std::io::Read;
use std::sync::{Arc, Mutex};

use once_cell::sync::Lazy;
use tracing::{info, warn};

use crate::error::{Result, VenoreError};
use super::session::TerminalSession;

// =============================================================================
// TerminalSessionManager
// =============================================================================

const MAX_BUFFER_LINES: usize = 500;

pub struct TerminalSessionManager {
    sessions: HashMap<String, TerminalSession>,
    counter: u32,
    output_buffers: HashMap<String, VecDeque<String>>,
    /// Monotonic line counter per terminal — increments per line, never resets.
    /// Used to track output position for echo-skip and baseline-based reads.
    line_counters: HashMap<String, u64>,
    /// Bidirectional mapping: dev_session_id → terminal_id
    session_to_terminal: HashMap<String, String>,
    /// Bidirectional mapping: terminal_id → dev_session_id
    terminal_to_session: HashMap<String, String>,
}

impl TerminalSessionManager {
    fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            counter: 0,
            output_buffers: HashMap::new(),
            line_counters: HashMap::new(),
            session_to_terminal: HashMap::new(),
            terminal_to_session: HashMap::new(),
        }
    }

    /// Global singleton instance.
    pub fn global() -> Arc<Mutex<Self>> {
        static INSTANCE: Lazy<Arc<Mutex<TerminalSessionManager>>> =
            Lazy::new(|| Arc::new(Mutex::new(TerminalSessionManager::new())));
        INSTANCE.clone()
    }

    /// Spawn a new terminal session. Returns `(terminal_id, reader_arc)`.
    ///
    /// The reader is returned separately so the caller can start a read-loop
    /// without holding the manager lock.
    ///
    /// `label` sets a short prompt label. If `None`, derived from `cwd`.
    pub fn spawn(
        &mut self,
        cwd: &str,
        cols: u16,
        rows: u16,
        label: Option<&str>,
    ) -> Result<(String, Arc<Mutex<Box<dyn Read + Send>>>)> {
        self.counter += 1;
        let id = format!("terminal-{}", self.counter);

        let session = TerminalSession::spawn(id.clone(), cwd, cols, rows, label)?;
        let reader = session.clone_reader();

        self.sessions.insert(id.clone(), session);
        info!(id = %id, total = self.sessions.len(), "terminal session created");

        Ok((id, reader))
    }

    /// Write data to a terminal session.
    pub fn write(&self, id: &str, data: &[u8]) -> Result<()> {
        let session = self
            .sessions
            .get(id)
            .ok_or_else(|| VenoreError::TerminalSessionNotFound(id.to_string()))?;
        super::debug::log(id, "write", data);
        session.write(data)
    }

    /// Resize a terminal session.
    pub fn resize(&mut self, id: &str, cols: u16, rows: u16) -> Result<()> {
        let session = self
            .sessions
            .get_mut(id)
            .ok_or_else(|| VenoreError::TerminalSessionNotFound(id.to_string()))?;
        session.resize(cols, rows)
    }

    /// Kill and remove a terminal session. Also cleans up any session binding.
    pub fn kill(&mut self, id: &str) -> Result<()> {
        let mut session = self
            .sessions
            .remove(id)
            .ok_or_else(|| VenoreError::TerminalSessionNotFound(id.to_string()))?;
        session.kill();
        // Clean up session binding if this terminal was bound
        if let Some(dev_session_id) = self.terminal_to_session.remove(id) {
            self.session_to_terminal.remove(&dev_session_id);
        }
        info!(id = %id, remaining = self.sessions.len(), "terminal session killed");
        Ok(())
    }

    /// List all active session IDs.
    pub fn list(&self) -> Vec<String> {
        self.sessions.keys().cloned().collect()
    }

    /// Number of active sessions.
    pub fn count(&self) -> usize {
        self.sessions.len()
    }

    /// Remove all sessions.
    pub fn clear(&mut self) {
        for (id, mut session) in self.sessions.drain() {
            warn!(id = %id, "clearing terminal session");
            session.kill();
        }
        self.output_buffers.clear();
        self.line_counters.clear();
        self.session_to_terminal.clear();
        self.terminal_to_session.clear();
    }

    /// Append output data to the terminal's buffer (called from read-loop).
    /// ANSI escape sequences are stripped so the AI reads clean text.
    /// The frontend still receives raw data with ANSI codes for xterm.js rendering.
    pub fn append_output(&mut self, id: &str, data: &str) {
        let clean = crate::utils::strip_ansi_escapes(data);
        let buffer = self.output_buffers.entry(id.to_string()).or_default();
        let counter = self.line_counters.entry(id.to_string()).or_insert(0);
        for line in clean.lines() {
            buffer.push_back(line.to_string());
            *counter += 1;
            if buffer.len() > MAX_BUFFER_LINES {
                buffer.pop_front();
            }
        }
    }

    /// Get the current monotonic line counter for a terminal.
    /// This value only increases and is used as a baseline for reading new output.
    pub fn line_counter(&self, id: &str) -> u64 {
        self.line_counters.get(id).copied().unwrap_or(0)
    }

    /// Get output lines produced after a given baseline counter value.
    /// Returns at most `max` lines. Used to read only new output after a command.
    pub fn get_output_after(&self, id: &str, after: u64, max: usize) -> Result<String> {
        let buffer = self.output_buffers.get(id)
            .ok_or_else(|| VenoreError::TerminalSessionNotFound(id.to_string()))?;
        let total = self.line_counters.get(id).copied().unwrap_or(0);
        // buffer_start is the line counter value of the first line still in the buffer
        let buffer_start = total.saturating_sub(buffer.len() as u64);
        let skip = after.saturating_sub(buffer_start) as usize;
        Ok(buffer.iter().skip(skip).take(max).cloned().collect::<Vec<_>>().join("\n"))
    }

    /// Remove a dead session without killing the child (already dead).
    /// Cleans up the session entry, output buffer, line counter, and session binding.
    pub fn remove_dead_session(&mut self, id: &str) {
        self.sessions.remove(id);
        self.output_buffers.remove(id);
        self.line_counters.remove(id);
        // Clean up session binding if this terminal was bound
        if let Some(dev_session_id) = self.terminal_to_session.remove(id) {
            self.session_to_terminal.remove(&dev_session_id);
        }
        info!(id = %id, remaining = self.sessions.len(), "dead terminal session removed");
    }

    // =========================================================================
    // Session ↔ Terminal binding
    // =========================================================================

    /// Bind a dev session to a terminal (1:1). Clears any previous bindings
    /// on both sides to maintain the bidirectional invariant.
    pub fn bind_session(&mut self, dev_session_id: &str, terminal_id: &str) {
        // Remove any previous binding for this dev session
        if let Some(old_tid) = self.session_to_terminal.remove(dev_session_id) {
            self.terminal_to_session.remove(&old_tid);
        }
        // Remove any previous binding for this terminal
        if let Some(old_sid) = self.terminal_to_session.remove(terminal_id) {
            self.session_to_terminal.remove(&old_sid);
        }
        self.session_to_terminal.insert(dev_session_id.to_string(), terminal_id.to_string());
        self.terminal_to_session.insert(terminal_id.to_string(), dev_session_id.to_string());
        info!(dev_session_id = %dev_session_id, terminal_id = %terminal_id, "bound session to terminal");
    }

    /// Get the terminal bound to a dev session, if it's still alive.
    pub fn get_session_terminal(&self, dev_session_id: &str) -> Option<&str> {
        self.session_to_terminal
            .get(dev_session_id)
            .filter(|tid| self.sessions.contains_key(tid.as_str()))
            .map(|s| s.as_str())
    }

    /// Unbind a dev session from its terminal.
    pub fn unbind_session(&mut self, dev_session_id: &str) {
        if let Some(tid) = self.session_to_terminal.remove(dev_session_id) {
            self.terminal_to_session.remove(&tid);
            info!(dev_session_id = %dev_session_id, "unbound session from terminal");
        }
    }

    /// List terminal IDs that are NOT bound to any dev session (unbound terminals).
    pub fn list_unbound(&self) -> Vec<String> {
        self.sessions
            .keys()
            .filter(|id| !self.terminal_to_session.contains_key(id.as_str()))
            .cloned()
            .collect()
    }

    /// Check whether a terminal is bound to a dev session.
    pub fn is_session_terminal(&self, terminal_id: &str) -> bool {
        self.terminal_to_session.contains_key(terminal_id)
    }

    /// Get recent output lines from a terminal.
    pub fn get_recent_output(&self, id: &str, lines: usize) -> Result<String> {
        let buffer = self.output_buffers.get(id)
            .ok_or_else(|| VenoreError::TerminalSessionNotFound(id.to_string()))?;
        let start = buffer.len().saturating_sub(lines);
        Ok(buffer.iter().skip(start).cloned().collect::<Vec<_>>().join("\n"))
    }
}
