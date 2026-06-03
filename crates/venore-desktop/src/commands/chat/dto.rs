//! Chat DTOs — request/response types for IPC with the frontend.

use serde::{Deserialize, Serialize};
use venore_core::chat::{ChatMessageRecord, ChatSession};

use super::state::TaskItem;

// ── Request/Response ─────────────────────────────────────────────────

#[derive(Deserialize, Clone)]
pub struct AttachmentInput {
    pub name: String,
    pub mime_type: String,
    pub data_base64: String,
}

#[derive(Deserialize)]
pub struct SendChatMessageRequest {
    pub messages: Vec<venore_core::chat::ChatMessageInput>,
    pub stream_id: String,
    pub session_id: Option<String>,
    pub project_path: Option<String>,
    pub context_modules: Option<Vec<ContextModuleInput>>,
    pub dev_session_id: Option<String>,
    pub knowledge_feature_id: Option<String>,
    pub attachments: Option<Vec<AttachmentInput>>,
}

#[derive(Deserialize, Clone)]
pub struct ContextModuleInput {
    pub name: String,
    pub path: String,
}

#[derive(Serialize)]
pub struct SendChatMessageResponse {
    pub stream_id: String,
}

// ── Stream event payloads ────────────────────────────────────────────

#[derive(Clone, Serialize)]
pub struct ChatStreamDeltaPayload {
    pub stream_id: String,
    pub session_id: Option<String>,
    pub content: String,
}

#[derive(Clone, Serialize)]
pub struct ChatStreamDonePayload {
    pub stream_id: String,
    pub session_id: Option<String>,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub provider: String,
    pub model: String,
}

#[derive(Clone, Serialize)]
pub struct ChatStreamErrorPayload {
    pub stream_id: String,
    pub session_id: Option<String>,
    pub message: String,
    pub code: String,
}

#[derive(Clone, Serialize)]
pub struct SessionFileChangedPayload {
    pub dev_session_id: String,
    pub filename: String,
    pub status: String,
    pub additions: u32,
    pub deletions: u32,
    pub patch: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct ChatSnapshotPayload {
    pub stream_id: String,
    pub session_id: Option<String>,
    pub tool_call_id: String,
    pub commit_hash: String,
}

#[derive(Clone, Serialize)]
pub struct ChatToolCallPayload {
    pub stream_id: String,
    pub session_id: Option<String>,
    pub tool_call_id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
}

#[derive(Clone, Serialize)]
pub struct ChatToolResultPayload {
    pub stream_id: String,
    pub session_id: Option<String>,
    pub tool_call_id: String,
    pub success: bool,
    pub output: String,
}

#[derive(Clone, Serialize)]
pub struct ChatToolConfirmPayload {
    pub stream_id: String,
    pub session_id: Option<String>,
    pub tool_call_id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub resource: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct ChatAskUserPayload {
    pub stream_id: String,
    pub session_id: Option<String>,
    pub tool_call_id: String,
    pub question: String,
    pub options: Vec<AskUserOption>,
}

#[derive(Clone, Serialize)]
pub struct AskUserOption {
    pub label: String,
    pub description: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct ChatTaskUpdatePayload {
    pub stream_id: String,
    pub session_id: Option<String>,
    pub tasks: Vec<TaskItem>,
}

#[derive(Clone, Serialize)]
pub struct ChatPlanReadyPayload {
    pub stream_id: String,
    pub session_id: Option<String>,
    pub tool_call_id: String,
    pub summary: String,
    pub steps: Vec<String>,
}

#[derive(Clone, Serialize)]
pub struct ChatSubAgentPayload {
    pub stream_id: String,
    pub session_id: Option<String>,
    pub agent_id: String,
    pub agent_type: String,
    pub task: String,
    pub status: String, // "started" | "completed" | "failed"
    pub result: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct ChatCompactedPayload {
    pub stream_id: String,
    pub session_id: Option<String>,
    pub action: String,      // "pruned" | "compacted"
    pub tokens_saved: u32,
}

// ── Session DTOs ─────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateChatSessionRequest {
    pub name: Option<String>,
    pub project_id: Option<String>,
}

#[derive(Serialize)]
pub struct ChatSessionDto {
    pub id: String,
    pub name: String,
    pub project_id: Option<String>,
    pub dev_session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<ChatSession> for ChatSessionDto {
    fn from(s: ChatSession) -> Self {
        Self {
            id: s.id,
            name: s.name,
            project_id: s.project_id,
            dev_session_id: s.dev_session_id,
            created_at: s.created_at,
            updated_at: s.updated_at,
        }
    }
}

#[derive(Serialize)]
pub struct ChatMessageDto {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub created_at: String,
    pub attachments_json: Option<String>,
}

impl From<ChatMessageRecord> for ChatMessageDto {
    fn from(m: ChatMessageRecord) -> Self {
        Self {
            id: m.id,
            session_id: m.session_id,
            role: m.role,
            content: m.content,
            provider: m.provider,
            model: m.model,
            prompt_tokens: m.prompt_tokens,
            completion_tokens: m.completion_tokens,
            created_at: m.created_at,
            attachments_json: m.attachments_json,
        }
    }
}

#[derive(Serialize)]
pub struct SnapshotDto {
    pub tool_call_id: String,
    pub commit_hash: String,
    pub created_at: String,
    pub tool_name: Option<String>,
    pub file_path: Option<String>,
}

#[derive(Serialize)]
pub struct ToolCallRecordDto {
    pub id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub success: Option<bool>,
    pub output: Option<String>,
    pub commit_hash: Option<String>,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct TokenSummaryDto {
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    pub message_count: u32,
}

#[derive(Serialize)]
pub struct SessionActivityDto {
    pub tool_calls: Vec<ToolCallRecordDto>,
    pub snapshots: Vec<SnapshotDto>,
    pub token_summary: TokenSummaryDto,
}

#[derive(Serialize)]
pub struct ChatContextOptionDto {
    pub name: String,
    pub path: String,
    pub has_context: bool,
}
