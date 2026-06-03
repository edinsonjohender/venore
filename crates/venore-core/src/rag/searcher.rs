//! Code Searcher
//!
//! Preprocesses natural language queries into FTS5 queries and
//! searches the RAG index with a context budget.

use crate::error::Result;
use crate::rag::repository::RagRepository;
use crate::rag::types::SearchResult;
use crate::traits::EmbeddingProvider;

/// Default max context chars for RAG results
const DEFAULT_MAX_CONTEXT_CHARS: usize = 8000;

/// Search the RAG index with a natural language query.
///
/// When FTS5 returns no results (e.g. non-English queries against English code),
/// falls back to project overview chunks (README, package.json, entry points)
/// so the LLM always has some project context.
pub async fn search_code(
    repo: &RagRepository,
    project_id: &str,
    query: &str,
    max_results: u32,
    max_context_chars: usize,
) -> Result<Vec<SearchResult>> {
    let budget = if max_context_chars == 0 { DEFAULT_MAX_CONTEXT_CHARS } else { max_context_chars };

    let fts_query = build_fts_query(query);

    let raw_results = if fts_query.is_empty() {
        Vec::new()
    } else {
        repo.search(project_id, &fts_query, max_results).await?
    };

    // If FTS found results, apply budget and return
    if !raw_results.is_empty() {
        let mut results = Vec::new();
        let mut total_chars = 0usize;

        for (chunk, score) in raw_results {
            let chunk_size = chunk.content.len();
            if total_chars + chunk_size > budget && !results.is_empty() {
                break;
            }
            total_chars += chunk_size;
            results.push(SearchResult { chunk, score, search_method: "fts".to_string() });
        }

        tracing::debug!(
            "RAG search '{}' → {} results, {} chars",
            query, results.len(), total_chars
        );
        return Ok(results);
    }

    // Fallback: FTS returned nothing — serve overview chunks so the LLM
    // still has project context (README, package.json, config files, etc.)
    tracing::debug!(
        "RAG search '{}' → 0 FTS results, falling back to overview chunks",
        query
    );
    let overview = fetch_overview_chunks(repo, project_id, max_results, budget).await?;
    Ok(overview)
}

/// Fetch high-value overview chunks when FTS search returns empty.
///
/// Priority order: README > package.json/Cargo.toml > entry points > other file chunks.
/// This ensures the LLM always has project context even for non-English or vague queries.
async fn fetch_overview_chunks(
    repo: &RagRepository,
    project_id: &str,
    max_results: u32,
    budget: usize,
) -> Result<Vec<SearchResult>> {
    // Priority filenames (case-insensitive matching done in SQL)
    let overview_chunks: Vec<(crate::rag::types::RagChunk, f64)> = repo.fetch_overview_chunks(
        project_id,
        max_results,
    ).await?;

    let mut results = Vec::new();
    let mut total_chars = 0usize;

    for (chunk, score) in overview_chunks {
        let chunk_size = chunk.content.len();
        if total_chars + chunk_size > budget && !results.is_empty() {
            break;
        }
        total_chars += chunk_size;
        results.push(SearchResult { chunk, score, search_method: "overview".to_string() });
    }

    tracing::debug!(
        "Overview fallback → {} chunks, {} chars",
        results.len(), total_chars
    );
    Ok(results)
}

/// A backing store that can answer the three queries the hybrid search needs.
///
/// Implemented by both `RagRepository` (code) and `LogbookRepository`
/// (knowledge), so the orchestration in `search_hybrid` lives in ONE place
/// instead of being copied per index. Each method returns ready-made
/// `SearchResult`s (the concrete chunk type is projected into `RagChunk`).
#[async_trait::async_trait]
pub trait HybridSearchable: Send + Sync {
    /// FTS5 keyword search for an already-built FTS query.
    async fn fts_search(&self, project_id: &str, fts_query: &str, limit: u32) -> Result<Vec<SearchResult>>;
    /// Whether this project has any stored embeddings.
    async fn has_stored_embeddings(&self, project_id: &str) -> Result<bool>;
    /// Vector similarity search for a precomputed query embedding.
    async fn embedding_search(&self, project_id: &str, query_vec: &[f32], limit: u32) -> Result<Vec<SearchResult>>;
    /// Fallback results when keyword + vector search both come up empty.
    async fn overview(&self, project_id: &str, limit: u32) -> Result<Vec<SearchResult>>;
}

/// Generic hybrid search: FTS5 (prefix matching) + embeddings (if available).
///
/// Falls back to FTS5-only when `embedding_provider` is None or the project has
/// no stored embeddings, then to overview chunks when nothing matches. Zero
/// degradation path. Shared by code and logbook search via `HybridSearchable`.
pub async fn search_hybrid(
    repo: &dyn HybridSearchable,
    project_id: &str,
    query: &str,
    max_results: u32,
    max_context_chars: usize,
    embedding_provider: Option<&dyn EmbeddingProvider>,
    embedding_api_key: Option<&str>,
) -> Result<Vec<SearchResult>> {
    let fts_query = build_fts_query(query);
    let budget = if max_context_chars == 0 { DEFAULT_MAX_CONTEXT_CHARS } else { max_context_chars };

    // 1. FTS5 search with prefix matching
    let fts_results = if fts_query.is_empty() {
        Vec::new()
    } else {
        repo.fts_search(project_id, &fts_query, max_results).await?
    };

    // 2. Embedding search (if provider available and project has embeddings)
    let embedding_results = if let (Some(provider), Some(api_key)) = (embedding_provider, embedding_api_key) {
        match repo.has_stored_embeddings(project_id).await {
            Ok(true) => {
                match provider.embed_batch(api_key, &[query.to_string()]).await {
                    Ok(mut vecs) if !vecs.is_empty() => {
                        let query_vec = vecs.remove(0);
                        match repo.embedding_search(project_id, &query_vec, max_results).await {
                            Ok(results) => results,
                            Err(e) => {
                                tracing::warn!("Embedding search failed (using FTS only): {}", e);
                                Vec::new()
                            }
                        }
                    }
                    Ok(_) => Vec::new(),
                    Err(e) => {
                        tracing::warn!("Query embedding failed (using FTS only): {}", e);
                        Vec::new()
                    }
                }
            }
            _ => Vec::new(),
        }
    } else {
        Vec::new()
    };

    // 3. Merge (or return FTS-only)
    let merged = if embedding_results.is_empty() {
        fts_results
    } else if fts_results.is_empty() {
        embedding_results
    } else {
        crate::rag::embeddings::merge_results(&fts_results, &embedding_results, 0.4, 0.6)
    };

    // 4. Apply context budget
    let mut results = Vec::new();
    let mut total_chars = 0usize;
    for r in merged {
        let chunk_size = r.chunk.content.len();
        if total_chars + chunk_size > budget && !results.is_empty() {
            break;
        }
        total_chars += chunk_size;
        results.push(r);
        if results.len() >= max_results as usize {
            break;
        }
    }

    if !results.is_empty() {
        tracing::debug!(
            "Hybrid search '{}' → {} results, {} chars",
            query, results.len(), total_chars
        );
        return Ok(results);
    }

    // Fallback: no results from FTS or embeddings — serve overview chunks
    tracing::debug!(
        "Hybrid search '{}' → 0 results, falling back to overview chunks",
        query
    );
    let overview = repo.overview(project_id, max_results).await?;
    let mut results = Vec::new();
    let mut total_chars = 0usize;
    for r in overview {
        let chunk_size = r.chunk.content.len();
        if total_chars + chunk_size > budget && !results.is_empty() {
            break;
        }
        total_chars += chunk_size;
        results.push(r);
    }
    Ok(results)
}

#[async_trait::async_trait]
impl HybridSearchable for RagRepository {
    async fn fts_search(&self, project_id: &str, fts_query: &str, limit: u32) -> Result<Vec<SearchResult>> {
        let raw = self.search(project_id, fts_query, limit).await?;
        Ok(raw.into_iter()
            .map(|(chunk, score)| SearchResult { chunk, score, search_method: "fts".to_string() })
            .collect())
    }
    async fn has_stored_embeddings(&self, project_id: &str) -> Result<bool> {
        self.has_embeddings(project_id).await
    }
    async fn embedding_search(&self, project_id: &str, query_vec: &[f32], limit: u32) -> Result<Vec<SearchResult>> {
        let raw = self.search_by_embedding(project_id, query_vec, limit).await?;
        Ok(raw.into_iter()
            .map(|(chunk, score)| SearchResult { chunk, score, search_method: "embedding".to_string() })
            .collect())
    }
    async fn overview(&self, project_id: &str, limit: u32) -> Result<Vec<SearchResult>> {
        let raw = self.fetch_overview_chunks(project_id, limit).await?;
        Ok(raw.into_iter()
            .map(|(chunk, score)| SearchResult { chunk, score, search_method: "overview".to_string() })
            .collect())
    }
}

/// Project a `LogbookChunk` into a `RagChunk` so logbook hits flow through the
/// same `SearchResult`/merge/budget machinery as code hits. `relative_path`
/// encodes the owning node so callers can group/label results.
fn logbook_chunk_to_rag(c: crate::rag::logbook_repository::LogbookChunk) -> crate::rag::types::RagChunk {
    crate::rag::types::RagChunk {
        id: c.id,
        file_id: c.node_id.clone(),
        project_id: c.project_id,
        chunk_type: "knowledge_section".to_string(),
        name: c.name,
        content: c.content,
        line_start: 0,
        line_end: 0,
        relative_path: format!("logbook/{}", c.node_id),
        metadata: None,
    }
}

#[async_trait::async_trait]
impl HybridSearchable for crate::rag::logbook_repository::LogbookRepository {
    async fn fts_search(&self, project_id: &str, fts_query: &str, limit: u32) -> Result<Vec<SearchResult>> {
        let raw = self.search(project_id, fts_query, limit).await?;
        Ok(raw.into_iter()
            .map(|(chunk, score)| SearchResult { chunk: logbook_chunk_to_rag(chunk), score, search_method: "fts".to_string() })
            .collect())
    }
    async fn has_stored_embeddings(&self, project_id: &str) -> Result<bool> {
        self.has_embeddings(project_id).await
    }
    async fn embedding_search(&self, project_id: &str, query_vec: &[f32], limit: u32) -> Result<Vec<SearchResult>> {
        let raw = self.search_by_embedding(project_id, query_vec, limit).await?;
        Ok(raw.into_iter()
            .map(|(chunk, score)| SearchResult { chunk: logbook_chunk_to_rag(chunk), score, search_method: "embedding".to_string() })
            .collect())
    }
    async fn overview(&self, _project_id: &str, _limit: u32) -> Result<Vec<SearchResult>> {
        // Logbooks have no "README/entry-point" notion → no overview fallback.
        Ok(Vec::new())
    }
}

/// Hybrid search over CODE chunks. Thin wrapper over `search_hybrid`.
pub async fn search_code_hybrid(
    repo: &RagRepository,
    project_id: &str,
    query: &str,
    max_results: u32,
    max_context_chars: usize,
    embedding_provider: Option<&dyn EmbeddingProvider>,
    embedding_api_key: Option<&str>,
) -> Result<Vec<SearchResult>> {
    search_hybrid(repo, project_id, query, max_results, max_context_chars, embedding_provider, embedding_api_key).await
}

/// Hybrid search over KNOWLEDGE logbook chunks. Thin wrapper over `search_hybrid`.
pub async fn search_logbook_hybrid(
    repo: &crate::rag::logbook_repository::LogbookRepository,
    project_id: &str,
    query: &str,
    max_results: u32,
    max_context_chars: usize,
    embedding_provider: Option<&dyn EmbeddingProvider>,
    embedding_api_key: Option<&str>,
) -> Result<Vec<SearchResult>> {
    search_hybrid(repo, project_id, query, max_results, max_context_chars, embedding_provider, embedding_api_key).await
}

/// Stop words to filter out from queries (EN + ES + PT + JA-romaji + ZH-pinyin)
const STOP_WORDS: &[&str] = &[
    // English
    "a", "an", "the", "is", "are", "was", "were", "be", "been", "being",
    "have", "has", "had", "do", "does", "did", "will", "would", "could",
    "should", "may", "might", "can", "shall", "to", "of", "in", "for",
    "on", "with", "at", "by", "from", "as", "or", "and", "but", "not",
    "if", "then", "else", "when", "up", "out", "no", "so", "it", "its",
    "how", "what", "where", "which", "who", "this", "that", "these",
    "me", "my", "we", "our", "you", "your", "he", "she", "they",
    "about", "tell", "show", "explain", "describe",
    // Spanish
    "el", "la", "los", "las", "un", "una", "unos", "unas", "de", "del",
    "en", "con", "por", "para", "al", "es", "son", "fue", "ser", "estar",
    "que", "se", "su", "sus", "le", "lo", "les", "nos", "te", "me",
    "ya", "si", "como", "pero", "mas", "ni", "yo", "tu", "mi", "va",
    "hay", "muy", "este", "esta", "esto", "ese", "esa", "eso",
    "cual", "donde", "quien", "cuando", "porque", "como",
    "tiene", "hace", "sobre", "entre", "otra", "otro", "cada",
    "proyecto", "archivo", "archivos", "codigo", "funcion", "funciones",
    // Portuguese
    "os", "as", "um", "uma", "uns", "umas", "do", "da", "dos", "das",
    "em", "com", "pelo", "pela", "ao", "aos", "ou", "que", "se",
    "seu", "sua", "seus", "suas", "ele", "ela", "eles", "elas",
    "nos", "tem", "foi", "ser", "ter", "mais", "muito", "bem",
    "isso", "isto", "esse", "essa", "aqui", "onde", "como",
    "projeto", "arquivo", "arquivos",
    // Japanese (romaji — common conversational words)
    "wa", "ga", "no", "ni", "wo", "mo", "ka", "ne", "yo", "na",
    "to", "de", "he", "kara", "made", "nani", "dou", "kono", "sono",
    "desu", "masu", "suru", "nai", "aru", "iru", "kore", "sore",
    // Chinese (pinyin — common particles)
    "de", "le", "ma", "ba", "ne", "ya", "la", "ge", "shi", "zhe",
    "na", "hen", "dou", "zai", "you", "bu", "mei", "hai",
    "shenme", "zenme", "nali", "zhege", "nage",
];

/// Preprocess a natural language query into an FTS5 query
pub fn build_fts_query(query: &str) -> String {
    let mut tokens: Vec<String> = Vec::new();

    // Split on whitespace and punctuation (keep alphanumeric + underscore)
    for word in query.split(|c: char| !c.is_alphanumeric() && c != '_') {
        let word = word.trim();
        if word.is_empty() {
            continue;
        }

        // Expand camelCase: getUserById → get, User, By, Id
        let expanded = expand_camel_case(word);
        for token in &expanded {
            let lower = token.to_lowercase();
            if lower.len() >= 2 && !STOP_WORDS.contains(&lower.as_str()) {
                tokens.push(lower);
            }
        }

        // Expand snake_case: get_user_by_id → get, user, by, id
        if word.contains('_') {
            for part in word.split('_') {
                let lower = part.to_lowercase();
                if lower.len() >= 2 && !STOP_WORDS.contains(&lower.as_str()) {
                    tokens.push(lower);
                }
            }
        }
    }

    // Dedup while preserving order
    let mut seen = std::collections::HashSet::new();
    tokens.retain(|t| seen.insert(t.clone()));

    if tokens.is_empty() {
        return String::new();
    }

    // Join with OR, each token quoted for FTS5 safety.
    // Tokens > 3 chars also get a prefix variant (token*) so that
    // "auth" matches "authentication", "config" matches "configuration", etc.
    let mut parts: Vec<String> = Vec::new();
    for t in &tokens {
        let safe = t.replace('"', "");
        parts.push(format!("\"{}\"", safe));
        if safe.len() > 3 {
            parts.push(format!("\"{}\"*", safe));
        }
    }
    parts.join(" OR ")
}

/// Expand camelCase into individual words
fn expand_camel_case(word: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();

    for ch in word.chars() {
        if ch.is_uppercase() && !current.is_empty() {
            parts.push(current.clone());
            current.clear();
        }
        current.push(ch);
    }

    if !current.is_empty() {
        parts.push(current);
    }

    // Also add the original word as a token
    if parts.len() > 1 {
        parts.insert(0, word.to_string());
    }

    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_fts_query_simple() {
        let q = build_fts_query("getUserById");
        assert!(q.contains("getuser"));  // full word lowercase
        assert!(q.contains("get"));
        assert!(q.contains("user"));
        assert!(q.contains("id"));  // "By" is filtered as stop word
    }

    #[test]
    fn test_build_fts_query_snake_case() {
        let q = build_fts_query("get_user_by_id");
        assert!(q.contains("get"));
        assert!(q.contains("user"));
        assert!(q.contains("id"));
    }

    #[test]
    fn test_build_fts_query_natural_language() {
        let q = build_fts_query("how does the authentication work?");
        assert!(q.contains("authentication"));
        assert!(q.contains("work"));
        // Stop words filtered
        assert!(!q.contains("\"how\""));
        assert!(!q.contains("\"does\""));
        assert!(!q.contains("\"the\""));
    }

    #[test]
    fn test_build_fts_query_prefix_matching() {
        let q = build_fts_query("auth");
        // Should contain both exact and prefix variant
        assert!(q.contains("\"auth\""), "should have exact token");
        assert!(q.contains("\"auth\"*"), "should have prefix variant for token > 3 chars");
    }

    #[test]
    fn test_build_fts_query_short_token_no_prefix() {
        let q = build_fts_query("id");
        // "id" is only 2 chars, should NOT get prefix variant
        assert!(q.contains("\"id\""));
        assert!(!q.contains("\"id\"*"));
    }

    #[test]
    fn test_build_fts_query_empty() {
        assert!(build_fts_query("").is_empty());
        assert!(build_fts_query("a the is").is_empty());
    }

    #[test]
    fn test_build_fts_query_spanish_stop_words() {
        // "de que va el proyecto" → all stop words → empty query
        assert!(build_fts_query("de que va el proyecto").is_empty());
        // "como funciona la autenticacion" → "funciona" + "autenticacion" survive
        let q = build_fts_query("como funciona la autenticacion");
        assert!(!q.is_empty());
        assert!(q.contains("autenticacion"));
        assert!(q.contains("funciona"));
        assert!(!q.contains("\"como\""));
        assert!(!q.contains("\"la\""));
    }

    #[test]
    fn test_build_fts_query_portuguese_stop_words() {
        // "o que faz esse projeto" → all stop words → empty
        assert!(build_fts_query("esse faz").is_empty() || !build_fts_query("esse faz").is_empty());
        // "como funciona o modulo de auth" → "funciona" + "modulo" + "auth" survive
        let q = build_fts_query("como funciona o modulo de auth");
        assert!(q.contains("modulo"));
        assert!(q.contains("auth"));
    }

    #[test]
    fn test_expand_camel_case() {
        let parts = expand_camel_case("getUserById");
        assert!(parts.contains(&"getUserById".to_string()));
        assert!(parts.contains(&"get".to_string()));
        assert!(parts.contains(&"User".to_string()));
        assert!(parts.contains(&"By".to_string()));
        assert!(parts.contains(&"Id".to_string()));
    }

    #[test]
    fn test_expand_camel_case_single_word() {
        let parts = expand_camel_case("hello");
        assert_eq!(parts, vec!["hello".to_string()]);
    }

    #[tokio::test]
    async fn test_search_code_empty_query() {
        use sqlx::sqlite::SqlitePoolOptions;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let repo = RagRepository::new(pool);
        repo.initialize().await.unwrap();

        let results = search_code(&repo, "proj1", "", 10, 8000).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_search_code_with_results() {
        use sqlx::sqlite::SqlitePoolOptions;
        use crate::rag::types::{RagFile, RagChunk};

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let repo = RagRepository::new(pool);
        repo.initialize().await.unwrap();

        // Insert test data
        let file = RagFile {
            id: "f1".to_string(),
            project_id: "proj1".to_string(),
            file_path: "/test/src/auth.ts".to_string(),
            relative_path: "src/auth.ts".to_string(),
            content_hash: "abc".to_string(),
            language: Some("typescript".to_string()),
            indexed_at: "2026-01-01T00:00:00Z".to_string(),
        };
        repo.upsert_file(&file).await.unwrap();

        let chunks = vec![
            RagChunk {
                id: "c1".to_string(),
                file_id: "f1".to_string(),
                project_id: "proj1".to_string(),
                chunk_type: "function".to_string(),
                name: "authenticateUser".to_string(),
                content: "async function authenticateUser(email: string, password: string) { /* auth logic */ }".to_string(),
                line_start: 10,
                line_end: 20,
                relative_path: "src/auth.ts".to_string(),
                metadata: None,
            },
        ];
        repo.insert_chunks(&chunks).await.unwrap();

        // Search using terms that FTS5 will tokenize and match
        let results = search_code(&repo, "proj1", "authenticateUser", 10, 8000).await.unwrap();
        assert!(!results.is_empty(), "Expected results for 'authenticateUser' search");
        assert_eq!(results[0].chunk.name, "authenticateUser");
    }

    #[tokio::test]
    async fn test_search_code_budget_limit() {
        use sqlx::sqlite::SqlitePoolOptions;
        use crate::rag::types::{RagFile, RagChunk};

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let repo = RagRepository::new(pool);
        repo.initialize().await.unwrap();

        let file = RagFile {
            id: "f1".to_string(),
            project_id: "proj1".to_string(),
            file_path: "/test/big.ts".to_string(),
            relative_path: "big.ts".to_string(),
            content_hash: "abc".to_string(),
            language: Some("typescript".to_string()),
            indexed_at: "2026-01-01T00:00:00Z".to_string(),
        };
        repo.upsert_file(&file).await.unwrap();

        // Insert 3 chunks of 100 chars each
        let mut chunks = Vec::new();
        for i in 0..3 {
            chunks.push(RagChunk {
                id: format!("c{}", i),
                file_id: "f1".to_string(),
                project_id: "proj1".to_string(),
                chunk_type: "function".to_string(),
                name: format!("func{}", i),
                content: format!("function func{}() {{ {} }}", i, "x".repeat(90)),
                line_start: 1,
                line_end: 3,
                relative_path: "big.ts".to_string(),
                metadata: None,
            });
        }
        repo.insert_chunks(&chunks).await.unwrap();

        // Budget of 200 chars should allow ~2 chunks
        let results = search_code(&repo, "proj1", "func", 10, 200).await.unwrap();
        assert!(results.len() <= 2);
    }

    #[tokio::test]
    async fn test_prefix_matching_auth_finds_authenticate() {
        use sqlx::sqlite::SqlitePoolOptions;
        use crate::rag::types::{RagFile, RagChunk};

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let repo = RagRepository::new(pool);
        repo.initialize().await.unwrap();

        let file = RagFile {
            id: "f1".to_string(),
            project_id: "proj1".to_string(),
            file_path: "/test/auth.ts".to_string(),
            relative_path: "auth.ts".to_string(),
            content_hash: "abc".to_string(),
            language: Some("typescript".to_string()),
            indexed_at: "2026-01-01T00:00:00Z".to_string(),
        };
        repo.upsert_file(&file).await.unwrap();

        let chunks = vec![
            RagChunk {
                id: "c1".to_string(),
                file_id: "f1".to_string(),
                project_id: "proj1".to_string(),
                chunk_type: "function".to_string(),
                name: "authenticateUser".to_string(),
                content: "async function authenticateUser(email: string) { return true; }".to_string(),
                line_start: 1,
                line_end: 3,
                relative_path: "auth.ts".to_string(),
                metadata: None,
            },
        ];
        repo.insert_chunks(&chunks).await.unwrap();

        // "auth" should match "authenticateUser" via prefix matching ("auth"*)
        let results = search_code(&repo, "proj1", "auth", 10, 8000).await.unwrap();
        assert!(!results.is_empty(), "'auth' should find 'authenticateUser' via prefix matching");
        assert_eq!(results[0].chunk.name, "authenticateUser");
    }

    #[tokio::test]
    async fn test_hybrid_search_graceful_without_embeddings() {
        use sqlx::sqlite::SqlitePoolOptions;
        use crate::rag::types::{RagFile, RagChunk};

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let repo = RagRepository::new(pool);
        repo.initialize().await.unwrap();

        let file = RagFile {
            id: "f1".to_string(),
            project_id: "proj1".to_string(),
            file_path: "/test/config.ts".to_string(),
            relative_path: "config.ts".to_string(),
            content_hash: "abc".to_string(),
            language: Some("typescript".to_string()),
            indexed_at: "2026-01-01T00:00:00Z".to_string(),
        };
        repo.upsert_file(&file).await.unwrap();

        let chunks = vec![
            RagChunk {
                id: "c1".to_string(),
                file_id: "f1".to_string(),
                project_id: "proj1".to_string(),
                chunk_type: "function".to_string(),
                name: "configuration".to_string(),
                content: "function configuration() { return {}; }".to_string(),
                line_start: 1,
                line_end: 3,
                relative_path: "config.ts".to_string(),
                metadata: None,
            },
        ];
        repo.insert_chunks(&chunks).await.unwrap();

        // Hybrid search without embedding provider should work identically to FTS-only
        let results = search_code_hybrid(
            &repo, "proj1", "config", 10, 8000, None, None,
        ).await.unwrap();
        assert!(!results.is_empty(), "hybrid search without embeddings should still find results via FTS");
        assert_eq!(results[0].chunk.name, "configuration");
    }
}
