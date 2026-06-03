//! Post-edit diagnostics facade
//!
//! High-level function called from chat.rs after file edits.
//! Fetches LSP diagnostics and formats them for the LLM to consume.

use crate::lsp::manager::LspManager;

/// A single diagnostic entry from the LSP server.
#[derive(Debug, Clone)]
pub struct DiagnosticEntry {
    pub severity: DiagnosticSeverity,
    pub message: String,
    /// 1-based line number
    pub line: u32,
    /// 1-based column number
    pub column: u32,
    pub source: Option<String>,
}

/// Diagnostic severity level.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

impl DiagnosticSeverity {
    /// Label for display in formatted output.
    fn label(&self) -> &'static str {
        match self {
            DiagnosticSeverity::Error => "ERROR",
            DiagnosticSeverity::Warning => "WARNING",
            DiagnosticSeverity::Information => "INFO",
            DiagnosticSeverity::Hint => "HINT",
        }
    }
}

/// Fetch post-edit diagnostics for a file.
///
/// This is the main entry point called from the chat agentic loop after
/// `write_file`, `edit_file`, or `multi_edit_file` succeed.
///
/// Returns `None` if:
/// - No LSP server is configured for this language
/// - The server fails to start (binary not installed)
/// - No errors/warnings are found
///
/// Returns `Some(formatted_string)` with errors/warnings to append to tool output.
pub async fn fetch_post_edit_diagnostics(
    file_path: &str,
    project_root: &str,
    timeout_ms: u64,
) -> Option<String> {
    let mgr_arc = LspManager::global();
    let mut mgr = mgr_arc.lock().await;

    let server = mgr.get_or_start(file_path, project_root).await?;

    // Read the current file content to send to the server
    let content = tokio::fs::read_to_string(file_path).await.ok()?;

    // Notify the server about the file change
    if let Err(e) = server.notify_file_changed(file_path, &content).await {
        tracing::debug!("Failed to notify LSP of file change: {}", e);
        return None;
    }

    // Wait for diagnostics with timeout
    let diagnostics = server.wait_for_diagnostics(file_path, timeout_ms).await;

    // Filter to errors and warnings only
    let relevant: Vec<&DiagnosticEntry> = diagnostics
        .iter()
        .filter(|d| matches!(d.severity, DiagnosticSeverity::Error | DiagnosticSeverity::Warning))
        .collect();

    if relevant.is_empty() {
        return None;
    }

    Some(format_diagnostics(file_path, &relevant))
}

/// Format diagnostics into a human-readable string for the LLM.
fn format_diagnostics(file_path: &str, diagnostics: &[&DiagnosticEntry]) -> String {
    // Use just the filename for display
    let display_path = std::path::Path::new(file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(file_path);

    let mut lines = vec![format!("LSP diagnostics for {}:", display_path)];

    for diag in diagnostics {
        let source_tag = diag
            .source
            .as_ref()
            .map(|s| format!(" [{}]", s))
            .unwrap_or_default();

        lines.push(format!(
            "  {} line {}: {}{}",
            diag.severity.label(),
            diag.line,
            diag.message,
            source_tag,
        ));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_diagnostics_errors() {
        let entries = [DiagnosticEntry {
                severity: DiagnosticSeverity::Error,
                message: "Property 'name' does not exist on type 'User'.".to_string(),
                line: 42,
                column: 10,
                source: Some("typescript".to_string()),
            },
            DiagnosticEntry {
                severity: DiagnosticSeverity::Warning,
                message: "'unused_var' is declared but its value is never read.".to_string(),
                line: 12,
                column: 5,
                source: Some("typescript".to_string()),
            }];

        let refs: Vec<&DiagnosticEntry> = entries.iter().collect();
        let output = format_diagnostics("/home/user/src/parser.ts", &refs);

        assert!(output.starts_with("LSP diagnostics for parser.ts:"));
        assert!(output.contains("ERROR line 42:"));
        assert!(output.contains("Property 'name' does not exist"));
        assert!(output.contains("[typescript]"));
        assert!(output.contains("WARNING line 12:"));
        assert!(output.contains("'unused_var'"));
    }

    #[test]
    fn test_format_diagnostics_no_source() {
        let entries = [DiagnosticEntry {
            severity: DiagnosticSeverity::Error,
            message: "syntax error".to_string(),
            line: 1,
            column: 1,
            source: None,
        }];

        let refs: Vec<&DiagnosticEntry> = entries.iter().collect();
        let output = format_diagnostics("test.rs", &refs);

        assert!(output.contains("ERROR line 1: syntax error"));
        assert!(!output.contains("[]"));
    }

    #[test]
    fn test_severity_labels() {
        assert_eq!(DiagnosticSeverity::Error.label(), "ERROR");
        assert_eq!(DiagnosticSeverity::Warning.label(), "WARNING");
        assert_eq!(DiagnosticSeverity::Information.label(), "INFO");
        assert_eq!(DiagnosticSeverity::Hint.label(), "HINT");
    }

    #[test]
    #[cfg(windows)] // Backslash basename extraction only applies on Windows
    fn test_format_diagnostics_windows_path() {
        let entries = [DiagnosticEntry {
            severity: DiagnosticSeverity::Error,
            message: "cannot find module".to_string(),
            line: 5,
            column: 1,
            source: None,
        }];

        let refs: Vec<&DiagnosticEntry> = entries.iter().collect();
        let output = format_diagnostics("D:\\project\\src\\main.ts", &refs);

        assert!(output.starts_with("LSP diagnostics for main.ts:"));
    }
}
