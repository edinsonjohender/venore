//! BFS placement algorithm for Ocean Canvas nodes.
//!
//! Positions modules on a grid using a BFS "army formation" pattern:
//! - Root node (most connections) placed at center (0,0)
//! - BFS layers radiate outward in Manhattan distance rings
//! - New nodes placed near their neighbors or via spiral fallback

use std::collections::{HashMap, HashSet, VecDeque};

use super::types::{GridCell, ModuleInfo};

// =============================================================================
// Initial Layout (BFS Army Formation)
// =============================================================================

/// Compute initial grid positions for all modules using BFS from the most-connected node.
///
/// Algorithm:
/// 1. Find root = module with most connections
/// 2. BFS from root: layer 0 = center, layer N = ring at distance N
/// 3. Each node placed at first free cell in its ring
pub fn compute_initial_layout(modules: &[ModuleInfo]) -> HashMap<String, GridCell> {
    if modules.is_empty() {
        return HashMap::new();
    }

    // Single module → place at origin
    if modules.len() == 1 {
        let mut result = HashMap::new();
        result.insert(modules[0].id.clone(), GridCell::new(0, 0));
        return result;
    }

    // Build adjacency set for quick lookup
    let module_ids: HashSet<&str> = modules.iter().map(|m| m.id.as_str()).collect();
    let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();

    for m in modules {
        let neighbors: Vec<&str> = m
            .dependencies
            .iter()
            .chain(m.dependents.iter())
            .filter(|id| module_ids.contains(id.as_str()))
            .map(|id| id.as_str())
            .collect();
        adjacency.insert(m.id.as_str(), neighbors);
    }

    // Find root: most connections (ties broken by first found)
    let root = modules
        .iter()
        .max_by_key(|m| adjacency.get(m.id.as_str()).map_or(0, |n| n.len()))
        .unwrap();

    // BFS to assign layers
    let mut layer_map: HashMap<&str, usize> = HashMap::new();
    let mut queue: VecDeque<&str> = VecDeque::new();

    queue.push_back(root.id.as_str());
    layer_map.insert(root.id.as_str(), 0);

    while let Some(current) = queue.pop_front() {
        let current_layer = layer_map[current];
        if let Some(neighbors) = adjacency.get(current) {
            for &neighbor in neighbors {
                if !layer_map.contains_key(neighbor) {
                    layer_map.insert(neighbor, current_layer + 1);
                    queue.push_back(neighbor);
                }
            }
        }
    }

    // Assign unconnected modules to the last layer + 1
    let max_layer = layer_map.values().copied().max().unwrap_or(0);
    for m in modules {
        if !layer_map.contains_key(m.id.as_str()) {
            layer_map.insert(m.id.as_str(), max_layer + 1);
        }
    }

    // Group modules by layer
    let mut layers: HashMap<usize, Vec<&str>> = HashMap::new();
    for (&id, &layer) in &layer_map {
        layers.entry(layer).or_default().push(id);
    }

    // Place each layer's modules in ring positions
    let center = GridCell::new(0, 0);
    let mut occupied: HashSet<GridCell> = HashSet::new();
    let mut result: HashMap<String, GridCell> = HashMap::new();

    let mut sorted_layers: Vec<usize> = layers.keys().copied().collect();
    sorted_layers.sort();

    for layer_idx in sorted_layers {
        let module_ids_in_layer = &layers[&layer_idx];

        if layer_idx == 0 {
            // Root at center
            for &id in module_ids_in_layer {
                let cell = spiral_find_free(center, &occupied);
                occupied.insert(cell);
                result.insert(id.to_string(), cell);
            }
        } else {
            // Generate ring positions at this distance, fill from ring
            let ring = ring_positions(center, layer_idx as i32);
            let mut ring_iter = ring.into_iter();

            for &id in module_ids_in_layer {
                // Try ring positions first, then fall back to spiral
                let cell = loop {
                    if let Some(candidate) = ring_iter.next() {
                        if !occupied.contains(&candidate) {
                            break candidate;
                        }
                    } else {
                        // Ring exhausted, use spiral from center at this distance
                        break spiral_find_free(
                            GridCell::new(layer_idx as i32, 0),
                            &occupied,
                        );
                    }
                };
                occupied.insert(cell);
                result.insert(id.to_string(), cell);
            }
        }
    }

    result
}

// =============================================================================
// Single Node Placement (for reconciliation)
// =============================================================================

/// Place a single new node near its neighbors, or at the next free spiral position.
pub fn place_new_node(
    module: &ModuleInfo,
    existing: &HashMap<String, GridCell>,
    occupied: &HashSet<GridCell>,
) -> GridCell {
    // Collect positions of connected nodes
    let neighbor_positions: Vec<GridCell> = module
        .dependencies
        .iter()
        .chain(module.dependents.iter())
        .filter_map(|id| existing.get(id))
        .copied()
        .collect();

    if !neighbor_positions.is_empty() {
        // Centroid of neighbor positions
        let sum_col: i32 = neighbor_positions.iter().map(|c| c.col).sum();
        let sum_row: i32 = neighbor_positions.iter().map(|c| c.row).sum();
        let count = neighbor_positions.len() as i32;
        let centroid = GridCell::new(sum_col / count, sum_row / count);

        // Find nearest free cell to centroid
        spiral_find_free(centroid, occupied)
    } else {
        // No connections → spiral from origin
        spiral_find_free(GridCell::new(0, 0), occupied)
    }
}

// =============================================================================
// Spiral Search
// =============================================================================

/// Find the nearest free cell to `center` using an expanding spiral search.
///
/// Checks center first, then expanding Manhattan distance rings.
pub fn spiral_find_free(center: GridCell, occupied: &HashSet<GridCell>) -> GridCell {
    if !occupied.contains(&center) {
        return center;
    }

    for distance in 1..100 {
        let ring = ring_positions(center, distance);
        for cell in ring {
            if !occupied.contains(&cell) {
                return cell;
            }
        }
    }

    // Fallback (should never happen with reasonable grid sizes)
    GridCell::new(center.col + 100, center.row + 100)
}

/// Generate all cells at exactly `distance` Manhattan distance from `center`.
///
/// Returns cells in a deterministic clockwise order starting from (center.col + distance, center.row).
pub fn ring_positions(center: GridCell, distance: i32) -> Vec<GridCell> {
    if distance == 0 {
        return vec![center];
    }

    let mut positions = Vec::with_capacity((4 * distance) as usize);

    // Walk the four edges of the Manhattan diamond:
    // Top-right edge:  (col+d, row) → (col, row+d)
    // Bottom-right:    (col, row+d) → (col-d, row)
    // Bottom-left:     (col-d, row) → (col, row-d)
    // Top-left:        (col, row-d) → (col+d, row)
    for i in 0..distance {
        // Top-right edge
        positions.push(GridCell::new(center.col + distance - i, center.row + i));
        // Bottom-right edge
        positions.push(GridCell::new(center.col - i, center.row + distance - i));
        // Bottom-left edge
        positions.push(GridCell::new(center.col - distance + i, center.row - i));
        // Top-left edge
        positions.push(GridCell::new(center.col + i, center.row - distance + i));
    }

    positions
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::test_utils::make_module;

    #[test]
    fn test_ring_positions_distance_0() {
        let ring = ring_positions(GridCell::new(0, 0), 0);
        assert_eq!(ring, vec![GridCell::new(0, 0)]);
    }

    #[test]
    fn test_ring_positions_distance_1() {
        let ring = ring_positions(GridCell::new(0, 0), 1);
        assert_eq!(ring.len(), 4);
        // All should be at Manhattan distance 1
        for cell in &ring {
            assert_eq!(cell.manhattan_distance(&GridCell::new(0, 0)), 1);
        }
    }

    #[test]
    fn test_ring_positions_distance_2() {
        let ring = ring_positions(GridCell::new(0, 0), 2);
        assert_eq!(ring.len(), 8);
        for cell in &ring {
            assert_eq!(cell.manhattan_distance(&GridCell::new(0, 0)), 2);
        }
    }

    #[test]
    fn test_ring_no_duplicates() {
        let ring = ring_positions(GridCell::new(3, 5), 3);
        let unique: HashSet<GridCell> = ring.iter().copied().collect();
        assert_eq!(unique.len(), ring.len(), "Ring should have no duplicates");
    }

    #[test]
    fn test_spiral_find_free_empty_grid() {
        let occupied = HashSet::new();
        let cell = spiral_find_free(GridCell::new(0, 0), &occupied);
        assert_eq!(cell, GridCell::new(0, 0));
    }

    #[test]
    fn test_spiral_find_free_center_occupied() {
        let mut occupied = HashSet::new();
        occupied.insert(GridCell::new(0, 0));
        let cell = spiral_find_free(GridCell::new(0, 0), &occupied);
        assert_ne!(cell, GridCell::new(0, 0));
        assert_eq!(cell.manhattan_distance(&GridCell::new(0, 0)), 1);
    }

    #[test]
    fn test_compute_initial_layout_empty() {
        let result = compute_initial_layout(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_compute_initial_layout_single() {
        let modules = vec![make_module("auth", vec![], vec![])];
        let result = compute_initial_layout(&modules);
        assert_eq!(result.len(), 1);
        assert_eq!(result["auth"], GridCell::new(0, 0));
    }

    #[test]
    fn test_compute_initial_layout_no_overlaps() {
        let modules = vec![
            make_module("api", vec!["auth", "db"], vec![]),
            make_module("auth", vec!["db"], vec!["api"]),
            make_module("db", vec![], vec!["api", "auth"]),
            make_module("ui", vec!["api"], vec![]),
            make_module("utils", vec![], vec![]),
        ];
        let result = compute_initial_layout(&modules);

        assert_eq!(result.len(), 5);

        // No two modules share a cell
        let cells: Vec<GridCell> = result.values().copied().collect();
        let unique: HashSet<GridCell> = cells.iter().copied().collect();
        assert_eq!(cells.len(), unique.len(), "All positions must be unique");
    }

    #[test]
    fn test_compute_initial_layout_root_at_center() {
        // db has most connections (3 total: api, auth, cache depend on it)
        let modules = vec![
            make_module("api", vec!["db"], vec![]),
            make_module("auth", vec!["db"], vec![]),
            make_module("cache", vec!["db"], vec![]),
            make_module("db", vec![], vec!["api", "auth", "cache"]),
        ];
        let result = compute_initial_layout(&modules);

        // Root (db) should be at center
        assert_eq!(result["db"], GridCell::new(0, 0));
    }

    #[test]
    fn test_place_new_node_with_neighbors() {
        let mut existing = HashMap::new();
        existing.insert("a".to_string(), GridCell::new(0, 0));
        existing.insert("b".to_string(), GridCell::new(2, 0));

        let occupied: HashSet<GridCell> = existing.values().copied().collect();

        let module = make_module("c", vec!["a", "b"], vec![]);
        let cell = place_new_node(&module, &existing, &occupied);

        // Centroid of (0,0) and (2,0) is (1,0). Should be placed there (it's free).
        assert_eq!(cell, GridCell::new(1, 0));
    }

    #[test]
    fn test_place_new_node_centroid_occupied() {
        let mut existing = HashMap::new();
        existing.insert("a".to_string(), GridCell::new(0, 0));
        existing.insert("b".to_string(), GridCell::new(2, 0));

        let mut occupied: HashSet<GridCell> = existing.values().copied().collect();
        occupied.insert(GridCell::new(1, 0)); // Block the centroid

        let module = make_module("c", vec!["a", "b"], vec![]);
        let cell = place_new_node(&module, &existing, &occupied);

        // Should find a cell near (1,0) but not at (1,0)
        assert!(!occupied.contains(&cell));
        assert!(cell.manhattan_distance(&GridCell::new(1, 0)) <= 2);
    }

    #[test]
    fn test_place_new_node_no_connections() {
        let existing = HashMap::new();
        let occupied = HashSet::new();

        let module = make_module("orphan", vec![], vec![]);
        let cell = place_new_node(&module, &existing, &occupied);

        // Should be placed at origin (first free cell)
        assert_eq!(cell, GridCell::new(0, 0));
    }
}
