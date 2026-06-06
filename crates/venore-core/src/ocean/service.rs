//! OceanLayoutService — owns grid positions, occupancy, and persistence.
//!
//! Follows the CheckpointManager pattern for atomic persistence and
//! the WizardSessionManager pattern for singleton access.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use once_cell::sync::Lazy;
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::error::{Result, VenoreError};
use super::placement::{compute_initial_layout, place_new_node, spiral_find_free};
use super::states::{NodeStateInstance, NodeStateKind};
use super::types::{
    CameraState, GridCell, KnowledgeNodeData, KnowledgeNodeSubtype, LayoutEntry, ManualConnection,
    ModuleInfo, MoveResult, NodeSection, NodeVariant, OceanLayout, SourceAttribution,
};

const LAYOUT_FILE: &str = ".venore/ocean-layout.json";

// =============================================================================
// Singleton Manager
// =============================================================================

/// Global map of project_path → OceanLayoutService
static OCEAN_LAYOUTS: Lazy<Mutex<HashMap<String, OceanLayoutService>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Run a closure with exclusive access to the OceanLayoutService for a project.
/// Creates the service if it doesn't exist yet. Single lock, no TOCTOU race.
pub fn with_service<T>(
    project_path: &str,
    f: impl FnOnce(&mut OceanLayoutService) -> T,
) -> Result<T> {
    let mut map = OCEAN_LAYOUTS.lock().map_err(|e| {
        VenoreError::Unknown(format!("Failed to lock ocean layouts: {}", e))
    })?;
    if !map.contains_key(project_path) {
        debug!(project_path, "Creating new OceanLayoutService");
        let mut service = OceanLayoutService::new(PathBuf::from(project_path));
        // Eagerly hydrate from disk if a saved layout exists. Without this,
        // any caller that hits with_service before the Ocean tab has been
        // opened (e.g. the chat context builder reading knowledge nodes,
        // or the headless eval harness) sees an empty layout and silently
        // skips features like the logbooks hint. Failures are logged
        // and ignored — corrupt files get backed up by load() itself.
        match service.load() {
            Ok(Some(saved)) => {
                service.positions = saved.positions;
                service.camera = saved.camera;
                service.knowledge_data = saved.knowledge_data;
                service.manual_connections = saved.manual_connections;
                service.lighthouse_colors = saved.lighthouse_colors;
                service.state_dismissals = saved.state_dismissals;
                service.rebuild_occupancy();
                service.mark_all_dirty();
                debug!(project_path, "OceanLayoutService hydrated from disk");
            }
            Ok(None) => {
                debug!(project_path, "No saved ocean layout — starting empty");
            }
            Err(e) => {
                warn!(project_path, error = %e, "Failed to hydrate ocean layout, starting empty");
            }
        }
        map.insert(project_path.to_string(), service);
    }
    let service = map.get_mut(project_path).unwrap(); // safe: just ensured it exists
    Ok(f(service))
}

// =============================================================================
// OceanLayoutService
// =============================================================================

pub struct OceanLayoutService {
    layout_path: PathBuf,
    /// module_id → layout entry
    positions: HashMap<String, LayoutEntry>,
    /// cell → module_id (inverse map for O(1) occupancy checks)
    occupancy: HashMap<GridCell, String>,
    /// Saved camera state
    camera: Option<CameraState>,
    /// Content layer for knowledge_node and lighthouse nodes.
    /// Same key as `positions`; missing entries treated as default (empty content).
    knowledge_data: HashMap<String, KnowledgeNodeData>,
    /// User-created directed connections between any two nodes.
    manual_connections: Vec<ManualConnection>,
    /// lighthouse_id → "#RRGGBB" override.
    lighthouse_colors: HashMap<String, String>,
    /// Persisted user dismissals: `node_id → kind_str → "ignore until ts"`.
    /// Survives restarts because it is pure user input — the rest of the
    /// state machinery is recomputed at runtime.
    state_dismissals: HashMap<String, HashMap<String, i64>>,
    /// In-memory: nodes whose state vectors are pending re-scan by the rover.
    /// Cleared per-node as the rover visits them.
    dirty: HashSet<String>,
    /// In-memory: latest computed state list per node. Empty / missing means
    /// "no active states" (or "not scanned yet"). Sent to the frontend in the
    /// `NodePosition` DTO and via `ocean-state-changed` events.
    node_states: HashMap<String, Vec<NodeStateInstance>>,
}

impl OceanLayoutService {
    /// Create a new service for the given project.
    pub fn new(project_path: PathBuf) -> Self {
        let layout_path = project_path.join(LAYOUT_FILE);
        Self {
            layout_path,
            positions: HashMap::new(),
            occupancy: HashMap::new(),
            camera: None,
            knowledge_data: HashMap::new(),
            manual_connections: Vec::new(),
            lighthouse_colors: HashMap::new(),
            state_dismissals: HashMap::new(),
            dirty: HashSet::new(),
            node_states: HashMap::new(),
        }
    }

    /// Initialize layout: load from disk or compute fresh.
    ///
    /// Flow:
    /// - Saved layout exists + hash matches → restore
    /// - Saved layout exists + modules empty (no session) → restore as-is
    /// - Saved layout exists + hash differs → reconcile
    /// - No saved layout → compute fresh via BFS
    pub fn initialize(&mut self, modules: &[ModuleInfo]) -> OceanLayout {
        let current_hash = Self::hash_sorted_ids(modules.iter().map(|m| m.id.as_str()));
        info!(modules_count = modules.len(), "Initializing ocean layout");

        match self.load() {
            Ok(Some(saved)) if saved.module_set_hash == current_hash => {
                info!("Restoring layout from disk (hash matches)");
                self.positions = saved.positions.clone();
                self.camera = saved.camera.clone();
                self.knowledge_data = saved.knowledge_data.clone();
                self.manual_connections = saved.manual_connections.clone();
                self.lighthouse_colors = saved.lighthouse_colors.clone();
                self.state_dismissals = saved.state_dismissals.clone();
                self.rebuild_occupancy();
                self.mark_all_dirty();
                self.get_layout()
            }
            Ok(Some(saved)) if modules.is_empty() => {
                info!("No modules provided, restoring saved layout as-is");
                self.positions = saved.positions;
                self.camera = saved.camera;
                self.knowledge_data = saved.knowledge_data;
                self.manual_connections = saved.manual_connections;
                self.lighthouse_colors = saved.lighthouse_colors;
                self.state_dismissals = saved.state_dismissals;
                self.rebuild_occupancy();
                self.mark_all_dirty();
                self.get_layout()
            }
            Ok(Some(saved)) => {
                info!("Layout hash differs, reconciling");
                self.positions = saved.positions;
                self.camera = saved.camera;
                self.knowledge_data = saved.knowledge_data;
                self.manual_connections = saved.manual_connections;
                self.lighthouse_colors = saved.lighthouse_colors;
                self.state_dismissals = saved.state_dismissals;
                self.rebuild_occupancy();
                self.reconcile(modules);
                self.mark_all_dirty();
                if let Err(e) = self.save() {
                    warn!("Failed to save layout after reconcile: {}", e);
                }
                self.get_layout()
            }
            Ok(None) | Err(_) if modules.is_empty() => {
                info!("No modules and no saved layout — returning empty");
                self.get_layout()
            }
            Ok(None) | Err(_) => {
                info!("Computing fresh layout via BFS");
                let cell_map = compute_initial_layout(modules);
                self.positions.clear();
                self.occupancy.clear();
                self.knowledge_data.clear();
                self.manual_connections.clear();
                self.lighthouse_colors.clear();
                self.state_dismissals.clear();
                self.node_states.clear();
                self.dirty.clear();

                for module in modules {
                    if let Some(&cell) = cell_map.get(&module.id) {
                        let entry = LayoutEntry {
                            module_id: module.id.clone(),
                            module_name: module.name.clone(),
                            module_path: module.path.clone(),
                            cell,
                            user_placed: false,
                            node_variant: NodeVariant::Module,
                            lighthouse_id: None,
                        };
                        self.occupancy.insert(cell, module.id.clone());
                        self.positions.insert(module.id.clone(), entry);
                    }
                }

                self.mark_all_dirty();
                if let Err(e) = self.save() {
                    warn!("Failed to save fresh layout: {}", e);
                }
                self.get_layout()
            }
        }
    }

    /// Attempt to move a node to a target cell.
    ///
    /// Returns `Accepted` if the cell is free, `Rejected` if occupied.
    /// Marks the node as `user_placed = true` on success.
    pub fn move_node(&mut self, node_id: &str, target: GridCell) -> MoveResult {
        debug!(node_id, col = target.col, row = target.row, "Move node request");

        // Check if node exists — extract only what we need (avoids cloning the full entry)
        let (old_cell, module_name, module_path, node_variant, lighthouse_id) = match self.positions.get(node_id) {
            Some(e) => (
                e.cell,
                e.module_name.clone(),
                e.module_path.clone(),
                e.node_variant,
                e.lighthouse_id.clone(),
            ),
            None => {
                warn!(node_id, "Move rejected: node not found");
                return MoveResult::Rejected {
                    node_id: node_id.to_string(),
                    reason: format!("Node '{}' not found", node_id),
                };
            }
        };

        // Check if target is occupied by another node
        if let Some(occupant) = self.occupancy.get(&target) {
            if occupant != node_id {
                debug!(node_id, occupant, "Move rejected: cell occupied");
                return MoveResult::Rejected {
                    node_id: node_id.to_string(),
                    reason: format!(
                        "Cell ({}, {}) is occupied by '{}'",
                        target.col, target.row, occupant
                    ),
                };
            }
        }

        // Move: remove from old cell, place at new cell
        self.occupancy.remove(&old_cell);
        self.occupancy.insert(target, node_id.to_string());

        let updated = LayoutEntry {
            module_id: node_id.to_string(),
            module_name,
            module_path,
            cell: target,
            user_placed: true,
            node_variant,
            lighthouse_id,
        };
        self.positions.insert(node_id.to_string(), updated);

        // Auto-save after move
        if let Err(e) = self.save() {
            warn!("Failed to save layout after move: {}", e);
        }

        MoveResult::Accepted {
            node_id: node_id.to_string(),
            cell: target,
        }
    }

    /// Create a new user-curated knowledge node at the given cell.
    ///
    /// When `lighthouse_id` is `Some`, the node is born already attached to
    /// that island — the attachment is validated *before* any mutation, so a
    /// bad id rejects the whole operation and creates nothing (no orphan).
    /// Pass `None` to create a floating node.
    ///
    /// Returns `Accepted { node_id, cell }` with the generated UUID on success,
    /// or `Rejected` if the cell is occupied or the lighthouse is invalid.
    /// Marks the node as `user_placed = true` so reconciliation never moves it.
    pub fn create_knowledge_node(
        &mut self,
        name: String,
        target: GridCell,
        lighthouse_id: Option<String>,
    ) -> MoveResult {
        debug!(name = %name, col = target.col, row = target.row, ?lighthouse_id, "Create knowledge node request");

        if let Some(occupant) = self.occupancy.get(&target) {
            debug!(occupant, "Create rejected: cell occupied");
            return MoveResult::Rejected {
                node_id: String::new(),
                reason: format!(
                    "Cell ({}, {}) is occupied by '{}'",
                    target.col, target.row, occupant
                ),
            };
        }

        // Validate the target lighthouse up front: a node attached to a
        // non-existent or non-lighthouse node would violate the island
        // invariant. Rejecting here (before the insert) is what keeps the
        // operation atomic — no half-created floating orphan on bad input.
        if let Some(ref lh_id) = lighthouse_id {
            match self.positions.get(lh_id) {
                Some(entry) if entry.node_variant == NodeVariant::Lighthouse => {}
                Some(_) => {
                    return MoveResult::Rejected {
                        node_id: String::new(),
                        reason: format!("Node '{}' is not a lighthouse", lh_id),
                    };
                }
                None => {
                    return MoveResult::Rejected {
                        node_id: String::new(),
                        reason: format!("Lighthouse '{}' not found", lh_id),
                    };
                }
            }
        }

        let node_id = Uuid::new_v4().to_string();
        let entry = LayoutEntry {
            module_id: node_id.clone(),
            module_name: name,
            module_path: String::new(),
            cell: target,
            user_placed: true,
            node_variant: NodeVariant::KnowledgeNode,
            lighthouse_id,
        };

        self.occupancy.insert(target, node_id.clone());
        self.positions.insert(node_id.clone(), entry);
        self.knowledge_data
            .insert(node_id.clone(), KnowledgeNodeData::with_now());
        self.dirty.insert(node_id.clone());

        if let Err(e) = self.save() {
            warn!("Failed to save layout after node creation: {}", e);
        }

        MoveResult::Accepted {
            node_id,
            cell: target,
        }
    }

    /// Move multiple nodes atomically. Either every move succeeds or none.
    ///
    /// `moves` is a list of `(node_id, target_cell)` tuples. The pre-check
    /// validates that each target cell is either free OR currently occupied by
    /// another node IN THE SAME GROUP (so the group can rotate within itself
    /// without false collisions). If any target collides with a node outside
    /// the group, the whole operation is rejected and no positions change.
    ///
    /// Returns one `MoveResult` per requested move, in the same order as input.
    pub fn move_nodes_atomic(
        &mut self,
        moves: Vec<(String, GridCell)>,
    ) -> Vec<MoveResult> {
        debug!(count = moves.len(), "Atomic multi-move request");

        if moves.is_empty() {
            return Vec::new();
        }

        let group_ids: HashSet<String> =
            moves.iter().map(|(id, _)| id.clone()).collect();

        // Pre-check: each node exists, target is free or belongs to the group
        for (node_id, target) in &moves {
            if !self.positions.contains_key(node_id) {
                debug!(node_id, "Multi-move rejected: node not found");
                return moves
                    .iter()
                    .map(|(id, _)| MoveResult::Rejected {
                        node_id: id.clone(),
                        reason: format!("Node '{}' not found", node_id),
                    })
                    .collect();
            }
            if let Some(occupant) = self.occupancy.get(target) {
                if !group_ids.contains(occupant) {
                    debug!(node_id, occupant, "Multi-move rejected: target occupied by non-group node");
                    return moves
                        .iter()
                        .map(|(id, _)| MoveResult::Rejected {
                            node_id: id.clone(),
                            reason: format!(
                                "Cell ({}, {}) is occupied by '{}'",
                                target.col, target.row, occupant
                            ),
                        })
                        .collect();
                }
            }
        }

        // Two-phase application: clear all old occupancy entries first, then
        // place all new ones. This handles intra-group swaps cleanly.
        for (node_id, _) in &moves {
            if let Some(entry) = self.positions.get(node_id) {
                self.occupancy.remove(&entry.cell);
            }
        }
        for (node_id, target) in &moves {
            self.occupancy.insert(*target, node_id.clone());
            if let Some(entry) = self.positions.get_mut(node_id) {
                entry.cell = *target;
                entry.user_placed = true;
            }
        }

        if let Err(e) = self.save() {
            warn!("Failed to save layout after atomic multi-move: {}", e);
        }

        moves
            .into_iter()
            .map(|(node_id, target)| MoveResult::Accepted {
                node_id,
                cell: target,
            })
            .collect()
    }

    /// Create a new lighthouse node at the given cell.
    ///
    /// A lighthouse is the anchor of a thematic cluster (island). For now this
    /// is purely a node with `node_variant = Lighthouse`; the Island entity
    /// (with members, name distinct from the lighthouse, etc.) lands in a
    /// follow-up step.
    pub fn create_lighthouse(&mut self, name: String, target: GridCell) -> MoveResult {
        debug!(name = %name, col = target.col, row = target.row, "Create lighthouse request");

        if let Some(occupant) = self.occupancy.get(&target) {
            debug!(occupant, "Create rejected: cell occupied");
            return MoveResult::Rejected {
                node_id: String::new(),
                reason: format!(
                    "Cell ({}, {}) is occupied by '{}'",
                    target.col, target.row, occupant
                ),
            };
        }

        let node_id = Uuid::new_v4().to_string();
        let entry = LayoutEntry {
            module_id: node_id.clone(),
            module_name: name,
            module_path: String::new(),
            cell: target,
            user_placed: true,
            node_variant: NodeVariant::Lighthouse,
            lighthouse_id: None,
        };

        self.occupancy.insert(target, node_id.clone());
        self.positions.insert(node_id.clone(), entry);
        self.knowledge_data
            .insert(node_id.clone(), KnowledgeNodeData::with_now());
        self.dirty.insert(node_id.clone());

        if let Err(e) = self.save() {
            warn!("Failed to save layout after lighthouse creation: {}", e);
        }

        MoveResult::Accepted {
            node_id,
            cell: target,
        }
    }

    /// Assign or unassign a node to a lighthouse cluster.
    ///
    /// `lighthouse_id = None` clears the assignment (node becomes "loose").
    /// Returns `Ok(true)` on success, `Err` if validation fails:
    ///   - target node does not exist
    ///   - lighthouse_id is set but the target lighthouse does not exist
    ///   - lighthouse_id points to a non-lighthouse node
    ///   - the node is itself a lighthouse (cannot be a child of another)
    ///   - the node is its own lighthouse (self-reference)
    pub fn set_node_lighthouse(
        &mut self,
        node_id: &str,
        lighthouse_id: Option<String>,
    ) -> std::result::Result<(), String> {
        debug!(node_id, ?lighthouse_id, "Set node lighthouse");

        // Validate the target node exists and is not itself a lighthouse
        match self.positions.get(node_id) {
            Some(entry) => {
                if entry.node_variant == NodeVariant::Lighthouse {
                    return Err(format!("'{}' is a lighthouse and cannot be assigned to another", node_id));
                }
            }
            None => return Err(format!("Node '{}' not found", node_id)),
        }

        // Validate the target lighthouse if any
        if let Some(ref lh_id) = lighthouse_id {
            if lh_id == node_id {
                return Err("A node cannot point to itself as lighthouse".to_string());
            }
            match self.positions.get(lh_id) {
                Some(entry) if entry.node_variant == NodeVariant::Lighthouse => {}
                Some(_) => return Err(format!("Node '{}' is not a lighthouse", lh_id)),
                None => return Err(format!("Lighthouse '{}' not found", lh_id)),
            }
        }

        // Apply
        if let Some(entry) = self.positions.get_mut(node_id) {
            entry.lighthouse_id = lighthouse_id;
            if let Err(e) = self.save() {
                warn!("Failed to save layout after lighthouse assignment: {}", e);
            }
        }
        Ok(())
    }

    /// Dissolve a lighthouse cluster: convert the lighthouse to a regular
    /// knowledge node and clear `lighthouse_id` on all its children.
    /// Nothing is deleted — only references and the variant change.
    /// Returns `true` if the lighthouse existed.
    pub fn dissolve_lighthouse(&mut self, lighthouse_id: &str) -> bool {
        debug!(lighthouse_id, "Dissolve lighthouse");

        match self.positions.get(lighthouse_id) {
            Some(e) if e.node_variant == NodeVariant::Lighthouse => {}
            _ => {
                debug!(lighthouse_id, "Dissolve: lighthouse not found or wrong variant");
                return false;
            }
        }

        // Demote the lighthouse to a regular knowledge node
        if let Some(entry) = self.positions.get_mut(lighthouse_id) {
            entry.node_variant = NodeVariant::KnowledgeNode;
        }
        // Drop the color override — it only applies while the node is a lighthouse
        self.lighthouse_colors.remove(lighthouse_id);

        // Clear lighthouse_id on every child
        for entry in self.positions.values_mut() {
            if entry.lighthouse_id.as_deref() == Some(lighthouse_id) {
                entry.lighthouse_id = None;
            }
        }

        if let Err(e) = self.save() {
            warn!("Failed to save layout after dissolve: {}", e);
        }
        true
    }

    /// Delete a lighthouse and all nodes that pointed to it. Returns the
    /// number of nodes deleted (lighthouse included).
    pub fn delete_lighthouse_cluster(&mut self, lighthouse_id: &str) -> u32 {
        debug!(lighthouse_id, "Delete lighthouse cluster");

        match self.positions.get(lighthouse_id) {
            Some(e) if e.node_variant == NodeVariant::Lighthouse => {}
            _ => {
                debug!(lighthouse_id, "Delete cluster: lighthouse not found or wrong variant");
                return 0;
            }
        }

        // Collect ids to remove (lighthouse + its children)
        let to_delete: Vec<String> = self
            .positions
            .iter()
            .filter(|(id, e)| {
                id.as_str() == lighthouse_id || e.lighthouse_id.as_deref() == Some(lighthouse_id)
            })
            .map(|(id, _)| id.clone())
            .collect();

        let mut count = 0u32;
        for id in &to_delete {
            if let Some(entry) = self.positions.remove(id) {
                self.occupancy.remove(&entry.cell);
                self.knowledge_data.remove(id);
                self.forget_node_state_tracking(id);
                count += 1;
            }
        }
        // Drop any manual connection touching one of the deleted nodes —
        // dangling endpoints would just be filtered by the renderer anyway.
        let dead: HashSet<&String> = to_delete.iter().collect();
        self.manual_connections
            .retain(|c| !dead.contains(&c.from_id) && !dead.contains(&c.to_id));
        // The lighthouse itself is in `to_delete`; drop its color override.
        self.lighthouse_colors.remove(lighthouse_id);

        if let Err(e) = self.save() {
            warn!("Failed to save layout after cluster delete: {}", e);
        }
        count
    }

    /// Delete a node from the layout. Frees its cell and persists.
    /// If the node is a lighthouse, children are orphaned (lighthouse_id
    /// cleared) before removal — never leaves dangling references.
    /// Returns `true` if the node existed, `false` otherwise.
    pub fn delete_node(&mut self, node_id: &str) -> bool {
        debug!(node_id, "Delete node request");

        let is_lighthouse = self
            .positions
            .get(node_id)
            .map(|e| e.node_variant == NodeVariant::Lighthouse)
            .unwrap_or(false);

        if is_lighthouse {
            for entry in self.positions.values_mut() {
                if entry.lighthouse_id.as_deref() == Some(node_id) {
                    entry.lighthouse_id = None;
                }
            }
            self.lighthouse_colors.remove(node_id);
        }

        match self.positions.remove(node_id) {
            Some(entry) => {
                self.occupancy.remove(&entry.cell);
                self.knowledge_data.remove(node_id);
                self.forget_node_state_tracking(node_id);
                self.manual_connections
                    .retain(|c| c.from_id != node_id && c.to_id != node_id);
                if let Err(e) = self.save() {
                    warn!("Failed to save layout after node deletion: {}", e);
                }
                true
            }
            None => {
                debug!(node_id, "Delete: node not found");
                false
            }
        }
    }

    /// Rename a node. Updates `module_name` in place and persists.
    /// Returns `true` if the node existed, `false` otherwise.
    pub fn rename_node(&mut self, node_id: &str, new_name: String) -> bool {
        debug!(node_id, new_name = %new_name, "Rename node request");
        match self.positions.get_mut(node_id) {
            Some(entry) => {
                entry.module_name = new_name;
                if let Err(e) = self.save() {
                    warn!("Failed to save layout after node rename: {}", e);
                }
                true
            }
            None => {
                debug!(node_id, "Rename: node not found");
                false
            }
        }
    }

    /// Reconcile layout when modules change (KEEP / NEW / DELETE).
    ///
    /// - Existing modules with positions: KEEP (never move user_placed)
    /// - New modules without positions: place via `place_new_node`
    /// - Old modules no longer in list: DELETE
    pub fn reconcile(&mut self, current_modules: &[ModuleInfo]) {
        debug!(modules_count = current_modules.len(), "Reconciling layout");
        let current_ids: HashSet<String> =
            current_modules.iter().map(|m| m.id.clone()).collect();
        let existing_ids: HashSet<String> = self.positions.keys().cloned().collect();

        // DELETE: remove modules no longer present
        let to_delete: Vec<String> = existing_ids.difference(&current_ids).cloned().collect();
        for id in &to_delete {
            if let Some(entry) = self.positions.remove(id) {
                self.occupancy.remove(&entry.cell);
                self.knowledge_data.remove(id);
                self.forget_node_state_tracking(id);
            }
        }

        // NEW: place modules that don't have positions yet
        let existing_cells: HashMap<String, GridCell> = self
            .positions
            .iter()
            .map(|(id, e)| (id.clone(), e.cell))
            .collect();
        let occupied: HashSet<GridCell> = self.occupancy.keys().copied().collect();

        for module in current_modules {
            if !self.positions.contains_key(&module.id) {
                let cell = place_new_node(module, &existing_cells, &occupied);
                let entry = LayoutEntry {
                    module_id: module.id.clone(),
                    module_name: module.name.clone(),
                    module_path: module.path.clone(),
                    cell,
                    user_placed: false,
                    node_variant: NodeVariant::Module,
                    lighthouse_id: None,
                };
                self.occupancy.insert(cell, module.id.clone());
                self.positions.insert(module.id.clone(), entry);
            }
        }
    }

    /// Save layout to disk (atomic write: temp + rename).
    /// Skips saving if there are no positions — prevents corrupting
    /// an existing layout file with empty data.
    pub fn save(&self) -> Result<()> {
        let layout = self.get_layout();
        crate::utils::atomic_json::write_atomic(&self.layout_path, &layout)?;
        debug!(path = ?self.layout_path, "Layout saved to disk");
        Ok(())
    }

    /// Load layout from disk. Returns None if file missing or corrupt.
    pub fn load(&self) -> Result<Option<OceanLayout>> {
        let result: Option<OceanLayout> =
            crate::utils::atomic_json::read_or_backup_corrupt(&self.layout_path)?;
        if result.is_some() {
            debug!(path = ?self.layout_path, "Layout loaded from disk");
        } else if self.layout_path.with_extension("json.corrupt").exists() {
            warn!(path = ?self.layout_path, "Corrupt layout file was backed up");
        }
        Ok(result)
    }

    /// Reset layout — force fresh BFS on next initialize.
    pub fn reset(&mut self) {
        info!("Resetting ocean layout");
        self.positions.clear();
        self.occupancy.clear();
        self.knowledge_data.clear();
        self.manual_connections.clear();
        self.lighthouse_colors.clear();
        self.state_dismissals.clear();
        self.node_states.clear();
        self.dirty.clear();
        self.camera = None;
        let _ = fs::remove_file(&self.layout_path);
    }

    /// Get current layout state for the frontend.
    pub fn get_layout(&self) -> OceanLayout {
        let module_hash = Self::hash_sorted_ids(self.positions.keys().map(|k| k.as_str()));
        OceanLayout {
            version: 1,
            module_set_hash: module_hash,
            positions: self.positions.clone(),
            camera: self.camera.clone(),
            knowledge_data: self.knowledge_data.clone(),
            manual_connections: self.manual_connections.clone(),
            lighthouse_colors: self.lighthouse_colors.clone(),
            state_dismissals: self.state_dismissals.clone(),
        }
    }

    /// Set the color override for a lighthouse. Validates that the node exists
    /// and is actually a lighthouse. Color must be a `#RRGGBB` string.
    pub fn set_lighthouse_color(
        &mut self,
        lighthouse_id: &str,
        color: String,
    ) -> std::result::Result<(), String> {
        debug!(lighthouse_id, %color, "Set lighthouse color");
        match self.positions.get(lighthouse_id) {
            Some(e) if e.node_variant == NodeVariant::Lighthouse => {}
            Some(_) => return Err(format!("'{}' is not a lighthouse", lighthouse_id)),
            None => return Err(format!("Lighthouse '{}' not found", lighthouse_id)),
        }
        if !is_hex_color(&color) {
            return Err(format!("Invalid color '{}', expected #RRGGBB", color));
        }
        self.lighthouse_colors.insert(lighthouse_id.to_string(), color);
        if let Err(e) = self.save() {
            warn!("Failed to save after set_lighthouse_color: {}", e);
        }
        Ok(())
    }

    /// Remove a lighthouse color override (revert to deterministic palette).
    pub fn clear_lighthouse_color(&mut self, lighthouse_id: &str) -> bool {
        debug!(lighthouse_id, "Clear lighthouse color");
        let removed = self.lighthouse_colors.remove(lighthouse_id).is_some();
        if removed {
            if let Err(e) = self.save() {
                warn!("Failed to save after clear_lighthouse_color: {}", e);
            }
        }
        removed
    }

    /// Save camera state.
    pub fn save_camera(&mut self, camera: CameraState) {
        self.camera = Some(camera);
        // Skip persisting camera-only updates when there are no positions to
        // accompany them — this prevents the camera-save path from clobbering
        // a valid on-disk layout with an empty one during pre-init events.
        if self.positions.is_empty() {
            debug!("save_camera: skipping disk write — no positions yet");
            return;
        }
        if let Err(e) = self.save() {
            warn!("Failed to save camera state: {}", e);
        }
    }

    // =========================================================================
    // Knowledge content layer — read + mutate
    // =========================================================================

    /// Read the content layer for a node, lazy-initializing an empty entry
    /// for nodes that exist in `positions` but were created before the
    /// content-layer schema (or for any reason haven't been touched yet).
    /// Returns `None` only when the node id doesn't exist at all.
    pub fn get_knowledge_data(&mut self, node_id: &str) -> Option<KnowledgeNodeData> {
        if !self.positions.contains_key(node_id) {
            return None;
        }
        if !self.knowledge_data.contains_key(node_id) {
            self.knowledge_data
                .insert(node_id.to_string(), KnowledgeNodeData::with_now());
            if let Err(e) = self.save() {
                warn!("Failed to save layout after lazy-init knowledge entry: {}", e);
            }
        }
        self.knowledge_data.get(node_id).cloned()
    }

    /// Internal: get-or-create the content entry for a node. Returns `None`
    /// only if the node doesn't exist in `positions`.
    fn ensure_knowledge_entry(&mut self, node_id: &str) -> Option<&mut KnowledgeNodeData> {
        if !self.positions.contains_key(node_id) {
            return None;
        }
        Some(
            self.knowledge_data
                .entry(node_id.to_string())
                .or_insert_with(KnowledgeNodeData::with_now),
        )
    }

    /// Change the subtype of a knowledge node.
    pub fn update_node_subtype(&mut self, node_id: &str, subtype: KnowledgeNodeSubtype) -> bool {
        debug!(node_id, ?subtype, "Update node subtype");
        let now = now_secs();
        match self.ensure_knowledge_entry(node_id) {
            Some(entry) => {
                entry.subtype = subtype;
                entry.updated_at = now;
            }
            None => return false,
        }
        if let Err(e) = self.save() {
            warn!("Failed to save after update_node_subtype: {}", e);
        }
        true
    }

    /// Append a new dynamic section to the node. Returns the created section.
    /// `ai_prompt`/`ai_model` are persisted only when the source is `Ai` —
    /// they enable a future "Regenerate" action for AI-generated sections.
    pub fn add_node_section(
        &mut self,
        node_id: &str,
        name: String,
        content_markdown: String,
        source: SourceAttribution,
        ai_prompt: Option<String>,
        ai_model: Option<String>,
    ) -> Option<NodeSection> {
        debug!(node_id, name = %name, "Add node section");
        let now = now_secs();
        let section = NodeSection {
            id: Uuid::new_v4().to_string(),
            name,
            content_markdown,
            source,
            created_at: now,
            updated_at: now,
            ai_prompt,
            ai_model,
        };
        match self.ensure_knowledge_entry(node_id) {
            Some(entry) => {
                entry.sections.push(section.clone());
                entry.updated_at = now;
            }
            None => return None,
        }
        self.mark_dirty(node_id);
        if let Err(e) = self.save() {
            warn!("Failed to save after add_node_section: {}", e);
        }
        Some(section)
    }

    /// Edit an existing section as an AI write. Replaces name + content,
    /// flips `source` to `Ai { model }`, and stamps `ai_prompt` / `ai_model`
    /// so the section can be regenerated later. Returns the post-update
    /// section if found.
    pub fn update_node_section_as_ai(
        &mut self,
        node_id: &str,
        section_id: &str,
        name: String,
        content_markdown: String,
        ai_prompt: String,
        ai_model: String,
    ) -> Option<NodeSection> {
        debug!(node_id, section_id, "Update node section as AI");
        let now = now_secs();
        let updated = match self.ensure_knowledge_entry(node_id) {
            Some(entry) => {
                if let Some(section) = entry.sections.iter_mut().find(|s| s.id == section_id) {
                    section.name = name;
                    section.content_markdown = content_markdown;
                    section.source = SourceAttribution::Ai {
                        model: ai_model.clone(),
                        timestamp: now,
                    };
                    section.ai_prompt = Some(ai_prompt);
                    section.ai_model = Some(ai_model);
                    section.updated_at = now;
                    entry.updated_at = now;
                    Some(section.clone())
                } else {
                    None
                }
            }
            None => return None,
        };
        if updated.is_some() {
            self.mark_dirty(node_id);
            if let Err(e) = self.save() {
                warn!("Failed to save after update_node_section_as_ai: {}", e);
            }
        }
        updated
    }

    /// Update an existing section by id. Either field may be omitted.
    /// Returns `true` if the section was found and updated.
    pub fn update_node_section(
        &mut self,
        node_id: &str,
        section_id: &str,
        name: Option<String>,
        content_markdown: Option<String>,
    ) -> bool {
        debug!(node_id, section_id, "Update node section");
        let now = now_secs();
        let updated = match self.ensure_knowledge_entry(node_id) {
            Some(entry) => {
                if let Some(section) = entry.sections.iter_mut().find(|s| s.id == section_id) {
                    if let Some(n) = name {
                        section.name = n;
                    }
                    if let Some(c) = content_markdown {
                        section.content_markdown = c;
                    }
                    section.updated_at = now;
                    entry.updated_at = now;
                    true
                } else {
                    false
                }
            }
            None => return false,
        };
        if updated {
            self.mark_dirty(node_id);
            if let Err(e) = self.save() {
                warn!("Failed to save after update_node_section: {}", e);
            }
        }
        updated
    }

    /// Delete a section by id. Returns `true` if removed.
    pub fn delete_node_section(&mut self, node_id: &str, section_id: &str) -> bool {
        debug!(node_id, section_id, "Delete node section");
        let now = now_secs();
        let removed = match self.ensure_knowledge_entry(node_id) {
            Some(entry) => {
                let len_before = entry.sections.len();
                entry.sections.retain(|s| s.id != section_id);
                let removed = entry.sections.len() < len_before;
                if removed {
                    entry.updated_at = now;
                }
                removed
            }
            None => return false,
        };
        if removed {
            self.mark_dirty(node_id);
            if let Err(e) = self.save() {
                warn!("Failed to save after delete_node_section: {}", e);
            }
        }
        removed
    }

    /// Promote a regular knowledge node to a lighthouse (anchor of an island).
    /// Auto-detaches the node from any current island it belongs to — a
    /// lighthouse cannot be a child of another lighthouse.
    ///
    /// Returns `Ok(())` on success, `Err` if the node doesn't exist, is a
    /// module (only knowledge nodes can be promoted), or is already a
    /// lighthouse.
    pub fn promote_to_lighthouse(&mut self, node_id: &str) -> std::result::Result<(), String> {
        debug!(node_id, "Promote node to lighthouse");
        let entry = match self.positions.get_mut(node_id) {
            Some(e) => e,
            None => return Err(format!("Node '{}' not found", node_id)),
        };
        match entry.node_variant {
            NodeVariant::Lighthouse => return Err(format!("'{}' is already a lighthouse", node_id)),
            // Module / Buoy / Cylinder are code-representational; only
            // knowledge nodes are user-curated concepts that can be promoted.
            NodeVariant::Module | NodeVariant::Buoy | NodeVariant::Cylinder => {
                return Err(format!(
                    "'{}' is not a knowledge node — only knowledge nodes can be promoted",
                    node_id
                ))
            }
            NodeVariant::KnowledgeNode => {}
        }
        entry.node_variant = NodeVariant::Lighthouse;
        entry.lighthouse_id = None;
        self.mark_dirty(node_id);

        if let Err(e) = self.save() {
            warn!("Failed to save after promote_to_lighthouse: {}", e);
        }
        Ok(())
    }

    /// Move one section out of its node into a brand-new knowledge node.
    /// The new node carries only the extracted section, is placed at the
    /// first free cell in a spiral around the source, and inherits no
    /// island membership (loose). Returns the new node id + cell.
    pub fn extract_section_to_node(
        &mut self,
        source_node_id: &str,
        section_id: &str,
    ) -> std::result::Result<(String, GridCell, String), String> {
        debug!(source_node_id, section_id, "Extract section to node");

        // 1. Pull the source node and the section out (validate first).
        let source_cell = match self.positions.get(source_node_id) {
            Some(entry) => entry.cell,
            None => return Err(format!("Source node '{}' not found", source_node_id)),
        };
        let entry = match self.knowledge_data.get_mut(source_node_id) {
            Some(e) => e,
            None => return Err(format!("Source node '{}' has no content layer", source_node_id)),
        };
        let position = entry.sections.iter().position(|s| s.id == section_id);
        let pos = match position {
            Some(p) => p,
            None => return Err(format!("Section '{}' not found in source node", section_id)),
        };
        let section = entry.sections.remove(pos);
        let now = now_secs();
        entry.updated_at = now;
        let new_name = section.name.clone();

        // 2. Find a free cell near the source via spiral.
        let target = spiral_find_free(source_cell, &self.occupancy.keys().copied().collect());

        // 3. Create the new knowledge node carrying only the extracted section.
        let new_node_id = Uuid::new_v4().to_string();
        let layout_entry = LayoutEntry {
            module_id: new_node_id.clone(),
            module_name: new_name.clone(),
            module_path: String::new(),
            cell: target,
            user_placed: true,
            node_variant: NodeVariant::KnowledgeNode,
            lighthouse_id: None,
        };
        let mut new_data = KnowledgeNodeData::with_now();
        // Replace the default sections with just the extracted one — the user
        // wanted this section as its own node, not a fresh template.
        new_data.sections = vec![NodeSection {
            id: Uuid::new_v4().to_string(),
            name: section.name,
            content_markdown: section.content_markdown,
            source: section.source,
            created_at: section.created_at,
            updated_at: now,
            ai_prompt: section.ai_prompt,
            ai_model: section.ai_model,
        }];
        new_data.updated_at = now;

        self.occupancy.insert(target, new_node_id.clone());
        self.positions.insert(new_node_id.clone(), layout_entry);
        self.knowledge_data.insert(new_node_id.clone(), new_data);

        // Both sides change shape: source lost a section, new node carries
        // it. Re-scan both so any state vectors flip immediately.
        self.mark_dirty(source_node_id);
        self.mark_dirty(&new_node_id);

        if let Err(e) = self.save() {
            warn!("Failed to save after extract_section_to_node: {}", e);
        }

        Ok((new_node_id, target, new_name))
    }

    /// Reorder sections to match the given ordered list of section ids.
    /// Any id not present in the current sections is skipped; any current
    /// section whose id is missing from the input list keeps its relative
    /// order at the end. Returns `true` if the entry exists.
    pub fn reorder_node_sections(&mut self, node_id: &str, ordered_ids: Vec<String>) -> bool {
        debug!(node_id, count = ordered_ids.len(), "Reorder node sections");
        let now = now_secs();
        let ok = match self.ensure_knowledge_entry(node_id) {
            Some(entry) => {
                let mut by_id: HashMap<String, NodeSection> = entry
                    .sections
                    .drain(..)
                    .map(|s| (s.id.clone(), s))
                    .collect();
                let mut new_order = Vec::with_capacity(by_id.len());
                for id in &ordered_ids {
                    if let Some(section) = by_id.remove(id) {
                        new_order.push(section);
                    }
                }
                // Append any sections the caller didn't list (e.g. one was just
                // added on another window) so we never silently drop content.
                for (_, section) in by_id.drain() {
                    new_order.push(section);
                }
                entry.sections = new_order;
                entry.updated_at = now;
                true
            }
            None => return false,
        };
        if ok {
            self.mark_dirty(node_id);
            if let Err(e) = self.save() {
                warn!("Failed to save after reorder_node_sections: {}", e);
            }
        }
        ok
    }

    // =========================================================================
    // Manual connections
    // =========================================================================

    /// Create a directed manual connection between two nodes.
    ///
    /// Validates: nodes exist and are different, and the same directed pair
    /// doesn't already exist (the inverse `to → from` IS allowed — bidirectional
    /// intent is expressed by two arrows).
    pub fn create_connection(
        &mut self,
        from_id: &str,
        to_id: &str,
    ) -> std::result::Result<ManualConnection, String> {
        debug!(from_id, to_id, "Create manual connection");
        if from_id == to_id {
            return Err("A node cannot connect to itself".to_string());
        }
        if !self.positions.contains_key(from_id) {
            return Err(format!("Source node '{}' not found", from_id));
        }
        if !self.positions.contains_key(to_id) {
            return Err(format!("Target node '{}' not found", to_id));
        }
        if self
            .manual_connections
            .iter()
            .any(|c| c.from_id == from_id && c.to_id == to_id)
        {
            return Err("Connection already exists".to_string());
        }
        let conn = ManualConnection {
            id: Uuid::new_v4().to_string(),
            from_id: from_id.to_string(),
            to_id: to_id.to_string(),
            created_at: now_secs(),
        };
        self.manual_connections.push(conn.clone());
        if let Err(e) = self.save() {
            warn!("Failed to save after create_connection: {}", e);
        }
        Ok(conn)
    }

    /// Delete a manual connection by id. Returns whether anything was removed.
    pub fn delete_connection(&mut self, connection_id: &str) -> bool {
        debug!(connection_id, "Delete manual connection");
        let len_before = self.manual_connections.len();
        self.manual_connections.retain(|c| c.id != connection_id);
        let removed = self.manual_connections.len() < len_before;
        if removed {
            if let Err(e) = self.save() {
                warn!("Failed to save after delete_connection: {}", e);
            }
        }
        removed
    }

    /// Read-only snapshot of manual connections.
    pub fn list_manual_connections(&self) -> Vec<ManualConnection> {
        self.manual_connections.clone()
    }

    /// Find the first free grid cell starting from `anchor` and spiralling
    /// outward in concentric rings (radius 0..=max_radius). Used by tool
    /// handlers that don't want to make the LLM pick coordinates.
    /// Returns `Some(cell)` if a free cell is found within the radius, else
    /// `None`. `max_radius=8` covers ~217 cells which is plenty for any
    /// realistic project size at the time of creation.
    pub fn find_free_cell_near(&self, anchor: GridCell, max_radius: i32) -> Option<GridCell> {
        if !self.occupancy.contains_key(&anchor) {
            return Some(anchor);
        }
        for r in 1..=max_radius {
            for dc in -r..=r {
                for dr in -r..=r {
                    // Only consider cells exactly on the ring perimeter to
                    // avoid revisiting interior cells from earlier rings.
                    if dc.abs() != r && dr.abs() != r {
                        continue;
                    }
                    let candidate = GridCell::new(anchor.col + dc, anchor.row + dr);
                    if !self.occupancy.contains_key(&candidate) {
                        return Some(candidate);
                    }
                }
            }
        }
        None
    }

    /// Find the first free grid cell whose Manhattan distance to **every**
    /// existing node (any variant) is at least `min_distance`. Spirals
    /// outward from the centroid of all existing nodes, then origin if no
    /// nodes exist. Used by `create_lighthouse` so a new isla doesn't land
    /// on top of another — gives each cluster room to breathe (its
    /// centroid placement of children needs ~3-4 cells of radius).
    ///
    /// `max_radius` bounds the spiral to avoid runaway searches; with the
    /// default of 32 the function inspects up to ~2113 candidate cells,
    /// enough for hundreds of well-separated islas.
    pub fn find_free_cell_min_distance(
        &self,
        min_distance: i32,
        max_radius: i32,
    ) -> Option<GridCell> {
        // Empty workspace → place at origin.
        if self.positions.is_empty() {
            return Some(GridCell::new(0, 0));
        }
        // Centre of mass of existing nodes — start the spiral here so the
        // search finds the nearest-but-still-distant free cell instead of
        // always biasing toward the eastern half of the grid.
        let n = self.positions.len() as i32;
        let sum_col: i32 = self.positions.values().map(|e| e.cell.col).sum();
        let sum_row: i32 = self.positions.values().map(|e| e.cell.row).sum();
        let centre = GridCell::new(sum_col / n, sum_row / n);

        for r in 0..=max_radius {
            for dc in -r..=r {
                for dr in -r..=r {
                    if r > 0 && dc.abs() != r && dr.abs() != r {
                        continue; // ring perimeter only
                    }
                    let cell = GridCell::new(centre.col + dc, centre.row + dr);
                    if self.occupancy.contains_key(&cell) {
                        continue;
                    }
                    let too_close = self
                        .positions
                        .values()
                        .any(|e| cell.manhattan_distance(&e.cell) < min_distance);
                    if !too_close {
                        return Some(cell);
                    }
                }
            }
        }
        None
    }

    /// Find the first free grid cell near the **centroid** of a set of
    /// anchor cells, optionally rejecting cells too close to any
    /// `forbidden` cell. This lets a new knowledge_node be placed inside
    /// its own isla (centroid of faro + siblings) without crossing into
    /// the territory of a *different* isla (forbidden = other faros'
    /// cells, with `min_forbidden_gap` enforced).
    ///
    /// Anchors empty → falls back to spiral from (0, 0). Forbidden empty
    /// or `min_forbidden_gap` <= 0 → no forbidden filter applied. If no
    /// cell satisfies the filter within the search radius, returns the
    /// centroid itself (caller layers on its own validation).
    pub fn find_free_cell_centroid(
        &self,
        anchors: &[GridCell],
        forbidden: &[GridCell],
        min_forbidden_gap: i32,
    ) -> GridCell {
        let centroid = if anchors.is_empty() {
            GridCell::new(0, 0)
        } else {
            let sum_col: i32 = anchors.iter().map(|c| c.col).sum();
            let sum_row: i32 = anchors.iter().map(|c| c.row).sum();
            let n = anchors.len() as i32;
            GridCell::new(sum_col / n, sum_row / n)
        };

        // Spiral up to 16 rings from the centroid, skipping cells either
        // already occupied OR too close to any forbidden cell.
        const MAX_RADIUS: i32 = 16;
        let enforce_gap = !forbidden.is_empty() && min_forbidden_gap > 0;
        for r in 0..=MAX_RADIUS {
            for dc in -r..=r {
                for dr in -r..=r {
                    if r > 0 && dc.abs() != r && dr.abs() != r {
                        continue;
                    }
                    let cell = GridCell::new(centroid.col + dc, centroid.row + dr);
                    if self.occupancy.contains_key(&cell) {
                        continue;
                    }
                    if enforce_gap
                        && forbidden
                            .iter()
                            .any(|f| cell.manhattan_distance(f) < min_forbidden_gap)
                    {
                        continue;
                    }
                    return cell;
                }
            }
        }
        // Fallback: just give back the centroid even if it's occupied —
        // upstream will treat collisions sensibly.
        centroid
    }

    // =========================================================================
    // Node states — dirty queue, scan application, dismissals
    // =========================================================================

    /// Mark a node as needing a re-scan. The rover picks dirty nodes up on
    /// its next tick. No-op if the node id doesn't exist.
    pub fn mark_dirty(&mut self, node_id: &str) {
        if self.positions.contains_key(node_id) {
            self.dirty.insert(node_id.to_string());
        }
    }

    /// Mark every node currently in the layout as dirty. Used after hydrating
    /// from disk (states aren't persisted) and after a wholesale rebuild.
    pub fn mark_all_dirty(&mut self) {
        self.dirty.extend(self.positions.keys().cloned());
    }

    /// Drop all state tracking for a node id (no-op if absent). Called from
    /// every place that removes a node from `positions` so the rover doesn't
    /// chase ghost ids.
    pub fn forget_node_state_tracking(&mut self, node_id: &str) {
        self.dirty.remove(node_id);
        self.node_states.remove(node_id);
        // Dismissals stay until save() rewrites them — drop them too so
        // they don't haunt the persisted layout when the user recreates a
        // node with the same id (rare but possible).
        if self.state_dismissals.remove(node_id).is_some() {
            // Persisted change — let the caller's `save()` flush it.
        }
    }

    /// Pop the dirty node nearest to `from` (Manhattan distance).
    /// Returns `(node_id, cell)` or `None` if the queue is empty. Ties broken
    /// by lexicographic id for determinism. If `from` is `None`, picks the
    /// dirty node with the smallest `(col, row)` (deterministic seed).
    pub fn pop_next_dirty(&mut self, from: Option<GridCell>) -> Option<(String, GridCell)> {
        // Prune ids whose nodes have disappeared (deleted but the dirty
        // entry was missed). Cheap: at most one O(N) pass per call.
        self.dirty.retain(|id| self.positions.contains_key(id));

        if self.dirty.is_empty() {
            return None;
        }

        let from = from.unwrap_or(GridCell::new(0, 0));
        let mut best: Option<(String, GridCell, i32)> = None;
        for id in &self.dirty {
            let entry = match self.positions.get(id) {
                Some(e) => e,
                None => continue,
            };
            let dist = from.manhattan_distance(&entry.cell);
            let candidate = (id.clone(), entry.cell, dist);
            best = match best {
                None => Some(candidate),
                Some(prev) => {
                    if candidate.2 < prev.2 || (candidate.2 == prev.2 && candidate.0 < prev.0) {
                        Some(candidate)
                    } else {
                        Some(prev)
                    }
                }
            };
        }

        match best {
            Some((id, cell, _)) => {
                self.dirty.remove(&id);
                Some((id, cell))
            }
            None => None,
        }
    }

    /// Apply the result of a scanner pass for one node. Filters out states
    /// that the user has dismissed (and whose dismissal hasn't expired).
    /// Returns `true` if `node_states[node_id]` actually changed (caller can
    /// then emit `ocean-state-changed`).
    pub fn apply_scan_result(
        &mut self,
        node_id: &str,
        results: Vec<NodeStateInstance>,
        now_ts: i64,
    ) -> bool {
        // Ghost ids don't reach this path unless the caller passes one — but
        // be defensive anyway.
        if !self.positions.contains_key(node_id) {
            self.dirty.remove(node_id);
            return false;
        }

        // Suppress dismissed kinds.
        let filtered: Vec<NodeStateInstance> = results
            .into_iter()
            .filter(|s| !self.is_dismissed(node_id, s.kind, now_ts))
            .collect();

        let changed = match self.node_states.get(node_id) {
            None => !filtered.is_empty(),
            Some(prev) => prev.len() != filtered.len()
                || prev.iter().zip(filtered.iter()).any(|(a, b)| !a.equivalent(b)),
        };

        if filtered.is_empty() {
            self.node_states.remove(node_id);
        } else {
            self.node_states.insert(node_id.to_string(), filtered);
        }

        changed
    }

    /// Read the current state list for a node. Returns empty if the node has
    /// no active states (or hasn't been scanned yet).
    pub fn get_node_states(&self, node_id: &str) -> Vec<NodeStateInstance> {
        self.node_states
            .get(node_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Read-only view of every node's current states. Used by the DTO mapper
    /// in `initialize_ocean_layout` to ship `states[]` per node.
    pub fn all_node_states(&self) -> &HashMap<String, Vec<NodeStateInstance>> {
        &self.node_states
    }

    /// Persist a "ignore until `until_ts`" dismissal for a state kind on a
    /// node. Setting `until_ts <= 0` clears the dismissal. Triggers a save
    /// because dismissals are pure user input.
    pub fn dismiss_state(&mut self, node_id: &str, kind: NodeStateKind, until_ts: i64) -> bool {
        if !self.positions.contains_key(node_id) {
            return false;
        }
        let key = kind.as_str().to_string();
        let entry = self.state_dismissals.entry(node_id.to_string()).or_default();
        if until_ts <= 0 {
            entry.remove(&key);
            if entry.is_empty() {
                self.state_dismissals.remove(node_id);
            }
        } else {
            entry.insert(key, until_ts);
        }
        // Re-scan so the suppressed (or newly-uncovered) state propagates.
        self.dirty.insert(node_id.to_string());
        if let Err(e) = self.save() {
            warn!("Failed to save after dismiss_state: {}", e);
        }
        true
    }

    /// True if the user has dismissed this state kind on this node and the
    /// dismissal hasn't expired. Used by `apply_scan_result` to suppress
    /// states the user explicitly silenced.
    pub fn is_dismissed(&self, node_id: &str, kind: NodeStateKind, now_ts: i64) -> bool {
        self.state_dismissals
            .get(node_id)
            .and_then(|m| m.get(kind.as_str()))
            .map(|until| *until > now_ts)
            .unwrap_or(false)
    }

    /// Number of nodes pending re-scan. Cheap diagnostic for the rover.
    pub fn dirty_count(&self) -> usize {
        self.dirty.len()
    }

    /// Read-only access to a node's `KnowledgeNodeData` without lazy-init.
    /// Scanners use this to evaluate without mutating the layout.
    pub fn peek_knowledge_data(&self, node_id: &str) -> Option<&KnowledgeNodeData> {
        self.knowledge_data.get(node_id)
    }

    /// Read-only access to a node's `LayoutEntry`. Scanners may need this
    /// for the variant or position.
    pub fn peek_layout_entry(&self, node_id: &str) -> Option<&LayoutEntry> {
        self.positions.get(node_id)
    }

    /// All node ids currently in the layout. Used by Currents to seed their
    /// own traversal queue (independent of the rover's `dirty` set).
    pub fn all_node_ids(&self) -> Vec<String> {
        self.positions.keys().cloned().collect()
    }

    // =========================================================================
    // Private helpers
    // =========================================================================

    /// Rebuild the occupancy map from positions.
    fn rebuild_occupancy(&mut self) {
        self.occupancy.clear();
        for (id, entry) in &self.positions {
            self.occupancy.insert(entry.cell, id.clone());
        }
    }

    /// Compute a SHA-256 hash of sorted IDs.
    fn hash_sorted_ids<'a>(ids: impl Iterator<Item = &'a str>) -> String {
        let mut sorted: Vec<&str> = ids.collect();
        sorted.sort();
        let joined = sorted.join(",");
        let mut hasher = Sha256::new();
        hasher.update(joined.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

/// Validate a `#RRGGBB` color string. Used by lighthouse color overrides.
fn is_hex_color(s: &str) -> bool {
    if s.len() != 7 || !s.starts_with('#') {
        return false;
    }
    s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Unix-epoch seconds, saturating to 0 if the system clock is before 1970.
fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::test_utils::make_module;
    use std::path::Path;
    use tempfile::TempDir;

    fn make_service(dir: &Path) -> OceanLayoutService {
        OceanLayoutService::new(dir.to_path_buf())
    }

    #[test]
    fn test_initialize_fresh_layout() {
        let dir = TempDir::new().unwrap();
        let mut svc = make_service(dir.path());

        let modules = vec![
            make_module("api", vec!["db"], vec![]),
            make_module("db", vec![], vec!["api"]),
        ];

        let layout = svc.initialize(&modules);
        assert_eq!(layout.positions.len(), 2);
        assert!(layout.positions.contains_key("api"));
        assert!(layout.positions.contains_key("db"));
    }

    #[test]
    fn test_initialize_no_overlapping_positions() {
        let dir = TempDir::new().unwrap();
        let mut svc = make_service(dir.path());

        let modules = vec![
            make_module("a", vec!["b", "c"], vec![]),
            make_module("b", vec!["c"], vec!["a"]),
            make_module("c", vec![], vec!["a", "b"]),
            make_module("d", vec![], vec![]),
        ];

        let layout = svc.initialize(&modules);
        let cells: Vec<GridCell> = layout.positions.values().map(|e| e.cell).collect();
        let unique: HashSet<GridCell> = cells.iter().copied().collect();
        assert_eq!(cells.len(), unique.len());
    }

    #[test]
    fn test_move_node_accepted() {
        let dir = TempDir::new().unwrap();
        let mut svc = make_service(dir.path());

        let modules = vec![
            make_module("a", vec![], vec![]),
            make_module("b", vec![], vec![]),
        ];
        svc.initialize(&modules);

        // Find a free cell to move "a" to
        let target = GridCell::new(10, 10);
        let result = svc.move_node("a", target);

        match result {
            MoveResult::Accepted { node_id, cell } => {
                assert_eq!(node_id, "a");
                assert_eq!(cell, target);
            }
            MoveResult::Rejected { .. } => panic!("Move should be accepted"),
        }

        // Verify user_placed is set
        assert!(svc.positions["a"].user_placed);
    }

    #[test]
    fn test_move_node_rejected_occupied() {
        let dir = TempDir::new().unwrap();
        let mut svc = make_service(dir.path());

        let modules = vec![
            make_module("a", vec![], vec![]),
            make_module("b", vec![], vec![]),
        ];
        svc.initialize(&modules);

        // Try to move "a" to "b"'s cell
        let b_cell = svc.positions["b"].cell;
        let result = svc.move_node("a", b_cell);

        match result {
            MoveResult::Rejected { node_id, reason } => {
                assert_eq!(node_id, "a");
                assert!(reason.contains("occupied"));
            }
            MoveResult::Accepted { .. } => panic!("Move should be rejected"),
        }
    }

    #[test]
    fn test_move_node_not_found() {
        let dir = TempDir::new().unwrap();
        let mut svc = make_service(dir.path());

        let modules = vec![make_module("a", vec![], vec![])];
        svc.initialize(&modules);

        let result = svc.move_node("nonexistent", GridCell::new(5, 5));
        match result {
            MoveResult::Rejected { reason, .. } => {
                assert!(reason.contains("not found"));
            }
            MoveResult::Accepted { .. } => panic!("Should be rejected"),
        }
    }

    #[test]
    fn test_reconcile_adds_new_modules() {
        let dir = TempDir::new().unwrap();
        let mut svc = make_service(dir.path());

        let initial = vec![make_module("a", vec![], vec![])];
        svc.initialize(&initial);

        let updated = vec![
            make_module("a", vec![], vec![]),
            make_module("b", vec!["a"], vec![]),
        ];
        svc.reconcile(&updated);

        assert_eq!(svc.positions.len(), 2);
        assert!(svc.positions.contains_key("b"));
    }

    #[test]
    fn test_reconcile_removes_deleted_modules() {
        let dir = TempDir::new().unwrap();
        let mut svc = make_service(dir.path());

        let initial = vec![
            make_module("a", vec![], vec![]),
            make_module("b", vec![], vec![]),
        ];
        svc.initialize(&initial);
        assert_eq!(svc.positions.len(), 2);

        // Remove "b"
        let updated = vec![make_module("a", vec![], vec![])];
        svc.reconcile(&updated);

        assert_eq!(svc.positions.len(), 1);
        assert!(!svc.positions.contains_key("b"));
    }

    #[test]
    fn test_reconcile_keeps_user_placed() {
        let dir = TempDir::new().unwrap();
        let mut svc = make_service(dir.path());

        let modules = vec![make_module("a", vec![], vec![])];
        svc.initialize(&modules);

        // User moves "a" to a specific spot
        svc.move_node("a", GridCell::new(5, 5));
        assert!(svc.positions["a"].user_placed);

        // Reconcile with same modules — "a" should stay put
        svc.reconcile(&modules);
        assert_eq!(svc.positions["a"].cell, GridCell::new(5, 5));
        assert!(svc.positions["a"].user_placed);
    }

    #[test]
    fn test_save_and_load_round_trip() {
        let dir = TempDir::new().unwrap();
        let mut svc = make_service(dir.path());

        let modules = vec![
            make_module("api", vec!["db"], vec![]),
            make_module("db", vec![], vec!["api"]),
        ];
        svc.initialize(&modules);
        svc.save_camera(CameraState {
            x: 10.0,
            z: 20.0,
            zoom: 30.0,
        });

        // Load in a new service instance
        let svc2 = make_service(dir.path());
        let loaded = svc2.load().unwrap().expect("Should load saved layout");

        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.positions.len(), 2);
        assert!(loaded.camera.is_some());

        let cam = loaded.camera.unwrap();
        assert!((cam.x - 10.0).abs() < f64::EPSILON);
        assert!((cam.z - 20.0).abs() < f64::EPSILON);
        assert!((cam.zoom - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_load_corrupt_file_returns_none() {
        let dir = TempDir::new().unwrap();
        let layout_path = dir.path().join(".venore/ocean-layout.json");
        fs::create_dir_all(layout_path.parent().unwrap()).unwrap();
        fs::write(&layout_path, "not valid json!!!").unwrap();

        let svc = make_service(dir.path());
        let result = svc.load().unwrap();
        assert!(result.is_none());

        // Corrupt file should be backed up
        let backup = layout_path.with_extension("json.corrupt");
        assert!(backup.exists());
    }

    #[test]
    fn test_reset_clears_everything() {
        let dir = TempDir::new().unwrap();
        let mut svc = make_service(dir.path());

        let modules = vec![make_module("a", vec![], vec![])];
        svc.initialize(&modules);
        assert!(!svc.positions.is_empty());

        svc.reset();
        assert!(svc.positions.is_empty());
        assert!(svc.occupancy.is_empty());
        assert!(svc.camera.is_none());
    }

    #[test]
    fn test_initialize_restores_from_disk() {
        let dir = TempDir::new().unwrap();

        let modules = vec![
            make_module("a", vec![], vec![]),
            make_module("b", vec![], vec![]),
        ];

        // First: initialize and save
        {
            let mut svc = make_service(dir.path());
            svc.initialize(&modules);
            svc.move_node("a", GridCell::new(7, 7));
        }

        // Second: new service, same modules — should restore from disk
        {
            let mut svc = make_service(dir.path());
            let layout = svc.initialize(&modules);

            // "a" should be at (7,7) from saved layout
            assert_eq!(layout.positions["a"].cell, GridCell::new(7, 7));
            assert!(layout.positions["a"].user_placed);
        }
    }

    #[test]
    fn test_move_node_same_cell() {
        let dir = TempDir::new().unwrap();
        let mut svc = make_service(dir.path());

        let modules = vec![make_module("a", vec![], vec![])];
        svc.initialize(&modules);

        let current_cell = svc.positions["a"].cell;
        let result = svc.move_node("a", current_cell);

        // Moving to own cell should be accepted
        match result {
            MoveResult::Accepted { .. } => {}
            MoveResult::Rejected { .. } => panic!("Moving to own cell should be accepted"),
        }
    }

    // ── create_knowledge_node: attachment validation ────────────────────

    fn lighthouse_id_of(svc: &mut OceanLayoutService, name: &str, cell: GridCell) -> String {
        match svc.create_lighthouse(name.to_string(), cell) {
            MoveResult::Accepted { node_id, .. } => node_id,
            MoveResult::Rejected { reason, .. } => panic!("lighthouse create rejected: {}", reason),
        }
    }

    #[test]
    fn test_create_knowledge_node_floating_when_no_lighthouse() {
        let dir = TempDir::new().unwrap();
        let mut svc = make_service(dir.path());

        let result = svc.create_knowledge_node("Topic".to_string(), GridCell::new(2, 2), None);
        match result {
            MoveResult::Accepted { node_id, .. } => {
                assert_eq!(svc.positions[&node_id].lighthouse_id, None);
            }
            MoveResult::Rejected { reason, .. } => panic!("should be accepted: {}", reason),
        }
    }

    #[test]
    fn test_create_knowledge_node_attached_to_valid_lighthouse() {
        let dir = TempDir::new().unwrap();
        let mut svc = make_service(dir.path());

        let lh = lighthouse_id_of(&mut svc, "Island", GridCell::new(0, 0));
        let result =
            svc.create_knowledge_node("Topic".to_string(), GridCell::new(1, 1), Some(lh.clone()));
        match result {
            MoveResult::Accepted { node_id, .. } => {
                // Born attached — no second mutation needed.
                assert_eq!(svc.positions[&node_id].lighthouse_id.as_deref(), Some(lh.as_str()));
            }
            MoveResult::Rejected { reason, .. } => panic!("should be accepted: {}", reason),
        }
    }

    #[test]
    fn test_create_knowledge_node_rejects_unknown_lighthouse_without_creating() {
        let dir = TempDir::new().unwrap();
        let mut svc = make_service(dir.path());

        let before = svc.positions.len();
        let result = svc.create_knowledge_node(
            "Topic".to_string(),
            GridCell::new(1, 1),
            Some("does-not-exist".to_string()),
        );
        match result {
            MoveResult::Rejected { reason, .. } => assert!(reason.contains("not found")),
            MoveResult::Accepted { .. } => panic!("should reject an unknown lighthouse"),
        }
        // The whole point: a bad id must NOT leave a floating orphan behind.
        assert_eq!(svc.positions.len(), before, "no node should have been created");
        assert!(svc.occupancy.get(&GridCell::new(1, 1)).is_none());
    }

    #[test]
    fn test_create_knowledge_node_rejects_non_lighthouse_target() {
        let dir = TempDir::new().unwrap();
        let mut svc = make_service(dir.path());

        // A plain knowledge node is not a valid attachment anchor.
        let anchor = match svc.create_knowledge_node("Anchor".to_string(), GridCell::new(0, 0), None)
        {
            MoveResult::Accepted { node_id, .. } => node_id,
            MoveResult::Rejected { reason, .. } => panic!("setup rejected: {}", reason),
        };

        let before = svc.positions.len();
        let result =
            svc.create_knowledge_node("Topic".to_string(), GridCell::new(1, 1), Some(anchor));
        match result {
            MoveResult::Rejected { reason, .. } => assert!(reason.contains("not a lighthouse")),
            MoveResult::Accepted { .. } => panic!("should reject a non-lighthouse anchor"),
        }
        assert_eq!(svc.positions.len(), before, "no node should have been created");
    }
}
