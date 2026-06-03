//! AI Connection registry — in-memory, transient.
//!
//! Tracks the visual Sparkles ↔ Sparkles connections shared across all Tauri
//! windows of the desktop process. State is intentionally NOT persisted:
//! connections are session-scoped UI state, not domain data. Lives in
//! `venore-desktop` (not `venore-core`) because it has no business logic and
//! is purely cross-window UI plumbing for the one running app process.
//!
//! Each entry carries a typed `target` so the chat backend can resolve the
//! attached entity into a context block at send time. The `id` (e.g.
//! `node:<uuid>`, `hex:<uuid>`, `module:<path>`) stays as the unique key
//! for the visual layer; kind information lives on `target` so the
//! resolver doesn't have to parse strings.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// Typed payload telling the chat backend WHICH entity is attached.
/// Mirror of `venore_core::chat::connection_resolver::ConnectionTarget`,
/// kept here so the desktop crate doesn't have to depend on core types
/// for UI plumbing — translation happens at the chat send site.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AiConnectionTarget {
    /// A knowledge node or lighthouse — both share the same data model
    /// (`KnowledgeNodeData` keyed by node_id inside the project's ocean
    /// layout).
    KnowledgeNode {
        project_path: String,
        node_id: String,
        /// Human-readable name captured at register time. Lives on the
        /// target so chips/badges can render the name even when the
        /// source panel isn't mounted (e.g. popped-out window). The
        /// resolver still re-fetches the canonical name from the layout
        /// each turn, so this is purely a display cache.
        display_name: String,
    },
    /// A code module from the codebase mode — resolves through the
    /// existing `.context.md` machinery.
    CodeModule {
        project_path: String,
        module_name: String,
        module_path: String,
    },
    /// A research hexagon inside a knowledge feature.
    Hexagon {
        project_path: String,
        feature_id: String,
        hexagon_id: String,
        /// Hexagon title cached for chips/badges (same rationale as
        /// KnowledgeNode.display_name).
        display_name: String,
    },
}

#[derive(Debug, Clone)]
pub struct AiConnectionRecord {
    pub id: String,
    pub active: bool,
    pub target: AiConnectionTarget,
    /// Tauri window label that owns the source endpoint (e.g. "main", "node:abc").
    /// Empty string means "unknown / legacy register without label".
    pub window_label: String,
}

pub struct AiConnectionRegistry {
    inner: Mutex<HashMap<String, AiConnectionRecord>>,
}

impl AiConnectionRegistry {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    pub fn snapshot(&self) -> Vec<AiConnectionRecord> {
        let map = self.inner.lock().unwrap();
        map.values().cloned().collect()
    }

    /// Snapshot of only the connections whose `active = true`. Useful for
    /// the chat send site, which only needs to resolve attached entities.
    pub fn snapshot_active(&self) -> Vec<AiConnectionRecord> {
        let map = self.inner.lock().unwrap();
        map.values().filter(|r| r.active).cloned().collect()
    }

    /// Insert or update target/window label. If the entry already exists,
    /// its `active` flag is preserved (re-registering doesn't surprise-
    /// disconnect the user) but `target` and `window_label` are refreshed
    /// in case the entity moved between windows or got renamed.
    pub fn register(&self, id: &str, target: AiConnectionTarget, window_label: &str) {
        let mut map = self.inner.lock().unwrap();
        match map.get_mut(id) {
            Some(existing) => {
                existing.target = target;
                existing.window_label = window_label.to_string();
            }
            None => {
                map.insert(
                    id.to_string(),
                    AiConnectionRecord {
                        id: id.to_string(),
                        active: false,
                        target,
                        window_label: window_label.to_string(),
                    },
                );
            }
        }
    }

    pub fn unregister(&self, id: &str) {
        let mut map = self.inner.lock().unwrap();
        map.remove(id);
    }

    /// Returns the new `active` value, or None if the id was unknown.
    pub fn toggle(&self, id: &str) -> Option<bool> {
        let mut map = self.inner.lock().unwrap();
        let entry = map.get_mut(id)?;
        entry.active = !entry.active;
        Some(entry.active)
    }

    pub fn disconnect_all(&self) {
        let mut map = self.inner.lock().unwrap();
        for entry in map.values_mut() {
            entry.active = false;
        }
    }
}

impl Default for AiConnectionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn knowledge_target(node: &str) -> AiConnectionTarget {
        AiConnectionTarget::KnowledgeNode {
            project_path: "/tmp/proj".to_string(),
            node_id: node.to_string(),
            display_name: format!("display-{}", node),
        }
    }

    #[test]
    fn register_inserts_with_active_false() {
        let reg = AiConnectionRegistry::new();
        reg.register("node:a", knowledge_target("a"), "main");
        let snap = reg.snapshot();
        assert_eq!(snap.len(), 1);
        assert!(!snap[0].active);
        assert_eq!(snap[0].id, "node:a");
        assert_eq!(snap[0].window_label, "main");
    }

    #[test]
    fn register_again_preserves_active_refreshes_target() {
        let reg = AiConnectionRegistry::new();
        reg.register("node:a", knowledge_target("a"), "main");
        reg.toggle("node:a");
        reg.register("node:a", knowledge_target("a"), "node-xyz");
        let snap = reg.snapshot();
        assert_eq!(snap.len(), 1);
        assert!(snap[0].active, "active flag should survive re-register");
        assert_eq!(snap[0].window_label, "node-xyz");
    }

    #[test]
    fn snapshot_active_filters_inactive() {
        let reg = AiConnectionRegistry::new();
        reg.register("a", knowledge_target("a"), "main");
        reg.register("b", knowledge_target("b"), "main");
        reg.toggle("a");
        let only_active = reg.snapshot_active();
        assert_eq!(only_active.len(), 1);
        assert_eq!(only_active[0].id, "a");
    }

    #[test]
    fn toggle_unknown_returns_none() {
        let reg = AiConnectionRegistry::new();
        assert_eq!(reg.toggle("missing"), None);
    }

    #[test]
    fn disconnect_all_keeps_entries_but_flips_active() {
        let reg = AiConnectionRegistry::new();
        reg.register("a", knowledge_target("a"), "main");
        reg.register("b", knowledge_target("b"), "main");
        reg.toggle("a");
        reg.toggle("b");
        reg.disconnect_all();
        let snap = reg.snapshot();
        assert_eq!(snap.len(), 2);
        assert!(snap.iter().all(|r| !r.active));
    }
}
