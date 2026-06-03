//! Chat global state — static registries for active streams, pending approvals, tasks, sub-agents.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use once_cell::sync::Lazy;
use serde::Serialize;
use tokio::task::AbortHandle;

// ── Active streams (cancellation registry) ───────────────────────────

pub(super) static ACTIVE_STREAMS: Lazy<Mutex<HashMap<String, AbortHandle>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

// ── Session → stream mapping (reconnection after window transfer) ────

/// Maps session_id → stream_id for the currently active stream of each session.
/// Allows frontends to reconnect to an active stream after window transfer.
pub(super) static SESSION_STREAMS: Lazy<Mutex<HashMap<String, String>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

// ── Pending approvals (tool call confirmation) ───────────────────────

pub(super) static PENDING_APPROVALS: Lazy<Mutex<HashMap<String, tokio::sync::oneshot::Sender<bool>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

// ── Session approvals (permission cache per dev session) ─────────────

pub(super) static SESSION_APPROVALS: Lazy<Mutex<HashMap<String, HashSet<String>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

// ── Pending user responses (ask_user tool) ───────────────────────────

pub(super) static PENDING_USER_RESPONSES: Lazy<Mutex<HashMap<String, tokio::sync::oneshot::Sender<String>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

// ── Pending plan approvals (submit_plan tool) ────────────────────────

pub(super) static PENDING_PLAN_APPROVALS: Lazy<Mutex<HashMap<String, tokio::sync::oneshot::Sender<bool>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

// ── Task stores (per stream_id in-memory task lists) ─────────────────

#[derive(Clone, Serialize)]
pub struct TaskItem {
    pub id: String,
    pub subject: String,
    pub status: String, // "pending" | "in_progress" | "completed"
    pub description: String,
}

pub(super) static TASK_STORES: Lazy<Mutex<HashMap<String, Vec<TaskItem>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

// ── Active sub-agents (count per stream_id, max 5) ───────────────────

pub(super) static ACTIVE_SUB_AGENTS: Lazy<Mutex<HashMap<String, u32>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

// ── Stream → tool_call_id tracking (cleanup on abort) ────────────────

pub(super) static STREAM_TOOL_CALL_IDS: Lazy<Mutex<HashMap<String, Vec<String>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
