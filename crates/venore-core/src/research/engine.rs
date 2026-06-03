//! Research Engine — orchestrates Manager + parallel Workers across phases
//!
//! The engine is the main entry point for running a research investigation.
//! It coordinates the Manager (LLM-driven decomposition and evaluation) with
//! parallel Worker agents (agentic loops with tools).

use std::sync::Arc;
use std::time::Instant;

use tokio::sync::watch;

use crate::knowledge::{KnowledgeFeature, KnowledgeRepository};
use crate::llm::{GatewayOptions, LlmGateway};
use crate::tools::ToolExecutionContext;
use crate::Result;

use super::manager;
use super::repository::ResearchRepository;
use super::types::*;
use super::worker;

/// Maximum evaluation rounds before forcing conclusion
const MAX_EVALUATION_ROUNDS: i32 = 3;

/// The Research Engine orchestrates multi-agent research for a knowledge feature.
pub struct ResearchEngine {
    knowledge_repo: Arc<KnowledgeRepository>,
    research_repo: Arc<ResearchRepository>,
    llm_gateway: Arc<LlmGateway>,
    web_search_api_key: Option<String>,
    rag_repo: Option<Arc<crate::rag::RagRepository>>,
}

impl ResearchEngine {
    pub fn new(
        knowledge_repo: Arc<KnowledgeRepository>,
        research_repo: Arc<ResearchRepository>,
        llm_gateway: Arc<LlmGateway>,
        web_search_api_key: Option<String>,
        rag_repo: Option<Arc<crate::rag::RagRepository>>,
    ) -> Self {
        Self {
            knowledge_repo,
            research_repo,
            llm_gateway,
            web_search_api_key,
            rag_repo,
        }
    }

    /// Run a full research investigation for a feature.
    ///
    /// This is meant to be called inside a `tokio::spawn` from the Tauri command.
    /// The `cancel_rx` allows pausing/stopping from the frontend.
    /// The `emit` callback sends events to the frontend for real-time updates.
    /// The `on_knowledge_changed` callback notifies the frontend when hexagons/evidence change.
    pub async fn run(
        &self,
        run: &mut ResearchRun,
        feature: &KnowledgeFeature,
        options: GatewayOptions,
        cancel_rx: watch::Receiver<bool>,
        emit: Arc<ResearchEventEmitter>,
        on_knowledge_changed: Arc<dyn Fn(&str) + Send + Sync>,
    ) {
        let start = Instant::now();

        emit(ResearchEvent::RunStarted {
            run_id: run.id.clone(),
            feature_id: feature.id.clone(),
        });

        let result = self
            .run_phases(run, feature, &options, &cancel_rx, &emit, &on_knowledge_changed)
            .await;

        let duration_ms = start.elapsed().as_millis() as i64;
        run.duration_ms = duration_ms;

        match result {
            Ok(()) => {
                if *cancel_rx.borrow() {
                    run.status = ResearchStatus::Paused.as_str().to_string();
                    run.phase = ResearchPhase::Paused.as_str().to_string();
                    emit(ResearchEvent::RunPaused {
                        run_id: run.id.clone(),
                    });
                } else {
                    run.status = ResearchStatus::Completed.as_str().to_string();
                    run.phase = ResearchPhase::Completed.as_str().to_string();
                    run.finished_at = Some(chrono::Utc::now().to_rfc3339());
                    emit(ResearchEvent::RunCompleted {
                        run_id: run.id.clone(),
                        duration_ms: duration_ms as u64,
                    });
                }
            }
            Err(e) => {
                run.status = ResearchStatus::Failed.as_str().to_string();
                run.phase = ResearchPhase::Failed.as_str().to_string();
                run.error = Some(e.to_string());
                run.finished_at = Some(chrono::Utc::now().to_rfc3339());
                emit(ResearchEvent::RunFailed {
                    run_id: run.id.clone(),
                    error: e.to_string(),
                });
            }
        }

        // Persist final state
        if let Err(e) = self.research_repo.update_run(run).await {
            tracing::error!(error = %e, "Failed to persist final research run state");
        }
    }

    /// Run through all phases: Decompose → (Investigate ↔ Evaluate)* → Conclude
    async fn run_phases(
        &self,
        run: &mut ResearchRun,
        feature: &KnowledgeFeature,
        options: &GatewayOptions,
        cancel_rx: &watch::Receiver<bool>,
        emit: &Arc<ResearchEventEmitter>,
        on_knowledge_changed: &Arc<dyn Fn(&str) + Send + Sync>,
    ) -> Result<()> {
        // Phase 1: Decompose
        if *cancel_rx.borrow() {
            return Ok(());
        }
        self.transition_phase(run, ResearchPhase::Decomposing, emit).await;

        emit(ResearchEvent::ManagerThinking {
            run_id: run.id.clone(),
            message: "Decomposing research topic into investigation points...".into(),
        });

        let (hexagon_ids, mut assignments) = manager::decompose(
            &self.llm_gateway,
            &self.knowledge_repo,
            &feature.id,
            &feature.name,
            &feature.description,
            &feature.objective,
            &feature.intensity,
            feature.max_hexagons_per_phase,
            run.max_workers,
            options,
        )
        .await?;

        // Notify frontend about new hexagons
        on_knowledge_changed(&feature.id);

        tracing::info!(hexagons = hexagon_ids.len(), "Decomposition complete");

        // Phase 2-3 loop: Investigate → Evaluate (max rounds)
        loop {
            if *cancel_rx.borrow() {
                return Ok(());
            }

            // Investigate
            self.transition_phase(run, ResearchPhase::Investigating, emit).await;
            self.run_workers(run, &assignments, feature, options, cancel_rx, emit, on_knowledge_changed)
                .await?;

            if *cancel_rx.borrow() {
                return Ok(());
            }

            // Evaluate
            run.evaluation_round += 1;
            self.transition_phase(run, ResearchPhase::Evaluating, emit).await;

            emit(ResearchEvent::ManagerThinking {
                run_id: run.id.clone(),
                message: format!(
                    "Evaluating research progress (round {}/{})...",
                    run.evaluation_round, MAX_EVALUATION_ROUNDS
                ),
            });

            let user_instructions: Vec<String> =
                serde_json::from_str(&run.user_instructions).unwrap_or_default();

            let eval = manager::evaluate(
                &self.llm_gateway,
                &self.knowledge_repo,
                &feature.id,
                &feature.name,
                &feature.objective,
                run.evaluation_round,
                MAX_EVALUATION_ROUNDS,
                &user_instructions,
                options,
            )
            .await?;

            // Persist evaluation state
            self.research_repo.update_run(run).await.ok();

            match eval.decision.as_str() {
                "conclude" => break,
                "continue" => {
                    if eval.new_hexagons.is_empty() || run.evaluation_round >= MAX_EVALUATION_ROUNDS
                    {
                        break;
                    }
                    // Create gap hexagons and build new assignments
                    let (_, new_assignments) = manager::create_gap_hexagons(
                        &self.knowledge_repo,
                        &feature.id,
                        &eval.new_hexagons,
                        &eval.assignments,
                        run.max_workers,
                        "discover",
                    )
                    .await?;
                    on_knowledge_changed(&feature.id);
                    assignments = new_assignments;
                }
                "next_phase" => {
                    let next_phase = eval
                        .phase_transition
                        .as_deref()
                        .unwrap_or("define");

                    // Advance non-dead-end hexagons to the next phase
                    let hexagons = self
                        .knowledge_repo
                        .list_hexagons_by_feature(&feature.id)
                        .await?;
                    for mut hex in hexagons {
                        if !hex.is_dead_end {
                            hex.phase = next_phase.to_string();
                            hex.updated_at = chrono::Utc::now().to_rfc3339();
                            self.knowledge_repo.update_hexagon(&hex).await.ok();
                        }
                    }
                    on_knowledge_changed(&feature.id);

                    if run.evaluation_round >= MAX_EVALUATION_ROUNDS {
                        break;
                    }

                    // Build new assignments for the next phase
                    let active_hexagons: Vec<String> = self
                        .knowledge_repo
                        .list_hexagons_by_feature(&feature.id)
                        .await?
                        .into_iter()
                        .filter(|h| !h.is_dead_end && h.phase == next_phase)
                        .map(|h| h.id)
                        .collect();

                    if active_hexagons.is_empty() {
                        break;
                    }

                    // Simple round-robin for phase advancement
                    let num_workers = (run.max_workers as usize).min(active_hexagons.len());
                    let mut new_assignments: Vec<WorkerAssignment> = (0..num_workers)
                        .map(|i| WorkerAssignment {
                            worker_id: format!("worker-{}-p{}", i + 1, next_phase),
                            hexagon_ids: Vec::new(),
                            instructions: format!(
                                "Phase: {next_phase}. Deepen your investigation on assigned hexagons."
                            ),
                            max_iterations: 5,
                            max_tool_calls: 20,
                        })
                        .collect();
                    for (i, id) in active_hexagons.iter().enumerate() {
                        new_assignments[i % num_workers].hexagon_ids.push(id.clone());
                    }
                    assignments = new_assignments;
                }
                _ => break, // Unknown decision → conclude
            }
        }

        // Phase 4: Conclude
        if *cancel_rx.borrow() {
            return Ok(());
        }
        self.transition_phase(run, ResearchPhase::Concluding, emit).await;

        emit(ResearchEvent::ManagerThinking {
            run_id: run.id.clone(),
            message: "Generating final research report...".into(),
        });

        // Update feature status
        let mut updated_feature = feature.clone();
        updated_feature.status = "completed".to_string();
        updated_feature.updated_at = chrono::Utc::now().to_rfc3339();
        self.knowledge_repo
            .update_feature(&updated_feature)
            .await
            .ok();

        Ok(())
    }

    /// Spawn workers in parallel and await all results
    async fn run_workers(
        &self,
        run: &mut ResearchRun,
        assignments: &[WorkerAssignment],
        feature: &KnowledgeFeature,
        options: &GatewayOptions,
        cancel_rx: &watch::Receiver<bool>,
        emit: &Arc<ResearchEventEmitter>,
        on_knowledge_changed: &Arc<dyn Fn(&str) + Send + Sync>,
    ) -> Result<()> {
        if assignments.is_empty() {
            return Ok(());
        }

        let llm_tools = crate::tools::definitions::knowledge_research_tools();

        let mut handles = Vec::new();

        for assignment in assignments {
            run.total_workers_spawned += 1;

            emit(ResearchEvent::WorkerStarted {
                run_id: run.id.clone(),
                worker_id: assignment.worker_id.clone(),
                hexagon_ids: assignment.hexagon_ids.clone(),
            });

            // Build tool context for this worker
            let tool_ctx = ToolExecutionContext {
                terminal_id: None,
                project_path: None,
                rag_repository: self.rag_repo.clone(),
                logbook_repository: None,
                project_id: None,
                embedding_provider: None,
                embedding_api_key: None,
                web_search_api_key: self.web_search_api_key.clone(),
                llm_gateway: Some(self.llm_gateway.clone()),
                mesh_follow_up: None,
                knowledge_repo: Some(self.knowledge_repo.clone()),
                knowledge_feature_id: Some(feature.id.clone()),
                model: None,
                session_id: None,
                allowed_tools: None,
            };

            let assignment = assignment.clone();
            let gateway = self.llm_gateway.clone();
            let opts = options.clone();
            let tools = llm_tools.clone();
            let rx = cancel_rx.clone();
            let emit_clone = emit.clone();
            let kc = on_knowledge_changed.clone();
            let fid = feature.id.clone();
            let rid = run.id.clone();

            let handle = tokio::spawn(async move {
                worker::run_research_worker(
                    assignment,
                    gateway,
                    tool_ctx,
                    opts,
                    tools,
                    rx,
                    emit_clone,
                    kc,
                    fid,
                    rid,
                )
                .await
            });
            handles.push(handle);
        }

        // Await all workers
        let results = futures::future::join_all(handles).await;

        // Aggregate results
        for result in results {
            match result {
                Ok(worker_result) => {
                    run.total_tool_calls += worker_result.tool_calls as i32;
                    run.total_tokens += worker_result.tokens_used as i32;
                }
                Err(e) => {
                    tracing::error!(error = %e, "Worker task panicked");
                }
            }
        }

        // Persist intermediate state
        self.research_repo.update_run(run).await.ok();

        Ok(())
    }

    /// Transition to a new phase, persist, and emit event
    async fn transition_phase(
        &self,
        run: &mut ResearchRun,
        new_phase: ResearchPhase,
        emit: &Arc<ResearchEventEmitter>,
    ) {
        let from = run.phase.clone();
        let to = new_phase.as_str().to_string();
        run.phase = to.clone();
        self.research_repo.update_run(run).await.ok();

        emit(ResearchEvent::PhaseTransition {
            run_id: run.id.clone(),
            from,
            to,
        });
    }
}
