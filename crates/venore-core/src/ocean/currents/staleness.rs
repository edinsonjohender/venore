//! Staleness Current — keeps code-drift badges in sync with the source tree.
//!
//! For every code module node it sweeps, it emits a `CheckModuleStale` intent.
//! The actual content hashing (the part that used to freeze project open for
//! ~15s) happens in the desktop bridge, off the service lock and one module per
//! background tick, so the canvas never blocks.
//!
//! Like every current, this runs on EVERY project regardless of kind — the
//! substrate hands it every node and the current decides what's relevant. On a
//! Knowledge project there are no `Module` nodes, so it simply emits nothing and
//! goes idle; the specialization lives here, not in the Currents system.

use crate::ocean::currents::runner::{Current, CurrentTask, VisitContext};
use crate::ocean::currents::traversal::{far_corner, route_nearest_from};
use crate::ocean::types::{GridCell, NodeVariant};

/// Code-drift detection current.
pub struct StalenessCurrent;

impl Current for StalenessCurrent {
    fn id(&self) -> &'static str {
        "staleness_current"
    }

    /// Organic nearest-neighbor flow, fanning in from the far (bottom-right)
    /// corner — distinct in both shape AND origin from the Index Current's
    /// row-major sweep, so the two scanners read as separate currents.
    fn plan_route(&self, nodes: Vec<(String, GridCell)>) -> Vec<String> {
        let start = far_corner(&nodes);
        route_nearest_from(&nodes, start)
    }

    fn visit(&self, ctx: &VisitContext<'_>) -> Vec<CurrentTask> {
        // Only code modules carry a hashable source subtree. Knowledge nodes,
        // lighthouses, buoys and cylinders have no committed code-hash baseline.
        if ctx.layout_entry.node_variant != NodeVariant::Module {
            return Vec::new();
        }

        // A module with no path can't be located on disk to hash (defensive:
        // legacy layouts default `module_path` to "").
        if ctx.layout_entry.module_path.is_empty() {
            return Vec::new();
        }

        vec![CurrentTask::CheckModuleStale {
            project_path: ctx.project_path.to_string(),
            module_name: ctx.layout_entry.module_name.clone(),
            module_path: ctx.layout_entry.module_path.clone(),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ocean::types::{GridCell, LayoutEntry};

    fn entry(variant: NodeVariant, module_path: &str) -> LayoutEntry {
        LayoutEntry {
            module_id: "m1".to_string(),
            module_name: "Auth".to_string(),
            module_path: module_path.to_string(),
            cell: GridCell::new(0, 0),
            user_placed: false,
            node_variant: variant,
            lighthouse_id: None,
        }
    }

    fn visit(variant: NodeVariant, module_path: &str) -> Vec<CurrentTask> {
        let e = entry(variant, module_path);
        let ctx = VisitContext {
            node_id: "m1",
            layout_entry: &e,
            knowledge: None,
            project_path: "/proj",
            now_ts: 0,
        };
        StalenessCurrent.visit(&ctx)
    }

    #[test]
    fn emits_check_for_code_module() {
        let tasks = visit(NodeVariant::Module, "src/auth");
        assert_eq!(tasks.len(), 1);
        match &tasks[0] {
            CurrentTask::CheckModuleStale { module_name, module_path, project_path } => {
                assert_eq!(module_name, "Auth");
                assert_eq!(module_path, "src/auth");
                assert_eq!(project_path, "/proj");
            }
            _ => panic!("expected CheckModuleStale"),
        }
    }

    #[test]
    fn skips_module_without_path() {
        assert!(visit(NodeVariant::Module, "").is_empty());
    }

    #[test]
    fn skips_knowledge_variants() {
        // Same current, knowledge project → no work, just goes idle.
        assert!(visit(NodeVariant::KnowledgeNode, "src/auth").is_empty());
        assert!(visit(NodeVariant::Lighthouse, "src/auth").is_empty());
        assert!(visit(NodeVariant::Buoy, "src/auth").is_empty());
        assert!(visit(NodeVariant::Cylinder, "src/auth").is_empty());
    }
}
