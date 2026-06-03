//! Project Indexer
//!
//! Indexes project source files into the RAG repository.
//! Supports incremental indexing via SHA-256 content hashing.

use std::collections::HashMap;
use std::path::Path;

use sha2::{Sha256, Digest};

use crate::analysis::file_scanner::{scan_directory, ScanConfig};
use crate::analysis::analysis_output::AnalysisOutput;
use crate::error::Result;
use crate::rag::chunker::chunk_file;
use crate::rag::repository::RagRepository;
use crate::rag::types::{ChangeSet, IndexGraphResult, IndexProgressEvent, RagFile, SymbolRef};
use crate::traits::EmbeddingProvider;

/// Configuration for project indexing
#[derive(Debug, Clone)]
pub struct IndexConfig {
    pub target_extensions: Vec<String>,
    pub ignore_patterns: Vec<String>,
    pub max_file_size_kb: u64,
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            target_extensions: vec![
                "ts", "tsx", "js", "jsx", "rs", "py", "go", "java",
                "json", "yaml", "toml", "md", "css", "html",
            ].into_iter().map(String::from).collect(),
            ignore_patterns: vec![
                "node_modules", "dist", ".git", "target", ".venore",
                "__pycache__", ".next", "build", "coverage", ".turbo",
            ].into_iter().map(String::from).collect(),
            max_file_size_kb: 500,
        }
    }
}

/// Index a project (full or incremental)
///
/// Returns (indexed_count, skipped_count, removed_count)
///
/// If `embedding_provider` and `embedding_api_key` are provided, chunks will
/// also be embedded after insertion. Embedding failures are logged but do not
/// break indexing (graceful degradation).
pub async fn index_project(
    repo: &RagRepository,
    project_id: &str,
    project_path: &Path,
    config: &IndexConfig,
    on_progress: Option<&(dyn Fn(IndexProgressEvent) + Send + Sync)>,
) -> Result<(u32, u32, u32)> {
    index_project_with_embeddings(repo, project_id, project_path, config, on_progress, None, None, None).await
}

/// Index a project with optional embedding support.
///
/// `cancel` lets long-running indexing jobs check for cancellation between
/// files and bail out cleanly with `VenoreError::Cancelled`.
pub async fn index_project_with_embeddings(
    repo: &RagRepository,
    project_id: &str,
    project_path: &Path,
    config: &IndexConfig,
    on_progress: Option<&(dyn Fn(IndexProgressEvent) + Send + Sync)>,
    embedding_provider: Option<&dyn EmbeddingProvider>,
    embedding_api_key: Option<&str>,
    cancel: Option<&tokio_util::sync::CancellationToken>,
) -> Result<(u32, u32, u32)> {
    tracing::info!(
        "Starting index for project {} at {}",
        project_id,
        project_path.display()
    );

    // 1. Scan directory for files
    let scan_config = ScanConfig {
        project_path: project_path.to_path_buf(),
        target_extensions: config.target_extensions.clone(),
        ignore_patterns: config.ignore_patterns.clone(),
        max_file_size_kb: config.max_file_size_kb,
    };

    let scan_result = scan_directory(scan_config)?;
    let total_files = scan_result.files.len() as u32;

    tracing::info!("Scanned {} files on disk", total_files);

    // 2. Get existing indexed files
    let existing_files = repo.get_files(project_id).await?;
    let mut existing_map: HashMap<String, RagFile> = existing_files
        .into_iter()
        .map(|f| (f.relative_path.clone(), f))
        .collect();

    let mut indexed_count = 0u32;
    let mut skipped_count = 0u32;

    // 3. Process each file on disk
    for (i, file_info) in scan_result.files.iter().enumerate() {
        // Cooperative cancellation: check before every file. Cheap; the
        // expensive work (read + hash + chunk + sqlx insert) comes next.
        if let Some(token) = cancel {
            if token.is_cancelled() {
                return Err(crate::VenoreError::Cancelled(
                    format!("Indexing cancelled after {} files", indexed_count + skipped_count),
                ));
            }
        }

        let relative_path = file_info.path
            .strip_prefix(project_path)
            .unwrap_or(&file_info.path)
            .to_string_lossy()
            .replace('\\', "/");

        // Emit progress
        if let Some(cb) = on_progress {
            cb(IndexProgressEvent {
                project_id: project_id.to_string(),
                current: (i + 1) as u32,
                total: total_files,
                current_file: relative_path.clone(),
                status: "indexing".to_string(),
            });
        }

        // Read file content
        let content = match std::fs::read_to_string(&file_info.path) {
            Ok(c) => c,
            Err(_) => {
                // Skip binary or unreadable files
                skipped_count += 1;
                continue;
            }
        };

        // Compute SHA-256 hash
        let content_hash = compute_hash(&content);

        // Check if already indexed with same hash
        let existing_file_id = if let Some(existing) = existing_map.get(&relative_path) {
            if existing.content_hash == content_hash {
                // Unchanged — skip
                existing_map.remove(&relative_path);
                skipped_count += 1;
                continue;
            }

            // Changed — delete old chunks, re-index
            tracing::debug!("File changed: {}", relative_path);
            repo.delete_chunks_for_file(&existing.id).await?;
            let id = existing.id.clone();
            existing_map.remove(&relative_path);
            Some(id)
        } else {
            None
        };

        // Reuse existing file ID if updating, or generate new one
        let file_id = existing_file_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let language = file_info.path
            .extension()
            .and_then(|e| e.to_str())
            .map(String::from);

        let rag_file = RagFile {
            id: file_id.clone(),
            project_id: project_id.to_string(),
            file_path: file_info.path.to_string_lossy().to_string(),
            relative_path: relative_path.clone(),
            content_hash,
            language,
            indexed_at: chrono::Utc::now().to_rfc3339(),
        };

        repo.upsert_file(&rag_file).await?;

        // Chunk and insert
        let chunks = chunk_file(
            &file_id,
            project_id,
            &file_info.path,
            &relative_path,
            &content,
        )?;

        if !chunks.is_empty() {
            repo.insert_chunks(&chunks).await?;
        }

        indexed_count += 1;
    }

    // 4. Remove files that no longer exist on disk
    let removed_count = existing_map.len() as u32;
    for orphan in existing_map.values() {
        tracing::debug!("Removing deleted file: {}", orphan.relative_path);
        repo.delete_file(&orphan.id).await?;
    }

    // 5. Embed chunks (if provider available, graceful on failure)
    if let (Some(provider), Some(api_key)) = (embedding_provider, embedding_api_key) {
        if let Err(e) = embed_chunks(repo, project_id, provider, api_key).await {
            tracing::warn!("Embedding failed (continuing without): {}", e);
        }
    }

    // Emit completion
    if let Some(cb) = on_progress {
        cb(IndexProgressEvent {
            project_id: project_id.to_string(),
            current: total_files,
            total: total_files,
            current_file: String::new(),
            status: "completed".to_string(),
        });
    }

    tracing::info!(
        "Indexing complete: {} indexed, {} skipped, {} removed",
        indexed_count,
        skipped_count,
        removed_count
    );

    Ok((indexed_count, skipped_count, removed_count))
}

/// Detect which files changed since the last index.
///
/// Compares on-disk files against `rag_files` content hashes.
/// Useful for deciding whether to re-run analysis or indexing.
pub async fn detect_changed_files(
    repo: &RagRepository,
    project_id: &str,
    project_path: &Path,
    config: &IndexConfig,
) -> Result<ChangeSet> {
    let scan_config = ScanConfig {
        project_path: project_path.to_path_buf(),
        target_extensions: config.target_extensions.clone(),
        ignore_patterns: config.ignore_patterns.clone(),
        max_file_size_kb: config.max_file_size_kb,
    };

    let scan_result = scan_directory(scan_config)?;

    let existing_files = repo.get_files(project_id).await?;
    let mut existing_map: HashMap<String, RagFile> = existing_files
        .into_iter()
        .map(|f| (f.relative_path.clone(), f))
        .collect();

    let mut change_set = ChangeSet::default();

    for file_info in &scan_result.files {
        let relative_path = file_info.path
            .strip_prefix(project_path)
            .unwrap_or(&file_info.path)
            .to_string_lossy()
            .replace('\\', "/");

        let content = match std::fs::read_to_string(&file_info.path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let content_hash = compute_hash(&content);

        if let Some(existing) = existing_map.remove(&relative_path) {
            if existing.content_hash != content_hash {
                change_set.modified_files.push(file_info.path.clone());
            }
        } else {
            change_set.new_files.push(file_info.path.clone());
        }
    }

    // Remaining in existing_map = files deleted from disk
    for (rel_path, _) in existing_map {
        change_set.deleted_paths.push(rel_path);
    }

    tracing::debug!(
        "Change detection: {} new, {} modified, {} deleted",
        change_set.new_files.len(),
        change_set.modified_files.len(),
        change_set.deleted_paths.len()
    );

    Ok(change_set)
}

/// Index a project and populate the code graph from analysis output.
///
/// Reuses the standard indexing pipeline, then populates graph tables
/// (file→module mappings, module dependencies, symbol references)
/// from the provided `AnalysisOutput`.
pub async fn index_project_with_graph(
    repo: &RagRepository,
    project_id: &str,
    project_path: &Path,
    config: &IndexConfig,
    on_progress: Option<&(dyn Fn(IndexProgressEvent) + Send + Sync)>,
    analysis: &AnalysisOutput,
    cancel: Option<&tokio_util::sync::CancellationToken>,
) -> Result<IndexGraphResult> {
    // 1. Run standard indexing (reuse existing pipeline)
    let (indexed, skipped, removed) = index_project_with_embeddings(
        repo, project_id, project_path, config, on_progress, None, None, cancel,
    ).await?;

    // Cancellation checkpoint between phases — bail before doing the
    // (idempotent but slow) graph populate if the caller already gave up.
    if let Some(token) = cancel {
        if token.is_cancelled() {
            return Err(crate::VenoreError::Cancelled("Indexing cancelled before graph populate".to_string()));
        }
    }

    // 2. Populate graph from analysis output
    let (modules_mapped, deps_created, refs_created) =
        populate_graph(repo, project_id, analysis).await?;

    tracing::info!(
        "Graph populated: {} modules mapped, {} deps, {} refs",
        modules_mapped, deps_created, refs_created
    );

    Ok(IndexGraphResult {
        indexed,
        skipped,
        removed,
        modules_mapped,
        deps_created,
        refs_created,
    })
}

/// Populate graph tables from AnalysisOutput.
/// Clears previous graph data for the project before inserting.
async fn populate_graph(
    repo: &RagRepository,
    project_id: &str,
    analysis: &AnalysisOutput,
) -> Result<(u32, u32, u32)> {
    // Clear previous graph data (idempotent re-population)
    repo.delete_symbol_refs(project_id).await?;
    repo.delete_module_deps(project_id).await?;
    repo.delete_module_mappings(project_id).await?;

    let indexed_files = repo.get_files(project_id).await?;
    let mut modules_mapped = 0u32;
    let mut deps_created = 0u32;
    let mut refs_created = 0u32;

    for module in &analysis.modules {
        let module_prefix = &module.path;

        // Map files to modules — match rag_files by relative_path prefix
        for file in &indexed_files {
            let belongs = if module_prefix.is_empty() {
                // Root module: files not in any subdirectory
                !file.relative_path.contains('/')
            } else {
                file.relative_path.starts_with(module_prefix)
                    && file.relative_path[module_prefix.len()..].starts_with('/')
            };

            if belongs {
                let is_entry_point = module.entry_point.as_ref()
                    .map(|ep| file.relative_path.ends_with(ep))
                    .unwrap_or(false);

                repo.upsert_file_module(
                    &file.id,
                    project_id,
                    &module.name,
                    module_prefix,
                    is_entry_point,
                ).await?;
                modules_mapped += 1;
            }
        }

        // Insert module dependencies
        for dep in &module.architecture.dependencies {
            repo.upsert_module_dep(project_id, &module.name, dep, "import").await?;
            deps_created += 1;
        }

        // Extract symbol references from imports
        for import_info in &module.imports {
            // Reconstruct the importing file's relative path
            let importing_file_path = if module_prefix.is_empty() {
                import_info.file.clone()
            } else {
                format!("{}/{}", module_prefix, import_info.file)
            };

            // Find the rag_file for this importing file
            let importing_file = indexed_files.iter()
                .find(|f| f.relative_path == importing_file_path);

            let file_id = match importing_file {
                Some(f) => &f.id,
                None => continue, // File not indexed (e.g., filtered by extension)
            };

            // Find the imports chunk for this file (created by chunker)
            let from_chunk_id = find_imports_chunk(repo, file_id).await?;
            let from_chunk_id = match from_chunk_id {
                Some(id) => id,
                None => continue, // No imports chunk for this file
            };

            // Create a SymbolRef for each imported item
            for item in &import_info.items {
                let ref_id = uuid::Uuid::new_v4().to_string();
                refs_created += 1;

                repo.insert_symbol_refs(&[SymbolRef {
                    id: ref_id,
                    project_id: project_id.to_string(),
                    from_chunk_id: from_chunk_id.clone(),
                    to_chunk_id: None,
                    to_symbol_name: item.clone(),
                    to_file_path: Some(import_info.module.clone()),
                    ref_type: "import".to_string(),
                    line_number: None,
                }]).await?;
            }
        }
    }

    // Resolve dangling symbol references (match to_symbol_name → chunk name)
    let resolved = repo.resolve_symbol_refs(project_id).await?;
    tracing::debug!("Resolved {} symbol references", resolved);

    Ok((modules_mapped, deps_created, refs_created))
}

/// Find the "imports" chunk for a file, or fall back to the first chunk.
async fn find_imports_chunk(repo: &RagRepository, file_id: &str) -> Result<Option<String>> {
    // Query for the imports chunk first, then any chunk as fallback
    let row = sqlx::query(
        "SELECT id, chunk_type FROM rag_chunks WHERE file_id = ?
         ORDER BY CASE WHEN chunk_type = 'imports' THEN 0 ELSE 1 END
         LIMIT 1"
    )
    .bind(file_id)
    .fetch_optional(repo.pool())
    .await
    .map_err(|e| crate::VenoreError::DatabaseError(format!("Failed to find imports chunk: {}", e)))?;

    Ok(row.map(|r| sqlx::Row::get::<String, _>(&r, "id")))
}

/// Embed chunks that don't have embeddings yet.
/// Processes in batches; failures are propagated but callers should catch them.
async fn embed_chunks(
    repo: &RagRepository,
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

        tracing::debug!("Embedded {} chunks with {}", chunk_ids.len(), model);

        if (chunk_ids.len() as u32) < BATCH_SIZE {
            break;
        }
    }

    tracing::info!("Embedding complete for project {} using {}", project_id, model);
    Ok(())
}

/// Compute SHA-256 hash of content
pub(crate) fn compute_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::analysis_output::{
        AnalysisOutput, ModuleAnalysis, ModuleArchitecture, ModuleSymbols,
        ImportInfo, RepositoryInfo,
    };
    use crate::rag::repository::RagRepository;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::fs;

    async fn create_test_repo() -> RagRepository {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let repo = RagRepository::new(pool);
        repo.initialize().await.unwrap();
        repo
    }

    #[test]
    fn test_compute_hash() {
        let hash = compute_hash("hello world");
        assert_eq!(hash.len(), 64); // SHA-256 hex
        // Same input = same hash
        assert_eq!(hash, compute_hash("hello world"));
        // Different input = different hash
        assert_ne!(hash, compute_hash("hello world!"));
    }

    #[tokio::test]
    async fn test_index_project_basic() {
        let repo = create_test_repo().await;

        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("src");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("index.ts"), "export function main() { console.log('hello'); }").unwrap();
        fs::write(src.join("utils.ts"), "export const add = (a: number, b: number) => a + b;").unwrap();

        let config = IndexConfig {
            target_extensions: vec!["ts".to_string()],
            ignore_patterns: vec!["node_modules".to_string()],
            max_file_size_kb: 500,
        };

        let (indexed, skipped, removed) = index_project(
            &repo, "proj1", temp.path(), &config, None,
        ).await.unwrap();

        assert_eq!(indexed, 2);
        assert_eq!(skipped, 0);
        assert_eq!(removed, 0);

        let status = repo.get_index_status("proj1").await.unwrap();
        assert!(status.is_indexed);
        assert_eq!(status.total_files, 2);
        assert!(status.total_chunks > 0);
    }

    #[tokio::test]
    async fn test_incremental_indexing() {
        let repo = create_test_repo().await;

        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("file.ts"), "const x = 1;").unwrap();

        let config = IndexConfig {
            target_extensions: vec!["ts".to_string()],
            ignore_patterns: vec![],
            max_file_size_kb: 500,
        };

        // First index
        let (indexed, _, _) = index_project(
            &repo, "proj1", temp.path(), &config, None,
        ).await.unwrap();
        assert_eq!(indexed, 1);

        // Re-index without changes → skipped
        let (indexed, skipped, _) = index_project(
            &repo, "proj1", temp.path(), &config, None,
        ).await.unwrap();
        assert_eq!(indexed, 0);
        assert_eq!(skipped, 1);

        // Modify file → re-indexed
        fs::write(temp.path().join("file.ts"), "const x = 2; const y = 3;").unwrap();
        let (indexed, skipped, _) = index_project(
            &repo, "proj1", temp.path(), &config, None,
        ).await.unwrap();
        assert_eq!(indexed, 1);
        assert_eq!(skipped, 0);
    }

    #[tokio::test]
    async fn test_removed_files_detected() {
        let repo = create_test_repo().await;

        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("a.ts"), "const a = 1;").unwrap();
        fs::write(temp.path().join("b.ts"), "const b = 2;").unwrap();

        let config = IndexConfig {
            target_extensions: vec!["ts".to_string()],
            ignore_patterns: vec![],
            max_file_size_kb: 500,
        };

        // First index
        index_project(&repo, "proj1", temp.path(), &config, None).await.unwrap();

        // Delete one file
        fs::remove_file(temp.path().join("b.ts")).unwrap();

        // Re-index
        let (_, _, removed) = index_project(
            &repo, "proj1", temp.path(), &config, None,
        ).await.unwrap();
        assert_eq!(removed, 1);

        let status = repo.get_index_status("proj1").await.unwrap();
        assert_eq!(status.total_files, 1);
    }

    // ========================================================================
    // CHANGE DETECTION TESTS
    // ========================================================================

    #[tokio::test]
    async fn test_detect_changed_files_all_new() {
        let repo = create_test_repo().await;
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("a.ts"), "const a = 1;").unwrap();
        fs::write(temp.path().join("b.ts"), "const b = 2;").unwrap();

        let config = IndexConfig {
            target_extensions: vec!["ts".to_string()],
            ignore_patterns: vec![],
            max_file_size_kb: 500,
        };

        let changes = detect_changed_files(&repo, "proj1", temp.path(), &config).await.unwrap();
        assert_eq!(changes.new_files.len(), 2);
        assert!(changes.modified_files.is_empty());
        assert!(changes.deleted_paths.is_empty());
    }

    #[tokio::test]
    async fn test_detect_changed_files_no_changes() {
        let repo = create_test_repo().await;
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("a.ts"), "const a = 1;").unwrap();

        let config = IndexConfig {
            target_extensions: vec!["ts".to_string()],
            ignore_patterns: vec![],
            max_file_size_kb: 500,
        };

        // Index first
        index_project(&repo, "proj1", temp.path(), &config, None).await.unwrap();

        // No changes
        let changes = detect_changed_files(&repo, "proj1", temp.path(), &config).await.unwrap();
        assert!(changes.is_empty());
    }

    #[tokio::test]
    async fn test_detect_changed_files_modified_and_deleted() {
        let repo = create_test_repo().await;
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("a.ts"), "const a = 1;").unwrap();
        fs::write(temp.path().join("b.ts"), "const b = 2;").unwrap();

        let config = IndexConfig {
            target_extensions: vec!["ts".to_string()],
            ignore_patterns: vec![],
            max_file_size_kb: 500,
        };

        index_project(&repo, "proj1", temp.path(), &config, None).await.unwrap();

        // Modify a, delete b, add c
        fs::write(temp.path().join("a.ts"), "const a = 999;").unwrap();
        fs::remove_file(temp.path().join("b.ts")).unwrap();
        fs::write(temp.path().join("c.ts"), "const c = 3;").unwrap();

        let changes = detect_changed_files(&repo, "proj1", temp.path(), &config).await.unwrap();
        assert_eq!(changes.modified_files.len(), 1);
        assert_eq!(changes.deleted_paths.len(), 1);
        assert_eq!(changes.new_files.len(), 1);
        assert_eq!(changes.total_changed(), 2);
    }

    // ========================================================================
    // GRAPH INDEXING TESTS
    // ========================================================================

    /// Build a minimal AnalysisOutput for testing graph population
    fn test_analysis(modules: Vec<ModuleAnalysis>) -> AnalysisOutput {
        AnalysisOutput {
            repository: RepositoryInfo {
                name: "test-project".to_string(),
                language: None,
                technologies: vec![],
                total_files: 0,
                total_modules: modules.len(),
            },
            modules,
            orphan_files: vec![],
        }
    }

    fn test_module(name: &str, path: &str, deps: Vec<&str>, imports: Vec<ImportInfo>, files: Vec<&str>) -> ModuleAnalysis {
        ModuleAnalysis {
            name: name.to_string(),
            path: path.to_string(),
            file_count: files.len(),
            entry_point: None,
            architecture: ModuleArchitecture {
                dependencies: deps.into_iter().map(String::from).collect(),
                dependents: vec![],
                external_deps: vec![],
            },
            symbols: ModuleSymbols { exports: vec![], all: vec![] },
            imports,
            code_snippets: String::new(),
            files: files.into_iter().map(String::from).collect(),
        }
    }

    #[tokio::test]
    async fn test_index_with_graph_populates_modules() {
        let repo = create_test_repo().await;

        let temp = tempfile::tempdir().unwrap();
        let auth_dir = temp.path().join("src").join("auth");
        let utils_dir = temp.path().join("src").join("utils");
        fs::create_dir_all(&auth_dir).unwrap();
        fs::create_dir_all(&utils_dir).unwrap();
        fs::write(auth_dir.join("login.ts"), "export function login() { return true; }").unwrap();
        fs::write(utils_dir.join("hash.ts"), "export function hashPassword(pwd: string) { return pwd; }").unwrap();

        let config = IndexConfig {
            target_extensions: vec!["ts".to_string()],
            ignore_patterns: vec![],
            max_file_size_kb: 500,
        };

        let analysis = test_analysis(vec![
            test_module("auth", "src/auth", vec!["utils"], vec![], vec!["login.ts"]),
            test_module("utils", "src/utils", vec![], vec![], vec!["hash.ts"]),
        ]);

        let result = index_project_with_graph(
            &repo, "proj1", temp.path(), &config, None, &analysis, None,
        ).await.unwrap();

        assert_eq!(result.indexed, 2);
        assert_eq!(result.modules_mapped, 2);
        assert_eq!(result.deps_created, 1);

        // Verify modules were created
        let modules = repo.get_modules("proj1").await.unwrap();
        assert_eq!(modules.len(), 2);

        // Verify module dependency
        let deps = repo.get_module_deps("proj1", "auth").await.unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].to_module, "utils");

        // Verify utils has auth as dependent
        let dependents = repo.get_module_dependents("proj1", "utils").await.unwrap();
        assert_eq!(dependents.len(), 1);
        assert_eq!(dependents[0].from_module, "auth");
    }

    #[tokio::test]
    async fn test_index_with_graph_creates_symbol_refs() {
        let repo = create_test_repo().await;

        let temp = tempfile::tempdir().unwrap();
        let auth_dir = temp.path().join("src").join("auth");
        let utils_dir = temp.path().join("src").join("utils");
        fs::create_dir_all(&auth_dir).unwrap();
        fs::create_dir_all(&utils_dir).unwrap();

        // auth/login.ts imports hashPassword from utils
        fs::write(
            auth_dir.join("login.ts"),
            "import { hashPassword } from '../utils/hash';\nexport function login(pwd: string) { return hashPassword(pwd); }",
        ).unwrap();
        fs::write(
            utils_dir.join("hash.ts"),
            "export function hashPassword(pwd: string) { return pwd; }",
        ).unwrap();

        let config = IndexConfig {
            target_extensions: vec!["ts".to_string()],
            ignore_patterns: vec![],
            max_file_size_kb: 500,
        };

        let analysis = test_analysis(vec![
            test_module(
                "auth", "src/auth",
                vec!["utils"],
                vec![ImportInfo {
                    module: "../utils/hash".to_string(),
                    items: vec!["hashPassword".to_string()],
                    file: "login.ts".to_string(),
                }],
                vec!["login.ts"],
            ),
            test_module("utils", "src/utils", vec![], vec![], vec!["hash.ts"]),
        ]);

        let result = index_project_with_graph(
            &repo, "proj1", temp.path(), &config, None, &analysis, None,
        ).await.unwrap();

        assert_eq!(result.refs_created, 1);

        // Verify the ref was resolved (hashPassword chunk exists in utils/hash.ts)
        let refs = repo.get_symbol_refs_to("proj1", "hashPassword").await.unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].ref_type, "import");
        // to_chunk_id should be resolved since hashPassword chunk exists
        assert!(refs[0].to_chunk_id.is_some(), "Symbol ref should be resolved");
    }

    #[tokio::test]
    async fn test_index_with_graph_idempotent() {
        let repo = create_test_repo().await;

        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("src").join("auth");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("login.ts"), "export function login() {}").unwrap();

        let config = IndexConfig {
            target_extensions: vec!["ts".to_string()],
            ignore_patterns: vec![],
            max_file_size_kb: 500,
        };

        let analysis = test_analysis(vec![
            test_module("auth", "src/auth", vec![], vec![], vec!["login.ts"]),
        ]);

        // Index twice — should not duplicate graph data
        index_project_with_graph(&repo, "proj1", temp.path(), &config, None, &analysis, None).await.unwrap();
        let result = index_project_with_graph(&repo, "proj1", temp.path(), &config, None, &analysis, None).await.unwrap();

        let modules = repo.get_modules("proj1").await.unwrap();
        assert_eq!(modules.len(), 1);
        assert_eq!(result.modules_mapped, 1);
    }
}
