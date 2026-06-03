//! Cancellation registry for the wizard pipeline.
//!
//! When `detect_project_modules` or `wizard_index_project` start work for a
//! given project path, they register a `CancellationToken` here keyed by
//! that path. The token is exposed via `CancellationGuard::token()` so the
//! pipeline can check `is_cancelled()` between heavy operations and bail
//! cleanly. Calling `cancel(path)` from anywhere (e.g. the Tauri command
//! `cancel_wizard_session`) flips the token; the in-flight pipeline notices
//! at its next checkpoint and returns `VenoreError::Cancelled`.
//!
//! The guard cleans up via `Drop` and is keyed by a monotonic id so a new
//! registration that supersedes a stale one does the right thing:
//!   - The new `register` finds the old (id_A, token_A) for the same path
//!     and cancels it, then inserts (id_B, token_B).
//!   - When the old pipeline's guard drops, it sees the entry now belongs to
//!     id_B and leaves it alone.
//!   - When the new pipeline finishes normally, its guard drops and removes
//!     (id_B, token_B) from the registry.
//!
//! All operations are O(1) on a `HashMap` guarded by a `Mutex`. Critical
//! sections are tiny (a single lookup/insert), so contention is negligible.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use once_cell::sync::Lazy;
use tokio_util::sync::CancellationToken;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

static REGISTRY: Lazy<Mutex<HashMap<String, (u64, CancellationToken)>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// RAII guard: while alive, the wizard pipeline for `project_path` has a
/// cancellation token registered. On drop, removes the entry if it still
/// belongs to this guard (a newer registration won't be touched).
pub struct CancellationGuard {
    project_path: String,
    id: u64,
    token: CancellationToken,
}

impl CancellationGuard {
    /// Register a fresh token for `project_path`. If a previous registration
    /// exists for the same path, it is cancelled first — that older pipeline
    /// will see `is_cancelled()` at its next checkpoint and bail.
    pub fn register(project_path: &str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let token = CancellationToken::new();
        let mut g = REGISTRY.lock().unwrap();
        if let Some((_, old)) = g.insert(project_path.to_string(), (id, token.clone())) {
            old.cancel();
        }
        Self {
            project_path: project_path.to_string(),
            id,
            token,
        }
    }

    pub fn token(&self) -> &CancellationToken {
        &self.token
    }

    /// True if a `cancel(path)` call (or a superseding `register`) flipped
    /// this token.
    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }
}

impl Drop for CancellationGuard {
    fn drop(&mut self) {
        let mut g = REGISTRY.lock().unwrap();
        if let Some((id, _)) = g.get(&self.project_path) {
            if *id == self.id {
                g.remove(&self.project_path);
            }
        }
    }
}

/// Request cancellation for the in-flight wizard pipeline (if any) for
/// `project_path`. Returns true if a token was found and cancelled.
pub fn cancel(project_path: &str) -> bool {
    let g = REGISTRY.lock().unwrap();
    if let Some((_, token)) = g.get(project_path) {
        token.cancel();
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_returns_uncancelled_token() {
        let guard = CancellationGuard::register("test-uncancelled-token");
        assert!(!guard.is_cancelled());
    }

    #[test]
    fn cancel_flips_active_token() {
        let guard = CancellationGuard::register("test-cancel-flips");
        assert!(cancel("test-cancel-flips"));
        assert!(guard.is_cancelled());
    }

    #[test]
    fn cancel_unknown_path_returns_false() {
        assert!(!cancel("nope-not-registered-anywhere"));
    }

    #[test]
    fn second_register_cancels_first() {
        let first = CancellationGuard::register("test-supersede");
        let _second = CancellationGuard::register("test-supersede");
        assert!(first.is_cancelled());
    }

    #[test]
    fn drop_of_superseded_guard_does_not_remove_new_entry() {
        let first = CancellationGuard::register("test-drop-superseded");
        let second = CancellationGuard::register("test-drop-superseded");
        drop(first);
        // The new token should still be reachable via cancel()
        assert!(cancel("test-drop-superseded"));
        assert!(second.is_cancelled());
    }
}
