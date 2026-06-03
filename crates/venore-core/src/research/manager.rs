//! Research Manager — LLM-driven orchestration decisions
//!
//! The Manager makes structured LLM calls to:
//! 1. Decompose a research seed into hexagons + worker assignments
//! 2. Evaluate worker results and decide next steps
//! 3. Generate a final conclusion

use std::collections::HashMap;
use std::sync::Arc;

use crate::error::VenoreError;
use crate::knowledge::{KnowledgeHexagon, KnowledgeRepository};
use crate::llm::{GatewayOptions, LlmGateway, LlmRequest};
use crate::llm::types::LlmMessage;
use crate::Result;

use super::prompts;
use super::types::WorkerAssignment;

// -----------------------------------------------------------------------------
// Decompose response
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Deserialize)]
pub struct DecomposeResponse {
    pub hexagons: Vec<DecomposedHexagon>,
    #[serde(default)]
    pub assignments: Vec<RawAssignment>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct DecomposedHexagon {
    pub title: String,
    pub description: String,
    #[serde(default = "default_priority")]
    pub priority: String,
}

fn default_priority() -> String {
    "medium".to_string()
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RawAssignment {
    pub hexagon_indices: Vec<usize>,
    pub instructions: String,
}

// -----------------------------------------------------------------------------
// Evaluate response
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Deserialize)]
pub struct EvaluateResponse {
    pub decision: String,
    #[serde(default)]
    pub reasoning: String,
    #[serde(default)]
    pub gaps: Vec<String>,
    #[serde(default)]
    pub new_hexagons: Vec<DecomposedHexagon>,
    #[serde(default)]
    pub assignments: Vec<RawAssignment>,
    #[serde(default)]
    pub phase_transition: Option<String>,
}

// -----------------------------------------------------------------------------
// Manager functions
// -----------------------------------------------------------------------------

/// Decompose a research seed into hexagons and worker assignments.
/// Returns (created hexagon IDs, worker assignments).
pub async fn decompose(
    gateway: &Arc<LlmGateway>,
    knowledge_repo: &Arc<KnowledgeRepository>,
    feature_id: &str,
    feature_name: &str,
    feature_description: &str,
    objective: &str,
    intensity: &str,
    max_hexagons: i32,
    max_workers: i32,
    options: &GatewayOptions,
) -> Result<(Vec<String>, Vec<WorkerAssignment>)> {
    let prompt = prompts::decompose_prompt(
        feature_name,
        feature_description,
        objective,
        intensity,
        max_hexagons,
        max_workers,
    );

    let response = call_manager_llm(gateway, &prompt, options).await?;
    let parsed = parse_json_response::<DecomposeResponse>(&response)?;

    // Create hexagons in DB
    let mut hexagon_ids: Vec<String> = Vec::new();
    for hex in &parsed.hexagons {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let hexagon = KnowledgeHexagon {
            id: id.clone(),
            feature_id: feature_id.to_string(),
            title: hex.title.clone(),
            description: hex.description.clone(),
            phase: "discover".to_string(),
            percentage: 0,
            confidence: "low".to_string(),
            risk: "unknown".to_string(),
            priority: hex.priority.clone(),
            is_dead_end: false,
            blocked_by: "[]".to_string(),
            notes_user: String::new(),
            agent_status: "idle".to_string(),
            created_at: now.clone(),
            updated_at: now,
        };
        knowledge_repo.create_hexagon(&hexagon).await?;
        hexagon_ids.push(id);
    }

    // Build worker assignments
    let assignments = build_assignments(&parsed.assignments, &hexagon_ids, max_workers);

    tracing::info!(
        hexagons = hexagon_ids.len(),
        workers = assignments.len(),
        "Manager decomposed research into hexagons"
    );

    Ok((hexagon_ids, assignments))
}

/// Evaluate current research state and decide next steps.
pub async fn evaluate(
    gateway: &Arc<LlmGateway>,
    knowledge_repo: &Arc<KnowledgeRepository>,
    feature_id: &str,
    feature_name: &str,
    objective: &str,
    evaluation_round: i32,
    max_rounds: i32,
    user_instructions: &[String],
    options: &GatewayOptions,
) -> Result<EvaluateResponse> {
    // Load current state from DB
    let hexagons = knowledge_repo.list_hexagons_by_feature(feature_id).await?;
    let mut evidence_counts: HashMap<String, usize> = HashMap::new();
    for hex in &hexagons {
        let count = knowledge_repo.count_evidence_by_hexagon(&hex.id).await?;
        evidence_counts.insert(hex.id.clone(), count);
    }
    let total_evidence: usize = evidence_counts.values().sum();

    let summary = prompts::build_hexagons_summary(&hexagons, &evidence_counts);
    let prompt = prompts::evaluate_prompt(
        feature_name,
        objective,
        &summary,
        total_evidence,
        evaluation_round,
        max_rounds,
        user_instructions,
    );

    let response = call_manager_llm(gateway, &prompt, options).await?;
    let parsed = parse_json_response::<EvaluateResponse>(&response)?;

    tracing::info!(
        decision = %parsed.decision,
        reasoning = %parsed.reasoning,
        gaps = parsed.gaps.len(),
        "Manager evaluation complete"
    );

    Ok(parsed)
}

/// Create new hexagons from evaluation gaps and build assignments for them.
pub async fn create_gap_hexagons(
    knowledge_repo: &Arc<KnowledgeRepository>,
    feature_id: &str,
    new_hexagons: &[DecomposedHexagon],
    raw_assignments: &[RawAssignment],
    max_workers: i32,
    phase: &str,
) -> Result<(Vec<String>, Vec<WorkerAssignment>)> {
    let mut hexagon_ids: Vec<String> = Vec::new();
    for hex in new_hexagons {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let hexagon = KnowledgeHexagon {
            id: id.clone(),
            feature_id: feature_id.to_string(),
            title: hex.title.clone(),
            description: hex.description.clone(),
            phase: phase.to_string(),
            percentage: 0,
            confidence: "low".to_string(),
            risk: "unknown".to_string(),
            priority: hex.priority.clone(),
            is_dead_end: false,
            blocked_by: "[]".to_string(),
            notes_user: String::new(),
            agent_status: "idle".to_string(),
            created_at: now.clone(),
            updated_at: now,
        };
        knowledge_repo.create_hexagon(&hexagon).await?;
        hexagon_ids.push(id);
    }

    let assignments = build_assignments(raw_assignments, &hexagon_ids, max_workers);
    Ok((hexagon_ids, assignments))
}

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

/// Call the Manager LLM with a prompt and return the text response
async fn call_manager_llm(
    gateway: &Arc<LlmGateway>,
    prompt: &str,
    options: &GatewayOptions,
) -> Result<String> {
    let request = LlmRequest {
        model: String::new(), // resolved by gateway
        messages: vec![LlmMessage {
            role: crate::llm::MessageRole::User,
            content: prompt.to_string(),
            tool_call_id: None,
            tool_calls: None,
            content_parts: None,
        }],
        temperature: Some(0.3),
        max_tokens: Some(4000),
        tools: None,
        json_schema: None,
        timeout_secs: Some(120),
        web_search: false,
    };

    let response = gateway.complete(request, options.clone()).await?;
    Ok(response.content)
}

/// Parse a JSON response from the LLM, handling markdown code fences
fn parse_json_response<T: serde::de::DeserializeOwned>(raw: &str) -> Result<T> {
    // Strip markdown code fences if present
    let cleaned = raw
        .trim()
        .strip_prefix("```json")
        .or_else(|| raw.trim().strip_prefix("```"))
        .unwrap_or(raw.trim());
    let cleaned = cleaned
        .strip_suffix("```")
        .unwrap_or(cleaned)
        .trim();

    serde_json::from_str(cleaned).map_err(|e| {
        tracing::warn!(error = %e, raw_len = raw.len(), "Failed to parse manager JSON response");
        VenoreError::ParseError(format!("Manager returned invalid JSON: {e}"))
    })
}

/// Build WorkerAssignments from raw LLM assignments + created hexagon IDs.
/// Falls back to round-robin distribution if assignments are empty or invalid.
fn build_assignments(
    raw: &[RawAssignment],
    hexagon_ids: &[String],
    max_workers: i32,
) -> Vec<WorkerAssignment> {
    if hexagon_ids.is_empty() {
        return Vec::new();
    }

    let max_workers = max_workers.max(1) as usize;

    // Try to use LLM-provided assignments
    if !raw.is_empty() {
        let mut assignments = Vec::new();
        for (i, ra) in raw.iter().enumerate() {
            let ids: Vec<String> = ra
                .hexagon_indices
                .iter()
                .filter_map(|&idx| hexagon_ids.get(idx).cloned())
                .collect();
            if !ids.is_empty() {
                assignments.push(WorkerAssignment {
                    worker_id: format!("worker-{}", i + 1),
                    hexagon_ids: ids,
                    instructions: ra.instructions.clone(),
                    max_iterations: 5,
                    max_tool_calls: 20,
                });
            }
        }
        if !assignments.is_empty() {
            return assignments;
        }
    }

    // Fallback: round-robin distribution
    let num_workers = max_workers.min(hexagon_ids.len());
    let mut assignments: Vec<WorkerAssignment> = (0..num_workers)
        .map(|i| WorkerAssignment {
            worker_id: format!("worker-{}", i + 1),
            hexagon_ids: Vec::new(),
            instructions: "Investigate your assigned hexagons thoroughly.".to_string(),
            max_iterations: 5,
            max_tool_calls: 20,
        })
        .collect();

    for (i, id) in hexagon_ids.iter().enumerate() {
        assignments[i % num_workers].hexagon_ids.push(id.clone());
    }

    assignments
}
