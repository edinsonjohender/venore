//! Tauri commands for pending logbook writes.
//!
//! AI-proposed section writes don't apply directly anymore — they sit in
//! `venore_core::chat::pending_writes::PENDING_WRITES` until the user
//! accepts, discards, or regenerates from the node panel. The
//! `ai-write-proposed` event is emitted by the chat tool dispatch layer
//! after the executor inserts a pending write; these commands drive the
//! lifecycle from the UI side.

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tracing::{debug, info, warn};

use venore_core::chat::pending_writes::{self, PendingKind, PendingSectionWrite};
use venore_core::error::VenoreError;
use venore_core::llm::{GatewayOptions, LlmMessage, LlmRequest, MessageRole};
use venore_core::traits::LlmTask;

use super::dto::pending_writes::{
    AcceptPendingWriteRequest, AcceptPendingWriteResponse, DiscardPendingWriteRequest,
    DiscardPendingWriteResponse, ListPendingWritesRequest, ListPendingWritesResponse,
    ListSessionPendingWritesRequest, PendingWriteDto, RegeneratePendingWriteRequest,
    RegeneratePendingWriteResponse,
};
use crate::state::{get_state_field, LazyAppState};
use crate::utils::{CommandResult, IntoStateCommandResult, StateCommandResult};

// =============================================================================
// Events
// =============================================================================

/// Emitted by these commands and by the chat tool dispatch when a pending
/// write is created or regenerated. NodeLogbook listens to refetch its
/// pending list; FloatingNodePanels listens to auto-open / bring-to-front.
const AI_WRITE_PROPOSED_EVENT: &str = "ai-write-proposed";
const LOGBOOK_CHANGED_EVENT: &str = "ocean-knowledge-changed";

#[derive(Debug, Clone, Serialize)]
struct AiWriteProposedPayload {
    project_path: String,
    node_id: String,
    write_id: String,
    /// "create" | "edit"
    kind: String,
}

#[derive(Debug, Clone, Serialize)]
struct LogbookChangedPayload {
    project_path: String,
    node_id: String,
}

fn emit_ai_write_proposed(app: &AppHandle, write: &PendingSectionWrite) {
    let kind = match &write.kind {
        PendingKind::Create => "create",
        PendingKind::Edit { .. } => "edit",
    };
    let payload = AiWriteProposedPayload {
        project_path: write.project_path.clone(),
        node_id: write.node_id.clone(),
        write_id: write.write_id.clone(),
        kind: kind.to_string(),
    };
    if let Err(e) = app.emit(AI_WRITE_PROPOSED_EVENT, payload) {
        warn!(error = %e, "Failed to emit ai-write-proposed");
    }
}

fn emit_logbook_changed(app: &AppHandle, project_path: &str, node_id: &str) {
    let _ = app.emit(
        LOGBOOK_CHANGED_EVENT,
        LogbookChangedPayload {
            project_path: project_path.to_string(),
            node_id: node_id.to_string(),
        },
    );
}

// =============================================================================
// Read commands
// =============================================================================

#[tauri::command]
pub async fn list_pending_writes(
    request: ListPendingWritesRequest,
) -> CommandResult<ListPendingWritesResponse> {
    debug!(project_path = %request.project_path, node_id = %request.node_id, "List pending writes");
    let writes: Vec<PendingWriteDto> = pending_writes::list_for_node(&request.project_path, &request.node_id)
        .into_iter()
        .map(PendingWriteDto::from)
        .collect();
    Ok::<ListPendingWritesResponse, VenoreError>(ListPendingWritesResponse { writes }).into()
}

#[tauri::command]
pub async fn list_session_pending_writes(
    request: ListSessionPendingWritesRequest,
) -> CommandResult<ListPendingWritesResponse> {
    debug!(session_id = %request.session_id, "List session pending writes");
    let writes: Vec<PendingWriteDto> = pending_writes::list_for_session(&request.session_id)
        .into_iter()
        .map(PendingWriteDto::from)
        .collect();
    Ok::<ListPendingWritesResponse, VenoreError>(ListPendingWritesResponse { writes }).into()
}

// =============================================================================
// Mutation commands
// =============================================================================

#[tauri::command]
pub async fn accept_pending_write(
    app: AppHandle,
    request: AcceptPendingWriteRequest,
) -> CommandResult<AcceptPendingWriteResponse> {
    info!(write_id = %request.write_id, "Accept pending write");
    let result: Result<AcceptPendingWriteResponse, VenoreError> = (|| {
        let write = pending_writes::get(&request.write_id).ok_or_else(|| {
            VenoreError::NotFound(format!("pending write '{}'", request.write_id))
        })?;

        let project_path = write.project_path.clone();
        let node_id = write.node_id.clone();

        // Persist via the same service entrypoints the executor used to
        // call directly. Reusing them keeps the AI-write codepath identical
        // to the manual UI path post-acceptance (timestamps, source
        // attribution, save-to-disk semantics).
        let section_opt = venore_core::ocean::service::with_service(&project_path, |service| {
            match write.kind.clone() {
                PendingKind::Create => {
                    let source = venore_core::ocean::SourceAttribution::Ai {
                        model: write.ai_model.clone(),
                        timestamp: chrono::Utc::now().timestamp(),
                    };
                    service
                        .add_node_section(
                            &node_id,
                            write.name.clone(),
                            write.content_markdown.clone(),
                            source,
                            Some(write.ai_prompt.clone()),
                            Some(write.ai_model.clone()),
                        )
                        .map(|s| s.id)
                }
                PendingKind::Edit { section_id, .. } => service
                    .update_node_section_as_ai(
                        &node_id,
                        &section_id,
                        write.name.clone(),
                        write.content_markdown.clone(),
                        write.ai_prompt.clone(),
                        write.ai_model.clone(),
                    )
                    .map(|s| s.id),
            }
        })?;

        let section_id = section_opt.ok_or_else(|| {
            VenoreError::NotFound(format!(
                "node '{}' (or referenced section) not found while accepting pending write",
                node_id
            ))
        })?;

        pending_writes::remove(&request.write_id);
        emit_logbook_changed(&app, &project_path, &node_id);

        Ok(AcceptPendingWriteResponse {
            ok: true,
            section_id: Some(section_id),
        })
    })();
    result.into()
}

#[tauri::command]
pub async fn discard_pending_write(
    app: AppHandle,
    request: DiscardPendingWriteRequest,
) -> CommandResult<DiscardPendingWriteResponse> {
    info!(write_id = %request.write_id, "Discard pending write");
    let removed = pending_writes::remove(&request.write_id);
    if let Some(write) = removed {
        // Tell listeners to refresh their pending lists. Reuse
        // `ai-write-proposed` (with the now-removed id) so NodeLogbook
        // can simply refetch on any pending event.
        emit_ai_write_proposed(&app, &write);
    }
    Ok::<DiscardPendingWriteResponse, VenoreError>(DiscardPendingWriteResponse { ok: true }).into()
}

#[tauri::command]
pub async fn regenerate_pending_write(
    app: AppHandle,
    lazy_state: tauri::State<'_, LazyAppState>,
    request: RegeneratePendingWriteRequest,
) -> StateCommandResult<RegeneratePendingWriteResponse> {
    info!(write_id = %request.write_id, "Regenerate pending write");
    let gateway = get_state_field!(&lazy_state, llm_gateway);

    let result: Result<RegeneratePendingWriteResponse, VenoreError> = async {
        let gateway = gateway?;
        let write = pending_writes::get(&request.write_id).ok_or_else(|| {
            VenoreError::NotFound(format!("pending write '{}'", request.write_id))
        })?;

        let baseline_block = match &write.kind {
            PendingKind::Edit { baseline_content, .. } => format!(
                "\n\nThe current content of the section is:\n\n{}\n",
                baseline_content
            ),
            PendingKind::Create => String::new(),
        };

        let user_prompt = format!(
            "Regenerate the markdown content for a logbook section named \"{}\". \
             Original intent: {}{}\n\n\
             Reply with ONLY the new markdown body — no preamble, no closing remark, no code fence.",
            write.name, write.ai_prompt, baseline_block,
        );

        let request_msg = LlmRequest {
            model: write.ai_model.clone(),
            messages: vec![LlmMessage {
                role: MessageRole::User,
                content: user_prompt,
                tool_call_id: None,
                tool_calls: None,
                content_parts: None,
            }],
            temperature: None,
            max_tokens: None,
            tools: None,
            json_schema: None,
            timeout_secs: None,
            web_search: false,
        };

        let response = gateway
            .complete(
                request_msg,
                GatewayOptions::for_task(LlmTask::Chat).with_model(write.ai_model.clone()),
            )
            .await?;

        let new_content = response.content.trim().to_string();

        // Recompute diff against baseline (Edit) or leave None (Create).
        let (diff_patch, additions, deletions) = match &write.kind {
            PendingKind::Edit { baseline_content, .. } => {
                let (patch, adds, dels) = pending_writes::compute_diff_patch(
                    &write.name,
                    baseline_content,
                    &new_content,
                );
                (Some(patch), adds, dels)
            }
            PendingKind::Create => {
                let adds = new_content.lines().filter(|l| !l.is_empty()).count() as u32;
                (None, adds, 0)
            }
        };

        let updated = pending_writes::replace_content(
            &request.write_id,
            write.name.clone(),
            new_content,
            diff_patch,
            additions,
            deletions,
            response.model.clone(),
        );

        match updated {
            Some(updated_write) => {
                emit_ai_write_proposed(&app, &updated_write);
                Ok(RegeneratePendingWriteResponse {
                    ok: true,
                    write: Some(PendingWriteDto::from(updated_write)),
                })
            }
            None => Err(VenoreError::NotFound(format!(
                "pending write '{}' was removed during regenerate",
                request.write_id
            ))),
        }
    }
    .await;

    result.into_state()
}
