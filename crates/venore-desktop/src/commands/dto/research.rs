//! Research Engine DTOs for Tauri IPC

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartResearchRequest {
    pub feature_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartResearchResponse {
    pub run_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchStatusResponse {
    pub run_id: String,
    pub phase: String,
    pub status: String,
    pub intensity: String,
    pub evaluation_round: i32,
    pub total_workers_spawned: i32,
    pub total_tool_calls: i32,
    pub duration_ms: i64,
}
