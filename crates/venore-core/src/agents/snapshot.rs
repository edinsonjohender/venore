//! Snapshot types and report parser for PR analysis results
//!
//! Parses structured JSON reports from reporter output and defines
//! types for category snapshots and author statistics tracking.

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

static RE_JSON_FENCE: Lazy<regex::Regex> = Lazy::new(||
    regex::Regex::new(r"```json\s*\n([\s\S]*?)\n\s*```").expect("Invalid regex")
);

// =============================================================================
// Parsed report types (from reporter JSON output)
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedReport {
    pub overall_score: u32,
    pub summary: String,
    pub categories: Vec<ParsedCategory>,
    pub findings: Vec<ParsedFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedCategory {
    pub name: String,
    pub score: u32,
    pub status: String,
    pub findings_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedFinding {
    pub title: String,
    pub category: String,
    pub severity: String,
    pub description: String,
}

// =============================================================================
// Persistence types
// =============================================================================

#[derive(Debug, Clone)]
pub struct CategorySnapshot {
    pub id: String,
    pub run_id: String,
    pub project_path: String,
    pub author_login: String,
    pub category_name: String,
    pub score: u32,
    pub status: String,
    pub findings_count: u32,
    pub overall_score: u32,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct AuthorStats {
    pub login: String,
    pub project_path: String,
    pub avatar_url: String,
    pub total_runs: u32,
    pub avg_overall_score: f64,
    pub last_overall_score: u32,
    pub last_run_at: String,
}

#[derive(Debug, Clone)]
pub struct CategoryAverage {
    pub category_name: String,
    pub avg_score: f64,
    pub run_count: u32,
}

// =============================================================================
// Parser
// =============================================================================

/// Extract a structured report from reporter step output.
///
/// Looks for a ```json ... ``` fenced block, parses it, and validates
/// basic shape (overall_score 0-100, categories non-empty).
/// Returns `None` on any failure (fail-soft).
pub fn parse_report_from_output(output: &str) -> Option<ParsedReport> {
    let caps = RE_JSON_FENCE.captures(output)?;
    let json_str = caps.get(1)?.as_str().trim();

    let report: ParsedReport = serde_json::from_str(json_str).ok()?;

    // Validate basic constraints
    if report.overall_score > 100 {
        return None;
    }
    if report.categories.is_empty() {
        return None;
    }
    for cat in &report.categories {
        if cat.score > 100 {
            return None;
        }
    }

    Some(report)
}
