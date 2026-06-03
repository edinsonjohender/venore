//! Base Provider Helpers
//!
//! Shared utilities for all provider implementations.

use crate::{Result, VenoreError};

/// Truncate a response body for error messages (avoids leaking huge HTML/JSON to UI)
fn truncate_body(body: &str, max_len: usize) -> String {
    let trimmed = body.trim();
    if trimmed.len() <= max_len {
        trimmed.to_string()
    } else {
        format!("{}...", &trimmed[..max_len])
    }
}

/// Build authorization header value
pub fn build_auth_header(api_key: &str, prefix: &str) -> String {
    format!("{} {}", prefix, api_key)
}

/// Parse HTTP error status into VenoreError
///
/// If `retry_after_header` is provided for 429 errors, it will be parsed and included.
pub fn map_http_error(status: u16, body: &str) -> VenoreError {
    map_http_error_with_retry_after(status, body, None)
}

/// Parse HTTP error status into VenoreError with optional Retry-After
pub fn map_http_error_with_retry_after(
    status: u16,
    body: &str,
    retry_after_header: Option<&str>,
) -> VenoreError {
    match status {
        401 | 403 => {
            VenoreError::LlmNoApiKey("Invalid or missing API key".into())
        }
        429 => {
            let retry_after_secs = retry_after_header.and_then(parse_retry_after);
            VenoreError::LlmRateLimit { retry_after_secs }
        }
        500..=599 => {
            VenoreError::LlmProviderError(format!(
                "AI provider server error (HTTP {}). {}",
                status,
                truncate_body(body, 200)
            ))
        }
        _ => {
            VenoreError::LlmProviderError(format!(
                "Unexpected AI provider response (HTTP {}). {}",
                status,
                truncate_body(body, 200)
            ))
        }
    }
}

/// Parse Retry-After header value (supports both seconds and HTTP-date)
fn parse_retry_after(value: &str) -> Option<u64> {
    // Try parsing as seconds first (most common)
    if let Ok(secs) = value.trim().parse::<u64>() {
        return Some(secs);
    }

    // TODO: Could parse HTTP-date format if needed
    // For now, just return None for date formats
    None
}

/// Validate model is supported by provider
pub fn validate_model(model: &str, supported: &[String]) -> Result<()> {
    if supported.iter().any(|m| m == model) {
        Ok(())
    } else {
        Err(VenoreError::LlmInvalidProvider(format!(
            "Model '{}' not supported",
            model
        )))
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_auth_header() {
        let header = build_auth_header("test-key", "Bearer");
        assert_eq!(header, "Bearer test-key");
    }

    #[test]
    fn test_map_http_error_auth() {
        let error = map_http_error(401, "Unauthorized");
        assert!(matches!(error, VenoreError::LlmNoApiKey(_)));
    }

    #[test]
    fn test_map_http_error_rate_limit() {
        let error = map_http_error(429, "Rate limit");
        assert!(matches!(error, VenoreError::LlmRateLimit { retry_after_secs: None }));
    }

    #[test]
    fn test_map_http_error_rate_limit_with_retry_after() {
        let error = map_http_error_with_retry_after(429, "Rate limit", Some("45"));
        match error {
            VenoreError::LlmRateLimit { retry_after_secs } => {
                assert_eq!(retry_after_secs, Some(45));
            }
            _ => panic!("Expected LlmRateLimit error"),
        }
    }

    #[test]
    fn test_parse_retry_after() {
        assert_eq!(parse_retry_after("30"), Some(30));
        assert_eq!(parse_retry_after("  60  "), Some(60));
        assert_eq!(parse_retry_after("invalid"), None);
    }

    #[test]
    fn test_map_http_error_server() {
        let error = map_http_error(500, "Internal error");
        match &error {
            VenoreError::LlmProviderError(msg) => {
                assert!(msg.contains("AI provider server error"));
                assert!(msg.contains("500"));
            }
            _ => panic!("Expected LlmProviderError"),
        }
    }

    #[test]
    fn test_truncate_body_short() {
        assert_eq!(truncate_body("short", 200), "short");
    }

    #[test]
    fn test_truncate_body_long() {
        let long = "x".repeat(300);
        let result = truncate_body(&long, 200);
        assert_eq!(result.len(), 203); // 200 + "..."
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_body_trims_whitespace() {
        assert_eq!(truncate_body("  hello  ", 200), "hello");
    }

    #[test]
    fn test_validate_model() {
        let supported = vec!["model-1".to_string(), "model-2".to_string()];

        assert!(validate_model("model-1", &supported).is_ok());
        assert!(validate_model("model-3", &supported).is_err());
    }
}
