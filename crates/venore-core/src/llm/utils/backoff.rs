//! Exponential backoff utilities

use rand::Rng;

/// Default maximum backoff delay (60 seconds)
const DEFAULT_MAX_DELAY_MS: u64 = 60_000;

/// Backoff configuration
#[derive(Debug, Clone)]
pub struct BackoffConfig {
    /// Base delay in milliseconds
    pub base_delay_ms: u64,
    /// Maximum delay cap in milliseconds
    pub max_delay_ms: u64,
    /// Retry-After value from server (overrides calculated delay)
    pub retry_after_secs: Option<u64>,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            base_delay_ms: 1000,
            max_delay_ms: DEFAULT_MAX_DELAY_MS,
            retry_after_secs: None,
        }
    }
}

/// Calculate exponential backoff delay with jitter
///
/// Formula: delay = base * 2^attempt with ±20% jitter
///
/// If `retry_after_secs` is provided, it takes precedence over the calculated delay.
pub fn exponential_backoff_with_config(attempt: u32, config: &BackoffConfig) -> u64 {
    // If server provided Retry-After, use it (no jitter, exact value)
    if let Some(retry_after) = config.retry_after_secs {
        return retry_after * 1000; // Convert to milliseconds
    }

    let exponential = config.base_delay_ms * 2_u64.pow(attempt);

    // Cap at max delay
    let delay = exponential.min(config.max_delay_ms);

    // Add jitter (±20%)
    let jitter_range = (delay as f64 * 0.2) as u64;
    let mut rng = rand::thread_rng();
    let jitter = rng.gen_range(0..=jitter_range * 2);

    delay.saturating_sub(jitter_range).saturating_add(jitter)
}

/// Calculate exponential backoff delay with jitter (legacy API)
///
/// Formula: delay = base * 2^attempt with ±20% jitter
///
/// Cap at 60 seconds by default.
pub fn exponential_backoff(attempt: u32, base_delay_ms: u64) -> u64 {
    let config = BackoffConfig {
        base_delay_ms,
        max_delay_ms: DEFAULT_MAX_DELAY_MS,
        retry_after_secs: None,
    };
    exponential_backoff_with_config(attempt, &config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backoff_increases() {
        let delay0 = exponential_backoff(0, 1000);
        let delay1 = exponential_backoff(1, 1000);
        let delay2 = exponential_backoff(2, 1000);

        assert!(delay0 < delay1);
        assert!(delay1 < delay2);
    }

    #[test]
    fn test_backoff_caps_at_60s() {
        let delay = exponential_backoff(10, 1000);
        // Changed: now caps at 60s (with jitter can go up to 72s)
        assert!(delay <= 72_000, "Delay {} exceeds cap + jitter", delay);
    }

    #[test]
    fn test_backoff_with_retry_after() {
        let config = BackoffConfig {
            base_delay_ms: 1000,
            max_delay_ms: 60_000,
            retry_after_secs: Some(45),
        };

        // Retry-After should take precedence
        let delay = exponential_backoff_with_config(0, &config);
        assert_eq!(delay, 45_000); // 45 seconds in ms

        // Even on later attempts, Retry-After still takes precedence
        let delay = exponential_backoff_with_config(5, &config);
        assert_eq!(delay, 45_000);
    }

    #[test]
    fn test_backoff_respects_custom_cap() {
        let config = BackoffConfig {
            base_delay_ms: 1000,
            max_delay_ms: 5_000, // 5s cap
            retry_after_secs: None,
        };

        // Attempt 10 would normally give 1024s, but cap is 5s
        // With ±20% jitter, actual delay can be 4000-6000ms
        let delay = exponential_backoff_with_config(10, &config);
        assert!(delay <= 6_000, "Delay {} exceeds cap + jitter", delay);
        assert!(delay >= 4_000, "Delay {} below cap - jitter", delay);
    }

    #[test]
    fn test_backoff_default_config() {
        let config = BackoffConfig::default();

        assert_eq!(config.base_delay_ms, 1000);
        assert_eq!(config.max_delay_ms, 60_000);
        assert_eq!(config.retry_after_secs, None);
    }

    #[test]
    fn test_backoff_legacy_api_still_works() {
        // Ensure old API still works
        let delay0 = exponential_backoff(0, 1000);
        let delay1 = exponential_backoff(1, 1000);

        // Should still have exponential growth (approximately)
        // Note: jitter can cause occasional overlap, so we test averages
        assert!(delay0 <= 1_200); // ~1000ms + jitter
        assert!((1_600..=2_400).contains(&delay1)); // ~2000ms ± jitter

        // Should still cap at 60s (with jitter can go up to 72s)
        let delay_large = exponential_backoff(20, 1000);
        assert!(delay_large <= 72_000, "Delay {} exceeds cap + jitter", delay_large);
    }
}
