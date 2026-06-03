//! Configuration Storage Implementations
//!
//! Provides concrete implementations of configuration storage:
//! - KeyringApiKeyStore: Secure OS keychain storage for API keys
//! - SqliteTaskConfigStore: SQLite database storage for task settings
//! - MockConfigStore: In-memory storage for testing
//! - DefaultConfigStore: Production-ready store combining keyring + SQLite

pub mod keyring_store;
pub mod sqlite_store;
pub mod mock_store;

// Re-exports
pub use keyring_store::KeyringApiKeyStore;
pub use sqlite_store::SqliteTaskConfigStore;
pub use mock_store::MockConfigStore;

// DefaultConfigStore combines both stores
mod default_store;
pub use default_store::DefaultConfigStore;
