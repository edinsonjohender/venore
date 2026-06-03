//! Agent DTOs — Request/Response types for agent profile and team commands

use serde::{Deserialize, Serialize};

// =============================================================================
// Profile DTOs
// =============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAgentProfileRequest {
    pub name: String,
    pub description: String,
    pub stage: String,
    pub system_prompt: String,
    pub provider: String,
    pub model: String,
    pub temperature: f32,
    pub is_enabled: Option<bool>,
    pub rules_json: Option<String>,
    pub criteria_json: Option<String>,
    pub tools_json: Option<String>,
    pub max_tokens_per_run: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAgentProfileRequest {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub stage: Option<String>,
    pub system_prompt: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub is_enabled: Option<bool>,
    pub rules_json: Option<String>,
    pub criteria_json: Option<String>,
    pub tools_json: Option<String>,
    pub max_tokens_per_run: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentProfileDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub stage: String,
    pub system_prompt: String,
    pub provider: String,
    pub model: String,
    pub temperature: f32,
    pub is_template: bool,
    pub is_enabled: bool,
    pub rules_json: String,
    pub criteria_json: String,
    pub tools_json: String,
    pub max_tokens_per_run: u32,
    pub created_at: String,
    pub updated_at: String,
}

impl From<venore_core::agents::AgentProfile> for AgentProfileDto {
    fn from(p: venore_core::agents::AgentProfile) -> Self {
        Self {
            id: p.id,
            name: p.name,
            description: p.description,
            stage: p.stage.as_str().to_string(),
            system_prompt: p.system_prompt,
            provider: p.provider,
            model: p.model,
            temperature: p.temperature,
            is_template: p.is_template,
            is_enabled: p.is_enabled,
            rules_json: p.rules_json,
            criteria_json: p.criteria_json,
            tools_json: p.tools_json,
            max_tokens_per_run: p.max_tokens_per_run,
            created_at: p.created_at,
            updated_at: p.updated_at,
        }
    }
}

// =============================================================================
// Rule DTOs
// =============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAgentRuleRequest {
    pub name: String,
    pub description: String,
    pub scope: Vec<String>,
    pub severity: String,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAgentRuleRequest {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub scope: Option<Vec<String>>,
    pub severity: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuleDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub scope: Vec<String>,
    pub severity: String,
    pub is_active: bool,
    pub is_template: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<venore_core::agents::AgentRule> for AgentRuleDto {
    fn from(r: venore_core::agents::AgentRule) -> Self {
        Self {
            id: r.id,
            name: r.name,
            description: r.description,
            scope: r.scope,
            severity: r.severity.as_str().to_string(),
            is_active: r.is_active,
            is_template: r.is_template,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

// =============================================================================
// Team DTOs
// =============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAgentTeamRequest {
    pub name: String,
    pub description: String,
    pub profile_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAgentTeamRequest {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub profile_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTeamDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub profile_ids: Vec<String>,
    pub is_template: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<venore_core::agents::AgentTeam> for AgentTeamDto {
    fn from(t: venore_core::agents::AgentTeam) -> Self {
        Self {
            id: t.id,
            name: t.name,
            description: t.description,
            profile_ids: t.profile_ids,
            is_template: t.is_template,
            created_at: t.created_at,
            updated_at: t.updated_at,
        }
    }
}

// =============================================================================
// Tool Category DTOs
// =============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateToolCategoryRequest {
    pub name: String,
    pub description: String,
    pub icon: String,
    pub color: String,
    pub display_order: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateToolCategoryRequest {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub display_order: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCategoryDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub color: String,
    pub display_order: u32,
    pub is_template: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<venore_core::agents::ToolCategory> for ToolCategoryDto {
    fn from(c: venore_core::agents::ToolCategory) -> Self {
        Self {
            id: c.id,
            name: c.name,
            description: c.description,
            icon: c.icon,
            color: c.color,
            display_order: c.display_order,
            is_template: c.is_template,
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }
}

// =============================================================================
// Tool Definition DTOs
// =============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateToolDefinitionRequest {
    pub name: String,
    pub description: String,
    pub category_id: String,
    pub parameters_json: Option<String>,
    pub is_read_only: Option<bool>,
    pub is_enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateToolDefinitionRequest {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub category_id: Option<String>,
    pub parameters_json: Option<String>,
    pub is_read_only: Option<bool>,
    pub is_enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinitionDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category_id: String,
    pub parameters_json: String,
    pub is_read_only: bool,
    pub is_enabled: bool,
    pub is_template: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<venore_core::agents::ToolDefinition> for ToolDefinitionDto {
    fn from(t: venore_core::agents::ToolDefinition) -> Self {
        Self {
            id: t.id,
            name: t.name,
            description: t.description,
            category_id: t.category_id,
            parameters_json: t.parameters_json,
            is_read_only: t.is_read_only,
            is_enabled: t.is_enabled,
            is_template: t.is_template,
            created_at: t.created_at,
            updated_at: t.updated_at,
        }
    }
}

// =============================================================================
// Chat Mode
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatModeDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category_ids: Vec<String>,
    pub tool_ids: Vec<String>,
    pub sub_agent_ids: Vec<String>,
    pub rule_ids: Vec<String>,
    pub prompt_id: Option<String>,
    pub is_template: bool,
    pub is_default_for_kind: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<venore_core::agents::ChatMode> for ChatModeDto {
    fn from(m: venore_core::agents::ChatMode) -> Self {
        Self {
            id: m.id,
            name: m.name,
            description: m.description,
            category_ids: m.category_ids,
            tool_ids: m.tool_ids,
            sub_agent_ids: m.sub_agent_ids,
            rule_ids: m.rule_ids,
            prompt_id: m.prompt_id,
            is_template: m.is_template,
            is_default_for_kind: m.is_default_for_kind,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateChatModeRequest {
    pub name: String,
    pub description: Option<String>,
    pub category_ids: Option<Vec<String>>,
    pub tool_ids: Option<Vec<String>>,
    pub sub_agent_ids: Option<Vec<String>>,
    pub rule_ids: Option<Vec<String>>,
    pub prompt_id: Option<String>,
    pub is_default_for_kind: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateChatModeRequest {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub category_ids: Option<Vec<String>>,
    pub tool_ids: Option<Vec<String>>,
    pub sub_agent_ids: Option<Vec<String>>,
    pub rule_ids: Option<Vec<String>>,
    pub prompt_id: Option<String>,
    pub is_default_for_kind: Option<String>,
}
