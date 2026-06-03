//! Research Engine Tauri commands
//!
//! Start, pause, resume, stop research runs and send instructions.

use std::collections::HashMap;
use std::sync::Arc;

use once_cell::sync::Lazy;
use std::sync::Mutex;
use tauri::AppHandle;
use tauri::Emitter;

use venore_core::error::VenoreError;
use venore_core::research::{
    ResearchEngine, ResearchEvent, ResearchRun, ResearchStatus,
    max_workers_for_intensity,
};

use crate::state::{LazyAppState, get_state_field};
use crate::utils::{IntoStateCommandResult, StateCommandResult};
use super::dto::research::*;

// ---------------------------------------------------------------------------
// Global state for active research handles
// ---------------------------------------------------------------------------

struct ResearchHandle {
    #[allow(dead_code)]
    run_id: String,
    cancel_tx: tokio::sync::watch::Sender<bool>,
}

static ACTIVE_RESEARCH: Lazy<Mutex<HashMap<String, ResearchHandle>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn start_research(
    app: AppHandle,
    lazy_state: tauri::State<'_, LazyAppState>,
    request: StartResearchRequest,
) -> StateCommandResult<StartResearchResponse> {
    let knowledge_repo = get_state_field!(lazy_state, knowledge_repository);
    let research_repo = get_state_field!(lazy_state, research_repository);
    let llm_gateway = get_state_field!(lazy_state, llm_gateway);
    let config_store = get_state_field!(lazy_state, config_store);
    let rag_repo = get_state_field!(lazy_state, rag_repository);

    let result: Result<StartResearchResponse, VenoreError> = async {
        let knowledge_repo = knowledge_repo?;
        let research_repo = research_repo?;
        let llm_gateway = llm_gateway?;
        let config_store = config_store?;
        let rag_repo = rag_repo?;

        // Check for existing active run
        if let Some(existing) = research_repo
            .get_active_run_for_feature(&request.feature_id)
            .await?
        {
            if existing.status == "running" {
                return Err(VenoreError::InvalidParams(
                    "Research is already running for this feature".into(),
                ));
            }
        }

        // Load feature
        let feature = knowledge_repo
            .get_feature(&request.feature_id)
            .await?
            .ok_or_else(|| VenoreError::NotFound(format!("Feature {}", request.feature_id)))?;

        // Resolve Tavily API key
        let web_search_api_key: Option<String> = {
            use venore_core::traits::ApiKeyStore;
            config_store
                .get_api_key(venore_core::traits::LlmProviderType::Tavily)
                .await
                .ok()
                .flatten()
        };

        // Resolve LLM options
        let options = venore_core::llm::GatewayOptions::for_task(
            venore_core::traits::LlmTask::Chat,
        );

        // Create run
        let run_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let max_workers = max_workers_for_intensity(&feature.intensity);

        let mut run = ResearchRun {
            id: run_id.clone(),
            feature_id: feature.id.clone(),
            phase: "decomposing".to_string(),
            status: "running".to_string(),
            intensity: feature.intensity.clone(),
            max_workers,
            evaluation_round: 0,
            total_workers_spawned: 0,
            total_tool_calls: 0,
            total_tokens: 0,
            manager_model: String::new(),
            worker_model: String::new(),
            user_instructions: "[]".to_string(),
            started_at: now,
            finished_at: None,
            duration_ms: 0,
            error: None,
        };

        research_repo.create_run(&run).await?;

        // Update feature status to active
        let mut updated_feature = feature.clone();
        updated_feature.status = "active".to_string();
        updated_feature.updated_at = chrono::Utc::now().to_rfc3339();
        knowledge_repo.update_feature(&updated_feature).await.ok();

        // Create cancellation channel
        let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);

        // Store handle
        if let Ok(mut handles) = ACTIVE_RESEARCH.lock() {
            handles.insert(
                run_id.clone(),
                ResearchHandle {
                    run_id: run_id.clone(),
                    cancel_tx,
                },
            );
        }

        // Build event emitter
        let app_clone = app.clone();
        let emit: Arc<venore_core::research::ResearchEventEmitter> =
            Arc::new(Box::new(move |event: ResearchEvent| {
                let event_name = event.event_name();
                let _ = app_clone.emit(event_name, &event);
            }));

        // Build knowledge change callback
        let app_clone2 = app.clone();
        let on_knowledge_changed: Arc<dyn Fn(&str) + Send + Sync> =
            Arc::new(move |feature_id: &str| {
                let _ = app_clone2.emit(
                    "knowledge-hexagons-changed",
                    serde_json::json!({
                        "featureId": feature_id,
                        "toolName": "research-worker",
                    }),
                );
            });

        // Create engine and spawn
        let engine = ResearchEngine::new(
            knowledge_repo,
            research_repo,
            llm_gateway,
            web_search_api_key,
            Some(rag_repo),
        );

        let run_id_clone = run_id.clone();
        tokio::spawn(async move {
            engine
                .run(&mut run, &feature, options, cancel_rx, emit, on_knowledge_changed)
                .await;

            // Cleanup handle
            if let Ok(mut handles) = ACTIVE_RESEARCH.lock() {
                handles.remove(&run_id_clone);
            }
        });

        Ok(StartResearchResponse { run_id })
    }
    .await;

    result.into_state()
}

#[tauri::command]
pub async fn pause_research(run_id: String) -> StateCommandResult<()> {
    let result: Result<(), VenoreError> = {
        if let Ok(handles) = ACTIVE_RESEARCH.lock() {
            if let Some(handle) = handles.get(&run_id) {
                let _ = handle.cancel_tx.send(true);
                Ok(())
            } else {
                Err(VenoreError::NotFound(format!("Research run {run_id}")))
            }
        } else {
            Err(VenoreError::NotFound("Lock failed".into()))
        }
    };
    result.into_state()
}

#[tauri::command]
pub async fn stop_research(
    lazy_state: tauri::State<'_, LazyAppState>,
    run_id: String,
) -> StateCommandResult<()> {
    let research_repo = get_state_field!(lazy_state, research_repository);
    let result: Result<(), VenoreError> = async {
        // Signal cancellation
        if let Ok(handles) = ACTIVE_RESEARCH.lock() {
            if let Some(handle) = handles.get(&run_id) {
                let _ = handle.cancel_tx.send(true);
            }
        }

        // Update status to cancelled
        let repo = research_repo?;
        if let Some(mut run) = repo.get_run(&run_id).await? {
            run.status = ResearchStatus::Cancelled.as_str().to_string();
            run.finished_at = Some(chrono::Utc::now().to_rfc3339());
            repo.update_run(&run).await?;
        }

        Ok(())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn send_research_instruction(
    lazy_state: tauri::State<'_, LazyAppState>,
    run_id: String,
    instruction: String,
) -> StateCommandResult<()> {
    let research_repo = get_state_field!(lazy_state, research_repository);
    let result: Result<(), VenoreError> = async {
        let repo = research_repo?;
        repo.append_user_instruction(&run_id, &instruction).await?;
        Ok(())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn get_research_status(
    lazy_state: tauri::State<'_, LazyAppState>,
    feature_id: String,
) -> StateCommandResult<Option<ResearchStatusResponse>> {
    let research_repo = get_state_field!(lazy_state, research_repository);
    let result: Result<Option<ResearchStatusResponse>, VenoreError> = async {
        let repo = research_repo?;
        let run = repo.get_active_run_for_feature(&feature_id).await?;
        Ok(run.map(|r| ResearchStatusResponse {
            run_id: r.id,
            phase: r.phase,
            status: r.status,
            intensity: r.intensity,
            evaluation_round: r.evaluation_round,
            total_workers_spawned: r.total_workers_spawned,
            total_tool_calls: r.total_tool_calls,
            duration_ms: r.duration_ms,
        }))
    }
    .await;
    result.into_state()
}
