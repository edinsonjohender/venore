//! Terminal session — wrapper around portable-pty
//!
//! Each session owns a PTY process (shell), providing write, resize, and kill.
//! The reader is cloneable (Arc) so the Tauri read-loop can hold it without
//! locking the entire manager.

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use tracing::{debug, info, warn};

use crate::error::{Result, VenoreError};

// =============================================================================
// TerminalSession
// =============================================================================

pub struct TerminalSession {
    id: String,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    reader: Arc<Mutex<Box<dyn Read + Send>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    cols: u16,
    rows: u16,
}

impl TerminalSession {
    /// Spawn a new PTY session with the OS default shell.
    ///
    /// - Windows: `cmd.exe`
    /// - Unix: `$SHELL` (fallback `/bin/sh`)
    ///
    /// `label` sets a short prompt (e.g. directory name). If `None`, the last
    /// segment of `cwd` is used automatically.
    pub fn spawn(id: String, cwd: &str, cols: u16, rows: u16, label: Option<&str>) -> Result<Self> {
        info!(id = %id, cwd = %cwd, cols, rows, "spawning terminal session");

        // Derive label from CWD last segment if not provided
        let effective_label = label
            .map(|l| l.to_string())
            .unwrap_or_else(|| {
                std::path::Path::new(cwd)
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_else(|| "venore".to_string())
            });

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| VenoreError::TerminalSpawnFailed(e.to_string()))?;

        let mut cmd = Self::build_shell_command(&effective_label);
        cmd.cwd(cwd);

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| VenoreError::TerminalSpawnFailed(e.to_string()))?;

        // Drop the slave — we only need the master side
        drop(pair.slave);

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| VenoreError::TerminalSpawnFailed(e.to_string()))?;

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| VenoreError::TerminalSpawnFailed(e.to_string()))?;

        info!(id = %id, "terminal session spawned successfully");

        Ok(Self {
            id,
            master: pair.master,
            child,
            reader: Arc::new(Mutex::new(reader)),
            writer: Arc::new(Mutex::new(writer)),
            cols,
            rows,
        })
    }

    /// Write data (keystrokes) to the PTY.
    pub fn write(&self, data: &[u8]) -> Result<()> {
        let mut writer = self.writer.lock()
            .map_err(|_| VenoreError::TerminalError("writer mutex poisoned".into()))?;
        writer
            .write_all(data)
            .map_err(|e| VenoreError::TerminalError(e.to_string()))?;
        writer
            .flush()
            .map_err(|e| VenoreError::TerminalError(e.to_string()))?;
        Ok(())
    }

    /// Resize the PTY.
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        debug!(id = %self.id, cols, rows, "resizing terminal");
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| VenoreError::TerminalError(e.to_string()))?;
        self.cols = cols;
        self.rows = rows;
        Ok(())
    }

    /// Kill the child process.
    pub fn kill(&mut self) {
        info!(id = %self.id, "killing terminal session");
        if let Err(e) = self.child.kill() {
            warn!(id = %self.id, error = %e, "failed to kill terminal child");
        }
    }

    /// Clone the reader Arc for the read-loop (avoids holding the manager lock).
    pub fn clone_reader(&self) -> Arc<Mutex<Box<dyn Read + Send>>> {
        Arc::clone(&self.reader)
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    // -------------------------------------------------------------------------
    // Private helpers
    // -------------------------------------------------------------------------

    #[cfg(windows)]
    fn build_shell_command(label: &str) -> CommandBuilder {
        // PowerShell 5.1 ships with every supported Windows version and has
        // `clear` (alias for Clear-Host) built-in alongside `cls`. cmd.exe —
        // the previous default — only knows `cls`, which surprises users
        // coming from bash/zsh.
        //
        // Resolve to an absolute path because Tauri's spawned process does not
        // always inherit a full PATH (CreateProcessW would fail with os error 2).
        // Prefer pwsh.exe (PS 7+) if present, otherwise fall back to Windows
        // PowerShell 5.1 which is guaranteed to exist under %SystemRoot%.
        let shell = resolve_powershell_path();
        let mut cmd = CommandBuilder::new(shell);
        cmd.arg("-NoLogo");
        cmd.arg("-NoExit");
        cmd.arg("-Command");
        // Override the prompt for this session. Escape single quotes in the
        // label by doubling (PowerShell literal-string convention).
        let safe_label = label.replace('\'', "''");
        cmd.arg(format!("function prompt {{ '[{}] > ' }}", safe_label));
        cmd
    }

    #[cfg(not(windows))]
    fn build_shell_command(label: &str) -> CommandBuilder {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let mut cmd = CommandBuilder::new(shell);
        // Custom prompt: [label] $
        cmd.env("PS1", format!("[{}] \\$ ", label));
        cmd
    }
}

#[cfg(windows)]
fn resolve_powershell_path() -> String {
    const PWSH_KNOWN: &[&str] = &[
        r"C:\Program Files\PowerShell\7\pwsh.exe",
        r"C:\Program Files (x86)\PowerShell\7\pwsh.exe",
    ];
    for p in PWSH_KNOWN {
        if std::path::Path::new(p).exists() {
            return (*p).to_string();
        }
    }
    let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    format!(
        r"{}\System32\WindowsPowerShell\v1.0\powershell.exe",
        system_root
    )
}
