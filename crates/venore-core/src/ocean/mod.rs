//! Ocean Canvas layout system.
//!
//! Backend-owned grid layout for the Ocean Canvas visualization.
//! Handles node placement, occupancy checking, and persistence.

pub mod currents;
pub mod placement;
pub mod scanner;
pub mod scanners;
pub mod service;
pub mod states;
pub mod types;

#[cfg(test)]
pub(crate) mod test_utils;

pub use scanner::{ScanContext, Scanner, ScannerRegistry};
pub use currents::{
    default_currents, ensure_currents_started, current_snapshot, stop_currents,
    Current, CurrentEvent, CurrentProgressEvent, CurrentSnapshot, CurrentStateChange, CurrentTask,
    IndexCurrent, StalenessCurrent, StateCurrent, Traversal,
};
pub use states::{NodeStateInstance, NodeStateKind, Severity};
pub use types::{
    CameraState, GridCell, KnowledgeNodeData, KnowledgeNodeSubtype, LayoutEntry, ManualConnection,
    ModuleInfo, MoveResult, NodeSection, NodeVariant, OceanLayout, SourceAttribution,
};
