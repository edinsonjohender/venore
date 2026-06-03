//! Saturation scanner — detects knowledge_nodes that have grown past the
//! coherence threshold. Two independent signals, each with its own
//! Warning/Severe step. The most severe signal wins the final severity.
//!
//!   sections          ≥ 7   → Warning,    ≥ 11   → Severe
//!   max_section_chars ≥ 3500 → Warning,   ≥ 5000 → Severe
//!
//! There's deliberately NO `total_chars` rule: it's redundant with the
//! other two (a node with many small sections is already caught by
//! `sections`; a node with one huge dump by `max_section_chars`) and adds
//! noise to the diagnosis.
//!
//! Only `KnowledgeNode` and `Lighthouse` variants carry sections, so other
//! variants are ignored. Modules / Buoys / Cylinders never overflow this
//! way (no logbook content).

use serde_json::json;

use crate::ocean::scanner::{ScanContext, Scanner};
use crate::ocean::states::{NodeStateInstance, NodeStateKind, Severity};
use crate::ocean::types::NodeVariant;

/// Section-count thresholds. Hitting `_SEVERE` implies the user has drifted
/// from a single coherent topic into a sub-topic catalog.
pub const SECTIONS_WARN: usize = 7;
pub const SECTIONS_SEVERE: usize = 11;

/// Single-section length thresholds (chars). A section past `_WARN` is
/// already long enough to suggest extracting it into its own node; past
/// `_SEVERE` it's clearly a separate topic in disguise.
pub const MAX_SECTION_WARN: usize = 3500;
pub const MAX_SECTION_SEVERE: usize = 5000;

/// Stateless scanner: no fields, evaluates against the context each call.
pub struct SaturationScanner;

impl Scanner for SaturationScanner {
    fn kind(&self) -> NodeStateKind {
        NodeStateKind::Overflow
    }

    fn evaluate(&self, ctx: &ScanContext<'_>) -> Option<NodeStateInstance> {
        // Only knowledge content carries sections. Module / Buoy / Cylinder
        // are code-representational; they never produce an overflow state.
        match ctx.layout_entry.node_variant {
            NodeVariant::KnowledgeNode | NodeVariant::Lighthouse => {}
            _ => return None,
        }

        let data = ctx.knowledge?;
        let sections = data.sections.len();
        if sections == 0 {
            return None;
        }
        let max_chars = data
            .sections
            .iter()
            .map(|s| s.content_markdown.len())
            .max()
            .unwrap_or(0);

        let sections_severity = severity_for(sections, SECTIONS_WARN, SECTIONS_SEVERE);
        let max_section_severity = severity_for(max_chars, MAX_SECTION_WARN, MAX_SECTION_SEVERE);

        // Most severe wins. If neither metric tripped, no flag.
        let severity = [sections_severity, max_section_severity]
            .into_iter()
            .flatten()
            .max()?;

        // Surface which signal(s) tripped so the UI / logs can give a
        // human-readable hint without re-running the rules.
        let triggers: Vec<&str> = [
            sections_severity.map(|_| "sections"),
            max_section_severity.map(|_| "max_section"),
        ]
        .into_iter()
        .flatten()
        .collect();

        Some(NodeStateInstance {
            kind: NodeStateKind::Overflow,
            severity,
            computed_at: ctx.now_ts,
            payload: json!({
                "sections": sections,
                "max_section_chars": max_chars,
                "triggers": triggers,
            }),
        })
    }
}

/// Map a count to a severity step. `None` means the count is below the
/// warning threshold — no flag.
fn severity_for(value: usize, warn: usize, severe: usize) -> Option<Severity> {
    if value >= severe {
        Some(Severity::Severe)
    } else if value >= warn {
        Some(Severity::Warning)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ocean::types::{
        GridCell, KnowledgeNodeData, KnowledgeNodeSubtype, LayoutEntry, NodeSection,
        SourceAttribution,
    };

    fn knowledge_entry(id: &str, variant: NodeVariant) -> LayoutEntry {
        LayoutEntry {
            module_id: id.to_string(),
            module_name: id.to_string(),
            module_path: String::new(),
            cell: GridCell::new(0, 0),
            user_placed: true,
            node_variant: variant,
            lighthouse_id: None,
        }
    }

    fn section(name: &str, chars: usize) -> NodeSection {
        NodeSection {
            id: format!("sec-{}", name),
            name: name.to_string(),
            content_markdown: "x".repeat(chars),
            source: SourceAttribution::User,
            created_at: 0,
            updated_at: 0,
            ai_prompt: None,
            ai_model: None,
        }
    }

    fn data_with_sections(sections: Vec<NodeSection>) -> KnowledgeNodeData {
        KnowledgeNodeData {
            subtype: KnowledgeNodeSubtype::Concept,
            sections,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn module_variant_never_overflows() {
        let entry = knowledge_entry("mod", NodeVariant::Module);
        let ctx = ScanContext {
            node_id: "mod",
            layout_entry: &entry,
            knowledge: None,
            project_path: "/tmp/test",
            now_ts: 100,
        };
        assert!(SaturationScanner.evaluate(&ctx).is_none());
    }

    #[test]
    fn empty_node_does_not_overflow() {
        let entry = knowledge_entry("k", NodeVariant::KnowledgeNode);
        let data = data_with_sections(vec![]);
        let ctx = ScanContext {
            node_id: "k",
            layout_entry: &entry,
            knowledge: Some(&data),
            project_path: "/tmp/test",
            now_ts: 100,
        };
        assert!(SaturationScanner.evaluate(&ctx).is_none());
    }

    #[test]
    fn three_small_sections_below_threshold() {
        let entry = knowledge_entry("k", NodeVariant::KnowledgeNode);
        let data = data_with_sections(vec![
            section("a", 50),
            section("b", 60),
            section("c", 80),
        ]);
        let ctx = ScanContext {
            node_id: "k",
            layout_entry: &entry,
            knowledge: Some(&data),
            project_path: "/tmp/test",
            now_ts: 100,
        };
        assert!(SaturationScanner.evaluate(&ctx).is_none());
    }

    #[test]
    fn auth_node_at_1671_chars_does_not_overflow() {
        // Regression: under the old 1500-chars-per-section threshold the
        // user's "AUTH" node (3 sections, longest one ~1671 chars) tripped
        // a Warning. Thresholds were raised because that scale of content
        // is still considered manageable.
        let entry = knowledge_entry("auth", NodeVariant::KnowledgeNode);
        let data = data_with_sections(vec![
            section("test", 20),
            section("texto-extenso", 1671),
            section("jwt", 263),
        ]);
        let ctx = ScanContext {
            node_id: "auth",
            layout_entry: &entry,
            knowledge: Some(&data),
            project_path: "/tmp/test",
            now_ts: 100,
        };
        assert!(SaturationScanner.evaluate(&ctx).is_none());
    }

    #[test]
    fn seven_sections_triggers_warning() {
        let entry = knowledge_entry("k", NodeVariant::KnowledgeNode);
        let data = data_with_sections((0..7).map(|i| section(&format!("s{}", i), 100)).collect());
        let ctx = ScanContext {
            node_id: "k",
            layout_entry: &entry,
            knowledge: Some(&data),
            project_path: "/tmp/test",
            now_ts: 100,
        };
        let state = SaturationScanner.evaluate(&ctx).expect("should overflow");
        assert_eq!(state.kind, NodeStateKind::Overflow);
        assert_eq!(state.severity, Severity::Warning);
    }

    #[test]
    fn eleven_sections_triggers_severe() {
        let entry = knowledge_entry("k", NodeVariant::KnowledgeNode);
        let data = data_with_sections(
            (0..SECTIONS_SEVERE).map(|i| section(&format!("s{}", i), 100)).collect(),
        );
        let ctx = ScanContext {
            node_id: "k",
            layout_entry: &entry,
            knowledge: Some(&data),
            project_path: "/tmp/test",
            now_ts: 100,
        };
        let state = SaturationScanner.evaluate(&ctx).expect("should overflow");
        assert_eq!(state.severity, Severity::Severe);
    }

    #[test]
    fn one_section_at_3500_chars_triggers_warning() {
        let entry = knowledge_entry("k", NodeVariant::KnowledgeNode);
        let data = data_with_sections(vec![section("only", MAX_SECTION_WARN)]);
        let ctx = ScanContext {
            node_id: "k",
            layout_entry: &entry,
            knowledge: Some(&data),
            project_path: "/tmp/test",
            now_ts: 100,
        };
        let state = SaturationScanner.evaluate(&ctx).expect("should overflow");
        assert_eq!(state.severity, Severity::Warning);
    }

    #[test]
    fn one_section_at_5000_chars_triggers_severe() {
        let entry = knowledge_entry("k", NodeVariant::KnowledgeNode);
        let data = data_with_sections(vec![section("only", MAX_SECTION_SEVERE)]);
        let ctx = ScanContext {
            node_id: "k",
            layout_entry: &entry,
            knowledge: Some(&data),
            project_path: "/tmp/test",
            now_ts: 100,
        };
        let state = SaturationScanner.evaluate(&ctx).expect("should overflow");
        assert_eq!(state.severity, Severity::Severe);
    }

    #[test]
    fn lighthouse_variant_with_seven_sections_overflows() {
        let entry = knowledge_entry("faro", NodeVariant::Lighthouse);
        let data = data_with_sections((0..7).map(|i| section(&format!("s{}", i), 200)).collect());
        let ctx = ScanContext {
            node_id: "faro",
            layout_entry: &entry,
            knowledge: Some(&data),
            project_path: "/tmp/test",
            now_ts: 100,
        };
        assert!(SaturationScanner.evaluate(&ctx).is_some());
    }

    #[test]
    fn payload_carries_counts_and_triggers() {
        let entry = knowledge_entry("k", NodeVariant::KnowledgeNode);
        let data = data_with_sections(vec![section("only", MAX_SECTION_SEVERE)]);
        let ctx = ScanContext {
            node_id: "k",
            layout_entry: &entry,
            knowledge: Some(&data),
            project_path: "/tmp/test",
            now_ts: 100,
        };
        let state = SaturationScanner.evaluate(&ctx).expect("should overflow");
        let payload = state.payload.as_object().unwrap();
        assert_eq!(payload.get("sections").unwrap().as_u64(), Some(1));
        assert_eq!(
            payload.get("max_section_chars").unwrap().as_u64(),
            Some(MAX_SECTION_SEVERE as u64),
        );
        let triggers = payload.get("triggers").unwrap().as_array().unwrap();
        assert!(triggers
            .iter()
            .any(|v| v.as_str() == Some("max_section")));
        assert!(!triggers.iter().any(|v| v.as_str() == Some("sections")));
    }

    #[test]
    fn most_severe_signal_wins() {
        // 7 sections (Warning) + one 5000-char section (Severe) → Severe.
        let entry = knowledge_entry("k", NodeVariant::KnowledgeNode);
        let mut sections: Vec<NodeSection> = (0..6)
            .map(|i| section(&format!("s{}", i), 50))
            .collect();
        sections.push(section("huge", MAX_SECTION_SEVERE));
        let data = data_with_sections(sections);
        let ctx = ScanContext {
            node_id: "k",
            layout_entry: &entry,
            knowledge: Some(&data),
            project_path: "/tmp/test",
            now_ts: 100,
        };
        let state = SaturationScanner.evaluate(&ctx).expect("should overflow");
        assert_eq!(state.severity, Severity::Severe);
        let triggers: Vec<&str> = state
            .payload
            .as_object()
            .unwrap()
            .get("triggers")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert!(triggers.contains(&"sections"));
        assert!(triggers.contains(&"max_section"));
    }
}
