//! Project-related Tauri commands

use std::path::Path;
use std::sync::Arc;

use serde::Serialize;

use venore_core::analysis::AnalysisOutput;
use venore_core::context::{file_storage as layers_file, hash_storage};
use venore_core::error::VenoreError;
use venore_core::memory::file_storage as memory_file;
use venore_core::project::{ProjectRepository, ProjectService};

use crate::state::LazyAppState;
use crate::utils::{IntoStateCommandResult, StateCommandResult};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectResponse {
    pub id: String,
    pub name: String,
    pub path: String,
    pub project_type: String,
}

/// Inventory of what was restored from a committed `.venore/` snapshot when
/// the user opens an existing project. Drives the success banner in the UI:
/// "Restored 40 modules, 79 layers, project memory ✓".
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenExistingReport {
    pub project: ProjectResponse,
    pub has_memory: bool,
    pub has_analysis: bool,
    pub has_ocean_layout: bool,
    pub layer_count: u32,
    pub module_count: u32,
    pub hashed_module_count: u32,
}

fn get_project_repo(lazy: &LazyAppState) -> Result<Arc<ProjectRepository>, VenoreError> {
    let guard = lazy.get();
    match guard.as_ref() {
        Some(state) => Ok(Arc::clone(&state.project_repository)),
        None => Err(VenoreError::NotFound("Backend not initialized".into())),
    }
}

/// Register a project: read/create .venore/project.json + upsert in SQLite
#[tauri::command]
pub async fn register_project(
    lazy_state: tauri::State<'_, LazyAppState>,
    project_path: String,
) -> StateCommandResult<ProjectResponse> {
    tracing::info!("register_project: {}", project_path);

    let repo = get_project_repo(&lazy_state);
    let result: Result<ProjectResponse, VenoreError> = async {
        let repo = repo?;

        // Read or auto-generate identity from .venore/project.json
        let identity = ProjectService::read_or_create_identity(
            std::path::Path::new(&project_path)
        )?;

        // Upsert in SQLite (updates path + last_opened_at if ID already exists)
        repo.upsert(
            &identity.id.to_string(),
            &identity.name,
            &project_path,
            &identity.created_at.to_rfc3339(),
            "code",
        ).await?;

        tracing::info!("Registered project: {} ({})", identity.name, identity.id);

        Ok(ProjectResponse {
            id: identity.id.to_string(),
            name: identity.name,
            path: project_path,
            project_type: "code".to_string(),
        })
    }.await;

    result.into_state()
}

/// Open an already-Venorized project from disk.
///
/// Strict: refuses to open a folder that doesn't have `.venore/project.json`.
/// The wizard is the only path that should create that file. Returns a
/// report enumerating what was found in `.venore/` so the UI can show a
/// "restored X / Y / Z" confirmation banner.
#[tauri::command]
pub async fn open_existing_project(
    lazy_state: tauri::State<'_, LazyAppState>,
    project_path: String,
) -> StateCommandResult<OpenExistingReport> {
    tracing::info!("open_existing_project: {}", project_path);

    let repo = get_project_repo(&lazy_state);
    let result: Result<OpenExistingReport, VenoreError> = async {
        let repo = repo?;
        let path = Path::new(&project_path);

        // Strict read — never auto-creates .venore/. NotFound bubbles up so
        // the UI can render a "this folder isn't a Venore project" message
        // and route the user to the wizard.
        let identity = ProjectService::read_identity_strict(path)?;

        repo.upsert(
            &identity.id.to_string(),
            &identity.name,
            &project_path,
            &identity.created_at.to_rfc3339(),
            "code",
        )
        .await?;

        // Inventory of portable artifacts. Each check is best-effort: a
        // missing file means a partial snapshot, not a fatal error. The UI
        // surfaces the counts so the user can decide whether to fill gaps
        // (e.g. re-run the wizard to backfill memory).
        let has_memory = memory_file::exists(path);

        let analysis = AnalysisOutput::load_from_disk(path).ok().flatten();
        let module_count = analysis
            .as_ref()
            .map(|a| a.modules.len() as u32)
            .unwrap_or(0);
        let has_analysis = analysis.is_some();

        let layer_count = layers_file::load(path, &identity.id.to_string())?
            .map(|v| v.len() as u32)
            .unwrap_or(0);

        let hashed_module_count = hash_storage::load(path)?
            .map(|v| v.len() as u32)
            .unwrap_or(0);

        // NOTE: staleness detection (re-hashing every module's source tree) used
        // to run here synchronously and froze open for ~15s on large projects.
        // It now runs passively in the background via the Staleness Current,
        // which sweeps module nodes and emits `ocean-stale-module` badge events.
        let has_ocean_layout = path.join(".venore").join("ocean-layout.json").exists();

        tracing::info!(
            project = %identity.name,
            modules = module_count,
            layers = layer_count,
            hashed = hashed_module_count,
            "Opened existing project"
        );

        Ok(OpenExistingReport {
            project: ProjectResponse {
                id: identity.id.to_string(),
                name: identity.name,
                path: project_path,
                project_type: "code".to_string(),
            },
            has_memory,
            has_analysis,
            has_ocean_layout,
            layer_count,
            module_count,
            hashed_module_count,
        })
    }
    .await;

    result.into_state()
}

/// Create a knowledge project (no codebase, auto-generates folder)
#[tauri::command]
pub async fn create_knowledge_project(
    lazy_state: tauri::State<'_, LazyAppState>,
    name: String,
    description: String,
) -> StateCommandResult<ProjectResponse> {
    tracing::info!("create_knowledge_project: {}", name);

    let repo = get_project_repo(&lazy_state);
    let config_dir = {
        let guard = lazy_state.get();
        match guard.as_ref() {
            Some(state) => state.config_dir.clone(),
            None => return Err(()),
        }
    };

    let result: Result<ProjectResponse, VenoreError> = async {
        let repo = repo?;

        let id = uuid::Uuid::new_v4();
        let now = chrono::Utc::now().to_rfc3339();

        // Create knowledge project folder: {config_dir}/knowledge/{id}/
        let knowledge_dir = config_dir.join("knowledge").join(id.to_string());
        std::fs::create_dir_all(&knowledge_dir)
            .map_err(|e| VenoreError::Io(format!(
                "Failed to create knowledge dir at {}: {}", knowledge_dir.display(), e
            )))?;

        let path = knowledge_dir.to_string_lossy().to_string();

        // Upsert with type "knowledge"
        repo.upsert(
            &id.to_string(),
            &name,
            &path,
            &now,
            "knowledge",
        ).await?;

        tracing::info!("Created knowledge project: {} ({}), description: {}", name, id, description);

        Ok(ProjectResponse {
            id: id.to_string(),
            name,
            path,
            project_type: "knowledge".to_string(),
        })
    }.await;

    result.into_state()
}

/// Get a project by ID
#[tauri::command]
pub async fn get_project(
    lazy_state: tauri::State<'_, LazyAppState>,
    id: String,
) -> StateCommandResult<ProjectResponse> {
    tracing::info!("get_project: {}", id);

    let repo = get_project_repo(&lazy_state);
    let result: Result<ProjectResponse, VenoreError> = async {
        let repo = repo?;
        let project = repo.find_by_id(&id).await?
            .ok_or_else(|| VenoreError::NotFound(format!("Project not found: {}", id)))?;

        Ok(ProjectResponse {
            id: project.id.to_string(),
            name: project.name,
            path: project.path,
            project_type: project.project_type,
        })
    }.await;

    result.into_state()
}

/// List all registered projects
#[tauri::command]
pub async fn list_projects(
    lazy_state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<Vec<ProjectResponse>> {
    tracing::info!("list_projects");

    let repo = get_project_repo(&lazy_state);
    let result: Result<Vec<ProjectResponse>, VenoreError> = async {
        let repo = repo?;
        let projects = repo.list().await?;

        Ok(projects.into_iter().map(|p| ProjectResponse {
            id: p.id.to_string(),
            name: p.name,
            path: p.path,
            project_type: p.project_type,
        }).collect())
    }.await;

    result.into_state()
}
