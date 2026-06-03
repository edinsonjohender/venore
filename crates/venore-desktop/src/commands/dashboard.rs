//! Tauri commands for the Project Dashboard.
//!
//! Thin wrapper that loads analysis data and delegates to venore-core's
//! dashboard module for context status computation.

use tracing::{debug, info};

use crate::utils::CommandResult;
use super::dto::dashboard::{
    GetProjectDashboardRequest, ModuleSummaryDto, ProjectDashboardResponse, ProjectStatsDto,
};
use venore_core::analysis::AnalysisOutput;
use venore_core::dashboard::{self, ContextStatus};
use venore_core::error::VenoreError;
use venore_core::wizard::WizardSessionManager;

// =============================================================================
// Commands
// =============================================================================

/// Get project dashboard data (module list with context status).
///
/// Tries in-memory WizardSession first, then falls back to the persisted
/// analysis file on disk (`.venore/analysis-output.json`).
#[tauri::command]
pub async fn get_project_dashboard(
    request: GetProjectDashboardRequest,
) -> CommandResult<ProjectDashboardResponse> {
    info!(project_path = %request.project_path, "Getting project dashboard");

    let result: Result<ProjectDashboardResponse, VenoreError> = (|| {
        // 1) Try in-memory session cache first
        let from_session = try_get_analysis_from_session(&request.project_path);

        // 2) Fall back to disk
        let analysis = match from_session {
            Some(a) => a,
            None => {
                debug!("No session cache, loading analysis from disk");
                AnalysisOutput::load_from_disk(std::path::Path::new(&request.project_path))
                    .map_err(|e| VenoreError::FileReadError(format!("{}", e)))?
                    .ok_or_else(|| {
                        VenoreError::NotFound(format!(
                            "Analysis for '{}' (run the wizard first)",
                            request.project_path
                        ))
                    })?
            }
        };

        // 3) Build dashboard
        let project_path = std::path::Path::new(&request.project_path);
        let data = dashboard::build_dashboard(&analysis, project_path);

        // 4) Convert to DTOs
        let modules: Vec<ModuleSummaryDto> = data
            .modules
            .iter()
            .map(|m| {
                let status_str = match m.context_status {
                    ContextStatus::Fresh => "fresh",
                    ContextStatus::Stale => "stale",
                    ContextStatus::Missing => "missing",
                };

                ModuleSummaryDto {
                    name: m.name.clone(),
                    path: m.path.clone(),
                    file_count: m.file_count,
                    dependency_count: m.dependency_count,
                    dependent_count: m.dependent_count,
                    context_status: status_str.to_string(),
                    generated_at: m.context_meta.as_ref().and_then(|cm| cm.generated_at.clone()),
                    model: m.context_meta.as_ref().and_then(|cm| cm.model.clone()),
                    provider: m.context_meta.as_ref().and_then(|cm| cm.provider.clone()),
                    context_path: m.context_meta.as_ref().map(|cm| cm.context_path.clone()),
                    files: m.files.clone(),
                }
            })
            .collect();

        let stats = ProjectStatsDto {
            total_modules: data.stats.total_modules,
            total_connections: data.stats.total_connections,
            fresh_count: data.stats.fresh_count,
            stale_count: data.stats.stale_count,
            missing_count: data.stats.missing_count,
        };

        info!(
            total = stats.total_modules,
            fresh = stats.fresh_count,
            stale = stats.stale_count,
            missing = stats.missing_count,
            "Dashboard built"
        );

        Ok(ProjectDashboardResponse {
            stats,
            modules,
            orphan_files: data.orphan_files,
        })
    })();

    result.into()
}

// =============================================================================
// Helpers
// =============================================================================

/// Try to get AnalysisOutput from the in-memory WizardSession.
/// Returns None if no session or no cached analysis (never errors).
fn try_get_analysis_from_session(project_path: &str) -> Option<AnalysisOutput> {
    let session_mgr = WizardSessionManager::global();
    let guard = session_mgr.lock().ok()?;
    let session = guard.get(project_path)?;
    session.get_cached_analysis().ok().cloned()
}
