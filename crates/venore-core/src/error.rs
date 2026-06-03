//! Error types para Venore
//!
//! Sistema de errores estructurado usando thiserror con 30+ variants
//! organizados por dominio (General, FileSystem, Parser, LLM, Database, etc.)

use thiserror::Error;

/// Tipo Result customizado para Venore
pub type Result<T> = std::result::Result<T, VenoreError>;

/// Todos los errores posibles en Venore
#[derive(Error, Debug, Clone)]
pub enum VenoreError {
    // =================================================================
    // GENERAL (6)
    // =================================================================
    #[error("Unknown error: {0}")]
    Unknown(String),

    #[error("Operation timed out after {0}ms")]
    Timeout(u64),

    #[error("Invalid parameters: {0}")]
    InvalidParams(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    // =================================================================
    // FILE SYSTEM (6)
    // =================================================================
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("File read error: {0}")]
    FileReadError(String),

    #[error("File write error: {0}")]
    FileWriteError(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Directory not found: {0}")]
    DirectoryNotFound(String),

    #[error("Path not safe: {0}")]
    PathNotSafe(String),

    // =================================================================
    // PARSER (3)
    // =================================================================
    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Invalid syntax in {file}: {message}")]
    InvalidSyntax { file: String, message: String },

    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),

    // =================================================================
    // ANALYSIS (1)
    // =================================================================
    #[error("Analysis error: {0}")]
    AnalysisError(String),

    // =================================================================
    // LLM (8)
    // =================================================================
    #[error("No API key configured for provider: {0}")]
    LlmNoApiKey(String),

    #[error("Invalid LLM provider: {0}")]
    LlmInvalidProvider(String),

    #[error("Invalid LLM request: {0}")]
    LlmInvalidRequest(String),

    #[error("LLM provider error: {0}")]
    LlmProviderError(String),

    #[error("LLM rate limit exceeded{}",
        .retry_after_secs.map(|s| format!(" (retry after {}s)", s)).unwrap_or_default()
    )]
    LlmRateLimit {
        /// Optional Retry-After value from server (in seconds)
        retry_after_secs: Option<u64>,
    },

    #[error("LLM context too long: {current} tokens (max: {max})")]
    LlmContextTooLong { current: usize, max: usize },

    #[error("LLM stream error: {0}")]
    LlmStreamError(String),

    #[error("Invalid LLM response: {0}")]
    LlmInvalidResponse(String),

    #[error("Model '{model}' is not available for provider '{provider}'")]
    LlmModelNotAvailable {
        provider: String,
        model: String,
    },

    // =================================================================
    // DATABASE (5)
    // =================================================================
    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Database not open")]
    DatabaseNotOpen,

    #[error("Database locked")]
    DatabaseLocked,

    #[error("Database query failed: {0}")]
    DatabaseQueryFailed(String),

    #[error("Database not initialized")]
    DatabaseNotInitialized,

    // =================================================================
    // CONTEXT GENERATION (4)
    // =================================================================
    #[error("Context generation failed: {0}")]
    ContextGenerationError(String),

    #[error("Context write failed: {0}")]
    ContextWriteError(String),

    #[error("Invalid context format: {0}")]
    InvalidContextFormat(String),

    #[error("Context generation cancelled")]
    ContextGenerationCancelled,

    #[error("Operation cancelled: {0}")]
    Cancelled(String),

    // =================================================================
    // GIT (4)
    // =================================================================
    #[error("Git error: {0}")]
    GitError(String),

    #[error("Not a git repository: {0}")]
    NotGitRepository(String),

    #[error("Git command failed: {0}")]
    GitCommandFailed(String),

    #[error("Invalid git reference: {0}")]
    InvalidGitReference(String),

    // =================================================================
    // RAG (4)
    // =================================================================
    #[error("RAG indexing error: {0}")]
    RagIndexError(String),

    #[error("RAG search error: {0}")]
    RagSearchError(String),

    #[error("RAG embedding error: {0}")]
    RagEmbeddingError(String),

    #[error("Graph query error: {0}")]
    GraphQueryError(String),

    // =================================================================
    // GITHUB (5)
    // =================================================================
    #[error("GitHub API error ({status}): {message}")]
    GitHubApiError { status: u16, message: String },

    #[error("GitHub authentication required")]
    GitHubAuthRequired,

    #[error("GitHub rate limit exceeded (resets in {reset_seconds}s)")]
    GitHubRateLimited { reset_seconds: u64 },

    #[error("GitHub device flow error: {0}")]
    GitHubDeviceFlowError(String),

    #[error("GitHub repo not detected for project: {0}")]
    GitHubRepoNotDetected(String),

    // =================================================================
    // TERMINAL (3)
    // =================================================================
    #[error("Terminal error: {0}")]
    TerminalError(String),

    #[error("Terminal session not found: {0}")]
    TerminalSessionNotFound(String),

    #[error("Terminal spawn failed: {0}")]
    TerminalSpawnFailed(String),

    // =================================================================
    // TOOLS (4)
    // =================================================================
    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Tool execution failed: {0}")]
    ToolExecutionFailed(String),

    #[error("Tool approval denied: {0}")]
    ToolApprovalDenied(String),

    #[error("Tool execution timed out: {0}")]
    ToolTimeout(String),

    // =================================================================
    // LSP (3)
    // =================================================================
    #[error("LSP error: {0}")]
    LspError(String),

    #[error("LSP server not running for language: {0}")]
    LspServerNotRunning(String),

    #[error("LSP server failed to start: {0}")]
    LspSpawnFailed(String),

    // =================================================================
    // MESH (3+1)
    // =================================================================
    #[error("Mesh error: {0}")]
    MeshError(String),

    #[error("Mesh peer not found: {0}")]
    MeshPeerNotFound(String),

    #[error("Mesh connection failed: {0}")]
    MeshConnectionFailed(String),

    #[error("Mesh transport not running")]
    MeshTransportNotRunning,

    // =================================================================
    // FROM CONVERSIONS (Standard library errors)
    // =================================================================
    #[error("IO error: {0}")]
    Io(String),

    #[error("JSON serialization error: {0}")]
    Json(String),

    #[error("UTF-8 error: {0}")]
    Utf8(String),
}

// =================================================================
// ERROR RESPONSE (Para IPC - Tauri)
// =================================================================

/// Error response para IPC (Tauri)
/// Serializable para enviar al frontend
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    pub timestamp: String,
}

impl From<VenoreError> for ErrorResponse {
    fn from(err: VenoreError) -> Self {
        // Extract the "variant name" as a code
        let code = match &err {
            VenoreError::Unknown(_) => "UNKNOWN_ERROR",
            VenoreError::Timeout(_) => "TIMEOUT",
            VenoreError::InvalidParams(_) => "INVALID_PARAMS",
            VenoreError::InvalidPath(_) => "INVALID_PATH",
            VenoreError::NotFound(_) => "NOT_FOUND",
            VenoreError::NotImplemented(_) => "NOT_IMPLEMENTED",

            VenoreError::FileNotFound(_) => "FILE_NOT_FOUND",
            VenoreError::FileReadError(_) => "FILE_READ_ERROR",
            VenoreError::FileWriteError(_) => "FILE_WRITE_ERROR",
            VenoreError::PermissionDenied(_) => "PERMISSION_DENIED",
            VenoreError::DirectoryNotFound(_) => "DIRECTORY_NOT_FOUND",
            VenoreError::PathNotSafe(_) => "PATH_NOT_SAFE",

            VenoreError::ParseError(_) => "PARSE_ERROR",
            VenoreError::InvalidSyntax { .. } => "INVALID_SYNTAX",
            VenoreError::UnsupportedFormat(_) => "UNSUPPORTED_FORMAT",

            VenoreError::AnalysisError(_) => "ANALYSIS_ERROR",

            VenoreError::LlmNoApiKey(_) => "LLM_NO_API_KEY",
            VenoreError::LlmInvalidProvider(_) => "LLM_INVALID_PROVIDER",
            VenoreError::LlmInvalidRequest(_) => "LLM_INVALID_REQUEST",
            VenoreError::LlmProviderError(_) => "LLM_PROVIDER_ERROR",
            VenoreError::LlmRateLimit { .. } => "LLM_RATE_LIMIT",
            VenoreError::LlmContextTooLong { .. } => "LLM_CONTEXT_TOO_LONG",
            VenoreError::LlmStreamError(_) => "LLM_STREAM_ERROR",
            VenoreError::LlmInvalidResponse(_) => "LLM_INVALID_RESPONSE",
            VenoreError::LlmModelNotAvailable { .. } => "LLM_MODEL_NOT_AVAILABLE",

            VenoreError::DatabaseError(_) => "DATABASE_ERROR",
            VenoreError::DatabaseNotOpen => "DATABASE_NOT_OPEN",
            VenoreError::DatabaseLocked => "DATABASE_LOCKED",
            VenoreError::DatabaseQueryFailed(_) => "DATABASE_QUERY_FAILED",
            VenoreError::DatabaseNotInitialized => "DATABASE_NOT_INITIALIZED",

            VenoreError::ContextGenerationError(_) => "CONTEXT_GENERATION_ERROR",
            VenoreError::ContextWriteError(_) => "CONTEXT_WRITE_ERROR",
            VenoreError::InvalidContextFormat(_) => "INVALID_CONTEXT_FORMAT",
            VenoreError::ContextGenerationCancelled => "CONTEXT_GENERATION_CANCELLED",
            VenoreError::Cancelled(_) => "CANCELLED",

            VenoreError::GitError(_) => "GIT_ERROR",
            VenoreError::NotGitRepository(_) => "NOT_GIT_REPOSITORY",
            VenoreError::GitCommandFailed(_) => "GIT_COMMAND_FAILED",
            VenoreError::InvalidGitReference(_) => "INVALID_GIT_REFERENCE",

            VenoreError::RagIndexError(_) => "RAG_INDEX_ERROR",
            VenoreError::RagSearchError(_) => "RAG_SEARCH_ERROR",
            VenoreError::RagEmbeddingError(_) => "RAG_EMBEDDING_ERROR",
            VenoreError::GraphQueryError(_) => "GRAPH_QUERY_ERROR",

            VenoreError::GitHubApiError { .. } => "GITHUB_API_ERROR",
            VenoreError::GitHubAuthRequired => "GITHUB_AUTH_REQUIRED",
            VenoreError::GitHubRateLimited { .. } => "GITHUB_RATE_LIMITED",
            VenoreError::GitHubDeviceFlowError(_) => "GITHUB_DEVICE_FLOW_ERROR",
            VenoreError::GitHubRepoNotDetected(_) => "GITHUB_REPO_NOT_DETECTED",

            VenoreError::TerminalError(_) => "TERMINAL_ERROR",
            VenoreError::TerminalSessionNotFound(_) => "TERMINAL_SESSION_NOT_FOUND",
            VenoreError::TerminalSpawnFailed(_) => "TERMINAL_SPAWN_FAILED",

            VenoreError::ToolNotFound(_) => "TOOL_NOT_FOUND",
            VenoreError::ToolExecutionFailed(_) => "TOOL_EXECUTION_FAILED",
            VenoreError::ToolApprovalDenied(_) => "TOOL_APPROVAL_DENIED",
            VenoreError::ToolTimeout(_) => "TOOL_TIMEOUT",

            VenoreError::LspError(_) => "LSP_ERROR",
            VenoreError::LspServerNotRunning(_) => "LSP_SERVER_NOT_RUNNING",
            VenoreError::LspSpawnFailed(_) => "LSP_SPAWN_FAILED",

            VenoreError::MeshError(_) => "MESH_ERROR",
            VenoreError::MeshPeerNotFound(_) => "MESH_PEER_NOT_FOUND",
            VenoreError::MeshConnectionFailed(_) => "MESH_CONNECTION_FAILED",
            VenoreError::MeshTransportNotRunning => "MESH_TRANSPORT_NOT_RUNNING",

            VenoreError::Io(_) => "IO_ERROR",
            VenoreError::Json(_) => "JSON_ERROR",
            VenoreError::Utf8(_) => "UTF8_ERROR",
        };

        let details = match &err {
            VenoreError::LlmRateLimit { retry_after_secs } => {
                retry_after_secs.map(|s| serde_json::json!({ "retry_after_secs": s }))
            }
            VenoreError::LlmContextTooLong { current, max } => {
                Some(serde_json::json!({ "current_tokens": current, "max_tokens": max }))
            }
            VenoreError::GitHubRateLimited { reset_seconds } => {
                Some(serde_json::json!({ "reset_seconds": reset_seconds }))
            }
            VenoreError::LlmModelNotAvailable { provider, model } => {
                Some(serde_json::json!({ "provider": provider, "model": model }))
            }
            _ => None,
        };

        ErrorResponse {
            code: code.to_string(),
            message: err.to_string(),
            details,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

// =================================================================
// FROM CONVERSIONS (Standard library → VenoreError)
// =================================================================

impl From<std::io::Error> for VenoreError {
    fn from(err: std::io::Error) -> Self {
        VenoreError::Io(err.to_string())
    }
}

impl From<serde_json::Error> for VenoreError {
    fn from(err: serde_json::Error) -> Self {
        VenoreError::Json(err.to_string())
    }
}

impl From<std::string::FromUtf8Error> for VenoreError {
    fn from(err: std::string::FromUtf8Error) -> Self {
        VenoreError::Utf8(err.to_string())
    }
}

// =================================================================
// DATABASE RESULT EXTENSION
// =================================================================

/// Extension trait that converts any `Result<T, E: Display>` into a
/// `Result<T, VenoreError::DatabaseError>` with a contextual message.
///
/// Replaces the ubiquitous `.map_err(|e| VenoreError::DatabaseError(format!("...: {e}")))`.
pub trait MapDbErr<T> {
    fn db_err(self, context: &str) -> Result<T>;
}

impl<T, E: std::fmt::Display> MapDbErr<T> for std::result::Result<T, E> {
    fn db_err(self, context: &str) -> Result<T> {
        self.map_err(|e| VenoreError::DatabaseError(format!("{}: {}", context, e)))
    }
}

// =================================================================
// HELPERS
// =================================================================

impl VenoreError {
    /// Get error code as static string
    pub fn code(&self) -> &'static str {
        match self {
            VenoreError::Unknown(_) => "UNKNOWN_ERROR",
            VenoreError::Timeout(_) => "TIMEOUT",
            VenoreError::InvalidParams(_) => "INVALID_PARAMS",
            VenoreError::InvalidPath(_) => "INVALID_PATH",
            VenoreError::NotFound(_) => "NOT_FOUND",
            VenoreError::NotImplemented(_) => "NOT_IMPLEMENTED",

            VenoreError::FileNotFound(_) => "FILE_NOT_FOUND",
            VenoreError::FileReadError(_) => "FILE_READ_ERROR",
            VenoreError::FileWriteError(_) => "FILE_WRITE_ERROR",
            VenoreError::PermissionDenied(_) => "PERMISSION_DENIED",
            VenoreError::DirectoryNotFound(_) => "DIRECTORY_NOT_FOUND",
            VenoreError::PathNotSafe(_) => "PATH_NOT_SAFE",

            VenoreError::ParseError(_) => "PARSE_ERROR",
            VenoreError::InvalidSyntax { .. } => "INVALID_SYNTAX",
            VenoreError::UnsupportedFormat(_) => "UNSUPPORTED_FORMAT",

            VenoreError::AnalysisError(_) => "ANALYSIS_ERROR",

            VenoreError::LlmNoApiKey(_) => "LLM_NO_API_KEY",
            VenoreError::LlmInvalidProvider(_) => "LLM_INVALID_PROVIDER",
            VenoreError::LlmInvalidRequest(_) => "LLM_INVALID_REQUEST",
            VenoreError::LlmProviderError(_) => "LLM_PROVIDER_ERROR",
            VenoreError::LlmRateLimit { .. } => "LLM_RATE_LIMIT",
            VenoreError::LlmContextTooLong { .. } => "LLM_CONTEXT_TOO_LONG",
            VenoreError::LlmStreamError(_) => "LLM_STREAM_ERROR",
            VenoreError::LlmInvalidResponse(_) => "LLM_INVALID_RESPONSE",
            VenoreError::LlmModelNotAvailable { .. } => "LLM_MODEL_NOT_AVAILABLE",

            VenoreError::DatabaseError(_) => "DATABASE_ERROR",
            VenoreError::DatabaseNotOpen => "DATABASE_NOT_OPEN",
            VenoreError::DatabaseLocked => "DATABASE_LOCKED",
            VenoreError::DatabaseQueryFailed(_) => "DATABASE_QUERY_FAILED",
            VenoreError::DatabaseNotInitialized => "DATABASE_NOT_INITIALIZED",

            VenoreError::ContextGenerationError(_) => "CONTEXT_GENERATION_ERROR",
            VenoreError::ContextWriteError(_) => "CONTEXT_WRITE_ERROR",
            VenoreError::InvalidContextFormat(_) => "INVALID_CONTEXT_FORMAT",
            VenoreError::ContextGenerationCancelled => "CONTEXT_GENERATION_CANCELLED",
            VenoreError::Cancelled(_) => "CANCELLED",

            VenoreError::GitError(_) => "GIT_ERROR",
            VenoreError::NotGitRepository(_) => "NOT_GIT_REPOSITORY",
            VenoreError::GitCommandFailed(_) => "GIT_COMMAND_FAILED",
            VenoreError::InvalidGitReference(_) => "INVALID_GIT_REFERENCE",

            VenoreError::RagIndexError(_) => "RAG_INDEX_ERROR",
            VenoreError::RagSearchError(_) => "RAG_SEARCH_ERROR",
            VenoreError::RagEmbeddingError(_) => "RAG_EMBEDDING_ERROR",
            VenoreError::GraphQueryError(_) => "GRAPH_QUERY_ERROR",

            VenoreError::GitHubApiError { .. } => "GITHUB_API_ERROR",
            VenoreError::GitHubAuthRequired => "GITHUB_AUTH_REQUIRED",
            VenoreError::GitHubRateLimited { .. } => "GITHUB_RATE_LIMITED",
            VenoreError::GitHubDeviceFlowError(_) => "GITHUB_DEVICE_FLOW_ERROR",
            VenoreError::GitHubRepoNotDetected(_) => "GITHUB_REPO_NOT_DETECTED",

            VenoreError::TerminalError(_) => "TERMINAL_ERROR",
            VenoreError::TerminalSessionNotFound(_) => "TERMINAL_SESSION_NOT_FOUND",
            VenoreError::TerminalSpawnFailed(_) => "TERMINAL_SPAWN_FAILED",

            VenoreError::ToolNotFound(_) => "TOOL_NOT_FOUND",
            VenoreError::ToolExecutionFailed(_) => "TOOL_EXECUTION_FAILED",
            VenoreError::ToolApprovalDenied(_) => "TOOL_APPROVAL_DENIED",
            VenoreError::ToolTimeout(_) => "TOOL_TIMEOUT",

            VenoreError::LspError(_) => "LSP_ERROR",
            VenoreError::LspServerNotRunning(_) => "LSP_SERVER_NOT_RUNNING",
            VenoreError::LspSpawnFailed(_) => "LSP_SPAWN_FAILED",

            VenoreError::MeshError(_) => "MESH_ERROR",
            VenoreError::MeshPeerNotFound(_) => "MESH_PEER_NOT_FOUND",
            VenoreError::MeshConnectionFailed(_) => "MESH_CONNECTION_FAILED",
            VenoreError::MeshTransportNotRunning => "MESH_TRANSPORT_NOT_RUNNING",

            VenoreError::Io(_) => "IO_ERROR",
            VenoreError::Json(_) => "JSON_ERROR",
            VenoreError::Utf8(_) => "UTF8_ERROR",
        }
    }
}

// =================================================================
// TESTS
// =================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_not_found_error() {
        let err = VenoreError::FileNotFound("test.txt".to_string());
        assert_eq!(err.to_string(), "File not found: test.txt");
        assert_eq!(err.code(), "FILE_NOT_FOUND");
    }

    #[test]
    fn test_timeout_error() {
        let err = VenoreError::Timeout(5000);
        assert_eq!(err.to_string(), "Operation timed out after 5000ms");
        assert_eq!(err.code(), "TIMEOUT");
    }

    #[test]
    fn test_llm_context_too_long() {
        let err = VenoreError::LlmContextTooLong {
            current: 150000,
            max: 128000,
        };
        assert!(err.to_string().contains("150000"));
        assert!(err.to_string().contains("128000"));
        assert_eq!(err.code(), "LLM_CONTEXT_TOO_LONG");
    }

    #[test]
    fn test_invalid_syntax() {
        let err = VenoreError::InvalidSyntax {
            file: "main.rs".to_string(),
            message: "unexpected token".to_string(),
        };
        assert!(err.to_string().contains("main.rs"));
        assert!(err.to_string().contains("unexpected token"));
        assert_eq!(err.code(), "INVALID_SYNTAX");
    }

    #[test]
    fn test_error_response_serialization() {
        let err = VenoreError::Timeout(5000);
        let response: ErrorResponse = err.into();

        assert_eq!(response.code, "TIMEOUT");
        assert!(response.message.contains("5000"));
        assert!(!response.timestamp.is_empty());

        // Test JSON serialization
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("TIMEOUT"));
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let venore_err: VenoreError = io_err.into();

        match venore_err {
            VenoreError::Io(_) => {},
            _ => panic!("Expected Io variant"),
        }
    }

    #[test]
    fn test_from_json_error() {
        let json_str = "{invalid json}";
        let json_err = serde_json::from_str::<serde_json::Value>(json_str).unwrap_err();
        let venore_err: VenoreError = json_err.into();

        match venore_err {
            VenoreError::Json(_) => {},
            _ => panic!("Expected Json variant"),
        }
    }

    #[test]
    fn test_from_utf8_error() {
        let invalid_utf8 = vec![0, 159, 146, 150];
        let utf8_err = String::from_utf8(invalid_utf8).unwrap_err();
        let venore_err: VenoreError = utf8_err.into();

        match venore_err {
            VenoreError::Utf8(_) => {},
            _ => panic!("Expected Utf8 variant"),
        }
    }

    #[test]
    fn test_result_type_alias() {
        fn returns_result() -> Result<String> {
            Ok("success".to_string())
        }

        assert!(returns_result().is_ok());

        fn returns_error() -> Result<String> {
            Err(VenoreError::NotFound("test".to_string()))
        }

        assert!(returns_error().is_err());
    }

    #[test]
    fn test_all_variants_have_codes() {
        // Verify every variant has a unique code
        let errors = vec![
            VenoreError::Unknown("test".to_string()),
            VenoreError::Timeout(1000),
            VenoreError::FileNotFound("file.txt".to_string()),
            VenoreError::LlmRateLimit { retry_after_secs: None },
            VenoreError::DatabaseNotOpen,
            VenoreError::ContextGenerationCancelled,
            VenoreError::GitError("error".to_string()),
        ];

        for err in errors {
            assert!(!err.code().is_empty());
            assert!(!err.code().is_empty());
        }
    }

    #[test]
    fn test_error_response_has_timestamp() {
        let err = VenoreError::FileNotFound("test.txt".to_string());
        let response: ErrorResponse = err.into();

        // Check timestamp is valid RFC3339
        assert!(chrono::DateTime::parse_from_rfc3339(&response.timestamp).is_ok());
    }

    #[test]
    fn test_clone_error() {
        let err = VenoreError::FileNotFound("test.txt".to_string());
        let cloned = err.clone();

        assert_eq!(err.to_string(), cloned.to_string());
        assert_eq!(err.code(), cloned.code());
    }
}
