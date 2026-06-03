//! Node state types for the Ocean Canvas.
//!
//! A "state" is a runtime-derived flag attached to a node that drives the
//! visual decorators (and, in the future, agent behaviour). Examples:
//! `Overflow` (the node has too much content and should be split),
//! `HostMissing`, `Stale`, etc. States are computed by registered scanners
//! that the rover walks across the canvas — they are NOT persisted to disk;
//! only user dismissals are.
//!
//! Multiple states can coexist on the same node. The frontend resolves them
//! by slot + priority via its decorator registry.
//!
//! Every state instance carries a free-form `payload` (`serde_json::Value`)
//! so individual scanners can ship debug data (score, char counts, etc.)
//! without having to extend a closed enum.

use serde::{Deserialize, Serialize};

/// Stable identifier for a state kind. Serialized as snake_case so the
/// frontend registry can key by string ("overflow", "host_missing"...).
///
/// Adding a new state means: extend this enum + register a `Scanner` that
/// produces it + add a row in the frontend decorator registry. No core
/// machinery changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeStateKind {
    /// A knowledge_node has accumulated too many sections / too much text and
    /// is losing semantic coherence. Suggests the user (or agent) split it.
    Overflow,
    /// One or more AI-proposed writes are awaiting user approval on this
    /// node (see `chat::pending_writes`). The user can review them by
    /// opening the node panel; the decorator just signals "look here" on
    /// the canvas so they aren't lost off-screen.
    PendingWrites,
}

impl NodeStateKind {
    /// Stable string id used as the JSON discriminator and as the key in
    /// dismissal maps. Must match the `serde(rename_all = "snake_case")`
    /// representation.
    pub fn as_str(self) -> &'static str {
        match self {
            NodeStateKind::Overflow => "overflow",
            NodeStateKind::PendingWrites => "pending_writes",
        }
    }
}

/// How loud the state should be presented. Drives both the decorator
/// rendering (intensity, color shifts) and the urgency the agent should
/// place on it. Ordered: `Info < Warning < Severe`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    #[default]
    Info,
    Warning,
    Severe,
}

/// One active state on a node. Computed by a `Scanner`, lives in the
/// service's in-memory `node_states` map, ships to the frontend in the
/// `NodePosition` DTO and via the `ocean-state-changed` event.
///
/// `payload` is a free-form JSON value — each scanner decides what to ship.
/// Keep it small (a handful of fields) so per-tick log volume stays sane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStateInstance {
    pub kind: NodeStateKind,
    #[serde(default)]
    pub severity: Severity,
    pub computed_at: i64,
    #[serde(default)]
    pub payload: serde_json::Value,
}

impl NodeStateInstance {
    /// Two instances are considered "the same observation" when their kind +
    /// severity + payload match. Used by the service to decide whether a
    /// re-scan changed anything (and therefore whether to emit a
    /// `ocean-state-changed` event).
    pub fn equivalent(&self, other: &NodeStateInstance) -> bool {
        self.kind == other.kind
            && self.severity == other.severity
            && self.payload == other.payload
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn kind_serializes_snake_case() {
        let json = serde_json::to_string(&NodeStateKind::Overflow).unwrap();
        assert_eq!(json, "\"overflow\"");
    }

    #[test]
    fn severity_default_is_info() {
        assert_eq!(Severity::default(), Severity::Info);
    }

    #[test]
    fn severity_orders_low_to_high() {
        assert!(Severity::Info < Severity::Warning);
        assert!(Severity::Warning < Severity::Severe);
    }

    #[test]
    fn instance_equivalence_ignores_timestamp() {
        let a = NodeStateInstance {
            kind: NodeStateKind::Overflow,
            severity: Severity::Warning,
            computed_at: 100,
            payload: json!({"score": 1.2}),
        };
        let b = NodeStateInstance {
            kind: NodeStateKind::Overflow,
            severity: Severity::Warning,
            computed_at: 999,
            payload: json!({"score": 1.2}),
        };
        assert!(a.equivalent(&b));
    }

    #[test]
    fn instance_equivalence_detects_severity_bump() {
        let a = NodeStateInstance {
            kind: NodeStateKind::Overflow,
            severity: Severity::Warning,
            computed_at: 0,
            payload: json!({}),
        };
        let b = NodeStateInstance {
            kind: NodeStateKind::Overflow,
            severity: Severity::Severe,
            computed_at: 0,
            payload: json!({}),
        };
        assert!(!a.equivalent(&b));
    }

    #[test]
    fn instance_round_trips_json() {
        let original = NodeStateInstance {
            kind: NodeStateKind::Overflow,
            severity: Severity::Severe,
            computed_at: 1700000000,
            payload: json!({"score": 1.7, "sections": 12}),
        };
        let json = serde_json::to_string(&original).unwrap();
        let back: NodeStateInstance = serde_json::from_str(&json).unwrap();
        assert!(original.equivalent(&back));
        assert_eq!(original.computed_at, back.computed_at);
    }

    #[test]
    fn kind_as_str_matches_serde() {
        let serde_form = serde_json::to_value(NodeStateKind::Overflow).unwrap();
        assert_eq!(serde_form.as_str().unwrap(), NodeStateKind::Overflow.as_str());
    }
}
