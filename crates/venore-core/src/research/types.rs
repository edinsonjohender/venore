//! Research Engine types
//!
//! Data types for the multi-agent research orchestration engine:
//! runs, phases, events, worker assignments, and results.

use serde::{Deserialize, Serialize};

// -----------------------------------------------------------------------------
// Research phases
// -----------------------------------------------------------------------------

/// Phases of a research run's lifecycle
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResearchPhase {
    Decomposing,
    Investigating,
    Evaluating,
    Concluding,
    Completed,
    Failed,
    Paused,
    Cancelled,
}

impl ResearchPhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Decomposing => "decomposing",
            Self::Investigating => "investigating",
            Self::Evaluating => "evaluating",
            Self::Concluding => "concluding",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Paused => "paused",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "decomposing" => Some(Self::Decomposing),
            "investigating" => Some(Self::Investigating),
            "evaluating" => Some(Self::Evaluating),
            "concluding" => Some(Self::Concluding),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "paused" => Some(Self::Paused),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }

    /// Whether the phase represents a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

// -----------------------------------------------------------------------------
// Research run status
// -----------------------------------------------------------------------------

/// Status of a research run (persisted to SQLite)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResearchStatus {
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl ResearchStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "running" => Some(Self::Running),
            "paused" => Some(Self::Paused),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

// -----------------------------------------------------------------------------
// Research run (persisted)
// -----------------------------------------------------------------------------

/// A research run — one execution of the research engine for a feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchRun {
    pub id: String,
    pub feature_id: String,
    pub phase: String,
    pub status: String,
    pub intensity: String,
    pub max_workers: i32,
    pub evaluation_round: i32,
    pub total_workers_spawned: i32,
    pub total_tool_calls: i32,
    pub total_tokens: i32,
    pub manager_model: String,
    pub worker_model: String,
    /// JSON array of user instructions sent via chat control channel
    pub user_instructions: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub duration_ms: i64,
    pub error: Option<String>,
}

// -----------------------------------------------------------------------------
// Worker assignment & result
// -----------------------------------------------------------------------------

/// Assignment from the Manager to a Worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerAssignment {
    pub worker_id: String,
    pub hexagon_ids: Vec<String>,
    pub instructions: String,
    pub max_iterations: u32,
    pub max_tool_calls: u32,
}

/// Result returned by a Worker after completing its assignment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerResult {
    pub worker_id: String,
    pub hexagons_updated: Vec<String>,
    pub evidence_added: u32,
    pub tool_calls: u32,
    pub tokens_used: u32,
    pub duration_ms: u64,
    pub error: Option<String>,
}

// -----------------------------------------------------------------------------
// Research events (emitted to frontend via Tauri)
// -----------------------------------------------------------------------------

/// Events emitted during research execution for real-time UI updates
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ResearchEvent {
    RunStarted {
        run_id: String,
        feature_id: String,
    },
    PhaseTransition {
        run_id: String,
        from: String,
        to: String,
    },
    WorkerStarted {
        run_id: String,
        worker_id: String,
        hexagon_ids: Vec<String>,
    },
    WorkerCompleted {
        run_id: String,
        worker_id: String,
        duration_ms: u64,
    },
    WorkerFailed {
        run_id: String,
        worker_id: String,
        error: String,
    },
    ManagerThinking {
        run_id: String,
        message: String,
    },
    RunCompleted {
        run_id: String,
        duration_ms: u64,
    },
    RunFailed {
        run_id: String,
        error: String,
    },
    RunPaused {
        run_id: String,
    },
}

impl ResearchEvent {
    /// Tauri event name for this event variant
    pub fn event_name(&self) -> &'static str {
        match self {
            Self::RunStarted { .. } => "research:run-started",
            Self::PhaseTransition { .. } => "research:phase-transition",
            Self::WorkerStarted { .. } => "research:worker-started",
            Self::WorkerCompleted { .. } => "research:worker-completed",
            Self::WorkerFailed { .. } => "research:worker-failed",
            Self::ManagerThinking { .. } => "research:manager-thinking",
            Self::RunCompleted { .. } => "research:run-completed",
            Self::RunFailed { .. } => "research:run-failed",
            Self::RunPaused { .. } => "research:run-paused",
        }
    }
}

/// Callback type for emitting research events
pub type ResearchEventEmitter = Box<dyn Fn(ResearchEvent) + Send + Sync>;

// -----------------------------------------------------------------------------
// Concurrency config
// -----------------------------------------------------------------------------

/// Map intensity level to max parallel workers
pub fn max_workers_for_intensity(intensity: &str) -> i32 {
    match intensity {
        "shallow" => 2,
        "moderate" => 3,
        "deep" => 5,
        _ => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phase_roundtrip() {
        let phases = [
            ResearchPhase::Decomposing,
            ResearchPhase::Investigating,
            ResearchPhase::Evaluating,
            ResearchPhase::Concluding,
            ResearchPhase::Completed,
            ResearchPhase::Failed,
            ResearchPhase::Paused,
            ResearchPhase::Cancelled,
        ];
        for phase in &phases {
            let s = phase.as_str();
            let parsed = ResearchPhase::from_str(s).unwrap();
            assert_eq!(&parsed, phase);
        }
    }

    #[test]
    fn test_terminal_phases() {
        assert!(ResearchPhase::Completed.is_terminal());
        assert!(ResearchPhase::Failed.is_terminal());
        assert!(ResearchPhase::Cancelled.is_terminal());
        assert!(!ResearchPhase::Investigating.is_terminal());
    }

    #[test]
    fn test_status_roundtrip() {
        let statuses = [
            ResearchStatus::Running,
            ResearchStatus::Paused,
            ResearchStatus::Completed,
            ResearchStatus::Failed,
            ResearchStatus::Cancelled,
        ];
        for status in &statuses {
            let s = status.as_str();
            let parsed = ResearchStatus::from_str(s).unwrap();
            assert_eq!(&parsed, status);
        }
    }

    #[test]
    fn test_max_workers_for_intensity() {
        assert_eq!(max_workers_for_intensity("shallow"), 2);
        assert_eq!(max_workers_for_intensity("moderate"), 3);
        assert_eq!(max_workers_for_intensity("deep"), 5);
        assert_eq!(max_workers_for_intensity("unknown"), 3);
    }

    #[test]
    fn test_event_names() {
        let event = ResearchEvent::RunStarted {
            run_id: "r1".into(),
            feature_id: "f1".into(),
        };
        assert_eq!(event.event_name(), "research:run-started");
    }
}
