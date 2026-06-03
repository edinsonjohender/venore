//! Prompt DTOs — Request/Response types for prompt registry commands

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptDto {
    pub id: String,
    pub name: String,
    pub category: String,
    pub provider: String,
    pub content: String,
    pub variables: Vec<String>,
    pub is_template: bool,
    pub is_enabled: bool,
    pub version: u32,
    pub created_at: String,
    pub updated_at: String,
}

impl From<venore_core::prompts::Prompt> for PromptDto {
    fn from(p: venore_core::prompts::Prompt) -> Self {
        let variables: Vec<String> = serde_json::from_str(&p.variables).unwrap_or_default();
        Self {
            id: p.id,
            name: p.name,
            category: p.category,
            provider: p.provider,
            content: p.content,
            variables,
            is_template: p.is_template,
            is_enabled: p.is_enabled,
            version: p.version,
            created_at: p.created_at,
            updated_at: p.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptVersionDto {
    pub id: String,
    pub prompt_id: String,
    pub version: u32,
    pub content: String,
    pub created_at: String,
}

impl From<venore_core::prompts::PromptVersion> for PromptVersionDto {
    fn from(v: venore_core::prompts::PromptVersion) -> Self {
        Self {
            id: v.id,
            prompt_id: v.prompt_id,
            version: v.version,
            content: v.content,
            created_at: v.created_at,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePromptRequest {
    pub id: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveTaskPromptRequest {
    pub category: String,
    pub provider: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetPromptEnabledRequest {
    pub id: String,
    pub enabled: bool,
}
