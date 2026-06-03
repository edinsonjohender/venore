//! Project Identity
//!
//! Types representing a project's stable identity stored in `.venore/project.json`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Stable project identity stored in `.venore/project.json`.
/// Travels with the project directory when moved.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectIdentity {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

impl ProjectIdentity {
    /// Create a new project identity with a fresh UUID
    pub fn new(name: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.to_string(),
            created_at: Utc::now(),
        }
    }
}

/// A project registered in the app database (identity + current path)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredProject {
    pub id: Uuid,
    pub name: String,
    pub path: String,
    /// Project type: "code" (default) or "knowledge"
    pub project_type: String,
    pub created_at: DateTime<Utc>,
    pub last_opened_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_generates_valid_uuid() {
        let identity = ProjectIdentity::new("my-project");
        assert_eq!(identity.name, "my-project");
        assert!(!identity.id.is_nil());
    }

    #[test]
    fn test_two_identities_have_different_ids() {
        let a = ProjectIdentity::new("a");
        let b = ProjectIdentity::new("b");
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let identity = ProjectIdentity::new("test-project");
        let json = serde_json::to_string(&identity).unwrap();
        let deserialized: ProjectIdentity = serde_json::from_str(&json).unwrap();
        assert_eq!(identity.id, deserialized.id);
        assert_eq!(identity.name, deserialized.name);
    }
}
