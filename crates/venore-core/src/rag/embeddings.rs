//! Embedding utilities for hybrid search
//!
//! Provides blob serialization for f32 vectors, cosine similarity,
//! result merging (FTS + embedding), and embedding provider factory.

use crate::error::{Result, VenoreError};
use crate::rag::types::SearchResult;
use crate::traits::EmbeddingProvider;

// ============================================================================
// BLOB SERIALIZATION (f32 ↔ bytes for SQLite BLOB storage)
// ============================================================================

/// Pack a Vec<f32> into little-endian bytes for SQLite BLOB storage.
pub fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
    let mut blob = Vec::with_capacity(embedding.len() * 4);
    for &val in embedding {
        blob.extend_from_slice(&val.to_le_bytes());
    }
    blob
}

/// Unpack little-endian bytes back into Vec<f32>.
pub fn blob_to_embedding(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| {
            let bytes: [u8; 4] = chunk.try_into().unwrap();
            f32::from_le_bytes(bytes)
        })
        .collect()
}

// ============================================================================
// COSINE SIMILARITY
// ============================================================================

/// Compute cosine similarity between two vectors.
/// Returns a value in [-1, 1] where 1 = identical direction.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let mut dot = 0.0f64;
    let mut norm_a = 0.0f64;
    let mut norm_b = 0.0f64;

    for i in 0..a.len() {
        let va = a[i] as f64;
        let vb = b[i] as f64;
        dot += va * vb;
        norm_a += va * va;
        norm_b += vb * vb;
    }

    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom < 1e-10 {
        return 0.0;
    }

    dot / denom
}

// ============================================================================
// RESULT MERGING (FTS + Embedding → Hybrid)
// ============================================================================

/// Merge FTS5 results and embedding results into a single ranked list.
///
/// - FTS scores are BM25 (negative, lower = better) → normalized to [0,1]
/// - Embedding scores are cosine similarity [0,1] (already normalized)
/// - Union by chunk_id, weighted: FTS weight + embedding weight
/// - Deduped, sorted descending by combined score
pub fn merge_results(
    fts_results: &[SearchResult],
    embedding_results: &[SearchResult],
    fts_weight: f64,
    embedding_weight: f64,
) -> Vec<SearchResult> {
    use std::collections::HashMap;

    // Normalize FTS BM25 scores to [0,1] — BM25 rank is negative, lower = better
    let fts_normalized: Vec<(String, f64)> = if fts_results.is_empty() {
        Vec::new()
    } else {
        // Find min/max of raw BM25 scores (they're negative)
        let min_score = fts_results.iter().map(|r| r.score).fold(f64::INFINITY, f64::min);
        let max_score = fts_results.iter().map(|r| r.score).fold(f64::NEG_INFINITY, f64::max);
        let range = max_score - min_score;

        fts_results.iter().map(|r| {
            let norm = if range.abs() < 1e-10 {
                1.0 // All same score → all equally good
            } else {
                // Invert because lower BM25 rank = better match
                1.0 - (r.score - min_score) / range
            };
            (r.chunk.id.clone(), norm)
        }).collect()
    };

    // Embedding scores are already cosine similarity [0,1]
    let emb_normalized: Vec<(String, f64)> = embedding_results.iter()
        .map(|r| (r.chunk.id.clone(), r.score))
        .collect();

    // Build combined score map
    let mut score_map: HashMap<String, f64> = HashMap::new();
    let mut chunk_map: HashMap<String, SearchResult> = HashMap::new();

    for (id, score) in &fts_normalized {
        *score_map.entry(id.clone()).or_insert(0.0) += score * fts_weight;
    }
    for r in fts_results {
        chunk_map.entry(r.chunk.id.clone()).or_insert_with(|| r.clone());
    }

    for (id, score) in &emb_normalized {
        *score_map.entry(id.clone()).or_insert(0.0) += score * embedding_weight;
    }
    for r in embedding_results {
        chunk_map.entry(r.chunk.id.clone()).or_insert_with(|| r.clone());
    }

    // Build final results sorted by combined score (descending)
    let mut results: Vec<SearchResult> = score_map.into_iter()
        .filter_map(|(id, score)| {
            chunk_map.remove(&id).map(|mut r| {
                r.score = score;
                r.search_method = "hybrid".to_string();
                r
            })
        })
        .collect();

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results
}

// ============================================================================
// FACTORY
// ============================================================================

/// Create an embedding provider from provider name and optional model override.
pub fn create_embedding_provider(
    provider: &str,
    model: Option<&str>,
) -> Result<Box<dyn EmbeddingProvider>> {
    match provider.to_lowercase().as_str() {
        "openai" => {
            let m = model.unwrap_or("text-embedding-3-small").to_string();
            Ok(Box::new(super::super::llm::providers::embedding_openai::OpenAIEmbeddingProvider::new(m)))
        }
        "gemini" => {
            let m = model.unwrap_or("text-embedding-004").to_string();
            Ok(Box::new(super::super::llm::providers::embedding_gemini::GeminiEmbeddingProvider::new(m)))
        }
        "ollama" => {
            let m = model.unwrap_or("nomic-embed-text").to_string();
            Ok(Box::new(super::super::llm::providers::embedding_ollama::OllamaEmbeddingProvider::new(m)))
        }
        _ => Err(VenoreError::RagEmbeddingError(format!(
            "Unknown embedding provider: '{}'. Supported: openai, gemini, ollama", provider
        ))),
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rag::types::RagChunk;

    #[test]
    fn test_blob_round_trip() {
        let original = vec![1.0f32, -0.5, 0.0, 3.14, -2.71828];
        let blob = embedding_to_blob(&original);
        let restored = blob_to_embedding(&blob);
        assert_eq!(original.len(), restored.len());
        for (a, b) in original.iter().zip(restored.iter()) {
            assert!((a - b).abs() < 1e-7);
        }
    }

    #[test]
    fn test_blob_empty() {
        let empty: Vec<f32> = Vec::new();
        let blob = embedding_to_blob(&empty);
        assert!(blob.is_empty());
        let restored = blob_to_embedding(&blob);
        assert!(restored.is_empty());
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![-1.0, -2.0, -3.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim + 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_different_lengths() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_cosine_similarity_empty() {
        let a: Vec<f32> = Vec::new();
        let b: Vec<f32> = Vec::new();
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    fn make_chunk(id: &str, name: &str) -> RagChunk {
        RagChunk {
            id: id.to_string(),
            file_id: "f1".to_string(),
            project_id: "proj1".to_string(),
            chunk_type: "function".to_string(),
            name: name.to_string(),
            content: format!("function {}() {{}}", name),
            line_start: 1,
            line_end: 3,
            relative_path: "src/test.ts".to_string(),
            metadata: None,
        }
    }

    #[test]
    fn test_merge_results_fts_only() {
        let fts = vec![
            SearchResult { chunk: make_chunk("c1", "auth"), score: -5.0, search_method: "fts".to_string() },
            SearchResult { chunk: make_chunk("c2", "login"), score: -3.0, search_method: "fts".to_string() },
        ];
        let emb: Vec<SearchResult> = Vec::new();

        let merged = merge_results(&fts, &emb, 0.4, 0.6);
        assert_eq!(merged.len(), 2);
        // c1 has lower (better) BM25 score, should rank higher
        assert_eq!(merged[0].chunk.id, "c1");
    }

    #[test]
    fn test_merge_results_embedding_only() {
        let fts: Vec<SearchResult> = Vec::new();
        let emb = vec![
            SearchResult { chunk: make_chunk("c1", "auth"), score: 0.9, search_method: "embedding".to_string() },
            SearchResult { chunk: make_chunk("c2", "login"), score: 0.7, search_method: "embedding".to_string() },
        ];

        let merged = merge_results(&fts, &emb, 0.4, 0.6);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].chunk.id, "c1");
    }

    #[test]
    fn test_merge_results_dedup() {
        // Same chunk appears in both FTS and embedding results
        let fts = vec![
            SearchResult { chunk: make_chunk("c1", "auth"), score: -5.0, search_method: "fts".to_string() },
        ];
        let emb = vec![
            SearchResult { chunk: make_chunk("c1", "auth"), score: 0.9, search_method: "embedding".to_string() },
            SearchResult { chunk: make_chunk("c2", "login"), score: 0.7, search_method: "embedding".to_string() },
        ];

        let merged = merge_results(&fts, &emb, 0.4, 0.6);
        assert_eq!(merged.len(), 2); // c1 deduped
        // c1 should be top because it has score from both sources
        assert_eq!(merged[0].chunk.id, "c1");
    }

    #[test]
    fn test_merge_results_empty() {
        let fts: Vec<SearchResult> = Vec::new();
        let emb: Vec<SearchResult> = Vec::new();
        let merged = merge_results(&fts, &emb, 0.4, 0.6);
        assert!(merged.is_empty());
    }

    #[test]
    fn test_create_embedding_provider_invalid() {
        let result = create_embedding_provider("anthropic", None);
        assert!(result.is_err());
    }
}
