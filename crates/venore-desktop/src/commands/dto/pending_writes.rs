//! DTOs for pending logbook writes — AI write preview / approval flow.
//!
//! The backing core type lives in
//! `venore_core::chat::pending_writes::PendingSectionWrite`. We expose a
//! flattened DTO so the frontend doesn't need to discriminate on `kind`.

use serde::{Deserialize, Serialize};

// =============================================================================
// Response DTO
// =============================================================================

/// Frontend-friendly view of a pending write. The discriminator is the
/// `kind` field (`"create"` | `"edit"`); for edits, the baseline fields
/// are populated so the UI can show "before" alongside the proposed value
/// without an extra round-trip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingWriteDto {
    pub write_id: String,
    pub project_path: String,
    pub node_id: String,
    pub session_id: Option<String>,
    /// "create" | "edit"
    pub kind: String,
    /// Only set when kind = "edit".
    pub section_id: Option<String>,
    pub baseline_name: Option<String>,
    pub baseline_content: Option<String>,
    pub name: String,
    pub content_markdown: String,
    pub ai_prompt: String,
    pub ai_model: String,
    pub diff_patch: Option<String>,
    pub additions: u32,
    pub deletions: u32,
    pub created_at: i64,
}

impl From<venore_core::chat::pending_writes::PendingSectionWrite> for PendingWriteDto {
    fn from(w: venore_core::chat::pending_writes::PendingSectionWrite) -> Self {
        let (kind, section_id, baseline_name, baseline_content) = match w.kind {
            venore_core::chat::pending_writes::PendingKind::Create => {
                ("create".to_string(), None, None, None)
            }
            venore_core::chat::pending_writes::PendingKind::Edit {
                section_id,
                baseline_name,
                baseline_content,
            } => (
                "edit".to_string(),
                Some(section_id),
                Some(baseline_name),
                Some(baseline_content),
            ),
        };
        Self {
            write_id: w.write_id,
            project_path: w.project_path,
            node_id: w.node_id,
            session_id: w.session_id,
            kind,
            section_id,
            baseline_name,
            baseline_content,
            name: w.name,
            content_markdown: w.content_markdown,
            ai_prompt: w.ai_prompt,
            ai_model: w.ai_model,
            diff_patch: w.diff_patch,
            additions: w.additions,
            deletions: w.deletions,
            created_at: w.created_at,
        }
    }
}

// =============================================================================
// Requests / responses
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListPendingWritesRequest {
    pub project_path: String,
    pub node_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListPendingWritesResponse {
    pub writes: Vec<PendingWriteDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListSessionPendingWritesRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptPendingWriteRequest {
    pub write_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptPendingWriteResponse {
    pub ok: bool,
    pub section_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscardPendingWriteRequest {
    pub write_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscardPendingWriteResponse {
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegeneratePendingWriteRequest {
    pub write_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegeneratePendingWriteResponse {
    pub ok: bool,
    pub write: Option<PendingWriteDto>,
}
