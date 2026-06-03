//! Traversal primitives for Currents.
//!
//! Two layers:
//!   - [`nearest_pending`]: pick the single next node nearest a cursor (the
//!     original per-tick primitive; still a reusable building block).
//!   - **Route strategies** (`route_*`): plan a current's WHOLE visiting order
//!     for one sweep, up front. A current picks a strategy in `plan_route` so
//!     distinct currents trace visibly distinct paths across the Ocean instead
//!     of overlapping (every strategy is deterministic — no RNG). Planning once
//!     per sweep is also cheaper than recomputing "nearest" every tick.

use std::collections::HashSet;

use crate::ocean::types::GridCell;

// =============================================================================
// Route strategies — plan a full visiting order over the ocean's nodes
// =============================================================================

/// Greedy nearest-neighbor tour from `start`. Organic, snake-like flow that
/// hugs clusters. Ties broken by lexicographically smallest id (deterministic).
/// O(n²) but computed once per sweep, not per tick.
pub fn route_nearest_from(nodes: &[(String, GridCell)], start: GridCell) -> Vec<String> {
    let mut remaining: Vec<(String, GridCell)> = nodes.to_vec();
    let mut route = Vec::with_capacity(remaining.len());
    let mut from = start;

    while !remaining.is_empty() {
        let mut best = 0usize;
        for i in 1..remaining.len() {
            let (d_i, d_best) = (
                from.manhattan_distance(&remaining[i].1),
                from.manhattan_distance(&remaining[best].1),
            );
            if d_i < d_best || (d_i == d_best && remaining[i].0 < remaining[best].0) {
                best = i;
            }
        }
        let (id, cell) = remaining.swap_remove(best);
        from = cell;
        route.push(id);
    }
    route
}

/// Row-major sweep: top-to-bottom by row, left-to-right within a row. Reads as a
/// clean horizontal scan line — deliberately distinct from a nearest tour.
pub fn route_row_major(nodes: &[(String, GridCell)]) -> Vec<String> {
    let mut v: Vec<(String, GridCell)> = nodes.to_vec();
    v.sort_by(|a, b| {
        a.1.row
            .cmp(&b.1.row)
            .then(a.1.col.cmp(&b.1.col))
            .then(a.0.cmp(&b.0))
    });
    v.into_iter().map(|(id, _)| id).collect()
}

/// Column-major sweep: left-to-right by column, top-to-bottom within a column.
pub fn route_column_major(nodes: &[(String, GridCell)]) -> Vec<String> {
    let mut v: Vec<(String, GridCell)> = nodes.to_vec();
    v.sort_by(|a, b| {
        a.1.col
            .cmp(&b.1.col)
            .then(a.1.row.cmp(&b.1.row))
            .then(a.0.cmp(&b.0))
    });
    v.into_iter().map(|(id, _)| id).collect()
}

/// Outward spiral from the centroid: nearest the center first, fanning out by
/// distance then angle. Reads as a swirl — a third distinct flow.
pub fn route_spiral(nodes: &[(String, GridCell)]) -> Vec<String> {
    if nodes.is_empty() {
        return Vec::new();
    }
    let n = nodes.len() as f64;
    let cx = nodes.iter().map(|(_, c)| c.col as f64).sum::<f64>() / n;
    let cy = nodes.iter().map(|(_, c)| c.row as f64).sum::<f64>() / n;

    let mut v: Vec<(String, GridCell)> = nodes.to_vec();
    v.sort_by(|a, b| {
        let da = ((a.1.col as f64 - cx).powi(2) + (a.1.row as f64 - cy).powi(2)).sqrt();
        let db = ((b.1.col as f64 - cx).powi(2) + (b.1.row as f64 - cy).powi(2)).sqrt();
        let aa = (a.1.row as f64 - cy).atan2(a.1.col as f64 - cx);
        let ab = (b.1.row as f64 - cy).atan2(b.1.col as f64 - cx);
        da.partial_cmp(&db)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(aa.partial_cmp(&ab).unwrap_or(std::cmp::Ordering::Equal))
            .then(a.0.cmp(&b.0))
    });
    v.into_iter().map(|(id, _)| id).collect()
}

/// Bottom-right-most cell among `nodes` (max col+row), or origin if empty. Used
/// as a nearest-tour start so a current fans in from the opposite corner.
pub fn far_corner(nodes: &[(String, GridCell)]) -> GridCell {
    nodes
        .iter()
        .map(|(_, c)| *c)
        .max_by_key(|c| c.col + c.row)
        .unwrap_or_else(|| GridCell::new(0, 0))
}

/// Pick the pending node nearest to `from` (Manhattan distance). Ties broken by
/// lexicographically smallest id so traversal is deterministic. Nodes whose
/// `cell_of` returns `None` (disappeared) are skipped. Returns the chosen
/// `(node_id, cell)` or `None` when nothing pending resolves to a cell.
pub fn nearest_pending(
    pending: &HashSet<String>,
    from: Option<GridCell>,
    cell_of: impl Fn(&str) -> Option<GridCell>,
) -> Option<(String, GridCell)> {
    let from = from.unwrap_or(GridCell::new(0, 0));
    let mut best: Option<(String, GridCell, i32)> = None;

    for id in pending {
        let cell = match cell_of(id) {
            Some(c) => c,
            None => continue,
        };
        let dist = from.manhattan_distance(&cell);
        let candidate = (id.clone(), cell, dist);
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

    best.map(|(id, cell, _)| (id, cell))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_empty_pending_returns_none() {
        let cells = |_: &str| Some(GridCell::new(0, 0));
        assert!(nearest_pending(&set(&[]), None, cells).is_none());
    }

    #[test]
    fn test_picks_nearest_to_from() {
        let positions: std::collections::HashMap<&str, GridCell> = [
            ("a", GridCell::new(10, 10)),
            ("b", GridCell::new(1, 1)),
            ("c", GridCell::new(5, 5)),
        ].into_iter().collect();
        let cell_of = |id: &str| positions.get(id).copied();

        let (id, _) = nearest_pending(&set(&["a", "b", "c"]), Some(GridCell::new(0, 0)), cell_of).unwrap();
        assert_eq!(id, "b"); // closest to origin
    }

    #[test]
    fn test_tie_break_is_lexicographic() {
        let positions: std::collections::HashMap<&str, GridCell> = [
            ("z", GridCell::new(1, 0)),
            ("a", GridCell::new(0, 1)),
        ].into_iter().collect();
        let cell_of = |id: &str| positions.get(id).copied();

        // Both at distance 1 from origin → smallest id wins.
        let (id, _) = nearest_pending(&set(&["z", "a"]), Some(GridCell::new(0, 0)), cell_of).unwrap();
        assert_eq!(id, "a");
    }

    #[test]
    fn test_skips_nodes_without_cell() {
        let cell_of = |id: &str| if id == "live" { Some(GridCell::new(3, 3)) } else { None };
        let (id, cell) = nearest_pending(&set(&["live", "ghost"]), None, cell_of).unwrap();
        assert_eq!(id, "live");
        assert_eq!(cell, GridCell::new(3, 3));
    }

    #[test]
    fn test_all_ghosts_returns_none() {
        let cell_of = |_: &str| None;
        assert!(nearest_pending(&set(&["x", "y"]), None, cell_of).is_none());
    }

    // ------------------------------------------------------------------------
    // Route strategies
    // ------------------------------------------------------------------------

    fn nodes(items: &[(&str, i32, i32)]) -> Vec<(String, GridCell)> {
        items
            .iter()
            .map(|(id, col, row)| (id.to_string(), GridCell::new(*col, *row)))
            .collect()
    }

    #[test]
    fn route_covers_every_node_once() {
        let ns = nodes(&[("a", 0, 0), ("b", 5, 1), ("c", 2, 3), ("d", 9, 9)]);
        for route in [
            route_nearest_from(&ns, GridCell::new(0, 0)),
            route_row_major(&ns),
            route_column_major(&ns),
            route_spiral(&ns),
        ] {
            assert_eq!(route.len(), 4);
            let unique: HashSet<&String> = route.iter().collect();
            assert_eq!(unique.len(), 4, "route must visit each node exactly once");
        }
    }

    #[test]
    fn route_nearest_is_greedy_from_start() {
        // From origin: a(0,0) then c(1,1) then b(5,1).
        let ns = nodes(&[("a", 0, 0), ("b", 5, 1), ("c", 1, 1)]);
        assert_eq!(route_nearest_from(&ns, GridCell::new(0, 0)), vec!["a", "c", "b"]);
    }

    #[test]
    fn route_row_and_column_major_differ() {
        // Two nodes on different rows AND columns → the two sweeps order them
        // oppositely, proving the patterns are distinct.
        let ns = nodes(&[("top_right", 9, 0), ("bottom_left", 0, 9)]);
        assert_eq!(route_row_major(&ns), vec!["top_right", "bottom_left"]); // row 0 first
        assert_eq!(route_column_major(&ns), vec!["bottom_left", "top_right"]); // col 0 first
    }

    #[test]
    fn far_corner_picks_max_sum() {
        let ns = nodes(&[("a", 0, 0), ("b", 3, 4), ("c", 9, 1)]);
        // c has the largest col+row (10).
        assert_eq!(far_corner(&ns), GridCell::new(9, 1));
        assert_eq!(far_corner(&[]), GridCell::new(0, 0));
    }

    #[test]
    fn empty_routes_are_empty() {
        let empty: Vec<(String, GridCell)> = Vec::new();
        assert!(route_nearest_from(&empty, GridCell::new(0, 0)).is_empty());
        assert!(route_row_major(&empty).is_empty());
        assert!(route_spiral(&empty).is_empty());
    }
}
