//! State Current — the node-state scanner, folded into the Currents engine.
//!
//! This is the old "rover": a single passive worker that, per node, runs every
//! registered [`Scanner`] (saturation → `Overflow`, pending-writes →
//! `PendingWrites`) and writes the resulting state vector to the Ocean service.
//! The frontend renders the halos/badges from the `ocean-state-changed` event
//! the bridge forwards.
//!
//! Why it differs from the other currents:
//!   - It manages **node state** (an effect inside `ocean`) rather than emitting
//!     `CurrentTask` intents — so it overrides `evaluate_states`, not `visit`.
//!   - It traverses the **dirty queue** (`Traversal::DirtyQueue`) instead of a
//!     planned full sweep, so a node mutated mid-session (a new pending write,
//!     an edited section) gets re-scanned within a tick instead of waiting up to
//!     a full refresh cycle. This preserves the rover's prompt feedback.
//!
//! Like every current it runs on every project kind; on a project with no nodes
//! the dirty queue is empty and it simply idles.

use std::sync::Arc;

use crate::ocean::currents::runner::{Current, CurrentTask, Traversal, VisitContext};
use crate::ocean::scanner::{ScanContext, ScannerRegistry};
use crate::ocean::scanners::default_registry;
use crate::ocean::states::NodeStateInstance;

/// Node-state scanning current. Holds the scanner registry it runs per node.
pub struct StateCurrent {
    registry: Arc<ScannerRegistry>,
}

impl StateCurrent {
    pub fn new() -> Self {
        Self { registry: Arc::new(default_registry()) }
    }
}

impl Default for StateCurrent {
    fn default() -> Self {
        Self::new()
    }
}

impl Current for StateCurrent {
    fn id(&self) -> &'static str {
        "state_current"
    }

    /// Drain the dirty queue, not a planned sweep — node state must react
    /// promptly to edits/moves/pending-writes (each marks the node dirty).
    fn traversal(&self) -> Traversal {
        Traversal::DirtyQueue
    }

    /// No cross-module side effects: this current only flags node state.
    fn visit(&self, _ctx: &VisitContext<'_>) -> Vec<CurrentTask> {
        Vec::new()
    }

    /// Run every scanner and return the union of the states they flag. Always
    /// `Some` (an empty vec clears any stale halo on a node that recovered), so
    /// the runner applies the result to the service authoritatively.
    fn evaluate_states(&self, ctx: &VisitContext<'_>) -> Option<Vec<NodeStateInstance>> {
        let scan_ctx = ScanContext {
            node_id: ctx.node_id,
            layout_entry: ctx.layout_entry,
            knowledge: ctx.knowledge,
            project_path: ctx.project_path,
            now_ts: ctx.now_ts,
        };
        let mut results = Vec::new();
        for scanner in self.registry.scanners() {
            if let Some(state) = scanner.evaluate(&scan_ctx) {
                results.push(state);
            }
        }
        Some(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ocean::types::{
        GridCell, KnowledgeNodeData, KnowledgeNodeSubtype, LayoutEntry, NodeSection, NodeVariant,
        SourceAttribution,
    };

    fn entry(variant: NodeVariant) -> LayoutEntry {
        LayoutEntry {
            module_id: "n1".to_string(),
            module_name: "Node".to_string(),
            module_path: String::new(),
            cell: GridCell::new(0, 0),
            user_placed: true,
            node_variant: variant,
            lighthouse_id: None,
        }
    }

    fn knowledge_with_sections(n: usize) -> KnowledgeNodeData {
        KnowledgeNodeData {
            subtype: KnowledgeNodeSubtype::Concept,
            sections: (0..n)
                .map(|i| NodeSection {
                    id: format!("s{i}"),
                    name: format!("Section {i}"),
                    content_markdown: "body".to_string(),
                    source: SourceAttribution::User,
                    created_at: 0,
                    updated_at: 0,
                    ai_prompt: None,
                    ai_model: None,
                })
                .collect(),
            created_at: 0,
            updated_at: 0,
        }
    }

    fn eval(variant: NodeVariant, knowledge: Option<&KnowledgeNodeData>) -> Vec<NodeStateInstance> {
        let e = entry(variant);
        let ctx = VisitContext {
            node_id: "n1",
            layout_entry: &e,
            knowledge,
            project_path: "/proj",
            now_ts: 0,
        };
        StateCurrent::new()
            .evaluate_states(&ctx)
            .expect("state current always returns Some")
    }

    #[test]
    fn traverses_the_dirty_queue() {
        assert_eq!(StateCurrent::new().traversal(), Traversal::DirtyQueue);
    }

    #[test]
    fn emits_no_intents() {
        let e = entry(NodeVariant::KnowledgeNode);
        let ctx = VisitContext {
            node_id: "n1",
            layout_entry: &e,
            knowledge: None,
            project_path: "/proj",
            now_ts: 0,
        };
        assert!(StateCurrent::new().visit(&ctx).is_empty());
    }

    #[test]
    fn clean_node_yields_empty_state_vector() {
        // A knowledge node with few small sections trips no scanner → empty
        // (which clears any prior halo when applied).
        assert!(eval(NodeVariant::KnowledgeNode, Some(&knowledge_with_sections(2))).is_empty());
    }

    #[test]
    fn oversized_node_flags_overflow() {
        // 11 sections trips the saturation scanner's severe step.
        let states = eval(NodeVariant::KnowledgeNode, Some(&knowledge_with_sections(11)));
        assert!(states
            .iter()
            .any(|s| s.kind == crate::ocean::states::NodeStateKind::Overflow));
    }
}
