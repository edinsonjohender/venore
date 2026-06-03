//! Context generation Tauri commands

use std::path::Path;

use serde::{Deserialize, Serialize};

use venore_core::context::hash_storage;
use venore_core::error::VenoreError;

use crate::state::LazyAppState;
use crate::utils::{CommandResult, IntoStateCommandResult, StateCommandResult};

#[derive(Deserialize)]
pub struct GenerateContextRequest {
    pub island_id: String,
    pub provider: String,
}

#[derive(Serialize)]
pub struct GenerateContextResponse {
    pub content: String,
    pub tokens_used: Option<u32>,
}

/// Genera contexto con LLM
#[tauri::command]
pub async fn generate_context(
    _state: tauri::State<'_, LazyAppState>,
    _request: GenerateContextRequest,
) -> StateCommandResult<GenerateContextResponse> {
    tracing::info!("generate_context");
    // TODO: call venore-core to generate
    Ok(CommandResult::ok(GenerateContextResponse {
        content: "# Generated context\n\nExample".to_string(),
        tokens_used: Some(150),
    }))
}

// =============================================================================
// Stale-module detection (portable code-hashes)
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StaleModuleDto {
    pub module_name: String,
    pub module_path: String,
    pub stored_hash: String,
    pub current_hash: String,
    pub missing_on_disk: bool,
}

impl From<hash_storage::StaleModule> for StaleModuleDto {
    fn from(s: hash_storage::StaleModule) -> Self {
        Self {
            module_name: s.module_name,
            module_path: s.module_path,
            stored_hash: s.stored_hash,
            current_hash: s.current_hash,
            missing_on_disk: s.missing_on_disk,
        }
    }
}

/// Compares the per-module fingerprints stored in
/// `<project>/.venore/code-hashes.json` against fresh hashes computed from
/// the working tree. Returns the modules whose code has drifted since the
/// snapshot was taken.
///
/// Returns an empty vec when:
///   - the project has no `code-hashes.json` (snapshot not yet written),
///   - or the project's code matches the stored snapshot.
///
/// Project-path based (not id) because the canvas already has the path and
/// staleness detection itself never touches SQLite.
#[tauri::command]
pub async fn get_stale_modules(
    _lazy_state: tauri::State<'_, LazyAppState>,
    project_path: String,
) -> StateCommandResult<Vec<StaleModuleDto>> {
    let result: Result<Vec<StaleModuleDto>, VenoreError> = async {
        let stale = hash_storage::detect_stale_modules(Path::new(&project_path))?;
        Ok(stale.into_iter().map(Into::into).collect())
    }
    .await;
    result.into_state()
}
