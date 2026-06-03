//! PTY debug log — append-only JSONL trace of raw bytes flowing through
//! every terminal session.
//!
//! Enabled by setting `VENORE_PTY_DEBUG=1`. When the flag is unset the
//! logger is a no-op (early return, zero allocations).
//!
//! Purpose: diagnose ConPTY/xterm interaction issues (escape-sequence
//! behaviour, `cls`/`clear` quirks, ANSI parsing) by capturing the exact
//! bytes that flow in and out of each PTY.
//!
//! Path resolution (first match wins):
//!   1. `$VENORE_PTY_DEBUG_LOG` — explicit override
//!   2. `%TEMP%/venore-dev/pty-debug.jsonl` — debug builds
//!   3. `~/.venore/pty-debug.jsonl` — release builds
//!
//! Format: one JSON object per line, `\n` terminated. Each entry has
//! `ts`, `terminal_id`, `dir` (read|write), `len`, `hex`, `text`.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use once_cell::sync::Lazy;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct PtyDebugEntry<'a> {
    ts: String,
    terminal_id: &'a str,
    dir: &'a str,
    len: usize,
    hex: String,
    text: String,
}

static LOG_FILE: Lazy<Mutex<Option<File>>> = Lazy::new(|| Mutex::new(open_log_file()));

fn enabled() -> bool {
    matches!(
        std::env::var("VENORE_PTY_DEBUG").as_deref(),
        Ok("1") | Ok("true")
    )
}

fn resolve_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("VENORE_PTY_DEBUG_LOG") {
        return Some(PathBuf::from(p));
    }
    let dir = if cfg!(debug_assertions) {
        std::env::temp_dir().join("venore-dev")
    } else {
        dirs::home_dir()?.join(".venore")
    };
    if !dir.exists() {
        let _ = std::fs::create_dir_all(&dir);
    }
    Some(dir.join("pty-debug.jsonl"))
}

fn open_log_file() -> Option<File> {
    if !enabled() {
        return None;
    }
    let path = resolve_path()?;
    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(f) => {
            tracing::info!(path = %path.display(), "PTY debug log open");
            Some(f)
        }
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "Could not open PTY debug log");
            None
        }
    }
}

fn hex_dump(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        let _ = write!(s, "{:02x}", b);
    }
    s
}

fn escape_printable(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len());
    for &b in bytes {
        match b {
            0x1b => s.push_str("\\x1b"),
            b'\r' => s.push_str("\\r"),
            b'\n' => s.push_str("\\n"),
            b'\t' => s.push_str("\\t"),
            b'\\' => s.push_str("\\\\"),
            0x20..=0x7e => s.push(b as char),
            _ => {
                let _ = write!(s, "\\x{:02x}", b);
            }
        }
    }
    s
}

/// Append one read/write entry. No-op when `VENORE_PTY_DEBUG` is unset.
pub fn log(terminal_id: &str, dir: &str, bytes: &[u8]) {
    if !enabled() {
        return;
    }
    let entry = PtyDebugEntry {
        ts: chrono::Utc::now().to_rfc3339(),
        terminal_id,
        dir,
        len: bytes.len(),
        hex: hex_dump(bytes),
        text: escape_printable(bytes),
    };

    let line = match serde_json::to_string(&entry) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "Could not serialize PTY debug entry");
            return;
        }
    };

    let Ok(mut guard) = LOG_FILE.lock() else {
        return;
    };
    let Some(file) = guard.as_mut() else {
        return;
    };

    if let Err(e) = writeln!(file, "{}", line) {
        tracing::warn!(error = %e, "Failed writing PTY debug log");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_dump_pads_each_byte_to_two_chars() {
        assert_eq!(hex_dump(&[0x1b, 0x5b, 0x32, 0x4a]), "1b 5b 32 4a");
        assert_eq!(hex_dump(&[0x00, 0x0f, 0xff]), "00 0f ff");
        assert_eq!(hex_dump(&[]), "");
    }

    #[test]
    fn escape_printable_keeps_ascii_and_escapes_control() {
        assert_eq!(escape_printable(b"cls\r"), "cls\\r");
        assert_eq!(
            escape_printable(&[0x1b, b'[', b'2', b'J']),
            "\\x1b[2J"
        );
        assert_eq!(escape_printable(&[0xff, 0x80]), "\\xff\\x80");
        assert_eq!(escape_printable(b"a\\b"), "a\\\\b");
    }

    #[test]
    fn resolve_path_honours_env_override() {
        std::env::set_var("VENORE_PTY_DEBUG_LOG", "/tmp/pty-test.jsonl");
        let p = resolve_path().unwrap();
        assert_eq!(p.to_string_lossy(), "/tmp/pty-test.jsonl");
        std::env::remove_var("VENORE_PTY_DEBUG_LOG");
    }
}
