//! Context Auto-Updater
//!
//! Detects new commits on a selected branch, maps changed files to modules,
//! and regenerates stale `.context.md` files.
//!
//! # DEPRECATED
//!
//! This module is superseded by **Project Memory**
//! (`.venore/project-memory.json`, see [`crate::memory`]). `.context.md`
//! per-module docs are no longer the source of truth, and the desktop
//! commands that wrap this orchestrator
//! (`venore-desktop/src/commands/context_updater.rs`) are registered but
//! **not called by any UI component** — the feature is orphaned. The code is
//! kept temporarily so the deprecation is explicit; do not build new features
//! on it. Slated for removal once the remaining `.context.md` readers
//! (`github::pr_analyzer`, `chat::connection_resolver`) migrate to Project
//! Memory.

pub mod updater_state;
pub mod branch_monitor;
pub mod change_mapper;
pub mod orchestrator;

pub use updater_state::UpdaterState;
pub use branch_monitor::CommitSummary;
pub use change_mapper::AffectedModule;
pub use orchestrator::{UpdateReport, RegenerationResult};
