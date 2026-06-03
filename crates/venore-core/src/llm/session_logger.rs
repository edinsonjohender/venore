//! Session Logging System
//!
//! Logs LLM sessions to JSONL files for debugging, metrics, and traceability.
//!
//! ## Usage
//!
//! ```rust
//! use venore_core::llm::session_logger::{SessionLogger, SessionEvent};
//!
//! # async fn example() -> venore_core::Result<()> {
//! let logger = SessionLogger::new("analysis");
//!
//! logger.log(SessionEvent::Started {
//!     timestamp: chrono::Utc::now().to_rfc3339(),
//!     task: "analysis".to_string(),
//!     provider: "anthropic".to_string(),
//!     model: "claude-sonnet-4-5".to_string(),
//! }).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Output Location
//!
//! - Windows: `%APPDATA%\venore\sessions\{task}-{session_id}.jsonl`
//! - macOS: `~/Library/Application Support/venore/sessions/{task}-{session_id}.jsonl`
//! - Linux: `~/.local/share/venore/sessions/{{task}-{session_id}.jsonl`
//!
//! ## Environment Variables
//!
//! - `VENORE_SESSION_LOGGING=1` or `VENORE_SESSION_LOGGING=true`: Enable logging

use crate::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

// ============================================================================
// SESSION EVENTS
// ============================================================================

/// Events that can be logged during an LLM session
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum SessionEvent {
    /// Session started
    Started {
        timestamp: String,
        task: String,
        provider: String,
        model: String,
    },
    /// Request sent to LLM provider
    RequestSent {
        timestamp: String,
        messages: usize,
        max_tokens: Option<u32>,
    },
    /// Response received from LLM provider
    ResponseReceived {
        timestamp: String,
        completion_tokens: u32,
        prompt_tokens: u32,
        total_tokens: u32,
        duration_ms: u64,
    },
    /// Retry attempt due to error
    RetryAttempt {
        timestamp: String,
        attempt: u32,
        reason: String,
        delay_ms: u64,
    },
    /// Fallback to different provider
    FallbackProvider {
        timestamp: String,
        from: String,
        to: String,
        reason: String,
    },
    /// Error occurred
    Error {
        timestamp: String,
        error_type: String,
        message: String,
    },
    /// Session completed
    Completed {
        timestamp: String,
        total_duration_ms: u64,
        total_tokens: u32,
    },
}

// ============================================================================
// SESSION LOGGER
// ============================================================================

/// Session logger that writes events to JSONL files
pub struct SessionLogger {
    session_id: String,
    log_path: PathBuf,
    enabled: bool,
}

impl SessionLogger {
    /// Create a new session logger
    ///
    /// The logger is only enabled if `VENORE_SESSION_LOGGING=1` or `VENORE_SESSION_LOGGING=true`
    /// is set in environment variables.
    ///
    /// # Arguments
    ///
    /// * `task` - Task name (e.g., "analysis", "module_detection", "wizard")
    ///
    /// # Example
    ///
    /// ```rust
    /// # use venore_core::llm::session_logger::SessionLogger;
    /// let logger = SessionLogger::new("analysis");
    /// ```
    pub fn new(task: &str) -> Self {
        let session_id = uuid::Uuid::new_v4().to_string();

        // Get data directory based on OS
        let log_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("venore")
            .join("sessions");

        // Create directory if it doesn't exist
        std::fs::create_dir_all(&log_dir).ok();

        let log_path = log_dir.join(format!("{}-{}.jsonl", task, session_id));

        Self {
            session_id,
            log_path,
            enabled: std::env::var("VENORE_SESSION_LOGGING")
                .map(|v| v == "1" || v.to_lowercase() == "true")
                .unwrap_or(false),
        }
    }

    /// Log an event to the session file
    ///
    /// If logging is disabled, this is a no-op.
    ///
    /// # Arguments
    ///
    /// * `event` - Event to log
    ///
    /// # Example
    ///
    /// ```rust
    /// # use venore_core::llm::session_logger::{SessionLogger, SessionEvent};
    /// # async fn example(logger: SessionLogger) -> venore_core::Result<()> {
    /// logger.log(SessionEvent::Started {
    ///     timestamp: chrono::Utc::now().to_rfc3339(),
    ///     task: "analysis".to_string(),
    ///     provider: "anthropic".to_string(),
    ///     model: "claude-sonnet-4-5".to_string(),
    /// }).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn log(&self, event: SessionEvent) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let json = serde_json::to_string(&event)?;
        let line = format!("{}\n", json);

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .await?;

        file.write_all(line.as_bytes()).await?;
        file.flush().await?;

        Ok(())
    }

    /// Get the session ID
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Get the log file path
    pub fn log_path(&self) -> &Path {
        &self.log_path
    }

    /// Check if logging is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Create a timestamp in RFC3339 format
pub fn timestamp() -> String {
    Utc::now().to_rfc3339()
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;
    use tokio::io::AsyncReadExt;

    #[test]
    #[serial]
    fn test_logger_disabled_by_default() {
        env::remove_var("VENORE_SESSION_LOGGING");
        let logger = SessionLogger::new("test");
        assert!(!logger.is_enabled());
    }

    #[test]
    #[serial]
    fn test_logger_enabled_with_env_var() {
        env::set_var("VENORE_SESSION_LOGGING", "1");
        let logger = SessionLogger::new("test");
        assert!(logger.is_enabled());
        env::remove_var("VENORE_SESSION_LOGGING");
    }

    #[test]
    #[serial]
    fn test_logger_enabled_with_true() {
        env::set_var("VENORE_SESSION_LOGGING", "true");
        let logger = SessionLogger::new("test");
        assert!(logger.is_enabled());
        env::remove_var("VENORE_SESSION_LOGGING");
    }

    #[test]
    #[serial]
    fn test_logger_disabled_with_false() {
        env::set_var("VENORE_SESSION_LOGGING", "false");
        let logger = SessionLogger::new("test");
        assert!(!logger.is_enabled());
        env::remove_var("VENORE_SESSION_LOGGING");
    }

    #[test]
    fn test_logger_has_session_id() {
        let logger = SessionLogger::new("test");
        assert!(!logger.session_id().is_empty());
    }

    #[test]
    fn test_logger_has_log_path() {
        let logger = SessionLogger::new("test");
        let path = logger.log_path();
        assert!(path.to_string_lossy().contains("test-"));
        assert!(path.extension().unwrap() == "jsonl");
    }

    #[tokio::test]
    #[serial]
    async fn test_log_event_when_disabled() {
        env::remove_var("VENORE_SESSION_LOGGING");
        let logger = SessionLogger::new("test");

        let event = SessionEvent::Started {
            timestamp: timestamp(),
            task: "test".to_string(),
            provider: "anthropic".to_string(),
            model: "claude-sonnet-4-5".to_string(),
        };

        // Should not error even if disabled
        let result = logger.log(event).await;
        assert!(result.is_ok());

        // File should not exist
        assert!(!logger.log_path().exists());
    }

    #[tokio::test]
    #[serial]
    async fn test_log_event_when_enabled() {
        env::set_var("VENORE_SESSION_LOGGING", "1");
        let logger = SessionLogger::new("test_enabled");

        let event = SessionEvent::Started {
            timestamp: timestamp(),
            task: "test".to_string(),
            provider: "anthropic".to_string(),
            model: "claude-sonnet-4-5".to_string(),
        };

        let result = logger.log(event).await;
        assert!(result.is_ok());

        // File should exist
        assert!(logger.log_path().exists());

        // Read file content
        let mut file = tokio::fs::File::open(logger.log_path()).await.unwrap();
        let mut content = String::new();
        file.read_to_string(&mut content).await.unwrap();

        // Should contain JSON
        assert!(content.contains("\"event\":\"started\""));
        assert!(content.contains("\"provider\":\"anthropic\""));

        // Cleanup
        tokio::fs::remove_file(logger.log_path()).await.ok();
        env::remove_var("VENORE_SESSION_LOGGING");
    }

    #[tokio::test]
    #[serial]
    async fn test_log_multiple_events() {
        env::set_var("VENORE_SESSION_LOGGING", "1");
        let logger = SessionLogger::new("test_multiple");

        let events = vec![
            SessionEvent::Started {
                timestamp: timestamp(),
                task: "test".to_string(),
                provider: "anthropic".to_string(),
                model: "claude-sonnet-4-5".to_string(),
            },
            SessionEvent::RequestSent {
                timestamp: timestamp(),
                messages: 2,
                max_tokens: Some(1000),
            },
            SessionEvent::ResponseReceived {
                timestamp: timestamp(),
                completion_tokens: 150,
                prompt_tokens: 50,
                total_tokens: 200,
                duration_ms: 1234,
            },
        ];

        for event in events {
            logger.log(event).await.unwrap();
        }

        // Read file content
        let mut file = tokio::fs::File::open(logger.log_path()).await.unwrap();
        let mut content = String::new();
        file.read_to_string(&mut content).await.unwrap();

        // Should have 3 lines
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 3);

        // Each line should be valid JSON
        for line in lines {
            let _: SessionEvent = serde_json::from_str(line).unwrap();
        }

        // Cleanup
        tokio::fs::remove_file(logger.log_path()).await.ok();
        env::remove_var("VENORE_SESSION_LOGGING");
    }

    #[test]
    fn test_session_event_serialization() {
        let event = SessionEvent::Started {
            timestamp: "2025-01-27T10:00:00Z".to_string(),
            task: "analysis".to_string(),
            provider: "anthropic".to_string(),
            model: "claude-sonnet-4-5".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"event\":\"started\""));
        assert!(json.contains("\"task\":\"analysis\""));
        assert!(json.contains("\"provider\":\"anthropic\""));
    }

    #[test]
    fn test_retry_attempt_event() {
        let event = SessionEvent::RetryAttempt {
            timestamp: timestamp(),
            attempt: 1,
            reason: "Rate limit exceeded".to_string(),
            delay_ms: 2000,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"event\":\"retry_attempt\""));
        assert!(json.contains("\"attempt\":1"));
        assert!(json.contains("\"delay_ms\":2000"));
    }

    #[test]
    fn test_fallback_provider_event() {
        let event = SessionEvent::FallbackProvider {
            timestamp: timestamp(),
            from: "anthropic".to_string(),
            to: "openai".to_string(),
            reason: "Rate limit exceeded".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"event\":\"fallback_provider\""));
        assert!(json.contains("\"from\":\"anthropic\""));
        assert!(json.contains("\"to\":\"openai\""));
    }

    #[test]
    fn test_error_event() {
        let event = SessionEvent::Error {
            timestamp: timestamp(),
            error_type: "LLM_RATE_LIMIT".to_string(),
            message: "Rate limit exceeded".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"event\":\"error\""));
        assert!(json.contains("\"error_type\":\"LLM_RATE_LIMIT\""));
    }

    #[test]
    fn test_timestamp_format() {
        let ts = timestamp();
        // Should be in RFC3339 format
        assert!(chrono::DateTime::parse_from_rfc3339(&ts).is_ok());
    }
}
