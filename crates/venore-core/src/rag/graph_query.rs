//! Graph Query Engine
//!
//! Answers structural questions about the codebase by composing graph
//! table queries (modules, dependencies, symbol references) with FTS5 search.
//! Includes a rule-based classifier that maps natural language to graph queries.

use crate::error::Result;
use crate::rag::repository::RagRepository;
use crate::rag::types::{ModuleDep, ModuleInfo, RagChunk, SymbolRef};

/// A structural query against the code graph
#[derive(Debug, Clone)]
pub enum GraphQuery {
    /// Modules that depend on the given module
    ModuleDependents { module_name: String },
    /// Modules that the given module depends on
    ModuleDependencies { module_name: String },
    /// Chunks that reference a given symbol name
    SymbolCallers { symbol_name: String },
    /// Symbols referenced by chunks matching a given symbol name
    SymbolCallees { symbol_name: String },
    /// Full module view: files, symbols, deps, dependents
    ModuleGraph { module_name: String },
    /// List files in a module
    ModuleFiles { module_name: String },
    /// FTS5 search scoped to a specific module
    ScopedSearch { module_name: String, query: String, max_results: u32 },
    /// List all modules in the project
    ListAllModules,
}

/// Result of a graph query
#[derive(Debug, Clone, Default)]
pub struct GraphQueryResult {
    /// Descriptive label of what was queried
    pub query_type: String,
    /// Modules in the result
    pub modules: Vec<ModuleInfo>,
    /// Code chunks in the result
    pub chunks: Vec<RagChunk>,
    /// Symbol references in the result
    pub refs: Vec<SymbolRef>,
    /// Module dependency edges in the result
    pub deps: Vec<ModuleDep>,
}

/// Execute a structural graph query
pub async fn execute_graph_query(
    repo: &RagRepository,
    project_id: &str,
    query: GraphQuery,
) -> Result<GraphQueryResult> {
    match query {
        GraphQuery::ModuleDependents { module_name } => {
            let deps = repo.get_module_dependents(project_id, &module_name).await?;
            let modules = fetch_modules_by_names(repo, project_id, &deps.iter().map(|d| d.from_module.as_str()).collect::<Vec<_>>()).await?;

            Ok(GraphQueryResult {
                query_type: format!("dependents_of:{}", module_name),
                modules,
                deps,
                ..Default::default()
            })
        }

        GraphQuery::ModuleDependencies { module_name } => {
            let deps = repo.get_module_deps(project_id, &module_name).await?;
            let modules = fetch_modules_by_names(repo, project_id, &deps.iter().map(|d| d.to_module.as_str()).collect::<Vec<_>>()).await?;

            Ok(GraphQueryResult {
                query_type: format!("dependencies_of:{}", module_name),
                modules,
                deps,
                ..Default::default()
            })
        }

        GraphQuery::SymbolCallers { symbol_name } => {
            let refs = repo.get_symbol_refs_to(project_id, &symbol_name).await?;

            Ok(GraphQueryResult {
                query_type: format!("callers_of:{}", symbol_name),
                refs,
                ..Default::default()
            })
        }

        GraphQuery::SymbolCallees { symbol_name } => {
            // Find chunks matching this symbol name, then get their outgoing refs
            let fts_query = format!("\"{}\"", symbol_name.replace('"', ""));
            let search_results = repo.search(project_id, &fts_query, 5).await?;

            let mut all_refs = Vec::new();
            for (chunk, _) in &search_results {
                if chunk.name == symbol_name {
                    let refs = repo.get_symbol_refs_from(&chunk.id).await?;
                    all_refs.extend(refs);
                }
            }

            let chunks = search_results.into_iter()
                .filter(|(c, _)| c.name == symbol_name)
                .map(|(c, _)| c)
                .collect();

            Ok(GraphQueryResult {
                query_type: format!("callees_of:{}", symbol_name),
                chunks,
                refs: all_refs,
                ..Default::default()
            })
        }

        GraphQuery::ModuleGraph { module_name } => {
            let all_modules = repo.get_modules(project_id).await?;
            let target = all_modules.into_iter()
                .find(|m| m.module_name == module_name);

            let modules = match target {
                Some(m) => vec![m],
                None => vec![],
            };

            let deps = repo.get_module_deps(project_id, &module_name).await?;
            let dependents = repo.get_module_dependents(project_id, &module_name).await?;

            let files = repo.get_module_files(project_id, &module_name).await?;
            // Get all chunks for the module's files
            let mut chunks = Vec::new();
            for file in &files {
                let file_chunks = repo.search(project_id, &format!("\"{}\"", file.relative_path.replace('"', "")), 100).await
                    .unwrap_or_default();
                for (chunk, _) in file_chunks {
                    if chunk.relative_path == file.relative_path {
                        chunks.push(chunk);
                    }
                }
            }

            let mut all_deps = deps;
            all_deps.extend(dependents);

            Ok(GraphQueryResult {
                query_type: format!("module_graph:{}", module_name),
                modules,
                chunks,
                deps: all_deps,
                ..Default::default()
            })
        }

        GraphQuery::ModuleFiles { module_name } => {
            let files = repo.get_module_files(project_id, &module_name).await?;
            let chunks: Vec<RagChunk> = files.iter().map(|f| RagChunk {
                id: f.id.clone(),
                file_id: f.id.clone(),
                project_id: f.project_id.clone(),
                chunk_type: "file".to_string(),
                name: f.relative_path.clone(),
                content: String::new(),
                line_start: 0,
                line_end: 0,
                relative_path: f.relative_path.clone(),
                metadata: None,
            }).collect();

            Ok(GraphQueryResult {
                query_type: format!("files_in:{}", module_name),
                chunks,
                ..Default::default()
            })
        }

        GraphQuery::ListAllModules => {
            let modules = repo.get_modules(project_id).await?;
            // Fetch all deps for a complete overview
            let mut all_deps = Vec::new();
            for m in &modules {
                let deps = repo.get_module_deps(project_id, &m.module_name).await?;
                all_deps.extend(deps);
            }

            Ok(GraphQueryResult {
                query_type: "all_modules".to_string(),
                modules,
                deps: all_deps,
                ..Default::default()
            })
        }

        GraphQuery::ScopedSearch { module_name, query, max_results } => {
            let fts_query = crate::rag::searcher::build_fts_query(&query);
            if fts_query.is_empty() {
                return Ok(GraphQueryResult {
                    query_type: format!("scoped_search:{}:{}", module_name, query),
                    ..Default::default()
                });
            }

            let results = repo.search_within_module(project_id, &module_name, &fts_query, max_results).await?;
            let chunks = results.into_iter().map(|(c, _)| c).collect();

            Ok(GraphQueryResult {
                query_type: format!("scoped_search:{}:{}", module_name, query),
                chunks,
                ..Default::default()
            })
        }
    }
}

/// Fetch module info for a list of module names
async fn fetch_modules_by_names(
    repo: &RagRepository,
    project_id: &str,
    names: &[&str],
) -> Result<Vec<ModuleInfo>> {
    if names.is_empty() {
        return Ok(vec![]);
    }

    let all_modules = repo.get_modules(project_id).await?;
    Ok(all_modules.into_iter()
        .filter(|m| names.contains(&m.module_name.as_str()))
        .collect())
}

// =============================================================================
// Query Classifier — rule-based, no LLM
// =============================================================================

/// Classify a natural language query into a GraphQuery.
/// Returns None for ambiguous queries (caller should fall back to FTS5).
pub fn classify_query(input: &str) -> Option<GraphQuery> {
    let lower = input.to_lowercase();
    let trimmed = lower.trim();

    // Pattern: "depends on X" / "dependencies of X" / "what does X depend on"
    if let Some(name) = extract_after_pattern(trimmed, "depends on ") {
        return Some(GraphQuery::ModuleDependents { module_name: name });
    }
    if let Some(name) = extract_after_pattern(trimmed, "dependencies of ") {
        return Some(GraphQuery::ModuleDependencies { module_name: name });
    }
    if let Some(name) = extract_between(trimmed, "what does ", " depend on") {
        return Some(GraphQuery::ModuleDependencies { module_name: name });
    }
    if let Some(name) = extract_between(trimmed, "what depends on ", "") {
        return Some(GraphQuery::ModuleDependents { module_name: clean_query_tail(&name) });
    }

    // Pattern: "who calls X" / "what calls X" / "callers of X"
    if let Some(name) = extract_after_pattern(trimmed, "who calls ") {
        return Some(GraphQuery::SymbolCallers { symbol_name: name });
    }
    if let Some(name) = extract_after_pattern(trimmed, "what calls ") {
        return Some(GraphQuery::SymbolCallers { symbol_name: name });
    }
    if let Some(name) = extract_after_pattern(trimmed, "callers of ") {
        return Some(GraphQuery::SymbolCallers { symbol_name: name });
    }
    if let Some(name) = extract_after_pattern(trimmed, "references to ") {
        return Some(GraphQuery::SymbolCallers { symbol_name: name });
    }

    // Pattern: "what does X call" / "callees of X"
    if let Some(name) = extract_between(trimmed, "what does ", " call") {
        return Some(GraphQuery::SymbolCallees { symbol_name: name });
    }
    if let Some(name) = extract_after_pattern(trimmed, "callees of ") {
        return Some(GraphQuery::SymbolCallees { symbol_name: name });
    }

    // Pattern: "show X graph" / "graph of X" / "module X"
    if let Some(name) = extract_between(trimmed, "show ", " graph") {
        return Some(GraphQuery::ModuleGraph { module_name: name });
    }
    if let Some(name) = extract_after_pattern(trimmed, "graph of ") {
        return Some(GraphQuery::ModuleGraph { module_name: name });
    }

    // Pattern: "files in X" / "list files in X"
    if let Some(name) = extract_after_pattern(trimmed, "files in ") {
        return Some(GraphQuery::ModuleFiles { module_name: name });
    }

    // Pattern: "modules" / "list modules" / "all modules" / "show modules"
    if matches!(trimmed, "modules" | "list modules" | "all modules" | "show modules") {
        return Some(GraphQuery::ListAllModules);
    }

    // No match — caller should fall back to standard FTS5
    None
}

/// Extract text after a pattern, trimmed and cleaned
fn extract_after_pattern(input: &str, pattern: &str) -> Option<String> {
    if let Some(pos) = input.find(pattern) {
        let rest = input[pos + pattern.len()..].trim();
        if !rest.is_empty() {
            return Some(clean_query_tail(rest));
        }
    }
    None
}

/// Extract text between two patterns
fn extract_between(input: &str, start: &str, end: &str) -> Option<String> {
    if let Some(start_pos) = input.find(start) {
        let after_start = &input[start_pos + start.len()..];
        let name = if end.is_empty() {
            after_start.trim()
        } else if let Some(end_pos) = after_start.find(end) {
            after_start[..end_pos].trim()
        } else {
            return None;
        };
        if !name.is_empty() {
            return Some(clean_query_tail(name));
        }
    }
    None
}

/// Remove trailing punctuation and common filler words
fn clean_query_tail(s: &str) -> String {
    s.trim_end_matches(['?', '.', '!', ','])
        .trim()
        .to_string()
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Classifier tests
    // ========================================================================

    #[test]
    fn test_classify_dependents() {
        let q = classify_query("what depends on auth?");
        assert!(matches!(q, Some(GraphQuery::ModuleDependents { module_name }) if module_name == "auth"));
    }

    #[test]
    fn test_classify_dependencies() {
        let q = classify_query("what does auth depend on?");
        assert!(matches!(q, Some(GraphQuery::ModuleDependencies { module_name }) if module_name == "auth"));

        let q2 = classify_query("dependencies of chat");
        assert!(matches!(q2, Some(GraphQuery::ModuleDependencies { module_name }) if module_name == "chat"));
    }

    #[test]
    fn test_classify_symbol_callers() {
        let q = classify_query("who calls getUserById");
        assert!(matches!(q, Some(GraphQuery::SymbolCallers { symbol_name }) if symbol_name == "getuserbyid"));

        let q2 = classify_query("what calls hashPassword?");
        assert!(matches!(q2, Some(GraphQuery::SymbolCallers { symbol_name }) if symbol_name == "hashpassword"));

        let q3 = classify_query("references to login");
        assert!(matches!(q3, Some(GraphQuery::SymbolCallers { symbol_name }) if symbol_name == "login"));
    }

    #[test]
    fn test_classify_symbol_callees() {
        let q = classify_query("what does login call?");
        assert!(matches!(q, Some(GraphQuery::SymbolCallees { symbol_name }) if symbol_name == "login"));
    }

    #[test]
    fn test_classify_module_graph() {
        let q = classify_query("show auth graph");
        assert!(matches!(q, Some(GraphQuery::ModuleGraph { module_name }) if module_name == "auth"));

        let q2 = classify_query("graph of utils");
        assert!(matches!(q2, Some(GraphQuery::ModuleGraph { module_name }) if module_name == "utils"));
    }

    #[test]
    fn test_classify_module_files() {
        let q = classify_query("files in auth");
        assert!(matches!(q, Some(GraphQuery::ModuleFiles { module_name }) if module_name == "auth"));
    }

    #[test]
    fn test_classify_ambiguous_returns_none() {
        assert!(classify_query("how does authentication work?").is_none());
        assert!(classify_query("fix the bug in login").is_none());
        assert!(classify_query("").is_none());
    }

    // ========================================================================
    // Execution tests (against in-memory SQLite)
    // ========================================================================

    use sqlx::sqlite::SqlitePoolOptions;
    use crate::rag::types::{RagFile, RagChunk};

    async fn setup_test_graph() -> (RagRepository, &'static str) {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let repo = RagRepository::new(pool);
        repo.initialize().await.unwrap();

        let project_id = "proj1";

        // Create files
        for (id, path) in [("f1", "src/auth/login.ts"), ("f2", "src/utils/hash.ts"), ("f3", "src/api/routes.ts")] {
            repo.upsert_file(&RagFile {
                id: id.to_string(),
                project_id: project_id.to_string(),
                file_path: format!("/test/{}", path),
                relative_path: path.to_string(),
                content_hash: "abc".to_string(),
                language: Some("typescript".to_string()),
                indexed_at: "2026-01-01T00:00:00Z".to_string(),
            }).await.unwrap();
        }

        // Create chunks
        let chunks = vec![
            RagChunk { id: "c1".into(), file_id: "f1".into(), project_id: project_id.into(),
                chunk_type: "function".into(), name: "login".into(),
                content: "function login() { hashPassword(); }".into(),
                line_start: 1, line_end: 3, relative_path: "src/auth/login.ts".into(), metadata: None },
            RagChunk { id: "c2".into(), file_id: "f2".into(), project_id: project_id.into(),
                chunk_type: "function".into(), name: "hashPassword".into(),
                content: "function hashPassword(pwd) { return hash(pwd); }".into(),
                line_start: 1, line_end: 3, relative_path: "src/utils/hash.ts".into(), metadata: None },
            RagChunk { id: "c3".into(), file_id: "f3".into(), project_id: project_id.into(),
                chunk_type: "function".into(), name: "routes".into(),
                content: "function routes() { login(); }".into(),
                line_start: 1, line_end: 3, relative_path: "src/api/routes.ts".into(), metadata: None },
        ];
        repo.insert_chunks(&chunks).await.unwrap();

        // Map files to modules
        repo.upsert_file_module("f1", project_id, "auth", "src/auth", true).await.unwrap();
        repo.upsert_file_module("f2", project_id, "utils", "src/utils", true).await.unwrap();
        repo.upsert_file_module("f3", project_id, "api", "src/api", true).await.unwrap();

        // Module deps: auth→utils, api→auth
        repo.upsert_module_dep(project_id, "auth", "utils", "import").await.unwrap();
        repo.upsert_module_dep(project_id, "api", "auth", "import").await.unwrap();

        // Symbol refs: login→hashPassword, routes→login
        repo.insert_symbol_refs(&[
            SymbolRef {
                id: "r1".into(), project_id: project_id.into(),
                from_chunk_id: "c1".into(), to_chunk_id: Some("c2".into()),
                to_symbol_name: "hashPassword".into(), to_file_path: None,
                ref_type: "import".into(), line_number: Some(1),
            },
            SymbolRef {
                id: "r2".into(), project_id: project_id.into(),
                from_chunk_id: "c3".into(), to_chunk_id: Some("c1".into()),
                to_symbol_name: "login".into(), to_file_path: None,
                ref_type: "import".into(), line_number: Some(1),
            },
        ]).await.unwrap();

        (repo, project_id)
    }

    #[tokio::test]
    async fn test_execute_module_dependents() {
        let (repo, pid) = setup_test_graph().await;

        let result = execute_graph_query(&repo, pid, GraphQuery::ModuleDependents {
            module_name: "auth".to_string(),
        }).await.unwrap();

        assert_eq!(result.deps.len(), 1);
        assert_eq!(result.deps[0].from_module, "api");
        assert_eq!(result.modules.len(), 1);
        assert_eq!(result.modules[0].module_name, "api");
    }

    #[tokio::test]
    async fn test_execute_module_dependencies() {
        let (repo, pid) = setup_test_graph().await;

        let result = execute_graph_query(&repo, pid, GraphQuery::ModuleDependencies {
            module_name: "auth".to_string(),
        }).await.unwrap();

        assert_eq!(result.deps.len(), 1);
        assert_eq!(result.deps[0].to_module, "utils");
    }

    #[tokio::test]
    async fn test_execute_symbol_callers() {
        let (repo, pid) = setup_test_graph().await;

        let result = execute_graph_query(&repo, pid, GraphQuery::SymbolCallers {
            symbol_name: "hashPassword".to_string(),
        }).await.unwrap();

        assert_eq!(result.refs.len(), 1);
        assert_eq!(result.refs[0].from_chunk_id, "c1");
    }

    #[tokio::test]
    async fn test_execute_symbol_callees() {
        let (repo, pid) = setup_test_graph().await;

        let result = execute_graph_query(&repo, pid, GraphQuery::SymbolCallees {
            symbol_name: "login".to_string(),
        }).await.unwrap();

        assert_eq!(result.chunks.len(), 1);
        assert_eq!(result.chunks[0].name, "login");
        assert_eq!(result.refs.len(), 1);
        assert_eq!(result.refs[0].to_symbol_name, "hashPassword");
    }

    #[tokio::test]
    async fn test_execute_module_files() {
        let (repo, pid) = setup_test_graph().await;

        let result = execute_graph_query(&repo, pid, GraphQuery::ModuleFiles {
            module_name: "auth".to_string(),
        }).await.unwrap();

        assert_eq!(result.chunks.len(), 1);
        assert!(result.chunks[0].relative_path.contains("auth"));
    }

    #[tokio::test]
    async fn test_execute_scoped_search() {
        let (repo, pid) = setup_test_graph().await;

        let result = execute_graph_query(&repo, pid, GraphQuery::ScopedSearch {
            module_name: "auth".to_string(),
            query: "login".to_string(),
            max_results: 10,
        }).await.unwrap();

        assert_eq!(result.chunks.len(), 1);
        assert_eq!(result.chunks[0].name, "login");

        // Same query scoped to utils should NOT find login
        let result2 = execute_graph_query(&repo, pid, GraphQuery::ScopedSearch {
            module_name: "utils".to_string(),
            query: "login".to_string(),
            max_results: 10,
        }).await.unwrap();

        assert!(result2.chunks.is_empty());
    }

    #[tokio::test]
    async fn test_execute_module_graph() {
        let (repo, pid) = setup_test_graph().await;

        let result = execute_graph_query(&repo, pid, GraphQuery::ModuleGraph {
            module_name: "auth".to_string(),
        }).await.unwrap();

        assert_eq!(result.modules.len(), 1);
        assert_eq!(result.modules[0].module_name, "auth");
        // auth has 1 dep (utils) + 1 dependent (api) = 2 dep edges
        assert_eq!(result.deps.len(), 2);
    }
}
