//! Scanner abstraction — node-state evaluators.
//!
//! A [`Scanner`] is a stateless rule that, given a node's layout entry plus its
//! optional `KnowledgeNodeData`, decides whether to flag a [`NodeStateInstance`]
//! (overflow, pending writes, …). Scanners are registered in a
//! [`ScannerRegistry`] (see `scanners::default_registry`).
//!
//! The traversal that walks these scanners across the Ocean is no longer a
//! standalone "rover": it lives in the Currents engine as `StateCurrent`
//! (`currents::state`), which runs the registry per node and applies the result
//! to the service. This module only defines the rule abstraction; the runner,
//! lifecycle and events live in `currents::runner`.

use std::sync::Arc;

use crate::ocean::states::{NodeStateInstance, NodeStateKind};
use crate::ocean::types::{KnowledgeNodeData, LayoutEntry};

// =============================================================================
// Scanner trait + context
// =============================================================================

/// Read-only view passed to every scanner per node visit.
pub struct ScanContext<'a> {
    pub node_id: &'a str,
    pub layout_entry: &'a LayoutEntry,
    pub knowledge: Option<&'a KnowledgeNodeData>,
    /// Path of the project this node lives in. Lets scanners reach into
    /// other process-scoped singletons (e.g. `chat::pending_writes`) keyed
    /// by `project_path` without requiring the caller to enumerate them.
    pub project_path: &'a str,
    /// Wall-clock seconds since the unix epoch — same value all scanners in
    /// one tick share, so timestamps line up.
    pub now_ts: i64,
}

/// One state evaluator. Scanners are registered once and reused across every
/// tick. Implementations must be `Send + Sync` because the registry is
/// `Arc`-shared with the running current.
pub trait Scanner: Send + Sync {
    /// The single state kind this scanner produces. Used for diagnostics.
    fn kind(&self) -> NodeStateKind;
    /// Decide whether this scanner flags the given node. Returning `None`
    /// means "no flag for this kind"; `Some(state)` means "active".
    fn evaluate(&self, ctx: &ScanContext<'_>) -> Option<NodeStateInstance>;
}

/// Container of registered scanners. Constructed once per process; the
/// running current holds an `Arc<ScannerRegistry>` and never mutates it.
pub struct ScannerRegistry {
    scanners: Vec<Arc<dyn Scanner>>,
}

impl ScannerRegistry {
    pub fn new() -> Self {
        Self {
            scanners: Vec::new(),
        }
    }

    pub fn register(&mut self, scanner: Arc<dyn Scanner>) {
        self.scanners.push(scanner);
    }

    pub fn scanners(&self) -> &[Arc<dyn Scanner>] {
        &self.scanners
    }
}

impl Default for ScannerRegistry {
    fn default() -> Self {
        super::scanners::default_registry()
    }
}
