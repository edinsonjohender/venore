//! # Batch Manager
//!
//! Manages batch generation state for pause/resume functionality.
//!
//! ## Design:
//! - Thread-safe singleton using Arc<Mutex<>>
//! - Prevents duplicate batches for same project
//! - Atomic pause/resume operations
//!
//! ## Usage:
//! ```rust,ignore
//! use venore_core::wizard::BatchManager;
//!
//! let manager = BatchManager::global();
//!
//! // Create batch
//! let batch_id = manager.create_batch("project_path", pause_flag)?;
//!
//! // Pause/Resume
//! manager.pause_batch(&batch_id)?;
//! manager.resume_batch(&batch_id)?;
//!
//! // Cleanup
//! manager.remove_batch(&batch_id);
//! ```

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use once_cell::sync::Lazy;
use uuid::Uuid;

// =============================================================================
// Types
// =============================================================================

/// Batch state for a single generation batch
#[derive(Clone)]
pub struct BatchState {
    /// Unique batch identifier
    pub batch_id: String,
    /// Project path (for duplicate prevention)
    pub project_path: String,
    /// Atomic pause flag (true = paused, false = running)
    pub paused: Arc<AtomicBool>,
}

/// Batch status
#[derive(Debug, Clone, PartialEq)]
pub enum BatchStatus {
    Running,
    Paused,
    NotFound,
}

/// Error types for batch operations
#[derive(Debug, Clone, PartialEq)]
pub enum BatchError {
    /// Batch already exists for this project
    DuplicateBatch { existing_batch_id: String },
    /// Batch not found
    BatchNotFound { batch_id: String },
}

impl std::fmt::Display for BatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BatchError::DuplicateBatch { existing_batch_id } => {
                write!(
                    f,
                    "Batch already running for this project (batch_id: {}). Only one batch per project is allowed.",
                    existing_batch_id
                )
            }
            BatchError::BatchNotFound { batch_id } => {
                write!(f, "Batch {} not found", batch_id)
            }
        }
    }
}

impl std::error::Error for BatchError {}

// =============================================================================
// BatchManager
// =============================================================================

/// Manages batch generation state
///
/// Thread-safe singleton that tracks active batches and enforces
/// "one batch per project" rule.
pub struct BatchManager {
    /// Active batches: batch_id → BatchState
    batches: HashMap<String, BatchState>,
}

impl BatchManager {
    /// Creates a new BatchManager (private - use global())
    fn new() -> Self {
        Self {
            batches: HashMap::new(),
        }
    }

    /// Gets the global BatchManager instance
    ///
    /// # Examples
    /// ```
    /// use venore_core::wizard::BatchManager;
    ///
    /// let manager = BatchManager::global();
    /// ```
    pub fn global() -> Arc<Mutex<Self>> {
        static INSTANCE: Lazy<Arc<Mutex<BatchManager>>> =
            Lazy::new(|| Arc::new(Mutex::new(BatchManager::new())));
        INSTANCE.clone()
    }

    /// Creates a new batch
    ///
    /// # Rules:
    /// - Only one batch per project path
    /// - Generates unique batch_id (UUID)
    /// - Returns pause flag for caller to use
    ///
    /// # Errors:
    /// Returns `BatchError::DuplicateBatch` if batch already exists for project
    ///
    /// # Examples
    /// ```rust,ignore
    /// let manager = BatchManager::global();
    /// let mut guard = manager.lock().unwrap();
    ///
    /// match guard.create_batch("/path/to/project") {
    ///     Ok((batch_id, pause_flag)) => {
    ///         // Use pause_flag in async task
    ///     }
    ///     Err(BatchError::DuplicateBatch { existing_batch_id }) => {
    ///         // Handle duplicate
    ///     }
    /// }
    /// ```
    pub fn create_batch(
        &mut self,
        project_path: impl Into<String>,
    ) -> Result<(String, Arc<AtomicBool>), BatchError> {
        let project_path = project_path.into();

        // Check for duplicate batch
        for (existing_id, state) in self.batches.iter() {
            if state.project_path == project_path {
                return Err(BatchError::DuplicateBatch {
                    existing_batch_id: existing_id.clone(),
                });
            }
        }

        // Create new batch
        let batch_id = Uuid::new_v4().to_string();
        let paused = Arc::new(AtomicBool::new(false));

        let state = BatchState {
            batch_id: batch_id.clone(),
            project_path,
            paused: paused.clone(),
        };

        self.batches.insert(batch_id.clone(), state);

        Ok((batch_id, paused))
    }

    /// Pauses a batch
    ///
    /// Sets the atomic pause flag to true, which will be detected
    /// by the generation loop.
    ///
    /// # Errors:
    /// Returns `BatchError::BatchNotFound` if batch doesn't exist
    ///
    /// # Examples
    /// ```rust,ignore
    /// let manager = BatchManager::global();
    /// let mut guard = manager.lock().unwrap();
    ///
    /// guard.pause_batch(&batch_id)?;
    /// ```
    pub fn pause_batch(&mut self, batch_id: &str) -> Result<(), BatchError> {
        match self.batches.get(batch_id) {
            Some(state) => {
                state.paused.store(true, Ordering::Relaxed);
                Ok(())
            }
            None => Err(BatchError::BatchNotFound {
                batch_id: batch_id.to_string(),
            }),
        }
    }

    /// Resumes a batch
    ///
    /// Sets the atomic pause flag to false, allowing the generation
    /// loop to continue.
    ///
    /// # Errors:
    /// Returns `BatchError::BatchNotFound` if batch doesn't exist
    ///
    /// # Examples
    /// ```rust,ignore
    /// let manager = BatchManager::global();
    /// let mut guard = manager.lock().unwrap();
    ///
    /// guard.resume_batch(&batch_id)?;
    /// ```
    pub fn resume_batch(&mut self, batch_id: &str) -> Result<(), BatchError> {
        match self.batches.get(batch_id) {
            Some(state) => {
                state.paused.store(false, Ordering::Relaxed);
                Ok(())
            }
            None => Err(BatchError::BatchNotFound {
                batch_id: batch_id.to_string(),
            }),
        }
    }

    /// Gets batch status
    ///
    /// # Examples
    /// ```rust,ignore
    /// let manager = BatchManager::global();
    /// let guard = manager.lock().unwrap();
    ///
    /// match guard.get_status(&batch_id) {
    ///     BatchStatus::Running => println!("Running"),
    ///     BatchStatus::Paused => println!("Paused"),
    ///     BatchStatus::NotFound => println!("Not found"),
    /// }
    /// ```
    pub fn get_status(&self, batch_id: &str) -> BatchStatus {
        match self.batches.get(batch_id) {
            Some(state) => {
                if state.paused.load(Ordering::Relaxed) {
                    BatchStatus::Paused
                } else {
                    BatchStatus::Running
                }
            }
            None => BatchStatus::NotFound,
        }
    }

    /// Removes a batch (cleanup after completion)
    ///
    /// Should be called when batch generation completes (success or error).
    ///
    /// # Examples
    /// ```rust,ignore
    /// let manager = BatchManager::global();
    /// let mut guard = manager.lock().unwrap();
    ///
    /// guard.remove_batch(&batch_id);
    /// ```
    pub fn remove_batch(&mut self, batch_id: &str) {
        self.batches.remove(batch_id);
    }

    /// Gets the number of active batches
    ///
    /// Useful for debugging and monitoring.
    pub fn active_count(&self) -> usize {
        self.batches.len()
    }

    /// Lists all active batch IDs
    ///
    /// Useful for debugging and monitoring.
    pub fn list_batches(&self) -> Vec<String> {
        self.batches.keys().cloned().collect()
    }

    /// Finds a batch by project path
    ///
    /// Returns the batch_id if a batch exists for the given project path.
    /// Useful for disconnect operations where only the project path is known.
    pub fn find_by_project_path(&self, project_path: &str) -> Option<String> {
        self.batches.iter()
            .find(|(_, state)| state.project_path == project_path)
            .map(|(id, _)| id.clone())
    }

    /// Gets the pause flag for a batch
    ///
    /// Returns `Some(Arc<AtomicBool>)` if batch exists, `None` otherwise.
    /// The returned flag can be used to check pause status without holding the lock.
    ///
    /// # Examples
    /// ```rust,ignore
    /// let manager = BatchManager::global();
    /// let guard = manager.lock().unwrap();
    ///
    /// if let Some(pause_flag) = guard.get_pause_flag(&batch_id) {
    ///     // Use pause_flag outside the lock
    ///     drop(guard);
    ///     while pause_flag.load(Ordering::Relaxed) {
    ///         // Wait...
    ///     }
    /// }
    /// ```
    pub fn get_pause_flag(&self, batch_id: &str) -> Option<Arc<AtomicBool>> {
        self.batches.get(batch_id).map(|state| state.paused.clone())
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_batch() {
        let mut manager = BatchManager::new();

        let result = manager.create_batch("/path/to/project");
        assert!(result.is_ok());

        let (batch_id, pause_flag) = result.unwrap();
        assert!(!batch_id.is_empty());
        assert!(!pause_flag.load(Ordering::Relaxed));
        assert_eq!(manager.active_count(), 1);
    }

    #[test]
    fn test_duplicate_batch_prevention() {
        let mut manager = BatchManager::new();

        // First batch - OK
        let result1 = manager.create_batch("/path/to/project");
        assert!(result1.is_ok());

        // Second batch same project - ERROR
        let result2 = manager.create_batch("/path/to/project");
        assert!(result2.is_err());

        match result2 {
            Err(BatchError::DuplicateBatch { .. }) => {
                // Expected
            }
            _ => panic!("Expected DuplicateBatch error"),
        }
    }

    #[test]
    fn test_pause_resume() {
        let mut manager = BatchManager::new();

        let (batch_id, pause_flag) = manager.create_batch("/path/to/project").unwrap();

        // Initially running
        assert!(!pause_flag.load(Ordering::Relaxed));
        assert_eq!(manager.get_status(&batch_id), BatchStatus::Running);

        // Pause
        let result = manager.pause_batch(&batch_id);
        assert!(result.is_ok());
        assert!(pause_flag.load(Ordering::Relaxed));
        assert_eq!(manager.get_status(&batch_id), BatchStatus::Paused);

        // Resume
        let result = manager.resume_batch(&batch_id);
        assert!(result.is_ok());
        assert!(!pause_flag.load(Ordering::Relaxed));
        assert_eq!(manager.get_status(&batch_id), BatchStatus::Running);
    }

    #[test]
    fn test_pause_nonexistent_batch() {
        let mut manager = BatchManager::new();

        let result = manager.pause_batch("nonexistent");
        assert!(result.is_err());

        match result {
            Err(BatchError::BatchNotFound { .. }) => {
                // Expected
            }
            _ => panic!("Expected BatchNotFound error"),
        }
    }

    #[test]
    fn test_remove_batch() {
        let mut manager = BatchManager::new();

        let (batch_id, _) = manager.create_batch("/path/to/project").unwrap();
        assert_eq!(manager.active_count(), 1);

        manager.remove_batch(&batch_id);
        assert_eq!(manager.active_count(), 0);
        assert_eq!(manager.get_status(&batch_id), BatchStatus::NotFound);
    }

    #[test]
    fn test_list_batches() {
        let mut manager = BatchManager::new();

        manager.create_batch("/project1").unwrap();
        manager.create_batch("/project2").unwrap();

        let batches = manager.list_batches();
        assert_eq!(batches.len(), 2);
    }

    #[test]
    fn test_find_by_project_path() {
        let mut manager = BatchManager::new();

        let (batch_id, _) = manager.create_batch("/path/to/project").unwrap();
        manager.create_batch("/other/project").unwrap();

        // Should find existing batch
        let found = manager.find_by_project_path("/path/to/project");
        assert_eq!(found, Some(batch_id));

        // Should return None for non-existent path
        let not_found = manager.find_by_project_path("/nonexistent");
        assert_eq!(not_found, None);
    }
}
