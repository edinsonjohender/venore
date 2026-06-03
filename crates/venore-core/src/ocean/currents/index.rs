//! Index Current — keeps the logbook search index in sync with the ocean.
//!
//! For every knowledge node it sweeps, it emits an `IndexLogbookNode` intent.
//! The actual diff-by-hash + reindex + embedding happens in the desktop bridge
//! (which can reach the `LogbookRepository`); this stays `rag`-free so the
//! `ocean` module keeps zero dependency on `rag`.

use crate::ocean::currents::runner::{Current, CurrentTask, VisitContext};
use crate::ocean::currents::traversal::route_row_major;
use crate::ocean::types::{GridCell, NodeVariant};

/// The first concrete current: logbook indexing.
pub struct IndexCurrent;

impl Current for IndexCurrent {
    fn id(&self) -> &'static str {
        "index_current"
    }

    /// Row-major sweep — a clean horizontal scan line, visually distinct from
    /// the Staleness Current's organic nearest-neighbor flow.
    fn plan_route(&self, nodes: Vec<(String, GridCell)>) -> Vec<String> {
        route_row_major(&nodes)
    }

    fn visit(&self, ctx: &VisitContext<'_>) -> Vec<CurrentTask> {
        // Only knowledge-bearing nodes have logbooks worth indexing.
        match ctx.layout_entry.node_variant {
            NodeVariant::KnowledgeNode | NodeVariant::Lighthouse => {}
            NodeVariant::Module | NodeVariant::Buoy | NodeVariant::Cylinder => return Vec::new(),
        }

        // No sections → nothing to index (and lets the bridge prune any stale
        // chunks if they were all deleted). Skip the intent when there's truly
        // no content layer yet to avoid churn on brand-new empty nodes.
        let has_content = ctx.knowledge.map(|k| !k.sections.is_empty()).unwrap_or(false);
        if !has_content {
            return Vec::new();
        }

        vec![CurrentTask::IndexLogbookNode {
            project_path: ctx.project_path.to_string(),
            node_id: ctx.node_id.to_string(),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ocean::types::{GridCell, KnowledgeNodeData, LayoutEntry, NodeSection, SourceAttribution};

    fn entry(variant: NodeVariant) -> LayoutEntry {
        LayoutEntry {
            module_id: "n1".to_string(),
            module_name: "Node".to_string(),
            module_path: String::new(),
            cell: GridCell::new(0, 0),
            user_placed: false,
            node_variant: variant,
            lighthouse_id: None,
        }
    }

    fn knowledge_with_sections(n: usize) -> KnowledgeNodeData {
        let mut data = KnowledgeNodeData::with_now();
        for i in 0..n {
            data.sections.push(NodeSection {
                id: format!("s{}", i),
                name: format!("Section {}", i),
                content_markdown: "body".to_string(),
                source: SourceAttribution::User,
                created_at: 0,
                updated_at: 0,
                ai_prompt: None,
                ai_model: None,
            });
        }
        data
    }

    fn visit(variant: NodeVariant, knowledge: Option<&KnowledgeNodeData>) -> Vec<CurrentTask> {
        let e = entry(variant);
        let ctx = VisitContext {
            node_id: "n1",
            layout_entry: &e,
            knowledge,
            project_path: "/proj",
            now_ts: 0,
        };
        IndexCurrent.visit(&ctx)
    }

    #[test]
    fn test_emits_intent_for_knowledge_node_with_sections() {
        let k = knowledge_with_sections(2);
        let tasks = visit(NodeVariant::KnowledgeNode, Some(&k));
        assert_eq!(tasks.len(), 1);
        match &tasks[0] {
            CurrentTask::IndexLogbookNode { node_id, project_path } => {
                assert_eq!(node_id, "n1");
                assert_eq!(project_path, "/proj");
            }
            other => panic!("expected IndexLogbookNode, got {other:?}"),
        }
    }

    #[test]
    fn test_emits_intent_for_lighthouse_with_sections() {
        let k = knowledge_with_sections(1);
        assert_eq!(visit(NodeVariant::Lighthouse, Some(&k)).len(), 1);
    }

    #[test]
    fn test_skips_node_without_sections() {
        let k = knowledge_with_sections(0);
        assert!(visit(NodeVariant::KnowledgeNode, Some(&k)).is_empty());
        assert!(visit(NodeVariant::KnowledgeNode, None).is_empty());
    }

    #[test]
    fn test_skips_code_variants() {
        let k = knowledge_with_sections(3);
        assert!(visit(NodeVariant::Module, Some(&k)).is_empty());
        assert!(visit(NodeVariant::Buoy, Some(&k)).is_empty());
        assert!(visit(NodeVariant::Cylinder, Some(&k)).is_empty());
    }
}
