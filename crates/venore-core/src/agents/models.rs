//! Agent profile and team domain models

use serde::{Deserialize, Serialize};

// =============================================================================
// Enums
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AgentStage {
    Triager,
    Specialist,
    Reporter,
    SubAgent,
}

impl AgentStage {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Triager => "triager",
            Self::Specialist => "specialist",
            Self::Reporter => "reporter",
            Self::SubAgent => "subagent",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "triager" => Some(Self::Triager),
            "specialist" => Some(Self::Specialist),
            "reporter" => Some(Self::Reporter),
            "subagent" => Some(Self::SubAgent),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    Warning,
    Info,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "critical" => Some(Self::Critical),
            "warning" => Some(Self::Warning),
            "info" => Some(Self::Info),
            _ => None,
        }
    }
}

// =============================================================================
// Agent Profile
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProfile {
    pub id: String,
    pub name: String,
    pub description: String,
    pub stage: AgentStage,
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

// =============================================================================
// Agent Team
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTeam {
    pub id: String,
    pub name: String,
    pub description: String,
    pub profile_ids: Vec<String>,
    pub is_template: bool,
    pub created_at: String,
    pub updated_at: String,
}

// =============================================================================
// Supporting structs (serialized as JSON inside profile fields)
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub scope: Vec<String>,
    pub severity: Severity,
    pub is_active: bool,
    pub is_template: bool,
    pub created_at: String,
    pub updated_at: String,
}

// =============================================================================
// Tool Category
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCategory {
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

// =============================================================================
// Tool Definition
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
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

// =============================================================================
// Chat Mode
// =============================================================================
//
// A named bundle that decides which tools/sub-agents/rules/prompt the chat
// runtime sees. Each project kind ("code", "knowledge") has a default mode
// resolved at message time via `is_default_for_kind`. Users can create
// custom modes (is_template = false). Plan mode is dynamic and orthogonal —
// it overrides any mode at runtime to read-only.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMode {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Tool categories enabled for this mode. Empty = no category filter
    /// (every enabled tool in the library is exposed).
    pub category_ids: Vec<String>,
    /// Specific tool IDs allowed in this mode. Empty = "all from selected
    /// categories". Non-empty = whitelist (intersected with category filter).
    pub tool_ids: Vec<String>,
    /// Sub-agent profile IDs available via `spawn_agent` in this mode.
    pub sub_agent_ids: Vec<String>,
    /// Rule IDs to enforce when this mode is active (post-v1 enforcement).
    pub rule_ids: Vec<String>,
    /// Optional FK to a prompt fragment that gets layered on top of the
    /// base chat prompt for this mode.
    pub prompt_id: Option<String>,
    /// True if seeded by Venore. Templates can't be deleted from UI.
    pub is_template: bool,
    /// "code" | "knowledge" | None. The default mode picked when a project
    /// of the matching kind starts a chat without an explicit override.
    pub is_default_for_kind: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
