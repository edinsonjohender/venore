//! Updater state — persistence for branch monitoring configuration.
//!
//! Stored at `.venore/updater-state.json`. Uses atomic write (temp + rename).

use std::fs;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::error::{Result, VenoreError};

const STATE_FILE: &str = ".venore/updater-state.json";

/// Persistent state for the context auto-updater.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdaterState {
    /// Branch to monitor (default: "main")
    pub selected_branch: String,
    /// SHA of the last commit we synced to
    pub last_sync_commit: Option<String>,
    /// Timestamp of the last sync
    pub last_sync_at: Option<DateTime<Utc>>,
    /// Whether auto-update polling is enabled
    pub auto_update_enabled: bool,
    /// Polling interval in minutes
    pub check_interval_minutes: u32,
}

impl Default for UpdaterState {
    fn default() -> Self {
        Self {
            selected_branch: "main".to_string(),
            last_sync_commit: None,
            last_sync_at: None,
            auto_update_enabled: true,
            check_interval_minutes: 20,
        }
    }
}

impl UpdaterState {
    /// Build default state for a project, detecting the repo's actual default
    /// branch instead of assuming "main". Falls back to "main" when detection
    /// fails (not a git repo, no `origin/HEAD`, detached HEAD).
    fn defaults_for(project_path: &Path) -> Self {
        let branch = crate::session::git_ops::get_default_branch(project_path)
            .unwrap_or_else(|| "main".to_string());
        Self {
            selected_branch: branch,
            ..Self::default()
        }
    }

    /// Load state from disk, deriving defaults (with detected branch) if the
    /// file does not exist.
    pub fn load(project_path: &Path) -> Result<Self> {
        let path = project_path.join(STATE_FILE);

        if !path.exists() {
            debug!("No updater state found, deriving defaults from repo");
            return Ok(Self::defaults_for(project_path));
        }

        let content = fs::read_to_string(&path).map_err(|e| {
            VenoreError::FileReadError(format!("Cannot read updater state: {}", e))
        })?;

        match serde_json::from_str::<Self>(&content) {
            Ok(state) => {
                debug!(branch = %state.selected_branch, "Updater state loaded");
                Ok(state)
            }
            Err(e) => {
                warn!("Corrupt updater state file, backing up: {}", e);
                let backup = path.with_extension("json.corrupt");
                let _ = fs::rename(&path, &backup);
                Ok(Self::default())
            }
        }
    }

    /// Save state to disk using atomic write (temp + rename).
    pub fn save(project_path: &Path, state: &UpdaterState) -> Result<()> {
        let path = project_path.join(STATE_FILE);

        // Ensure .venore directory exists
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| {
                    VenoreError::FileWriteError(format!("Cannot create .venore dir: {}", e))
                })?;
            }
        }

        let content = serde_json::to_string_pretty(state).map_err(|e| {
            VenoreError::Unknown(format!("Cannot serialize updater state: {}", e))
        })?;

        let temp_path = path.with_extension("json.tmp");
        fs::write(&temp_path, &content).map_err(|e| {
            VenoreError::FileWriteError(format!("Cannot write updater state temp file: {}", e))
        })?;

        fs::rename(&temp_path, &path).map_err(|e| {
            VenoreError::FileWriteError(format!("Cannot rename updater state file: {}", e))
        })?;

        info!(branch = %state.selected_branch, "Updater state saved");
        Ok(())
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_returns_defaults_when_no_file() {
        let dir = TempDir::new().unwrap();
        let state = UpdaterState::load(dir.path()).unwrap();
        assert_eq!(state.selected_branch, "main");
        assert!(state.last_sync_commit.is_none());
        assert!(state.auto_update_enabled);
        assert_eq!(state.check_interval_minutes, 20);
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join(".venore")).unwrap();

        let state = UpdaterState {
            selected_branch: "develop".to_string(),
            last_sync_commit: Some("abc123".to_string()),
            last_sync_at: Some(Utc::now()),
            auto_update_enabled: false,
            check_interval_minutes: 10,
        };

        UpdaterState::save(dir.path(), &state).unwrap();
        let loaded = UpdaterState::load(dir.path()).unwrap();

        assert_eq!(loaded.selected_branch, "develop");
        assert_eq!(loaded.last_sync_commit, Some("abc123".to_string()));
        assert!(!loaded.auto_update_enabled);
        assert_eq!(loaded.check_interval_minutes, 10);
    }

    #[test]
    fn test_load_corrupt_file_returns_defaults() {
        let dir = TempDir::new().unwrap();
        let venore_dir = dir.path().join(".venore");
        fs::create_dir_all(&venore_dir).unwrap();
        fs::write(venore_dir.join("updater-state.json"), "not json").unwrap();

        let state = UpdaterState::load(dir.path()).unwrap();
        assert_eq!(state.selected_branch, "main");
    }
}
