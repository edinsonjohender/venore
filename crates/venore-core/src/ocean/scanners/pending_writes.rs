//! Pending writes scanner — flags any node whose `chat::pending_writes`
//! singleton has at least one entry awaiting user approval.
//!
//! Severity is fixed at `Info`: pending writes are not a problem, just a
//! signal "look here". Payload carries the count + a small breakdown so
//! the frontend decorator can show "+3" without re-fetching.

use serde_json::json;

use crate::chat::pending_writes;
use crate::ocean::scanner::{ScanContext, Scanner};
use crate::ocean::states::{NodeStateInstance, NodeStateKind, Severity};

pub struct PendingWritesScanner;

impl Scanner for PendingWritesScanner {
    fn kind(&self) -> NodeStateKind {
        NodeStateKind::PendingWrites
    }

    fn evaluate(&self, ctx: &ScanContext<'_>) -> Option<NodeStateInstance> {
        let pendings = pending_writes::list_for_node(ctx.project_path, ctx.node_id);
        if pendings.is_empty() {
            return None;
        }

        let mut creates: u32 = 0;
        let mut edits: u32 = 0;
        for w in &pendings {
            match w.kind {
                pending_writes::PendingKind::Create => creates += 1,
                pending_writes::PendingKind::Edit { .. } => edits += 1,
            }
        }

        Some(NodeStateInstance {
            kind: NodeStateKind::PendingWrites,
            severity: Severity::Info,
            computed_at: ctx.now_ts,
            payload: json!({
                "count": pendings.len(),
                "creates": creates,
                "edits": edits,
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::pending_writes::{
        compute_diff_patch, new_write_id, PendingKind, PendingSectionWrite,
    };
    use crate::ocean::types::{GridCell, LayoutEntry, NodeVariant};

    fn make_entry(id: &str) -> LayoutEntry {
        LayoutEntry {
            module_id: id.to_string(),
            module_name: id.to_string(),
            module_path: String::new(),
            cell: GridCell::new(0, 0),
            user_placed: true,
            node_variant: NodeVariant::KnowledgeNode,
            lighthouse_id: None,
        }
    }

    fn fresh_pending(project: &str, node: &str, kind: PendingKind, name: &str) -> PendingSectionWrite {
        PendingSectionWrite {
            write_id: new_write_id(),
            project_path: project.to_string(),
            node_id: node.to_string(),
            session_id: None,
            kind,
            name: name.to_string(),
            content_markdown: String::new(),
            ai_prompt: String::new(),
            ai_model: String::new(),
            diff_patch: None,
            additions: 0,
            deletions: 0,
            created_at: 0,
        }
    }

    #[test]
    fn no_pending_means_no_state() {
        // Use a unique project path so other tests don't pollute this scan.
        let project = format!("/tmp/scan-test-{}", new_write_id());
        let entry = make_entry("node-fresh-no-pending");
        let ctx = ScanContext {
            node_id: "node-fresh-no-pending",
            layout_entry: &entry,
            knowledge: None,
            project_path: &project,
            now_ts: 0,
        };
        assert!(PendingWritesScanner.evaluate(&ctx).is_none());
    }

    #[test]
    fn one_create_emits_info_with_count() {
        let project = format!("/tmp/scan-test-{}", new_write_id());
        let node_id = format!("node-{}", new_write_id());
        pending_writes::insert(fresh_pending(
            &project,
            &node_id,
            PendingKind::Create,
            "Resumen",
        ));

        let entry = make_entry(&node_id);
        let ctx = ScanContext {
            node_id: &node_id,
            layout_entry: &entry,
            knowledge: None,
            project_path: &project,
            now_ts: 42,
        };
        let state = PendingWritesScanner.evaluate(&ctx).expect("should flag");
        assert_eq!(state.kind, NodeStateKind::PendingWrites);
        assert_eq!(state.severity, Severity::Info);
        assert_eq!(state.payload["count"].as_u64(), Some(1));
        assert_eq!(state.payload["creates"].as_u64(), Some(1));
        assert_eq!(state.payload["edits"].as_u64(), Some(0));
    }

    #[test]
    fn mixed_creates_and_edits_count_separately() {
        let project = format!("/tmp/scan-test-{}", new_write_id());
        let node_id = format!("node-{}", new_write_id());
        pending_writes::insert(fresh_pending(
            &project,
            &node_id,
            PendingKind::Create,
            "Notes",
        ));
        pending_writes::insert(fresh_pending(
            &project,
            &node_id,
            PendingKind::Edit {
                section_id: "sec-1".into(),
                baseline_name: "Old".into(),
                baseline_content: String::new(),
            },
            "Notes",
        ));

        let entry = make_entry(&node_id);
        let ctx = ScanContext {
            node_id: &node_id,
            layout_entry: &entry,
            knowledge: None,
            project_path: &project,
            now_ts: 0,
        };
        let state = PendingWritesScanner.evaluate(&ctx).expect("should flag");
        assert_eq!(state.payload["count"].as_u64(), Some(2));
        assert_eq!(state.payload["creates"].as_u64(), Some(1));
        assert_eq!(state.payload["edits"].as_u64(), Some(1));
    }

    /// Sanity: compute_diff_patch is in scope (compile-only check —
    /// keeps the import live for test-only imports of helpers).
    #[allow(dead_code)]
    fn _ignore() {
        let _ = compute_diff_patch("a", "b", "c");
    }
}
