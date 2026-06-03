//! Concrete scanners used by the rover.
//!
//! Each scanner is a stateless evaluator that, given a node's layout entry
//! plus its (optional) `KnowledgeNodeData`, decides whether to flag a state.
//! Add new scanners here and register them in [`default_registry`] so the
//! rover picks them up automatically.

use std::sync::Arc;

use super::scanner::ScannerRegistry;

pub mod pending_writes;
pub mod saturation;

pub use pending_writes::PendingWritesScanner;
pub use saturation::SaturationScanner;

/// Build the scanner registry with every scanner enabled by default.
/// Currently only saturation; new scanners just append here.
pub fn default_registry() -> ScannerRegistry {
    let mut registry = ScannerRegistry::new();
    registry.register(Arc::new(SaturationScanner));
    registry.register(Arc::new(PendingWritesScanner));
    registry
}
