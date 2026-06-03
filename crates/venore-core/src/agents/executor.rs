//! Pipeline executor — runs a team of agents sequentially against a PR

use std::sync::Arc;
use std::time::Instant;

use serde::Serialize;
use tracing::{error, info, warn};

use crate::error::VenoreError;
use crate::llm::prelude::*;
use crate::traits::{LlmProviderType, LlmTask};
use crate::Result;
use super::models::AgentStage;
use super::pipeline::{
    PipelineRun, PipelineRunStatus, PipelineStep, PipelineStepStatus,
};
use super::repository::AgentRepository;
use super::snapshot::{self, CategorySnapshot};

// =============================================================================
// Events
// =============================================================================

pub type EventEmitter = Box<dyn Fn(PipelineEvent) + Send + Sync>;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PipelineEvent {
    #[serde(rename_all = "camelCase")]
    RunStarted {
        run_id: String,
        title: String,
        team_name: String,
    },
    #[serde(rename_all = "camelCase")]
    StepStarted {
        run_id: String,
        step_id: String,
        agent_name: String,
        stage: String,
    },
    #[serde(rename_all = "camelCase")]
    StepCompleted {
        run_id: String,
        step_id: String,
        agent_name: String,
        stage: String,
        duration_ms: u64,
        tokens: u32,
    },
    #[serde(rename_all = "camelCase")]
    StepFailed {
        run_id: String,
        step_id: String,
        agent_name: String,
        stage: String,
        error: String,
    },
    #[serde(rename_all = "camelCase")]
    ConsoleLog {
        run_id: String,
        agent_name: String,
        stage: String,
        message: String,
    },
    #[serde(rename_all = "camelCase")]
    RunCompleted {
        run_id: String,
        duration_ms: u64,
        total_tokens: u32,
    },
    #[serde(rename_all = "camelCase")]
    RunFailed {
        run_id: String,
        error: String,
    },
}

// =============================================================================
// Executor
// =============================================================================

pub struct PipelineExecutor {
    agent_repo: Arc<AgentRepository>,
    llm_gateway: Arc<LlmGateway>,
}

impl PipelineExecutor {
    pub fn new(agent_repo: Arc<AgentRepository>, llm_gateway: Arc<LlmGateway>) -> Self {
        Self {
            agent_repo,
            llm_gateway,
        }
    }

    /// Execute a pipeline: run each agent in stage order (triager → specialist → reporter).
    ///
    /// - `run` is pre-created with status Running and persisted
    /// - `profile_ids` are the team's profile IDs
    /// - `pr_context_prompt` is the assembled PR analysis prompt
    /// - `provider` / `model` come from the user's configured task settings
    /// - `emit` sends events back to the frontend
    pub async fn execute(
        &self,
        run: &mut PipelineRun,
        profile_ids: &[String],
        pr_context_prompt: &str,
        provider: LlmProviderType,
        model: &str,
        emit: &EventEmitter,
    ) {
        let pipeline_start = Instant::now();

        emit(PipelineEvent::RunStarted {
            run_id: run.id.clone(),
            title: run.title.clone(),
            team_name: run.team_name.clone(),
        });

        // Load and sort profiles by stage order
        let mut profiles = Vec::new();
        for pid in profile_ids {
            match self.agent_repo.get_profile(pid).await {
                Ok(p) if p.is_enabled => profiles.push(p),
                Ok(p) => {
                    info!(id = %p.id, name = %p.name, "Skipping disabled profile");
                }
                Err(e) => {
                    warn!(id = %pid, error = %e, "Failed to load profile, skipping");
                }
            }
        }

        // Sort: triager (0) → specialist (1) → reporter (2) → subagent (3)
        profiles.sort_by_key(|p| match p.stage {
            AgentStage::Triager => 0,
            AgentStage::Specialist => 1,
            AgentStage::Reporter => 2,
            AgentStage::SubAgent => 3,
        });

        if profiles.is_empty() {
            let err_msg = "No enabled profiles found in team".to_string();
            error!(run_id = %run.id, "{}", err_msg);
            self.fail_run(run, &err_msg, &pipeline_start, emit).await;
            return;
        }

        let mut accumulated_outputs: Vec<String> = Vec::new();
        let mut total_tokens: u32 = 0;
        let mut any_completed = false;

        for (idx, profile) in profiles.iter().enumerate() {
            let step_id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now().to_rfc3339();
            let stage_str = profile.stage.as_str().to_string();

            // Create step record
            let mut step = PipelineStep {
                id: step_id.clone(),
                run_id: run.id.clone(),
                profile_id: profile.id.clone(),
                profile_name: profile.name.clone(),
                stage: stage_str.clone(),
                status: PipelineStepStatus::Running,
                input_context: String::new(),
                output: String::new(),
                provider: profile.provider.clone(),
                model: profile.model.clone(),
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
                duration_ms: 0,
                error: None,
                step_order: idx as u32,
                started_at: now.clone(),
                finished_at: None,
            };

            if let Err(e) = self.agent_repo.create_pipeline_step(&step).await {
                warn!(error = %e, "Failed to persist pipeline step");
            }

            emit(PipelineEvent::StepStarted {
                run_id: run.id.clone(),
                step_id: step_id.clone(),
                agent_name: profile.name.clone(),
                stage: stage_str.clone(),
            });

            emit(PipelineEvent::ConsoleLog {
                run_id: run.id.clone(),
                agent_name: profile.name.clone(),
                stage: stage_str.clone(),
                message: format!(
                    "Starting {} analysis with {}/{}...",
                    stage_str, provider.as_str(), model
                ),
            });

            // Build messages: system prompt + PR context + accumulated findings
            let mut user_content = pr_context_prompt.to_string();

            // Inject historical context for reporter stage
            if profile.stage == AgentStage::Reporter {
                if let Some(ref author) = run.pr_author {
                    let history = self.build_historical_context(author, &run.project_path).await;
                    if !history.is_empty() {
                        user_content.push_str("\n\n---\n\n");
                        user_content.push_str(&history);
                    }
                }
            }

            if !accumulated_outputs.is_empty() {
                user_content.push_str("\n\n---\n\n## Previous Agent Findings\n\n");
                for (i, output) in accumulated_outputs.iter().enumerate() {
                    user_content.push_str(&format!(
                        "### Agent {} Output\n\n{}\n\n",
                        i + 1,
                        output
                    ));
                }
            }

            step.input_context = user_content.clone();

            let messages = vec![
                LlmMessage {
                    role: MessageRole::System,
                    content: profile.system_prompt.clone(),
                    tool_call_id: None,
                    tool_calls: None,
                    content_parts: None,
                },
                LlmMessage {
                    role: MessageRole::User,
                    content: user_content,
                    tool_call_id: None,
                    tool_calls: None,
                    content_parts: None,
                },
            ];

            let request = LlmRequest {
                model: String::new(), // Router overrides with resolved model
                messages,
                temperature: Some(profile.temperature),
                max_tokens: Some(profile.max_tokens_per_run),
                tools: None,
                json_schema: None,
                timeout_secs: Some(120),
                web_search: false,
            };

            // Use user's configured provider/model (resolved in start_pipeline command)
            let options = GatewayOptions::for_task(LlmTask::Analysis)
                .with_provider(provider)
                .with_model(model)
                .with_temperature(profile.temperature)
                .with_max_tokens(profile.max_tokens_per_run);

            let step_start = Instant::now();

            match self.llm_gateway.complete(request, options).await {
                Ok(response) => {
                    let step_duration = step_start.elapsed().as_millis() as u64;
                    let usage = response.usage.as_ref();
                    let prompt_tok = usage.map(|u| u.prompt_tokens).unwrap_or(0);
                    let completion_tok = usage.map(|u| u.completion_tokens).unwrap_or(0);
                    let step_tokens = usage.map(|u| u.total_tokens).unwrap_or(0);

                    step.status = PipelineStepStatus::Completed;
                    step.output = response.content.clone();
                    step.provider = response.provider.as_str().to_string();
                    step.model = response.model.clone();
                    step.prompt_tokens = prompt_tok;
                    step.completion_tokens = completion_tok;
                    step.total_tokens = step_tokens;
                    step.duration_ms = step_duration;
                    step.finished_at = Some(chrono::Utc::now().to_rfc3339());

                    if let Err(e) = self.agent_repo.update_pipeline_step(&step).await {
                        warn!(error = %e, "Failed to update pipeline step");
                    }

                    accumulated_outputs.push(response.content);
                    total_tokens += step_tokens;
                    any_completed = true;

                    emit(PipelineEvent::ConsoleLog {
                        run_id: run.id.clone(),
                        agent_name: profile.name.clone(),
                        stage: stage_str.clone(),
                        message: format!(
                            "Completed in {:.1}s — {} tokens",
                            step_duration as f64 / 1000.0,
                            step_tokens
                        ),
                    });

                    emit(PipelineEvent::StepCompleted {
                        run_id: run.id.clone(),
                        step_id: step_id.clone(),
                        agent_name: profile.name.clone(),
                        stage: stage_str,
                        duration_ms: step_duration,
                        tokens: step_tokens,
                    });
                }
                Err(e) => {
                    let step_duration = step_start.elapsed().as_millis() as u64;
                    let err_msg = format!("{}", e);

                    step.status = PipelineStepStatus::Failed;
                    step.error = Some(err_msg.clone());
                    step.duration_ms = step_duration;
                    step.finished_at = Some(chrono::Utc::now().to_rfc3339());

                    if let Err(e) = self.agent_repo.update_pipeline_step(&step).await {
                        warn!(error = %e, "Failed to update failed pipeline step");
                    }

                    emit(PipelineEvent::ConsoleLog {
                        run_id: run.id.clone(),
                        agent_name: profile.name.clone(),
                        stage: stage_str.clone(),
                        message: format!("Failed: {}", err_msg),
                    });

                    emit(PipelineEvent::StepFailed {
                        run_id: run.id.clone(),
                        step_id: step_id.clone(),
                        agent_name: profile.name.clone(),
                        stage: stage_str,
                        error: err_msg,
                    });

                    // Fail-soft: continue to next agent
                }
            }
        }

        // Finalize run
        let total_duration = pipeline_start.elapsed().as_millis() as u64;
        run.duration_ms = total_duration;
        run.total_tokens = total_tokens;
        run.finished_at = Some(chrono::Utc::now().to_rfc3339());

        if any_completed {
            run.status = PipelineRunStatus::Completed;

            // Save analysis snapshots (fail-soft)
            self.save_run_snapshots(run, &accumulated_outputs).await;

            if let Err(e) = self.agent_repo.update_pipeline_run(run).await {
                warn!(error = %e, "Failed to update completed pipeline run");
            }
            emit(PipelineEvent::RunCompleted {
                run_id: run.id.clone(),
                duration_ms: total_duration,
                total_tokens,
            });
            info!(
                run_id = %run.id,
                duration_ms = total_duration,
                total_tokens,
                "Pipeline completed"
            );
        } else {
            self.fail_run(run, "All agents failed", &pipeline_start, emit)
                .await;
        }
    }

    /// Parse the reporter output and save category snapshots + author stats.
    async fn save_run_snapshots(&self, run: &PipelineRun, accumulated_outputs: &[String]) {
        let author = match run.pr_author.as_ref() {
            Some(a) => a,
            None => return,
        };

        // Take the last output (reporter)
        let reporter_output = match accumulated_outputs.last() {
            Some(o) => o,
            None => return,
        };

        let report = match snapshot::parse_report_from_output(reporter_output) {
            Some(r) => r,
            None => {
                warn!(run_id = %run.id, "Could not parse report for snapshots");
                return;
            }
        };

        let now = chrono::Utc::now().to_rfc3339();

        // Create category snapshots
        let snapshots: Vec<CategorySnapshot> = report.categories.iter().map(|cat| {
            CategorySnapshot {
                id: uuid::Uuid::new_v4().to_string(),
                run_id: run.id.clone(),
                project_path: run.project_path.clone(),
                author_login: author.clone(),
                category_name: cat.name.clone(),
                score: cat.score,
                status: cat.status.clone(),
                findings_count: cat.findings_count,
                overall_score: report.overall_score,
                created_at: now.clone(),
            }
        }).collect();

        if let Err(e) = self.agent_repo.save_category_snapshots(&snapshots).await {
            warn!(error = %e, "Failed to save category snapshots");
        }

        // Update author stats (running average)
        let existing = self.agent_repo.get_author_stats(author, &run.project_path).await.ok().flatten();
        let (total_runs, avg_score) = match existing {
            Some(ref s) => {
                let new_total = s.total_runs + 1;
                let new_avg = (s.avg_overall_score * s.total_runs as f64 + report.overall_score as f64) / new_total as f64;
                (new_total, new_avg)
            }
            None => (1, report.overall_score as f64),
        };

        let stats = super::snapshot::AuthorStats {
            login: author.clone(),
            project_path: run.project_path.clone(),
            avatar_url: run.pr_author_avatar.clone().unwrap_or_default(),
            total_runs,
            avg_overall_score: avg_score,
            last_overall_score: report.overall_score,
            last_run_at: now,
        };

        if let Err(e) = self.agent_repo.upsert_author_stats(&stats).await {
            warn!(error = %e, "Failed to upsert author stats");
        }
    }

    /// Build historical context string for the reporter prompt.
    async fn build_historical_context(&self, author: &str, project_path: &str) -> String {
        let mut sections = Vec::new();

        // Author stats
        if let Ok(Some(stats)) = self.agent_repo.get_author_stats(author, project_path).await {
            sections.push(format!(
                "- Author @{} has {} previous analyses, average overall score: {:.0}/100",
                stats.login, stats.total_runs, stats.avg_overall_score,
            ));
        }

        // Author category averages
        if let Ok(avgs) = self.agent_repo.get_author_category_averages(author, project_path).await {
            if !avgs.is_empty() {
                let lines: Vec<String> = avgs.iter().map(|a| {
                    format!("  - {}: avg {:.0} ({} runs)", a.category_name, a.avg_score, a.run_count)
                }).collect();
                sections.push(format!("- Author category averages:\n{}", lines.join("\n")));
            }
        }

        // Project category averages
        if let Ok(avgs) = self.agent_repo.get_project_category_averages(project_path).await {
            if !avgs.is_empty() {
                let lines: Vec<String> = avgs.iter().map(|a| {
                    format!("  - {}: avg {:.0} ({} runs)", a.category_name, a.avg_score, a.run_count)
                }).collect();
                sections.push(format!("- Project-wide category averages:\n{}", lines.join("\n")));
            }
        }

        if sections.is_empty() {
            return String::new();
        }

        format!(
            "## Historical Context\n\n{}\n\nUse these historical averages to contextualize your report. Mention notable patterns — whether the author is improving, declining, or consistently strong/weak in specific categories compared to the project average.",
            sections.join("\n")
        )
    }

    async fn fail_run(
        &self,
        run: &mut PipelineRun,
        error: &str,
        start: &Instant,
        emit: &EventEmitter,
    ) {
        run.status = PipelineRunStatus::Failed;
        run.duration_ms = start.elapsed().as_millis() as u64;
        run.finished_at = Some(chrono::Utc::now().to_rfc3339());

        if let Err(e) = self.agent_repo.update_pipeline_run(run).await {
            warn!(error = %e, "Failed to update failed pipeline run");
        }

        emit(PipelineEvent::RunFailed {
            run_id: run.id.clone(),
            error: error.to_string(),
        });

        error!(run_id = %run.id, error, "Pipeline failed");
    }
}

// =============================================================================
// Pipeline orchestration
// =============================================================================

/// Dependencies for starting a pipeline.
pub struct PipelineDeps {
    pub repo: Arc<AgentRepository>,
    pub gateway: Arc<LlmGateway>,
    pub config_store: Arc<crate::infrastructure::config::DefaultConfigStore>,
    pub rag_repo: Option<Arc<crate::rag::RagRepository>>,
    pub project_repo: Option<Arc<crate::project::ProjectRepository>>,
}

/// Request to start a pipeline analysis.
pub struct PipelineRequest {
    pub project_path: String,
    pub pr_number: u64,
    pub pr_title: String,
    pub team_id: Option<String>,
}

/// Orchestrate a full pipeline: resolve settings, fetch PR data, create run,
/// and return `(run_id, PipelineRun, profile_ids, prompt, provider, model)`.
///
/// The caller is responsible for spawning async execution and wiring events.
pub async fn prepare_pipeline(
    deps: &PipelineDeps,
    request: &PipelineRequest,
) -> Result<(PipelineRun, Vec<String>, String, LlmProviderType, String)> {
    use crate::github::{auth, client::GitHubClient, pr_analyzer, pr_detail, pulls, repo as gh_repo};
    use crate::github::pr_analyzer::AnalysisDepthLevel;
    use std::path::Path;

    // Resolve user's configured provider/model
    let task_settings = deps.config_store.get_task_settings(LlmTask::Analysis).await?;
    let configured_provider = task_settings.provider;
    let configured_model = task_settings.model.clone();
    info!(
        provider = %configured_provider.as_str(),
        model = %configured_model,
        "Pipeline using configured provider"
    );

    // Read depth level
    let depth_str = deps.config_store
        .get_app_setting("pr_analysis.depth_level")
        .await?
        .unwrap_or_else(|| "normal".to_string());
    let depth = AnalysisDepthLevel::from_str(&depth_str)
        .unwrap_or(AnalysisDepthLevel::Normal);
    info!(depth = depth.as_str(), "Analysis depth level");

    // Resolve team
    let team = if let Some(ref team_id) = request.team_id {
        deps.repo.get_team(team_id).await?
    } else {
        let teams = deps.repo.list_teams().await?;
        teams.into_iter().next().ok_or_else(|| {
            VenoreError::NotFound("No teams available. Create a team first.".into())
        })?
    };

    // Fetch PR data via GitHub API
    let token = auth::resolve_token().await?
        .ok_or(VenoreError::GitHubAuthRequired)?;
    let client = GitHubClient::new(token);

    let project_path = Path::new(&request.project_path);
    let (owner, repo_name) = gh_repo::detect_github_repo(project_path)?
        .ok_or_else(|| VenoreError::GitHubRepoNotDetected(request.project_path.clone()))?;

    let pr = pulls::get_pull_request(&client, &owner, &repo_name, request.pr_number).await?;
    let files = pr_detail::list_pr_files(&client, &owner, &repo_name, request.pr_number, 1, 100).await?;

    // Assemble PR context
    let mut analysis_ctx = pr_analyzer::assemble_pr_context(
        project_path,
        &pr.title,
        pr.body.as_deref(),
        &pr.user.login,
        &pr.head.ref_name,
        &pr.base.ref_name,
        &files,
    );

    // Enrich context based on depth level
    if depth != AnalysisDepthLevel::Normal {
        let project_id = if let Some(ref proj_repo) = deps.project_repo {
            proj_repo.find_by_path(&request.project_path).await.ok().flatten().map(|p| p.id.to_string())
        } else {
            None
        };

        pr_analyzer::enrich_context(
            &mut analysis_ctx,
            depth,
            project_path,
            deps.rag_repo.as_deref(),
            project_id.as_deref(),
        ).await;
    }

    let pr_prompt = pr_analyzer::build_pr_analysis_prompt(&analysis_ctx, depth);

    // Create pipeline run
    let run_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let run = PipelineRun {
        id: run_id.clone(),
        team_id: team.id.clone(),
        team_name: team.name.clone(),
        task_type: "pr-analysis".to_string(),
        title: format!("PR #{} — {}", request.pr_number, request.pr_title),
        status: PipelineRunStatus::Running,
        pr_number: Some(request.pr_number),
        project_path: request.project_path.clone(),
        started_at: now.clone(),
        finished_at: None,
        duration_ms: 0,
        total_tokens: 0,
        created_at: now,
        pr_author: Some(pr.user.login.clone()),
        pr_author_avatar: Some(pr.user.avatar_url.clone()),
        pr_additions: pr.additions,
        pr_deletions: pr.deletions,
        pr_changed_files: pr.changed_files,
        depth_level: Some(depth.as_str().to_string()),
    };

    deps.repo.create_pipeline_run(&run).await?;

    let profile_ids = team.profile_ids.clone();

    info!(
        run_id = %run_id,
        team = %team.name,
        pr = request.pr_number,
        "Pipeline prepared"
    );

    Ok((run, profile_ids, pr_prompt, configured_provider, configured_model))
}
