//! Currents — passive workers that navigate the Ocean doing independent tasks.
//!
//! See [`runner`] for the abstraction. The first concrete current is
//! [`index::IndexCurrent`] (logbook search indexing). Future currents (incl.
//! user-defined ones) register the same way.

pub mod runner;
pub mod traversal;
pub mod index;
pub mod staleness;
pub mod state;

use std::sync::Arc;

pub use runner::{
    Current, CurrentEvent, CurrentProgressEvent, CurrentSnapshot, CurrentStateChange, CurrentTask,
    Traversal, VisitContext, ensure_currents_started, current_snapshot, stop_currents,
    CURRENTS_INTERVAL, CURRENTS_REFRESH,
};
pub use index::IndexCurrent;
pub use staleness::StalenessCurrent;
pub use state::StateCurrent;
pub use traversal::nearest_pending;

/// The default set of currents every project runs. Each current decides per
/// node whether it has work; one with no relevant nodes simply stays idle. The
/// set is project-type agnostic — specialization lives in each current:
///   - `IndexCurrent`     — logbook search index (knowledge nodes).
///   - `StalenessCurrent` — code-drift badges (code module nodes).
///   - `StateCurrent`     — node-state halos/badges (overflow, pending writes).
pub fn default_currents() -> Vec<Arc<dyn Current>> {
    vec![
        Arc::new(IndexCurrent),
        Arc::new(StalenessCurrent),
        Arc::new(StateCurrent::new()),
    ]
}
