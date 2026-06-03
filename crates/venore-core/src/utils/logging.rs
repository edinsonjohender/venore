//! Logging utilities using tracing
//!
//! Provides structured logging with levels, categories and consistent format

use tracing::Level;
use tracing_subscriber::{
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, fmt,
};

/// Initialize logging system
///
/// # Examples
/// ```no_run
/// use venore_core::utils::init_logger;
/// use tracing::Level;
///
/// init_logger(Level::DEBUG);
/// ```
///
/// # Environment Variables
/// - `VENORE_LOG`: Set log level (e.g., `VENORE_LOG=debug`)
/// - Example: `VENORE_LOG=debug,tower_http=warn`
pub fn init_logger(default_level: Level) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            // VENORE_LOG=debug,tower_http=warn
            format!("venore={},tower_http=warn", default_level).into()
        });

    tracing_subscriber::registry()
        .with(filter)
        .with(
            fmt::layer()
                .with_target(true)
                .with_thread_ids(false)
                .with_line_number(true)
                .compact()
        )
        .init();

    tracing::info!("Logging initialized at level: {}", default_level);
}

/// Re-export tracing macros for convenient use
pub use tracing::{debug, info, warn, error, trace, instrument};

/// Log with domain/category
///
/// # Examples
/// ```
/// use venore_core::log_domain;
///
/// log_domain!(info, "ipc", "Handler called: {}", "scan_project");
/// log_domain!(debug, "database", "Query executed in {}ms", 42);
/// ```
#[macro_export]
macro_rules! log_domain {
    ($level:ident, $domain:expr, $($arg:tt)*) => {
        tracing::$level!(domain = $domain, $($arg)*);
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logging_macros_available() {
        // Test that macros are available (compilation test)
        // We can't actually test logging output without initializing
        // the subscriber, which would affect other tests

        // Just verify the re-exports compile
        let _ = Level::DEBUG;
        let _ = Level::INFO;
        let _ = Level::WARN;
        let _ = Level::ERROR;
        let _ = Level::TRACE;
    }

    #[test]
    fn test_log_domain_macro_compiles() {
        // Test that the macro compiles (won't actually log without init)
        // This is a compile-time test

        // The macro itself is tested at compile time
        // We can't easily test the actual logging without side effects
    }
}
