//! Agent Tauri commands
//!
//! CRUD operations for agent profiles, teams, and pipeline execution.

use std::sync::Arc;

use tauri::{AppHandle, Emitter};
use tracing::info;

use venore_core::agents::{
    AgentProfile, AgentRule, AgentStage, AgentTeam, Severity,
    ToolCategory, ToolDefinition, ChatMode,
    PipelineExecutor, PipelineEvent,
};
use venore_core::error::VenoreError;
use venore_core::github::pr_analyzer::AnalysisDepthLevel;

use crate::state::{get_state_field, LazyAppState};
use crate::utils::{IntoStateCommandResult, StateCommandResult};

use super::dto::agents::*;
use super::dto::pipeline::*;

// =============================================================================
// Profile commands
// =============================================================================

#[tauri::command]
pub async fn list_agent_profiles(
    lazy_state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<Vec<AgentProfileDto>> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<Vec<AgentProfileDto>, VenoreError> = async {
        let repo = repo?;
        let profiles = repo.list_profiles().await?;
        Ok(profiles.into_iter().map(|p| p.into()).collect())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn get_agent_profile(
    lazy_state: tauri::State<'_, LazyAppState>,
    id: String,
) -> StateCommandResult<AgentProfileDto> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<AgentProfileDto, VenoreError> = async {
        let repo = repo?;
        let profile = repo.get_profile(&id).await?;
        Ok(profile.into())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn create_agent_profile(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: CreateAgentProfileRequest,
) -> StateCommandResult<AgentProfileDto> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<AgentProfileDto, VenoreError> = async {
        let repo = repo?;
        let now = chrono::Utc::now().to_rfc3339();
        let id = uuid::Uuid::new_v4().to_string();

        let stage = AgentStage::from_str(&request.stage)
            .ok_or_else(|| VenoreError::InvalidParams(format!("Invalid stage: {}", request.stage)))?;

        let profile = AgentProfile {
            id,
            name: request.name,
            description: request.description,
            stage,
            system_prompt: request.system_prompt,
            provider: request.provider,
            model: request.model,
            temperature: request.temperature,
            is_template: false,
            is_enabled: request.is_enabled.unwrap_or(true),
            rules_json: request.rules_json.unwrap_or_else(|| "[]".into()),
            criteria_json: request.criteria_json.unwrap_or_else(|| "[]".into()),
            tools_json: request.tools_json.unwrap_or_else(|| "[]".into()),
            max_tokens_per_run: request.max_tokens_per_run.unwrap_or(30000),
            created_at: now.clone(),
            updated_at: now,
        };

        repo.create_profile(&profile).await?;
        Ok(profile.into())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn update_agent_profile(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: UpdateAgentProfileRequest,
) -> StateCommandResult<AgentProfileDto> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<AgentProfileDto, VenoreError> = async {
        let repo = repo?;
        let mut profile = repo.get_profile(&request.id).await?;

        if let Some(name) = request.name { profile.name = name; }
        if let Some(desc) = request.description { profile.description = desc; }
        if let Some(stage_str) = request.stage {
            profile.stage = AgentStage::from_str(&stage_str)
                .ok_or_else(|| VenoreError::InvalidParams(format!("Invalid stage: {}", stage_str)))?;
        }
        if let Some(sp) = request.system_prompt { profile.system_prompt = sp; }
        if let Some(prov) = request.provider { profile.provider = prov; }
        if let Some(model) = request.model { profile.model = model; }
        if let Some(temp) = request.temperature { profile.temperature = temp; }
        if let Some(enabled) = request.is_enabled { profile.is_enabled = enabled; }
        if let Some(rules) = request.rules_json { profile.rules_json = rules; }
        if let Some(criteria) = request.criteria_json { profile.criteria_json = criteria; }
        if let Some(tools) = request.tools_json { profile.tools_json = tools; }
        if let Some(tokens) = request.max_tokens_per_run { profile.max_tokens_per_run = tokens; }

        profile.updated_at = chrono::Utc::now().to_rfc3339();
        repo.update_profile(&profile).await?;
        Ok(profile.into())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn delete_agent_profile(
    lazy_state: tauri::State<'_, LazyAppState>,
    id: String,
) -> StateCommandResult<()> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<(), VenoreError> = async {
        let repo = repo?;
        repo.delete_profile(&id).await?;
        Ok(())
    }
    .await;
    result.into_state()
}

// =============================================================================
// Team commands
// =============================================================================

#[tauri::command]
pub async fn list_agent_teams(
    lazy_state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<Vec<AgentTeamDto>> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<Vec<AgentTeamDto>, VenoreError> = async {
        let repo = repo?;
        let teams = repo.list_teams().await?;
        Ok(teams.into_iter().map(|t| t.into()).collect())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn get_agent_team(
    lazy_state: tauri::State<'_, LazyAppState>,
    id: String,
) -> StateCommandResult<AgentTeamDto> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<AgentTeamDto, VenoreError> = async {
        let repo = repo?;
        let team = repo.get_team(&id).await?;
        Ok(team.into())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn create_agent_team(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: CreateAgentTeamRequest,
) -> StateCommandResult<AgentTeamDto> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<AgentTeamDto, VenoreError> = async {
        let repo = repo?;
        let now = chrono::Utc::now().to_rfc3339();
        let id = uuid::Uuid::new_v4().to_string();

        let team = AgentTeam {
            id,
            name: request.name,
            description: request.description,
            profile_ids: request.profile_ids,
            is_template: false,
            created_at: now.clone(),
            updated_at: now,
        };

        repo.create_team(&team).await?;
        Ok(team.into())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn update_agent_team(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: UpdateAgentTeamRequest,
) -> StateCommandResult<AgentTeamDto> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<AgentTeamDto, VenoreError> = async {
        let repo = repo?;
        let mut team = repo.get_team(&request.id).await?;

        if let Some(name) = request.name { team.name = name; }
        if let Some(desc) = request.description { team.description = desc; }
        if let Some(ids) = request.profile_ids { team.profile_ids = ids; }

        team.updated_at = chrono::Utc::now().to_rfc3339();
        repo.update_team(&team).await?;
        Ok(team.into())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn delete_agent_team(
    lazy_state: tauri::State<'_, LazyAppState>,
    id: String,
) -> StateCommandResult<()> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<(), VenoreError> = async {
        let repo = repo?;
        repo.delete_team(&id).await?;
        Ok(())
    }
    .await;
    result.into_state()
}

// =============================================================================
// Rule commands
// =============================================================================

#[tauri::command]
pub async fn list_agent_rules(
    lazy_state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<Vec<AgentRuleDto>> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<Vec<AgentRuleDto>, VenoreError> = async {
        let repo = repo?;
        let rules = repo.list_rules().await?;
        Ok(rules.into_iter().map(|r| r.into()).collect())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn get_agent_rule(
    lazy_state: tauri::State<'_, LazyAppState>,
    id: String,
) -> StateCommandResult<AgentRuleDto> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<AgentRuleDto, VenoreError> = async {
        let repo = repo?;
        let rule = repo.get_rule(&id).await?;
        Ok(rule.into())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn create_agent_rule(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: CreateAgentRuleRequest,
) -> StateCommandResult<AgentRuleDto> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<AgentRuleDto, VenoreError> = async {
        let repo = repo?;
        let now = chrono::Utc::now().to_rfc3339();
        let id = uuid::Uuid::new_v4().to_string();

        let severity = Severity::from_str(&request.severity)
            .ok_or_else(|| VenoreError::InvalidParams(format!("Invalid severity: {}", request.severity)))?;

        let rule = AgentRule {
            id,
            name: request.name,
            description: request.description,
            scope: request.scope,
            severity,
            is_active: request.is_active.unwrap_or(true),
            is_template: false,
            created_at: now.clone(),
            updated_at: now,
        };

        repo.create_rule(&rule).await?;
        Ok(rule.into())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn update_agent_rule(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: UpdateAgentRuleRequest,
) -> StateCommandResult<AgentRuleDto> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<AgentRuleDto, VenoreError> = async {
        let repo = repo?;
        let mut rule = repo.get_rule(&request.id).await?;

        if let Some(name) = request.name { rule.name = name; }
        if let Some(desc) = request.description { rule.description = desc; }
        if let Some(scope) = request.scope { rule.scope = scope; }
        if let Some(sev_str) = request.severity {
            rule.severity = Severity::from_str(&sev_str)
                .ok_or_else(|| VenoreError::InvalidParams(format!("Invalid severity: {}", sev_str)))?;
        }
        if let Some(active) = request.is_active { rule.is_active = active; }

        rule.updated_at = chrono::Utc::now().to_rfc3339();
        repo.update_rule(&rule).await?;
        Ok(rule.into())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn delete_agent_rule(
    lazy_state: tauri::State<'_, LazyAppState>,
    id: String,
) -> StateCommandResult<()> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<(), VenoreError> = async {
        let repo = repo?;
        repo.delete_rule(&id).await?;
        Ok(())
    }
    .await;
    result.into_state()
}

// =============================================================================
// Tool Category commands
// =============================================================================

#[tauri::command]
pub async fn list_tool_categories(
    lazy_state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<Vec<ToolCategoryDto>> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<Vec<ToolCategoryDto>, VenoreError> = async {
        let repo = repo?;
        let categories = repo.list_tool_categories().await?;
        Ok(categories.into_iter().map(|c| c.into()).collect())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn get_tool_category(
    lazy_state: tauri::State<'_, LazyAppState>,
    id: String,
) -> StateCommandResult<ToolCategoryDto> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<ToolCategoryDto, VenoreError> = async {
        let repo = repo?;
        let category = repo.get_tool_category(&id).await?;
        Ok(category.into())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn create_tool_category(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: CreateToolCategoryRequest,
) -> StateCommandResult<ToolCategoryDto> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<ToolCategoryDto, VenoreError> = async {
        let repo = repo?;
        let now = chrono::Utc::now().to_rfc3339();
        let id = uuid::Uuid::new_v4().to_string();

        let category = ToolCategory {
            id,
            name: request.name,
            description: request.description,
            icon: request.icon,
            color: request.color,
            display_order: request.display_order.unwrap_or(99),
            is_template: false,
            created_at: now.clone(),
            updated_at: now,
        };

        repo.create_tool_category(&category).await?;
        Ok(category.into())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn update_tool_category(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: UpdateToolCategoryRequest,
) -> StateCommandResult<ToolCategoryDto> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<ToolCategoryDto, VenoreError> = async {
        let repo = repo?;
        let mut category = repo.get_tool_category(&request.id).await?;

        if let Some(name) = request.name { category.name = name; }
        if let Some(desc) = request.description { category.description = desc; }
        if let Some(icon) = request.icon { category.icon = icon; }
        if let Some(color) = request.color { category.color = color; }
        if let Some(order) = request.display_order { category.display_order = order; }

        category.updated_at = chrono::Utc::now().to_rfc3339();
        repo.update_tool_category(&category).await?;
        Ok(category.into())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn delete_tool_category(
    lazy_state: tauri::State<'_, LazyAppState>,
    id: String,
) -> StateCommandResult<()> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<(), VenoreError> = async {
        let repo = repo?;
        repo.delete_tool_category(&id).await?;
        Ok(())
    }
    .await;
    result.into_state()
}

// =============================================================================
// Tool Definition commands
// =============================================================================

#[tauri::command]
pub async fn list_tool_definitions(
    lazy_state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<Vec<ToolDefinitionDto>> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<Vec<ToolDefinitionDto>, VenoreError> = async {
        let repo = repo?;
        let tools = repo.list_tool_definitions().await?;
        Ok(tools.into_iter().map(|t| t.into()).collect())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn get_tool_definition(
    lazy_state: tauri::State<'_, LazyAppState>,
    id: String,
) -> StateCommandResult<ToolDefinitionDto> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<ToolDefinitionDto, VenoreError> = async {
        let repo = repo?;
        let tool = repo.get_tool_definition(&id).await?;
        Ok(tool.into())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn create_tool_definition(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: CreateToolDefinitionRequest,
) -> StateCommandResult<ToolDefinitionDto> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<ToolDefinitionDto, VenoreError> = async {
        let repo = repo?;
        let now = chrono::Utc::now().to_rfc3339();
        let id = uuid::Uuid::new_v4().to_string();

        let tool = ToolDefinition {
            id,
            name: request.name,
            description: request.description,
            category_id: request.category_id,
            parameters_json: request.parameters_json.unwrap_or_else(|| "{}".into()),
            is_read_only: request.is_read_only.unwrap_or(false),
            is_enabled: request.is_enabled.unwrap_or(true),
            is_template: false,
            created_at: now.clone(),
            updated_at: now,
        };

        repo.create_tool_definition(&tool).await?;
        Ok(tool.into())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn update_tool_definition(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: UpdateToolDefinitionRequest,
) -> StateCommandResult<ToolDefinitionDto> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<ToolDefinitionDto, VenoreError> = async {
        let repo = repo?;
        let mut tool = repo.get_tool_definition(&request.id).await?;

        if let Some(name) = request.name { tool.name = name; }
        if let Some(desc) = request.description { tool.description = desc; }
        if let Some(cat) = request.category_id { tool.category_id = cat; }
        if let Some(params) = request.parameters_json { tool.parameters_json = params; }
        if let Some(ro) = request.is_read_only { tool.is_read_only = ro; }
        if let Some(en) = request.is_enabled { tool.is_enabled = en; }

        tool.updated_at = chrono::Utc::now().to_rfc3339();
        repo.update_tool_definition(&tool).await?;
        Ok(tool.into())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn delete_tool_definition(
    lazy_state: tauri::State<'_, LazyAppState>,
    id: String,
) -> StateCommandResult<()> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<(), VenoreError> = async {
        let repo = repo?;
        repo.delete_tool_definition(&id).await?;
        Ok(())
    }
    .await;
    result.into_state()
}

// =============================================================================
// Pipeline commands
// =============================================================================


#[tauri::command]
pub async fn start_pipeline(
    app: AppHandle,
    lazy_state: tauri::State<'_, LazyAppState>,
    request: StartPipelineRequest,
) -> StateCommandResult<StartPipelineResponse> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let gateway = get_state_field!(&lazy_state, llm_gateway);
    let config_store = get_state_field!(&lazy_state, config_store);
    let rag_repository = get_state_field!(&lazy_state, rag_repository);
    let project_repository = get_state_field!(&lazy_state, project_repository);

    let result: Result<StartPipelineResponse, VenoreError> = async {
        let repo = repo?;
        let gateway = gateway?;
        let config_store = config_store?;

        let deps = venore_core::agents::PipelineDeps {
            repo: Arc::clone(&repo),
            gateway,
            config_store,
            rag_repo: rag_repository.ok(),
            project_repo: project_repository.ok(),
        };
        let core_request = venore_core::agents::PipelineRequest {
            project_path: request.project_path.clone(),
            pr_number: request.pr_number,
            pr_title: request.pr_title.clone(),
            team_id: request.team_id.clone(),
        };

        let (mut run, profile_ids, pr_prompt, configured_provider, configured_model) =
            venore_core::agents::prepare_pipeline(&deps, &core_request).await?;

        let run_id = run.id.clone();
        let app_clone = app.clone();

        tokio::spawn(async move {
            let executor = PipelineExecutor::new(repo, deps.gateway);

            let emit: Box<dyn Fn(PipelineEvent) + Send + Sync> = Box::new(move |event| {
                let event_name = match &event {
                    PipelineEvent::RunStarted { .. } => "pipeline:run-started",
                    PipelineEvent::StepStarted { .. } => "pipeline:step-started",
                    PipelineEvent::StepCompleted { .. } => "pipeline:step-completed",
                    PipelineEvent::StepFailed { .. } => "pipeline:step-failed",
                    PipelineEvent::ConsoleLog { .. } => "pipeline:console",
                    PipelineEvent::RunCompleted { .. } => "pipeline:run-completed",
                    PipelineEvent::RunFailed { .. } => "pipeline:run-failed",
                };

                if let Err(e) = app_clone.emit(event_name, &event) {
                    tracing::warn!(error = %e, event = event_name, "Failed to emit pipeline event");
                }
            });

            executor.execute(
                &mut run,
                &profile_ids,
                &pr_prompt,
                configured_provider,
                &configured_model,
                &emit,
            ).await;
        });

        Ok(StartPipelineResponse { run_id })
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn list_pipeline_runs(
    lazy_state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<Vec<PipelineRunDto>> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<Vec<PipelineRunDto>, VenoreError> = async {
        let repo = repo?;
        let runs = repo.list_pipeline_runs().await?;
        Ok(runs.into_iter().map(|r| r.into()).collect())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn get_pipeline_steps(
    lazy_state: tauri::State<'_, LazyAppState>,
    run_id: String,
) -> StateCommandResult<Vec<PipelineStepDto>> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<Vec<PipelineStepDto>, VenoreError> = async {
        let repo = repo?;
        let steps = repo.list_pipeline_steps(&run_id).await?;
        Ok(steps.into_iter().map(|s| s.into()).collect())
    }
    .await;
    result.into_state()
}

// =============================================================================
// Analysis Depth commands
// =============================================================================

#[tauri::command]
pub async fn get_analysis_depth(
    lazy_state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<String> {
    let config_store = get_state_field!(&lazy_state, config_store);
    let result: Result<String, VenoreError> = async {
        let config_store = config_store?;
        let depth = config_store
            .get_app_setting("pr_analysis.depth_level")
            .await?
            .unwrap_or_else(|| "normal".to_string());
        Ok(depth)
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn set_analysis_depth(
    lazy_state: tauri::State<'_, LazyAppState>,
    depth: String,
) -> StateCommandResult<()> {
    let config_store = get_state_field!(&lazy_state, config_store);
    let result: Result<(), VenoreError> = async {
        let config_store = config_store?;
        // Validate the depth value
        AnalysisDepthLevel::from_str(&depth)
            .ok_or_else(|| VenoreError::InvalidParams(format!("Invalid depth level: {}. Must be minimal, normal, detailed, or expert.", depth)))?;
        config_store.set_app_setting("pr_analysis.depth_level", &depth).await?;
        info!(depth = %depth, "Analysis depth level updated");
        Ok(())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn get_run_analysis_context(
    lazy_state: tauri::State<'_, LazyAppState>,
    run_id: String,
) -> StateCommandResult<RunAnalysisContextDto> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<RunAnalysisContextDto, VenoreError> = async {
        let repo = repo?;
        let run = repo.get_pipeline_run(&run_id).await?;

        let (author_stats, author_avgs, project_avgs) = if let Some(ref author) = run.pr_author {
            let stats = repo.get_author_stats(author, &run.project_path).await?;
            let a_avgs = repo.get_author_category_averages(author, &run.project_path).await?;
            let p_avgs = repo.get_project_category_averages(&run.project_path).await?;
            (stats, a_avgs, p_avgs)
        } else {
            (None, Vec::new(), Vec::new())
        };

        Ok(RunAnalysisContextDto {
            run: run.into(),
            author_stats: author_stats.map(|s| s.into()),
            author_category_averages: author_avgs.into_iter().map(|a| a.into()).collect(),
            project_category_averages: project_avgs.into_iter().map(|a| a.into()).collect(),
        })
    }
    .await;
    result.into_state()
}

// =============================================================================
// Chat Mode commands
// =============================================================================

#[tauri::command]
pub async fn list_chat_modes(
    lazy_state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<Vec<ChatModeDto>> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<Vec<ChatModeDto>, VenoreError> = async {
        let repo = repo?;
        let modes = repo.list_chat_modes().await?;
        Ok(modes.into_iter().map(|m| m.into()).collect())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn get_chat_mode(
    lazy_state: tauri::State<'_, LazyAppState>,
    id: String,
) -> StateCommandResult<ChatModeDto> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<ChatModeDto, VenoreError> = async {
        let repo = repo?;
        let mode = repo.get_chat_mode(&id).await?;
        Ok(mode.into())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn create_chat_mode(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: CreateChatModeRequest,
) -> StateCommandResult<ChatModeDto> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<ChatModeDto, VenoreError> = async {
        let repo = repo?;
        let now = chrono::Utc::now().to_rfc3339();
        let mode = ChatMode {
            id: uuid::Uuid::new_v4().to_string(),
            name: request.name,
            description: request.description.unwrap_or_default(),
            category_ids: request.category_ids.unwrap_or_default(),
            tool_ids: request.tool_ids.unwrap_or_default(),
            sub_agent_ids: request.sub_agent_ids.unwrap_or_default(),
            rule_ids: request.rule_ids.unwrap_or_default(),
            prompt_id: request.prompt_id,
            is_template: false,
            is_default_for_kind: request.is_default_for_kind,
            created_at: now.clone(),
            updated_at: now,
        };
        repo.create_chat_mode(&mode).await?;
        Ok(mode.into())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn update_chat_mode(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: UpdateChatModeRequest,
) -> StateCommandResult<ChatModeDto> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<ChatModeDto, VenoreError> = async {
        let repo = repo?;
        let mut mode = repo.get_chat_mode(&request.id).await?;
        if let Some(n) = request.name { mode.name = n; }
        if let Some(d) = request.description { mode.description = d; }
        if let Some(c) = request.category_ids { mode.category_ids = c; }
        if let Some(t) = request.tool_ids { mode.tool_ids = t; }
        if let Some(s) = request.sub_agent_ids { mode.sub_agent_ids = s; }
        if let Some(r) = request.rule_ids { mode.rule_ids = r; }
        if let Some(p) = request.prompt_id { mode.prompt_id = Some(p); }
        if let Some(k) = request.is_default_for_kind {
            mode.is_default_for_kind = if k.is_empty() { None } else { Some(k) };
        }
        mode.updated_at = chrono::Utc::now().to_rfc3339();
        repo.update_chat_mode(&mode).await?;
        Ok(mode.into())
    }
    .await;
    result.into_state()
}

#[tauri::command]
pub async fn delete_chat_mode(
    lazy_state: tauri::State<'_, LazyAppState>,
    id: String,
) -> StateCommandResult<()> {
    let repo = get_state_field!(&lazy_state, agent_repository);
    let result: Result<(), VenoreError> = async {
        let repo = repo?;
        repo.delete_chat_mode(&id).await?;
        Ok(())
    }
    .await;
    result.into_state()
}
