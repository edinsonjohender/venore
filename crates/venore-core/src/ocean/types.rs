//! Core types for the Ocean Canvas layout system.
//!
//! All layout data is stored in grid coordinates (col, row).
//! The frontend converts to world coordinates:
//!   x = col * cellSize + halfCell
//!   z = row * cellSize + halfCell

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// Grid Types
// =============================================================================

/// Grid position (col, row) — integer-based, NOT world coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GridCell {
    pub col: i32,
    pub row: i32,
}

impl GridCell {
    pub fn new(col: i32, row: i32) -> Self {
        Self { col, row }
    }

    /// Manhattan distance to another cell
    pub fn manhattan_distance(&self, other: &GridCell) -> i32 {
        (self.col - other.col).abs() + (self.row - other.row).abs()
    }
}

// =============================================================================
// Node Variant
// =============================================================================

/// Kind of node living in the Ocean.
///
/// `Module` = node mirrors a code module detected by analysis (default for
/// backwards compatibility with existing layout files).
/// `KnowledgeNode` = node created by the user inside a knowledge island.
/// `Lighthouse` = anchor node of a thematic cluster (island). Visually a
/// tall pillar; functionally the entry point and gateway of its island.
/// `Buoy` = three mini-buildings cluster — represents utils / helpers /
/// constants. Code-representational like `Module`; not a knowledge node.
/// `Cylinder` = stacked cylinders — represents external services / APIs /
/// databases. Same semantics as `Module` (no logbook, not promotable).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NodeVariant {
    #[default]
    Module,
    KnowledgeNode,
    Lighthouse,
    Buoy,
    Cylinder,
}

// =============================================================================
// Layout Types
// =============================================================================

/// One node's layout entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutEntry {
    pub module_id: String,
    pub module_name: String,
    #[serde(default)]
    pub module_path: String,
    pub cell: GridCell,
    /// true = user dragged it here, reconciliation never moves it
    pub user_placed: bool,
    /// Kind of node — defaults to Module for backwards compat with existing layouts
    #[serde(default)]
    pub node_variant: NodeVariant,
    /// Lighthouse this node belongs to, if any. The lighthouse IS the cluster
    /// — there is no separate Island entity. `None` = the node is "loose"
    /// (not part of any cluster).
    /// Accepts the legacy "island_id" key in persisted JSON for backwards compat.
    #[serde(default, alias = "island_id")]
    pub lighthouse_id: Option<String>,
}

/// Full layout state — persisted to `.venore/ocean-layout.json`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OceanLayout {
    /// Schema version (currently 1)
    pub version: u32,
    /// Hash of sorted module IDs for fast change detection
    pub module_set_hash: String,
    /// module_id → layout entry
    pub positions: HashMap<String, LayoutEntry>,
    /// Saved camera position
    #[serde(default)]
    pub camera: Option<CameraState>,
    /// Content layer for knowledge_node and lighthouse nodes.
    /// Keyed by the same id used in `positions`. Empty for module-only layouts.
    #[serde(default)]
    pub knowledge_data: HashMap<String, KnowledgeNodeData>,
    /// User-created directed connections between any two nodes. Distinct from
    /// the code-dependency connections — those are derived from analysis on
    /// the fly and never persisted here.
    #[serde(default)]
    pub manual_connections: Vec<ManualConnection>,
    /// Per-lighthouse color override (`#RRGGBB`). Lighthouses without an entry
    /// fall back to the deterministic-from-id palette on the frontend.
    #[serde(default)]
    pub lighthouse_colors: HashMap<String, String>,
    /// User-issued dismissals of derived node states. Outer key = `node_id`,
    /// inner key = `NodeStateKind::as_str()` (e.g. `"overflow"`), value =
    /// "ignore until this unix timestamp (seconds)". The rover suppresses
    /// any state whose dismissal timestamp is still in the future.
    ///
    /// This is the ONLY state-related thing we persist — the actual state
    /// list is recomputed in-memory by the rover on load + on mutation. We
    /// can't recompute a dismissal because it's pure user input.
    #[serde(default)]
    pub state_dismissals: HashMap<String, HashMap<String, i64>>,
}

/// A user-drawn directed edge between two nodes. Direction matters:
/// `from_id → to_id`. To express bidirectional intent the user creates two.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualConnection {
    pub id: String,
    pub from_id: String,
    pub to_id: String,
    pub created_at: i64,
}

/// Persisted camera state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraState {
    pub x: f64,
    pub z: f64,
    pub zoom: f64,
}

// =============================================================================
// Move Result
// =============================================================================

/// Result of a move attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MoveResult {
    Accepted { node_id: String, cell: GridCell },
    Rejected { node_id: String, reason: String },
}

// =============================================================================
// Module Info (lightweight input)
// =============================================================================

// =============================================================================
// Knowledge Node Data — content layer for knowledge_node and lighthouse nodes.
// =============================================================================
// Stored separately from LayoutEntry (which only carries position + variant)
// in OceanLayout.knowledge_data: HashMap<NodeId, KnowledgeNodeData>.
//
// All fields are #[serde(default)] so layouts persisted before this schema
// existed deserialize cleanly with empty content.

/// Subtype hint for a knowledge node. Drives UX (icons, default sections,
/// etc.) but doesn't change the core data shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeNodeSubtype {
    #[default]
    Concept,
    Feature,
    Decision,
    Finding,
    Question,
}

/// Who or what produced a piece of content. AI entries also carry the model
/// name and the moment they were generated for traceability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[derive(Default)]
pub enum SourceAttribution {
    #[default]
    User,
    Ai {
        model: String,
        timestamp: i64,
    },
}


/// One markdown-backed section of a knowledge node. The section list is the
/// only structure: nodes are bags of sections, and the user (or AI) names,
/// orders, edits, and deletes them at will. New nodes start empty — the
/// visual stack only grows once the user (or an agent) adds content, so a
/// brand-new node never looks artificially "important".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSection {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub content_markdown: String,
    #[serde(default)]
    pub source: SourceAttribution,
    pub created_at: i64,
    pub updated_at: i64,
    /// Original prompt/intent that the AI used to generate this section.
    /// Only set when source = Ai. Enables a future "Regenerate" action.
    #[serde(default)]
    pub ai_prompt: Option<String>,
    /// Model id (redundant with `source.Ai.model` but exposed flat to the
    /// frontend so it doesn't need to parse the SourceAttribution variant).
    #[serde(default)]
    pub ai_model: Option<String>,
}

/// Persistent content layer for a knowledge node (or lighthouse).
/// Distinct from LayoutEntry (which only carries position + variant).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KnowledgeNodeData {
    #[serde(default)]
    pub subtype: KnowledgeNodeSubtype,
    /// The only content structure — a flat ordered list of named markdown
    /// sections. Stack height in the ocean visual mirrors this length.
    #[serde(default)]
    pub sections: Vec<NodeSection>,
    /// Creation and last-edit timestamps for the content layer.
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
}

impl KnowledgeNodeData {
    /// Build a fresh, empty content layer with timestamps set to now.
    /// No default sections — the node grows organically as content is added.
    pub fn with_now() -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        Self {
            subtype: KnowledgeNodeSubtype::default(),
            sections: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }
}

/// Lightweight module info for layout computation.
/// Extracted from `ModuleAnalysis` by the caller.
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub id: String,
    pub name: String,
    pub path: String,
    /// Module IDs this module depends on
    pub dependencies: Vec<String>,
    /// Module IDs that depend on this module
    pub dependents: Vec<String>,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_cell_manhattan_distance() {
        let a = GridCell::new(0, 0);
        let b = GridCell::new(3, 4);
        assert_eq!(a.manhattan_distance(&b), 7);
    }

    #[test]
    fn test_grid_cell_manhattan_distance_negative() {
        let a = GridCell::new(-2, -3);
        let b = GridCell::new(1, 2);
        assert_eq!(a.manhattan_distance(&b), 8);
    }

    #[test]
    fn test_grid_cell_equality() {
        let a = GridCell::new(1, 2);
        let b = GridCell::new(1, 2);
        let c = GridCell::new(2, 1);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_ocean_layout_serialization() {
        let mut positions = HashMap::new();
        positions.insert("mod-1".to_string(), LayoutEntry {
            module_id: "mod-1".to_string(),
            module_name: "Auth".to_string(),
            module_path: "src/auth".to_string(),
            cell: GridCell::new(0, 0),
            user_placed: false,
            node_variant: NodeVariant::Module,
            lighthouse_id: None,
        });

        let layout = OceanLayout {
            version: 1,
            module_set_hash: "abc123".to_string(),
            positions,
            camera: Some(CameraState { x: 10.0, z: 20.0, zoom: 30.0 }),
            knowledge_data: HashMap::new(),
            manual_connections: Vec::new(),
            lighthouse_colors: HashMap::new(),
            state_dismissals: HashMap::new(),
        };

        let json = serde_json::to_string(&layout).unwrap();
        let deserialized: OceanLayout = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.version, 1);
        assert_eq!(deserialized.positions.len(), 1);
        assert!(deserialized.camera.is_some());
    }

    #[test]
    fn ocean_layout_back_compat_without_state_dismissals() {
        // A layout file persisted before the state_dismissals field existed
        // must still deserialize cleanly thanks to #[serde(default)].
        let legacy_json = r#"{
            "version": 1,
            "module_set_hash": "deadbeef",
            "positions": {},
            "camera": null,
            "knowledge_data": {},
            "manual_connections": [],
            "lighthouse_colors": {}
        }"#;
        let layout: OceanLayout = serde_json::from_str(legacy_json).unwrap();
        assert!(layout.state_dismissals.is_empty());
    }

    #[test]
    fn ocean_layout_round_trips_state_dismissals() {
        let mut dismissals = HashMap::new();
        let mut per_node = HashMap::new();
        per_node.insert("overflow".to_string(), 1_700_000_000_i64);
        dismissals.insert("node-a".to_string(), per_node);

        let layout = OceanLayout {
            version: 1,
            module_set_hash: "x".to_string(),
            positions: HashMap::new(),
            camera: None,
            knowledge_data: HashMap::new(),
            manual_connections: Vec::new(),
            lighthouse_colors: HashMap::new(),
            state_dismissals: dismissals,
        };
        let json = serde_json::to_string(&layout).unwrap();
        let back: OceanLayout = serde_json::from_str(&json).unwrap();
        assert_eq!(
            back.state_dismissals
                .get("node-a")
                .and_then(|m| m.get("overflow"))
                .copied(),
            Some(1_700_000_000),
        );
    }
}
