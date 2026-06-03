//! Prompt models — domain types for the prompt registry

/// A prompt template stored in the registry
#[derive(Debug, Clone)]
pub struct Prompt {
    pub id: String,
    pub name: String,
    pub category: String,
    pub provider: String,
    pub content: String,
    pub variables: String,
    pub is_template: bool,
    /// User toggle. Used by chat-fragment prompts to disable blocks of the
    /// system prompt without deleting them. Other categories ignore it.
    pub is_enabled: bool,
    pub version: u32,
    pub created_at: String,
    pub updated_at: String,
}

/// A historical version of a prompt's content
#[derive(Debug, Clone)]
pub struct PromptVersion {
    pub id: String,
    pub prompt_id: String,
    pub version: u32,
    pub content: String,
    pub created_at: String,
}
