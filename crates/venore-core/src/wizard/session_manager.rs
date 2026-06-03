//! # Wizard Session Manager
//!
//! Manages wizard sessions for multi-tenancy and state isolation.
//!
//! ## Design:
//! - Thread-safe singleton using Arc<Mutex<>>
//! - One session per project path
//! - Automatic session creation on demand
//! - Manual session removal for cleanup
//!
//! ## Usage:
//! ```rust,ignore
//! use venore_core::wizard::WizardSessionManager;
//! use std::path::PathBuf;
//!
//! let manager = WizardSessionManager::global();
//! let mut guard = manager.lock().unwrap();
//!
//! // Get or create session
//! let session = guard.get_or_create(PathBuf::from("/path/to/project"));
//!
//! // Access existing session
//! if let Some(session) = guard.get("/path/to/project") {
//!     // Use session
//! }
//!
//! // Remove session when done
//! guard.remove("/path/to/project");
//! ```

use crate::wizard::session::WizardSession;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use once_cell::sync::Lazy;

// =============================================================================
// Session Manager
// =============================================================================

/// Manages wizard sessions
///
/// Thread-safe singleton that tracks active wizard sessions.
/// Each project has at most one session at a time.
pub struct WizardSessionManager {
    /// Active sessions: project_path → WizardSession
    sessions: HashMap<String, WizardSession>,
}

impl WizardSessionManager {
    /// Creates a new WizardSessionManager (private - use global())
    fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// Gets the global WizardSessionManager instance
    ///
    /// # Examples
    /// ```
    /// use venore_core::wizard::WizardSessionManager;
    ///
    /// let manager = WizardSessionManager::global();
    /// ```
    pub fn global() -> Arc<Mutex<Self>> {
        static INSTANCE: Lazy<Arc<Mutex<WizardSessionManager>>> =
            Lazy::new(|| Arc::new(Mutex::new(WizardSessionManager::new())));
        INSTANCE.clone()
    }

    /// Gets or creates a session for the given project path
    ///
    /// If a session already exists for this project, returns a mutable reference.
    /// Otherwise, creates a new session and returns it.
    ///
    /// # Arguments
    ///
    /// * `project_path` - Absolute path to project root
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let manager = WizardSessionManager::global();
    /// let mut guard = manager.lock().unwrap();
    ///
    /// let session = guard.get_or_create(PathBuf::from("/path/to/project"));
    /// session.cache_analysis(analysis);
    /// ```
    pub fn get_or_create(&mut self, project_path: PathBuf) -> &mut WizardSession {
        let key = project_path.to_string_lossy().to_string();
        self.sessions
            .entry(key)
            .or_insert_with(|| WizardSession::new(project_path))
    }

    /// Gets an existing session for the given project path
    ///
    /// Returns `None` if no session exists for this project.
    ///
    /// # Arguments
    ///
    /// * `project_path` - Project path as string
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let manager = WizardSessionManager::global();
    /// let guard = manager.lock().unwrap();
    ///
    /// if let Some(session) = guard.get("/path/to/project") {
    ///     println!("Session exists!");
    /// }
    /// ```
    pub fn get(&self, project_path: &str) -> Option<&WizardSession> {
        self.sessions.get(project_path)
    }

    /// Gets a mutable reference to an existing session
    ///
    /// Returns `None` if no session exists for this project.
    ///
    /// # Arguments
    ///
    /// * `project_path` - Project path as string
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let manager = WizardSessionManager::global();
    /// let mut guard = manager.lock().unwrap();
    ///
    /// if let Some(session) = guard.get_mut("/path/to/project") {
    ///     session.set_wizard_config(config);
    /// }
    /// ```
    pub fn get_mut(&mut self, project_path: &str) -> Option<&mut WizardSession> {
        self.sessions.get_mut(project_path)
    }

    /// Removes a session for the given project path
    ///
    /// Returns `true` if a session was removed, `false` if none existed.
    ///
    /// # Arguments
    ///
    /// * `project_path` - Project path as string
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let manager = WizardSessionManager::global();
    /// let mut guard = manager.lock().unwrap();
    ///
    /// if guard.remove("/path/to/project") {
    ///     println!("Session removed");
    /// }
    /// ```
    pub fn remove(&mut self, project_path: &str) -> bool {
        self.sessions.remove(project_path).is_some()
    }

    /// Lists all active project paths
    ///
    /// Returns a vector of project paths that have active sessions.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let manager = WizardSessionManager::global();
    /// let guard = manager.lock().unwrap();
    ///
    /// for path in guard.list() {
    ///     println!("Active session: {}", path);
    /// }
    /// ```
    pub fn list(&self) -> Vec<String> {
        self.sessions.keys().cloned().collect()
    }

    /// Returns the number of active sessions
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let manager = WizardSessionManager::global();
    /// let guard = manager.lock().unwrap();
    ///
    /// println!("Active sessions: {}", guard.count());
    /// ```
    pub fn count(&self) -> usize {
        self.sessions.len()
    }

    /// Removes all sessions
    ///
    /// Useful for cleanup in tests or when resetting state.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let manager = WizardSessionManager::global();
    /// let mut guard = manager.lock().unwrap();
    ///
    /// guard.clear();
    /// ```
    pub fn clear(&mut self) {
        self.sessions.clear();
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_or_create_new_session() {
        let manager = WizardSessionManager::global();
        let mut guard = manager.lock().unwrap();

        let project_path = PathBuf::from("/test/project");
        let session = guard.get_or_create(project_path.clone());

        assert_eq!(session.project_path(), &project_path);
    }

    #[test]
    fn test_get_or_create_existing_session() {
        let manager = WizardSessionManager::global();
        let mut guard = manager.lock().unwrap();

        // Clear any existing sessions from other tests
        guard.clear();

        let project_path = PathBuf::from("/test/project2");

        // Create session
        let _session1 = guard.get_or_create(project_path.clone());

        // Get same session again
        let _session2 = guard.get_or_create(project_path.clone());

        // Should only have one session
        assert_eq!(guard.count(), 1);
    }

    #[test]
    fn test_get_nonexistent_session() {
        let manager = WizardSessionManager::global();
        let guard = manager.lock().unwrap();

        let result = guard.get("/nonexistent/project");
        assert!(result.is_none());
    }

    #[test]
    fn test_get_mut_session() {
        let manager = WizardSessionManager::global();
        let mut guard = manager.lock().unwrap();

        // Clear for test isolation
        guard.clear();

        let project_path = PathBuf::from("/test/project3");
        guard.get_or_create(project_path.clone());

        let path_str = project_path.to_string_lossy().to_string();
        let session = guard.get_mut(&path_str);
        assert!(session.is_some());
    }

    #[test]
    fn test_remove_session() {
        let manager = WizardSessionManager::global();
        let mut guard = manager.lock().unwrap();

        // Clear for test isolation
        guard.clear();

        let project_path = PathBuf::from("/test/project4");
        guard.get_or_create(project_path.clone());

        assert_eq!(guard.count(), 1);

        let path_str = project_path.to_string_lossy().to_string();
        let removed = guard.remove(&path_str);
        assert!(removed);
        assert_eq!(guard.count(), 0);
    }

    #[test]
    fn test_remove_nonexistent_session() {
        let manager = WizardSessionManager::global();
        let mut guard = manager.lock().unwrap();

        let removed = guard.remove("/nonexistent/project");
        assert!(!removed);
    }

    #[test]
    fn test_list_sessions() {
        let manager = WizardSessionManager::global();
        let mut guard = manager.lock().unwrap();

        // Clear for test isolation
        guard.clear();

        guard.get_or_create(PathBuf::from("/test/project5"));
        guard.get_or_create(PathBuf::from("/test/project6"));

        let list = guard.list();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_count_sessions() {
        let manager = WizardSessionManager::global();
        let mut guard = manager.lock().unwrap();

        // Clear for test isolation
        guard.clear();

        assert_eq!(guard.count(), 0);

        guard.get_or_create(PathBuf::from("/test/project7"));
        assert_eq!(guard.count(), 1);

        guard.get_or_create(PathBuf::from("/test/project8"));
        assert_eq!(guard.count(), 2);
    }

    #[test]
    fn test_clear_sessions() {
        let manager = WizardSessionManager::global();
        let mut guard = manager.lock().unwrap();

        guard.get_or_create(PathBuf::from("/test/project9"));
        guard.get_or_create(PathBuf::from("/test/project10"));

        assert!(guard.count() >= 2);

        guard.clear();
        assert_eq!(guard.count(), 0);
    }

    #[test]
    fn test_thread_safety() {
        // This test verifies the manager can be accessed from multiple threads
        use std::thread;

        let manager = WizardSessionManager::global();

        let handle = thread::spawn(move || {
            let mut guard = manager.lock().unwrap();
            guard.get_or_create(PathBuf::from("/test/thread_project"));
        });

        handle.join().unwrap();
    }
}
