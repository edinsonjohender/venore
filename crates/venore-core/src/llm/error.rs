//! Error handling helpers for LLM module
//!
//! Provides error mapping and utility functions for LLM operations.

use crate::VenoreError;

/// Error type classification for LLM operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmErrorType {
    RateLimit,
    QuotaExceeded,
    InvalidRequest,
    AuthError,
    ServerError,
    Timeout,
    Cancelled,
    Unknown,
}

impl LlmErrorType {
    /// Should retry this error?
    pub fn should_retry(&self) -> bool {
        matches!(self, Self::RateLimit | Self::ServerError | Self::Timeout)
    }

    /// Should try fallback provider?
    pub fn should_fallback(&self) -> bool {
        matches!(self, Self::QuotaExceeded | Self::AuthError)
    }
}

/// Classify an error for retry/fallback logic
pub fn classify_error(error: &VenoreError) -> LlmErrorType {
    match error {
        VenoreError::LlmRateLimit { .. } => LlmErrorType::RateLimit,
        VenoreError::LlmNoApiKey(_) => LlmErrorType::AuthError,
        VenoreError::LlmInvalidProvider(_) => LlmErrorType::InvalidRequest,
        VenoreError::LlmInvalidResponse(_) => LlmErrorType::InvalidRequest,
        VenoreError::Timeout(_) => LlmErrorType::Timeout,
        VenoreError::LlmProviderError(msg) if msg.contains("quota") => LlmErrorType::QuotaExceeded,
        VenoreError::LlmProviderError(_) => LlmErrorType::ServerError,
        VenoreError::LlmStreamError(_) => LlmErrorType::ServerError,
        VenoreError::LlmContextTooLong { .. } => LlmErrorType::InvalidRequest,
        _ => LlmErrorType::Unknown,
    }
}

/// Extract Retry-After value from rate limit error
pub fn extract_retry_after(error: &VenoreError) -> Option<u64> {
    match error {
        VenoreError::LlmRateLimit { retry_after_secs } => *retry_after_secs,
        _ => None,
    }
}

/// Check if error should trigger retry
pub fn should_retry(error: &VenoreError) -> bool {
    classify_error(error).should_retry()
}

/// Check if error should trigger fallback
pub fn should_fallback(error: &VenoreError) -> bool {
    classify_error(error).should_fallback()
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_rate_limit() {
        let err = VenoreError::LlmRateLimit { retry_after_secs: None };
        assert_eq!(classify_error(&err), LlmErrorType::RateLimit);
        assert!(should_retry(&err));
        assert!(!should_fallback(&err));
    }

    #[test]
    fn test_extract_retry_after() {
        let err_with_retry = VenoreError::LlmRateLimit {
            retry_after_secs: Some(45),
        };
        assert_eq!(extract_retry_after(&err_with_retry), Some(45));

        let err_without_retry = VenoreError::LlmRateLimit {
            retry_after_secs: None,
        };
        assert_eq!(extract_retry_after(&err_without_retry), None);

        let err_other = VenoreError::Timeout(5000);
        assert_eq!(extract_retry_after(&err_other), None);
    }

    #[test]
    fn test_classify_auth_error() {
        let err = VenoreError::LlmNoApiKey("anthropic".into());
        assert_eq!(classify_error(&err), LlmErrorType::AuthError);
        assert!(!should_retry(&err));
        assert!(should_fallback(&err));
    }

    #[test]
    fn test_classify_timeout() {
        let err = VenoreError::Timeout(5000);
        assert_eq!(classify_error(&err), LlmErrorType::Timeout);
        assert!(should_retry(&err));
    }

    #[test]
    fn test_classify_invalid_request() {
        let err = VenoreError::LlmInvalidResponse("bad json".into());
        assert_eq!(classify_error(&err), LlmErrorType::InvalidRequest);
        assert!(!should_retry(&err));
        assert!(!should_fallback(&err));
    }

    #[test]
    fn test_quota_exceeded() {
        let err = VenoreError::LlmProviderError("quota exceeded".into());
        assert_eq!(classify_error(&err), LlmErrorType::QuotaExceeded);
        assert!(!should_retry(&err));
        assert!(should_fallback(&err));
    }
}
