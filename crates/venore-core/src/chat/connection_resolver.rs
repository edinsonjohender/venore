//! Connection resolver — turns the desktop AI-connection registry's typed
//! targets into markdown context blocks ready for system-prompt injection.
//!
//! Each kind has its own resolver path so the source of truth stays
//! consistent: knowledge nodes come from `ocean::service`, hexagons from
//! `KnowledgeRepository`, code modules from disk (`.context.md`). The
//! resolver runs once per chat send, so every turn gets a fresh snapshot
//! — if the user/AI just edited a connected node, the next message sees
//! the updated content without the user having to reconnect anything.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::knowledge::KnowledgeRepository;
use crate::ocean::types::NodeVariant;

// =============================================================================
// Public types
// =============================================================================

/// Mirror of `venore_desktop::ai_connections::AiConnectionTarget`. Lives in
/// core so the resolver can be unit-tested in isolation and so other
/// non-desktop callers (eval harness, mesh) can reuse it eventually.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConnectionTarget {
    KnowledgeNode {
        project_path: String,
        node_id: String,
        /// Display name cached at register time so the chat-input badges
        /// render the entity name even when the source panel is in a
        /// pop-out window (and therefore not in the in-app panels list).
        /// The resolver itself prefers the layout's canonical name.
        #[serde(default)]
        display_name: String,
    },
    CodeModule {
        project_path: String,
        module_name: String,
        module_path: String,
    },
    Hexagon {
        project_path: String,
        feature_id: String,
        hexagon_id: String,
        #[serde(default)]
        display_name: String,
    },
}

/// One resolved attachment ready to be appended to the system prompt.
/// `header` is a short label ("Auth (faro)") used by the renderer to
/// section the block; `body_markdown` carries the actual content.
#[derive(Debug, Clone)]
pub struct ConnectionBlock {
    pub header: String,
    pub body_markdown: String,
}

/// Per-target outcome. Distinguishes "couldn't resolve right now, retry
/// next turn" (Transient) from "this entity is gone for good, drop the
/// connection" (Stale). The desktop layer uses `stale_ids` to evict
/// dead entries from the registry.
enum Outcome {
    Resolved(ConnectionBlock),
    Stale,
    Transient,
}

/// Result of resolving a batch of connections.
/// - `blocks` is what gets injected into the system prompt.
/// - `stale_ids` is the subset of input ids whose target was definitively
///   not found (deleted node, missing hex). The caller should evict
///   them from its registry so we don't re-resolve them every turn.
#[derive(Debug, Clone, Default)]
pub struct ResolveResult {
    pub blocks: Vec<ConnectionBlock>,
    pub stale_ids: Vec<String>,
}

// =============================================================================
// Public entry point
// =============================================================================

/// Resolve every active connection to a markdown block.
///
/// `inputs` carries `(connection_id, target)` pairs so the caller can
/// match the returned `stale_ids` back to its registry without having to
/// re-derive the id from a target. `knowledge_repo` is optional because
/// not every caller has a live repo handle (CLI eval harness, tests).
pub async fn resolve_connections(
    inputs: &[(String, ConnectionTarget)],
    knowledge_repo: Option<&Arc<KnowledgeRepository>>,
) -> ResolveResult {
    let mut result = ResolveResult {
        blocks: Vec::with_capacity(inputs.len()),
        stale_ids: Vec::new(),
    };
    for (id, target) in inputs {
        let outcome = match target {
            ConnectionTarget::KnowledgeNode { project_path, node_id, .. } => {
                resolve_knowledge_node(project_path, node_id)
            }
            ConnectionTarget::CodeModule {
                project_path: _,
                module_name,
                module_path,
            } => resolve_code_module(module_name, module_path),
            ConnectionTarget::Hexagon {
                project_path: _,
                feature_id,
                hexagon_id,
                ..
            } => match knowledge_repo {
                Some(repo) => resolve_hexagon(repo.as_ref(), feature_id, hexagon_id).await,
                None => {
                    tracing::debug!(
                        feature_id,
                        hexagon_id,
                        "connection_resolver: skipping hexagon, no knowledge_repo provided"
                    );
                    Outcome::Transient
                }
            },
        };
        match outcome {
            Outcome::Resolved(block) => result.blocks.push(block),
            Outcome::Stale => result.stale_ids.push(id.clone()),
            Outcome::Transient => {}
        }
    }
    result
}

// =============================================================================
// Per-kind resolvers
// =============================================================================

fn resolve_knowledge_node(project_path: &str, node_id: &str) -> Outcome {
    let outcome = crate::ocean::service::with_service(project_path, |svc| {
        let layout = svc.get_layout();
        let entry = layout.positions.get(node_id).cloned();
        let data = svc.get_knowledge_data(node_id);
        // Capture lighthouse name (if any) and any manual connections involving
        // this node so the AI sees the relational context too.
        let lighthouse_name = entry.as_ref().and_then(|e| e.lighthouse_id.as_ref()).and_then(
            |lh_id| layout.positions.get(lh_id).map(|lh| lh.module_name.clone()),
        );
        let manual_connections: Vec<(String, String, &'static str)> = layout
            .manual_connections
            .iter()
            .filter_map(|c| {
                if c.from_id == node_id {
                    let to_name = layout
                        .positions
                        .get(&c.to_id)
                        .map(|n| n.module_name.clone())
                        .unwrap_or_else(|| c.to_id.clone());
                    Some((to_name, c.to_id.clone(), "→"))
                } else if c.to_id == node_id {
                    let from_name = layout
                        .positions
                        .get(&c.from_id)
                        .map(|n| n.module_name.clone())
                        .unwrap_or_else(|| c.from_id.clone());
                    Some((from_name, c.from_id.clone(), "←"))
                } else {
                    None
                }
            })
            .collect();
        (entry, data, lighthouse_name, manual_connections)
    });

    let (entry, data, lighthouse_name, manual_connections) = match outcome {
        Ok(t) => t,
        Err(e) => {
            // Lock unavailable / project still loading — could succeed
            // next turn, don't evict.
            tracing::warn!(
                project_path,
                node_id,
                error = %e,
                "connection_resolver: ocean service unavailable for KnowledgeNode"
            );
            return Outcome::Transient;
        }
    };

    let entry = match entry {
        Some(e) => e,
        None => {
            // Layout has no such node — definitively gone (deleted).
            tracing::warn!(
                project_path,
                node_id,
                "connection_resolver: connected KnowledgeNode no longer exists, evicting"
            );
            return Outcome::Stale;
        }
    };

    let variant_label = match entry.node_variant {
        NodeVariant::Lighthouse => "lighthouse",
        NodeVariant::KnowledgeNode => "node",
        NodeVariant::Module | NodeVariant::Buoy | NodeVariant::Cylinder => {
            // The id was reused for a code variant — the original
            // knowledge node it referred to is gone for our purposes.
            tracing::warn!(
                project_path,
                node_id,
                variant = ?entry.node_variant,
                "connection_resolver: KnowledgeNode target points at non-knowledge variant, evicting"
            );
            return Outcome::Stale;
        }
    };

    let header = format!("{} ({})", entry.module_name, variant_label);

    let mut body = String::new();
    body.push_str(&format!("**id**: `{}` · **variant**: {}\n", node_id, variant_label));
    if let Some(ref isla) = lighthouse_name {
        body.push_str(&format!("**island**: {}\n", isla));
    }
    if let Some(ref d) = data {
        body.push_str(&format!("**subtype**: {:?}\n", d.subtype));
    }
    if !manual_connections.is_empty() {
        body.push_str("**connections**:\n");
        for (other_name, other_id, arrow) in &manual_connections {
            body.push_str(&format!("- {} `{}` ({})\n", arrow, other_id, other_name));
        }
    }

    body.push('\n');
    if let Some(d) = data {
        if d.sections.is_empty() {
            body.push_str("_(no sections)_\n");
        } else {
            body.push_str("**Sections:**\n\n");
            for (i, sec) in d.sections.iter().enumerate() {
                let source_label = match &sec.source {
                    crate::ocean::types::SourceAttribution::User => "user".to_string(),
                    crate::ocean::types::SourceAttribution::Ai { model, .. } => {
                        format!("ai · {}", model)
                    }
                };
                body.push_str(&format!(
                    "### {}. {} [id: `{}` · {}]\n{}\n\n",
                    i + 1,
                    sec.name,
                    sec.id,
                    source_label,
                    if sec.content_markdown.trim().is_empty() {
                        "_(empty)_"
                    } else {
                        &sec.content_markdown
                    },
                ));
            }
        }
    } else {
        body.push_str("_(no content)_\n");
    }

    Outcome::Resolved(ConnectionBlock {
        header,
        body_markdown: body,
    })
}

fn resolve_code_module(module_name: &str, module_path: &str) -> Outcome {
    if module_path.is_empty() {
        // Empty path is suspicious (probably a knowledge node mis-classified
        // as code on the frontend) but doesn't prove the entity is gone.
        // Treat as transient — operator can fix or it might come back.
        tracing::debug!(
            module_name,
            "connection_resolver: code module has empty path, skipping"
        );
        return Outcome::Transient;
    }
    let context_file = PathBuf::from(module_path).join(".context.md");
    match std::fs::read_to_string(&context_file) {
        Ok(content) => Outcome::Resolved(ConnectionBlock {
            header: format!("{} (module)", module_name),
            body_markdown: content,
        }),
        Err(e) => {
            // The file might be temporarily missing (branch switch, ongoing
            // edit, etc.). Don't evict — the user explicitly attached this
            // module and we should retry next turn.
            tracing::warn!(
                module_name,
                module_path,
                error = %e,
                "connection_resolver: missing .context.md for code module, skipping"
            );
            Outcome::Transient
        }
    }
}

async fn resolve_hexagon(
    repo: &KnowledgeRepository,
    feature_id: &str,
    hexagon_id: &str,
) -> Outcome {
    let hex = match repo.get_hexagon(hexagon_id).await {
        Ok(Some(h)) => h,
        Ok(None) => {
            // Definitively gone — DB returned no row.
            tracing::warn!(
                feature_id,
                hexagon_id,
                "connection_resolver: connected hexagon not found, evicting"
            );
            return Outcome::Stale;
        }
        Err(e) => {
            // DB error — could recover next turn, don't evict.
            tracing::warn!(
                feature_id,
                hexagon_id,
                error = %e,
                "connection_resolver: failed to load hexagon, skipping"
            );
            return Outcome::Transient;
        }
    };

    let evidence = repo
        .list_evidence_by_hexagon(hexagon_id)
        .await
        .unwrap_or_default();

    let header = format!("{} (hex)", hex.title);

    let mut body = String::new();
    body.push_str(&format!(
        "**id**: `{}` · **feature_id**: `{}` · **phase**: {} · **{}%** · confidence: {} · risk: {}\n\n",
        hex.id, hex.feature_id, hex.phase, hex.percentage, hex.confidence, hex.risk,
    ));
    if !hex.description.trim().is_empty() {
        body.push_str(&format!("{}\n\n", hex.description));
    }
    if !hex.notes_user.trim().is_empty() {
        body.push_str(&format!("**User notes:**\n{}\n\n", hex.notes_user));
    }
    if !evidence.is_empty() {
        body.push_str(&format!("**Evidence ({}):**\n", evidence.len()));
        for ev in &evidence {
            body.push_str(&format!(
                "- _{}_ [{}]: {}\n",
                ev.source_type,
                ev.confidence,
                ev.content.lines().next().unwrap_or(""),
            ));
            if !ev.source_url.trim().is_empty() {
                body.push_str(&format!("  ↳ {}\n", ev.source_url));
            }
        }
    }

    Outcome::Resolved(ConnectionBlock {
        header,
        body_markdown: body,
    })
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn assert_resolved(outcome: Outcome) -> ConnectionBlock {
        match outcome {
            Outcome::Resolved(b) => b,
            other => panic!("expected Resolved, got {:?}", outcome_kind(&other)),
        }
    }

    fn outcome_kind(o: &Outcome) -> &'static str {
        match o {
            Outcome::Resolved(_) => "Resolved",
            Outcome::Stale => "Stale",
            Outcome::Transient => "Transient",
        }
    }

    #[test]
    fn code_module_returns_block_when_context_md_exists() {
        let dir = TempDir::new().unwrap();
        let mut f = std::fs::File::create(dir.path().join(".context.md")).unwrap();
        writeln!(f, "# Auth module").unwrap();
        let block = assert_resolved(resolve_code_module("auth", dir.path().to_str().unwrap()));
        assert_eq!(block.header, "auth (module)");
        assert!(block.body_markdown.contains("# Auth module"));
    }

    #[test]
    fn code_module_missing_path_is_transient() {
        // Missing fs path is treated as transient (not stale) — file might
        // be added back, branch might switch back, etc.
        let outcome = resolve_code_module("ghost", "/path/that/does/not/exist");
        assert_eq!(outcome_kind(&outcome), "Transient");
    }

    #[test]
    fn code_module_empty_path_is_transient() {
        let outcome = resolve_code_module("knowledge_node_with_empty_path", "");
        assert_eq!(outcome_kind(&outcome), "Transient");
    }

    #[test]
    fn knowledge_node_unknown_id_is_stale() {
        let dir = TempDir::new().unwrap();
        let project = dir.path().to_string_lossy().to_string();
        let _ = crate::ocean::service::with_service(&project, |_svc| {});
        let outcome = resolve_knowledge_node(&project, "nonexistent-node");
        assert_eq!(outcome_kind(&outcome), "Stale");
    }
}
