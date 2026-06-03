//! RAG Tauri commands
//!
//! Exposes code indexing, search, and index status to the frontend.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri::Emitter;

use venore_core::analysis::AnalysisOutput;
use venore_core::analysis::pipeline::{RunAnalysisConfig, run_analysis};
use venore_core::error::VenoreError;
use venore_core::project::ProjectService;
use venore_core::rag::{self, IndexConfig, RagRepository, SearchResult};
use venore_core::rag::{GraphQueryResult, ModuleDep, ModuleInfo, SymbolRef};

use crate::state::LazyAppState;
use crate::utils::{IntoStateCommandResult, StateCommandResult};

// ============================================================================
// DTOs
// ============================================================================

#[derive(Deserialize)]
pub struct IndexProjectRequest {
    pub project_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexProjectResponse {
    pub indexed: u32,
    pub skipped: u32,
    pub removed: u32,
    pub graph_populated: bool,
    pub modules_mapped: Option<u32>,
    pub deps_created: Option<u32>,
    pub refs_created: Option<u32>,
}

#[derive(Deserialize)]
pub struct SearchCodeRequest {
    pub project_id: String,
    pub query: String,
    pub max_results: Option<u32>,
    pub max_context_chars: Option<usize>,
}

#[derive(Serialize)]
pub struct SearchCodeResponse {
    pub results: Vec<SearchResultDto>,
}

#[derive(Serialize)]
pub struct SearchResultDto {
    pub name: String,
    pub chunk_type: String,
    pub content: String,
    pub relative_path: String,
    pub line_start: u32,
    pub line_end: u32,
    pub score: f64,
}

impl From<SearchResult> for SearchResultDto {
    fn from(r: SearchResult) -> Self {
        Self {
            name: r.chunk.name,
            chunk_type: r.chunk.chunk_type,
            content: r.chunk.content,
            relative_path: r.chunk.relative_path,
            line_start: r.chunk.line_start,
            line_end: r.chunk.line_end,
            score: r.score,
        }
    }
}

#[derive(Serialize)]
pub struct IndexStatusDto {
    pub project_id: String,
    pub is_indexed: bool,
    pub total_files: u32,
    pub total_chunks: u32,
    pub last_indexed_at: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct RagIndexProgressPayload {
    pub project_id: String,
    pub current: u32,
    pub total: u32,
    pub current_file: String,
    pub status: String,
}

// ============================================================================
// HELPER
// ============================================================================

fn get_rag_repo(lazy: &LazyAppState) -> Result<Arc<RagRepository>, VenoreError> {
    let guard = lazy.get();
    match guard.as_ref() {
        Some(state) => Ok(Arc::clone(&state.rag_repository)),
        None => Err(VenoreError::NotFound("Backend not initialized".into())),
    }
}

// ============================================================================
// COMMANDS
// ============================================================================

/// Index project code into RAG database with progress events
#[tauri::command]
pub async fn index_project_code(
    app: AppHandle,
    lazy_state: tauri::State<'_, LazyAppState>,
    request: IndexProjectRequest,
) -> StateCommandResult<IndexProjectResponse> {
    tracing::info!("index_project_code: {}", request.project_path);

    let repo = get_rag_repo(&lazy_state);
    let result: Result<IndexProjectResponse, VenoreError> = async {
        let repo = repo?;
        let project_path = std::path::Path::new(&request.project_path);

        // Resolve project_id from .venore/project.json
        let identity = ProjectService::read_or_create_identity(project_path)?;
        let project_id = identity.id.to_string();

        let config = IndexConfig::default();

        let app_clone = app.clone();
        let pid = project_id.clone();

        let progress_cb = move |event: rag::IndexProgressEvent| {
            let _ = app_clone.emit("rag-index-progress", RagIndexProgressPayload {
                project_id: pid.clone(),
                current: event.current,
                total: event.total,
                current_file: event.current_file.clone(),
                status: event.status.clone(),
            });
        };

        // Opportunistic: if analysis output exists, use graph-aware indexing
        let analysis = AnalysisOutput::load_from_disk(project_path)?;

        if let Some(ref analysis) = analysis {
            tracing::info!("index_project_code: analysis output found, using graph indexing");
            let graph_result = rag::index_project_with_graph(
                &repo,
                &project_id,
                project_path,
                &config,
                Some(&progress_cb),
                analysis,
                None,
            ).await?;

            Ok(IndexProjectResponse {
                indexed: graph_result.indexed,
                skipped: graph_result.skipped,
                removed: graph_result.removed,
                graph_populated: true,
                modules_mapped: Some(graph_result.modules_mapped),
                deps_created: Some(graph_result.deps_created),
                refs_created: Some(graph_result.refs_created),
            })
        } else {
            tracing::info!("index_project_code: no analysis output, using plain indexing");
            let (indexed, skipped, removed) = rag::index_project(
                &repo,
                &project_id,
                project_path,
                &config,
                Some(&progress_cb),
            ).await?;

            Ok(IndexProjectResponse {
                indexed,
                skipped,
                removed,
                graph_populated: false,
                modules_mapped: None,
                deps_created: None,
                refs_created: None,
            })
        }
    }.await;

    result.into_state()
}

/// Search project code index
#[tauri::command]
pub async fn search_project_code(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: SearchCodeRequest,
) -> StateCommandResult<SearchCodeResponse> {
    let repo = get_rag_repo(&lazy_state);
    let result: Result<SearchCodeResponse, VenoreError> = async {
        let repo = repo?;

        let results = rag::search_code(
            &repo,
            &request.project_id,
            &request.query,
            request.max_results.unwrap_or(10),
            request.max_context_chars.unwrap_or(8000),
        ).await?;

        Ok(SearchCodeResponse {
            results: results.into_iter().map(|r| r.into()).collect(),
        })
    }.await;

    result.into_state()
}

/// Get RAG index status for a project
#[tauri::command]
pub async fn get_rag_index_status(
    lazy_state: tauri::State<'_, LazyAppState>,
    project_id: String,
) -> StateCommandResult<IndexStatusDto> {
    let repo = get_rag_repo(&lazy_state);
    let result: Result<IndexStatusDto, VenoreError> = async {
        let repo = repo?;
        let status = repo.get_index_status(&project_id).await?;

        Ok(IndexStatusDto {
            project_id: status.project_id,
            is_indexed: status.is_indexed,
            total_files: status.total_files,
            total_chunks: status.total_chunks,
            last_indexed_at: status.last_indexed_at,
        })
    }.await;

    result.into_state()
}

// ============================================================================
// GRAPH QUERY DTOs
// ============================================================================

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphQueryRequest {
    pub project_id: String,
    pub query: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphQueryResponse {
    pub query_type: String,
    pub modules: Vec<ModuleInfoDto>,
    pub symbols: Vec<SearchResultDto>,
    pub dependencies: Vec<ModuleDepDto>,
    pub refs: Vec<SymbolRefDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleInfoDto {
    pub module_name: String,
    pub module_path: String,
    pub file_count: u32,
}

impl From<ModuleInfo> for ModuleInfoDto {
    fn from(m: ModuleInfo) -> Self {
        Self {
            module_name: m.module_name,
            module_path: m.module_path,
            file_count: m.file_count,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleDepDto {
    pub from_module: String,
    pub to_module: String,
    pub dep_type: String,
}

impl From<ModuleDep> for ModuleDepDto {
    fn from(d: ModuleDep) -> Self {
        Self {
            from_module: d.from_module,
            to_module: d.to_module,
            dep_type: d.dep_type,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolRefDto {
    pub from_chunk_id: String,
    pub to_chunk_id: Option<String>,
    pub to_symbol_name: String,
    pub to_file_path: Option<String>,
    pub ref_type: String,
    pub line_number: Option<u32>,
}

impl From<SymbolRef> for SymbolRefDto {
    fn from(r: SymbolRef) -> Self {
        Self {
            from_chunk_id: r.from_chunk_id,
            to_chunk_id: r.to_chunk_id,
            to_symbol_name: r.to_symbol_name,
            to_file_path: r.to_file_path,
            ref_type: r.ref_type,
            line_number: r.line_number,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleDetailDto {
    pub module: ModuleInfoDto,
    pub files: Vec<String>,
    pub dependencies: Vec<ModuleDepDto>,
    pub dependents: Vec<ModuleDepDto>,
}

impl From<GraphQueryResult> for GraphQueryResponse {
    fn from(r: GraphQueryResult) -> Self {
        Self {
            query_type: r.query_type,
            modules: r.modules.into_iter().map(|m| m.into()).collect(),
            symbols: r.chunks.into_iter().map(|c| SearchResultDto {
                name: c.name,
                chunk_type: c.chunk_type,
                content: c.content,
                relative_path: c.relative_path,
                line_start: c.line_start,
                line_end: c.line_end,
                score: 0.0,
            }).collect(),
            dependencies: r.deps.into_iter().map(|d| d.into()).collect(),
            refs: r.refs.into_iter().map(|r| r.into()).collect(),
        }
    }
}

// ============================================================================
// GRAPH COMMANDS
// ============================================================================

/// Query the code graph with natural language or structured queries
#[tauri::command]
pub async fn query_code_graph(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: GraphQueryRequest,
) -> StateCommandResult<GraphQueryResponse> {
    let repo = get_rag_repo(&lazy_state);
    let result: Result<GraphQueryResponse, VenoreError> = async {
        let repo = repo?;

        // Try to classify as a structural query first
        let graph_query = rag::classify_query(&request.query);

        match graph_query {
            Some(query) => {
                let result = rag::execute_graph_query(&repo, &request.project_id, query).await?;
                Ok(result.into())
            }
            None => {
                // Fall back to standard FTS5 search
                let results = rag::search_code(
                    &repo,
                    &request.project_id,
                    &request.query,
                    10,
                    8000,
                ).await?;

                Ok(GraphQueryResponse {
                    query_type: "fts_fallback".to_string(),
                    modules: vec![],
                    symbols: results.into_iter().map(|r| r.into()).collect(),
                    dependencies: vec![],
                    refs: vec![],
                })
            }
        }
    }.await;

    result.into_state()
}

/// Get all modules for a project
#[tauri::command]
pub async fn get_project_modules(
    lazy_state: tauri::State<'_, LazyAppState>,
    project_id: String,
) -> StateCommandResult<Vec<ModuleInfoDto>> {
    let repo = get_rag_repo(&lazy_state);
    let result: Result<Vec<ModuleInfoDto>, VenoreError> = async {
        let repo = repo?;
        let modules = repo.get_modules(&project_id).await?;
        Ok(modules.into_iter().map(|m| m.into()).collect())
    }.await;

    result.into_state()
}

/// Get detailed info for a specific module
#[tauri::command]
pub async fn get_module_detail(
    lazy_state: tauri::State<'_, LazyAppState>,
    project_id: String,
    module_name: String,
) -> StateCommandResult<ModuleDetailDto> {
    let repo = get_rag_repo(&lazy_state);
    let result: Result<ModuleDetailDto, VenoreError> = async {
        let repo = repo?;

        let all_modules = repo.get_modules(&project_id).await?;
        let module = all_modules.into_iter()
            .find(|m| m.module_name == module_name)
            .ok_or_else(|| VenoreError::NotFound(format!("Module '{}' not found", module_name)))?;

        let files = repo.get_module_files(&project_id, &module_name).await?;
        let file_paths: Vec<String> = files.iter().map(|f| f.relative_path.clone()).collect();

        let deps = repo.get_module_deps(&project_id, &module_name).await?;
        let dependents = repo.get_module_dependents(&project_id, &module_name).await?;

        Ok(ModuleDetailDto {
            module: module.into(),
            files: file_paths,
            dependencies: deps.into_iter().map(|d| d.into()).collect(),
            dependents: dependents.into_iter().map(|d| d.into()).collect(),
        })
    }.await;

    result.into_state()
}

// ============================================================================
// ANALYZE + INDEX PIPELINE
// ============================================================================

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeAndIndexRequest {
    pub project_path: String,
    pub depth: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeAndIndexResponse {
    pub modules_detected: u32,
    pub indexed: u32,
    pub skipped: u32,
    pub removed: u32,
    pub modules_mapped: u32,
    pub deps_created: u32,
    pub refs_created: u32,
}

/// Run analysis pipeline + RAG indexing + graph population in one step.
///
/// Use this when the wizard hasn't been run but you want full graph support.
#[tauri::command]
pub async fn analyze_and_index_project(
    app: AppHandle,
    lazy_state: tauri::State<'_, LazyAppState>,
    request: AnalyzeAndIndexRequest,
) -> StateCommandResult<AnalyzeAndIndexResponse> {
    tracing::info!("analyze_and_index_project: {}", request.project_path);

    let repo = get_rag_repo(&lazy_state);
    let result: Result<AnalyzeAndIndexResponse, VenoreError> = async {
        let repo = repo?;
        let project_path = std::path::PathBuf::from(&request.project_path);

        // Parse depth
        let depth = match request.depth.as_deref() {
            Some("minimal") => venore_core::analysis::AnalysisDepth::Minimal,
            Some("detailed") => venore_core::analysis::AnalysisDepth::Detailed,
            Some("expert") => venore_core::analysis::AnalysisDepth::Expert,
            _ => venore_core::analysis::AnalysisDepth::Normal,
        };

        // Emit progress: analyzing
        let _ = app.emit("rag-index-progress", RagIndexProgressPayload {
            project_id: String::new(),
            current: 0,
            total: 0,
            current_file: String::new(),
            status: "analyzing".to_string(),
        });

        // 1. Run analysis pipeline
        let analysis_config = RunAnalysisConfig {
            project_path: project_path.clone(),
            depth,
            ..RunAnalysisConfig::default()
        };

        let analysis = run_analysis(analysis_config).await?;
        let modules_detected = analysis.modules.len() as u32;

        // 2. Resolve project identity
        let identity = ProjectService::read_or_create_identity(&project_path)?;
        let project_id = identity.id.to_string();

        // 3. Index + populate graph
        let app_clone = app.clone();
        let pid = project_id.clone();

        let progress_cb = move |event: rag::IndexProgressEvent| {
            let _ = app_clone.emit("rag-index-progress", RagIndexProgressPayload {
                project_id: pid.clone(),
                current: event.current,
                total: event.total,
                current_file: event.current_file.clone(),
                status: "indexing".to_string(),
            });
        };

        let index_config = IndexConfig::default();

        let graph_result = rag::index_project_with_graph(
            &repo,
            &project_id,
            &project_path,
            &index_config,
            Some(&progress_cb),
            &analysis,
            None,
        ).await?;

        Ok(AnalyzeAndIndexResponse {
            modules_detected,
            indexed: graph_result.indexed,
            skipped: graph_result.skipped,
            removed: graph_result.removed,
            modules_mapped: graph_result.modules_mapped,
            deps_created: graph_result.deps_created,
            refs_created: graph_result.refs_created,
        })
    }.await;

    result.into_state()
}
