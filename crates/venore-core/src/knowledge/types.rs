//! Knowledge Island types
//!
//! Data types for the Knowledge Island feature:
//! features, hexagons (research points), and evidence.

use serde::{Deserialize, Serialize};

/// Agent status for a knowledge hexagon
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    Idle,
    Running,
    Paused,
    Completed,
}

impl AgentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Completed => "completed",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "idle" => Some(Self::Idle),
            "running" => Some(Self::Running),
            "paused" => Some(Self::Paused),
            "completed" => Some(Self::Completed),
            _ => None,
        }
    }
}

/// Research phases for a knowledge hexagon
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HexagonPhase {
    Discover,
    Define,
    Validate,
    Conclude,
}

impl HexagonPhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Discover => "discover",
            Self::Define => "define",
            Self::Validate => "validate",
            Self::Conclude => "conclude",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "discover" => Some(Self::Discover),
            "define" => Some(Self::Define),
            "validate" => Some(Self::Validate),
            "conclude" => Some(Self::Conclude),
            _ => None,
        }
    }
}

/// A knowledge feature — a high-level research topic within a knowledge project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeFeature {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub objective: String,
    pub intensity: String,
    pub max_hexagons_per_phase: i32,
    pub auto_advance: bool,
    pub tags: String,
    pub created_at: String,
    pub updated_at: String,
}

/// A hexagon — a specific research point within a feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeHexagon {
    pub id: String,
    pub feature_id: String,
    pub title: String,
    pub description: String,
    pub phase: String,
    pub percentage: i32,
    pub confidence: String,
    pub risk: String,
    pub priority: String,
    pub is_dead_end: bool,
    /// JSON string of hexagon IDs that block this one
    pub blocked_by: String,
    pub notes_user: String,
    pub agent_status: String,
    pub created_at: String,
    pub updated_at: String,
}

/// A file attached to a knowledge feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeFile {
    pub id: String,
    pub feature_id: String,
    pub filename: String,
    pub filepath: String,
    pub filetype: String,
    pub filesize: i64,
    pub indexed: bool,
    pub created_at: String,
}

/// A project linked to a knowledge feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeProjectLink {
    pub id: String,
    pub feature_id: String,
    pub project_id: String,
    pub project_path: String,
    pub created_at: String,
}

/// Evidence collected for a hexagon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEvidence {
    pub id: String,
    pub hexagon_id: String,
    pub content: String,
    pub source_url: String,
    pub source_type: String,
    pub confidence: String,
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phase_roundtrip() {
        let phases = [
            HexagonPhase::Discover,
            HexagonPhase::Define,
            HexagonPhase::Validate,
            HexagonPhase::Conclude,
        ];
        for phase in &phases {
            let s = phase.as_str();
            let parsed = HexagonPhase::from_str(s).unwrap();
            assert_eq!(&parsed, phase);
        }
    }

    #[test]
    fn test_phase_unknown_returns_none() {
        assert!(HexagonPhase::from_str("unknown").is_none());
    }

    #[test]
    fn test_agent_status_roundtrip() {
        let statuses = [
            AgentStatus::Idle,
            AgentStatus::Running,
            AgentStatus::Paused,
            AgentStatus::Completed,
        ];
        for status in &statuses {
            let s = status.as_str();
            let parsed = AgentStatus::from_str(s).unwrap();
            assert_eq!(&parsed, status);
        }
    }

    #[test]
    fn test_agent_status_unknown_returns_none() {
        assert!(AgentStatus::from_str("unknown").is_none());
    }
}
