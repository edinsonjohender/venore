//! Pending logbook writes — preview/diff approval queue for AI-proposed
//! section creates and edits.
//!
//! When the AI calls `propose_logbook_write`, the executor stashes the
//! proposal here instead of mutating the node directly. The desktop layer
//! emits `ai-write-proposed`, the node panel auto-opens, and the user
//! accepts / discards / regenerates from the panel after reviewing a diff.
//! Same singleton pattern as `OCEAN_LAYOUTS` in `ocean::service`.

use std::collections::HashMap;
use std::sync::Mutex;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};
use uuid::Uuid;

// =============================================================================
// Types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PendingKind {
    Create,
    Edit {
        section_id: String,
        baseline_name: String,
        baseline_content: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingSectionWrite {
    pub write_id: String,
    pub project_path: String,
    pub node_id: String,
    pub session_id: Option<String>,
    pub kind: PendingKind,
    pub name: String,
    pub content_markdown: String,
    pub ai_prompt: String,
    pub ai_model: String,
    pub diff_patch: Option<String>,
    pub additions: u32,
    pub deletions: u32,
    pub created_at: i64,
}

impl PendingSectionWrite {
    /// Stable identity for replace-on-conflict: a re-tried Edit on the same
    /// section_id, or a re-proposed Create with the same section name on
    /// the same node, overwrites the prior pending instead of stacking.
    /// Different section names / different section_ids coexist.
    fn dedupe_key(&self) -> (String, String, &'static str, String) {
        match &self.kind {
            PendingKind::Create => (
                self.project_path.clone(),
                self.node_id.clone(),
                "create",
                self.name.clone(),
            ),
            PendingKind::Edit { section_id, .. } => (
                self.project_path.clone(),
                self.node_id.clone(),
                "edit",
                section_id.clone(),
            ),
        }
    }
}

// =============================================================================
// Singleton
// =============================================================================

static PENDING_WRITES: Lazy<Mutex<HashMap<String, PendingSectionWrite>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn lock_map() -> std::sync::MutexGuard<'static, HashMap<String, PendingSectionWrite>> {
    match PENDING_WRITES.lock() {
        Ok(g) => g,
        Err(e) => {
            tracing::warn!(error = %e, "PENDING_WRITES mutex poisoned — recovering inner");
            e.into_inner()
        }
    }
}

pub fn new_write_id() -> String {
    Uuid::new_v4().to_string()
}

/// Insert a pending write, replacing any prior pending with the same dedupe
/// key. Returns the inserted `write_id`. Also marks the host node as dirty
/// in the ocean service so the rover re-runs scanners and the canvas gets
/// the `pending_writes` decorator on the next tick.
pub fn insert(write: PendingSectionWrite) -> String {
    let mut map = lock_map();
    let key = write.dedupe_key();
    map.retain(|_, existing| existing.dedupe_key() != key);
    let write_id = write.write_id.clone();
    let project_path = write.project_path.clone();
    let node_id = write.node_id.clone();
    tracing::info!(
        write_id = %write_id,
        project_path = %project_path,
        node_id = %node_id,
        "Pending write inserted"
    );
    map.insert(write_id.clone(), write);
    drop(map);
    mark_node_dirty(&project_path, &node_id);
    write_id
}

pub fn list_for_node(project_path: &str, node_id: &str) -> Vec<PendingSectionWrite> {
    let map = lock_map();
    let mut out: Vec<PendingSectionWrite> = map
        .values()
        .filter(|w| w.project_path == project_path && w.node_id == node_id)
        .cloned()
        .collect();
    out.sort_by_key(|w| w.created_at);
    out
}

pub fn list_for_session(session_id: &str) -> Vec<PendingSectionWrite> {
    let map = lock_map();
    let mut out: Vec<PendingSectionWrite> = map
        .values()
        .filter(|w| w.session_id.as_deref() == Some(session_id))
        .cloned()
        .collect();
    out.sort_by_key(|w| w.created_at);
    out
}

pub fn get(write_id: &str) -> Option<PendingSectionWrite> {
    let map = lock_map();
    map.get(write_id).cloned()
}

pub fn remove(write_id: &str) -> Option<PendingSectionWrite> {
    let mut map = lock_map();
    let removed = map.remove(write_id);
    drop(map);
    if let Some(ref w) = removed {
        tracing::info!(write_id, "Pending write removed");
        mark_node_dirty(&w.project_path, &w.node_id);
    }
    removed
}

/// Best-effort: ask the ocean service to re-scan this node so the
/// pending_writes decorator (un)appears on the canvas. Failures are logged
/// and ignored — the canvas will catch up on the rover's periodic refresh.
fn mark_node_dirty(project_path: &str, node_id: &str) {
    let res = crate::ocean::service::with_service(project_path, |svc| {
        svc.mark_dirty(node_id);
    });
    if let Err(e) = res {
        tracing::debug!(
            project_path,
            node_id,
            error = %e,
            "pending_writes: could not mark node dirty (rover will catch up on periodic refresh)"
        );
    }
}

/// Update content of an existing pending in-place (used by regenerate).
/// Preserves identity (write_id, kind, project, node, ai_prompt) so any
/// frontend listener already pointing at this id keeps working.
pub fn replace_content(
    write_id: &str,
    name: String,
    content_markdown: String,
    diff_patch: Option<String>,
    additions: u32,
    deletions: u32,
    ai_model: String,
) -> Option<PendingSectionWrite> {
    let mut map = lock_map();
    let entry = map.get_mut(write_id)?;
    entry.name = name;
    entry.content_markdown = content_markdown;
    entry.diff_patch = diff_patch;
    entry.additions = additions;
    entry.deletions = deletions;
    entry.ai_model = ai_model;
    let snapshot = entry.clone();
    drop(map);
    // Re-mark dirty so the canvas refreshes the count payload after a
    // regenerate (the count itself doesn't change but severity/payload may).
    mark_node_dirty(&snapshot.project_path, &snapshot.node_id);
    Some(snapshot)
}

// =============================================================================
// Diff helper
// =============================================================================

/// Unified diff between two markdown blocks plus add/delete line counts.
/// Output format is compatible with the frontend `parsePatch` and
/// `DiffViewer` reused from the GitHub PR detail view.
pub fn compute_diff_patch(filename: &str, old: &str, new: &str) -> (String, u32, u32) {
    let diff = TextDiff::from_lines(old, new);
    let mut additions = 0u32;
    let mut deletions = 0u32;
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => additions += 1,
            ChangeTag::Delete => deletions += 1,
            ChangeTag::Equal => {}
        }
    }
    let patch = diff
        .unified_diff()
        .context_radius(3)
        .header(filename, filename)
        .to_string();
    (patch, additions, deletions)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make(project: &str, node: &str, kind: PendingKind, name: &str) -> PendingSectionWrite {
        PendingSectionWrite {
            write_id: new_write_id(),
            project_path: project.to_string(),
            node_id: node.to_string(),
            session_id: None,
            kind,
            name: name.to_string(),
            content_markdown: "body".to_string(),
            ai_prompt: "prompt".to_string(),
            ai_model: "test-model".to_string(),
            diff_patch: None,
            additions: 0,
            deletions: 0,
            created_at: 0,
        }
    }

    #[test]
    fn compute_diff_patch_counts_changes() {
        let (patch, adds, dels) = compute_diff_patch("section.md", "alpha\nbeta\n", "alpha\ngamma\n");
        assert_eq!(adds, 1);
        assert_eq!(dels, 1);
        assert!(patch.contains("section.md"));
        assert!(patch.contains("-beta"));
        assert!(patch.contains("+gamma"));
    }

    #[test]
    fn create_with_same_name_replaces_prior() {
        let project = format!("/tmp/test-{}", new_write_id());
        let id1 = insert(make(&project, "node-A", PendingKind::Create, "Resumen"));
        let id2 = insert(make(&project, "node-A", PendingKind::Create, "Resumen"));
        assert_ne!(id1, id2);
        let listed = list_for_node(&project, "node-A");
        assert_eq!(listed.len(), 1, "create with same name should replace prior");
        assert!(get(&id1).is_none());
        assert!(get(&id2).is_some());
    }

    #[test]
    fn create_with_different_names_coexist() {
        let project = format!("/tmp/test-{}", new_write_id());
        insert(make(&project, "node-A", PendingKind::Create, "Resumen"));
        insert(make(&project, "node-A", PendingKind::Create, "Glosario"));
        let listed = list_for_node(&project, "node-A");
        assert_eq!(listed.len(), 2, "different create names should coexist");
    }

    #[test]
    fn edit_on_same_section_replaces_prior() {
        let project = format!("/tmp/test-{}", new_write_id());
        let edit_kind = || PendingKind::Edit {
            section_id: "sec-1".to_string(),
            baseline_name: "Old".to_string(),
            baseline_content: "old body".to_string(),
        };
        let id1 = insert(make(&project, "node-A", edit_kind(), "v1"));
        let id2 = insert(make(&project, "node-A", edit_kind(), "v2"));
        let listed = list_for_node(&project, "node-A");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "v2");
        assert!(get(&id1).is_none());
        assert!(get(&id2).is_some());
    }

    #[test]
    fn list_for_node_filters_by_project_and_node() {
        let project_a = format!("/tmp/test-{}", new_write_id());
        let project_b = format!("/tmp/test-{}", new_write_id());
        insert(make(&project_a, "n1", PendingKind::Create, "a1"));
        insert(make(&project_a, "n2", PendingKind::Create, "a2"));
        insert(make(&project_b, "n1", PendingKind::Create, "b1"));
        let only_a_n1 = list_for_node(&project_a, "n1");
        assert_eq!(only_a_n1.len(), 1);
        assert_eq!(only_a_n1[0].name, "a1");
    }

    #[test]
    fn replace_content_updates_in_place() {
        let project = format!("/tmp/test-{}", new_write_id());
        let id = insert(make(&project, "node-X", PendingKind::Create, "Notes"));
        let updated = replace_content(
            &id,
            "Notes".to_string(),
            "new body".to_string(),
            None,
            0,
            0,
            "model-v2".to_string(),
        );
        assert!(updated.is_some());
        let got = get(&id).expect("still present");
        assert_eq!(got.content_markdown, "new body");
        assert_eq!(got.ai_model, "model-v2");
    }
}
