//! Logbook indexing — turn knowledge-node sections into searchable chunks.
//!
//! Pure, in-memory-testable functions used by the Index Current (via the
//! desktop bridge). The Current detects *which* node to (re)index; this module
//! does the cheap diff-by-hash (`index_logbook_node`) and the slow embedding
//! pass (`embed_logbook_chunks`) is run separately so it never blocks the
//! Current's visual tick.
//!
//! `ocean` never imports `rag`; the coupling lives in the desktop command
//! layer, which already holds both the ocean service and the `LogbookRepository`.

use crate::ocean::types::{NodeSection, SourceAttribution};
use crate::rag::indexer::compute_hash;
use crate::rag::logbook_repository::{LogbookChunk, LogbookRepository};
use crate::traits::EmbeddingProvider;
use crate::Result;

/// Compute the change-detection hash for a section.
///
/// Folds in the name so a rename also re-embeds. Mirrors `indexer::compute_hash`
/// (SHA-256) but over `name + "\0" + content`.
fn section_hash(name: &str, content: &str) -> String {
    compute_hash(&format!("{}\0{}", name, content))
}

/// Map a section's source attribution to a compact string for storage.
fn source_str(source: &SourceAttribution) -> String {
    match source {
        SourceAttribution::User => "user".to_string(),
        SourceAttribution::Ai { .. } => "ai".to_string(),
    }
}

/// Reindex one knowledge node's sections into the logbook index.
///
/// Idempotent and incremental:
/// - unchanged sections (hash match) are skipped — no DB write,
/// - new/edited sections are upserted (stale embedding dropped by the repo),
/// - sections that disappeared from the node are deleted from the index.
///
/// Returns `(upserted, deleted)` counts.
pub async fn index_logbook_node(
    repo: &LogbookRepository,
    project_id: &str,
    node_id: &str,
    sections: &[NodeSection],
) -> Result<(u32, u32)> {
    let mut upserted = 0u32;
    let mut deleted = 0u32;

    // Upsert changed/new sections.
    for section in sections {
        let hash = section_hash(&section.name, &section.content_markdown);
        let prev = repo.get_section_hash(project_id, &section.id).await?;
        if prev.as_deref() == Some(hash.as_str()) {
            continue; // unchanged — cheap skip
        }

        repo.upsert_chunk(&LogbookChunk {
            id: section.id.clone(),
            project_id: project_id.to_string(),
            node_id: node_id.to_string(),
            name: section.name.clone(),
            content: section.content_markdown.clone(),
            content_hash: hash,
            source: source_str(&section.source),
            updated_at: section.updated_at,
        }).await?;
        upserted += 1;
    }

    // Delete sections that no longer exist on the node.
    let current_ids: std::collections::HashSet<&str> =
        sections.iter().map(|s| s.id.as_str()).collect();
    let indexed_ids = repo.get_node_section_ids(project_id, node_id).await?;
    for indexed_id in indexed_ids {
        if !current_ids.contains(indexed_id.as_str()) {
            repo.delete_chunk(&indexed_id).await?;
            deleted += 1;
        }
    }

    if upserted > 0 || deleted > 0 {
        tracing::debug!(
            "Logbook node {} reindexed: {} upserted, {} deleted",
            node_id, upserted, deleted
        );
    }

    Ok((upserted, deleted))
}

/// Remove all logbook chunks for a node (e.g. when the node is deleted).
pub async fn remove_logbook_node(
    repo: &LogbookRepository,
    project_id: &str,
    node_id: &str,
) -> Result<()> {
    repo.delete_node(project_id, node_id).await
}

/// Embed logbook chunks that don't have an embedding yet (batched).
///
/// Mirrors `indexer::embed_chunks` against the logbook tables. Failures are
/// propagated; callers run this off the Current's tick and degrade gracefully
/// (FTS-only) when no embedding provider/key is configured.
pub async fn embed_logbook_chunks(
    repo: &LogbookRepository,
    project_id: &str,
    provider: &dyn EmbeddingProvider,
    api_key: &str,
) -> Result<()> {
    use crate::rag::embeddings::embedding_to_blob;

    let model = provider.model();
    let dims = provider.dimensions();
    const BATCH_SIZE: u32 = 100;

    loop {
        let pending = repo.get_chunks_without_embeddings(project_id, model, BATCH_SIZE).await?;
        if pending.is_empty() {
            break;
        }

        let chunk_ids: Vec<String> = pending.iter().map(|(id, _)| id.clone()).collect();
        let texts: Vec<String> = pending.into_iter().map(|(_, content)| content).collect();

        let embeddings = provider.embed_batch(api_key, &texts).await?;

        for (chunk_id, embedding) in chunk_ids.iter().zip(embeddings.iter()) {
            let blob = embedding_to_blob(embedding);
            repo.upsert_embedding(chunk_id, &blob, model, dims).await?;
        }

        tracing::debug!("Embedded {} logbook chunks with {}", chunk_ids.len(), model);

        if (chunk_ids.len() as u32) < BATCH_SIZE {
            break;
        }
    }

    tracing::info!("Logbook embedding complete for project {} using {}", project_id, model);
    Ok(())
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn create_test_repo() -> LogbookRepository {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let repo = LogbookRepository::new(pool);
        repo.initialize().await.unwrap();
        repo
    }

    fn section(id: &str, name: &str, content: &str) -> NodeSection {
        NodeSection {
            id: id.to_string(),
            name: name.to_string(),
            content_markdown: content.to_string(),
            source: SourceAttribution::User,
            created_at: 1,
            updated_at: 2,
            ai_prompt: None,
            ai_model: None,
        }
    }

    #[tokio::test]
    async fn test_index_node_inserts_all_then_skips_unchanged() {
        let repo = create_test_repo().await;
        let secs = vec![
            section("s1", "Auth", "login validates credentials"),
            section("s2", "Store", "persist to disk"),
        ];

        let (up, del) = index_logbook_node(&repo, "p1", "n1", &secs).await.unwrap();
        assert_eq!((up, del), (2, 0));

        // Re-run with identical sections → everything skipped.
        let (up, del) = index_logbook_node(&repo, "p1", "n1", &secs).await.unwrap();
        assert_eq!((up, del), (0, 0));

        // And it's searchable.
        assert_eq!(repo.search("p1", "login", 10).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_index_node_reindexes_edited_section() {
        let repo = create_test_repo().await;
        index_logbook_node(&repo, "p1", "n1", &[section("s1", "T", "about cats")]).await.unwrap();

        let (up, del) = index_logbook_node(&repo, "p1", "n1", &[section("s1", "T", "about dogs")]).await.unwrap();
        assert_eq!((up, del), (1, 0));

        assert!(repo.search("p1", "cats", 10).await.unwrap().is_empty());
        assert_eq!(repo.search("p1", "dogs", 10).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_index_node_reindexes_on_rename() {
        let repo = create_test_repo().await;
        index_logbook_node(&repo, "p1", "n1", &[section("s1", "OldName", "body")]).await.unwrap();
        // Same content, different name → hash differs → upsert.
        let (up, _) = index_logbook_node(&repo, "p1", "n1", &[section("s1", "NewName", "body")]).await.unwrap();
        assert_eq!(up, 1);
        assert_eq!(repo.search("p1", "NewName", 10).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_index_node_deletes_removed_section() {
        let repo = create_test_repo().await;
        index_logbook_node(&repo, "p1", "n1", &[
            section("s1", "A", "alpha"),
            section("s2", "B", "beta"),
        ]).await.unwrap();

        // s2 removed from the node.
        let (up, del) = index_logbook_node(&repo, "p1", "n1", &[section("s1", "A", "alpha")]).await.unwrap();
        assert_eq!((up, del), (0, 1));
        assert!(repo.search("p1", "beta", 10).await.unwrap().is_empty());
        assert_eq!(repo.search("p1", "alpha", 10).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_remove_node() {
        let repo = create_test_repo().await;
        index_logbook_node(&repo, "p1", "n1", &[section("s1", "A", "gamma")]).await.unwrap();
        remove_logbook_node(&repo, "p1", "n1").await.unwrap();
        assert!(repo.search("p1", "gamma", 10).await.unwrap().is_empty());
    }
}
