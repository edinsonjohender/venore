//! Domain entities for Venore.
//!
//! Entities have a unique identity (ID) and lifecycle.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

// ============================================================================
// PROJECT ENTITY
// ============================================================================

/// Represents a project analyzed by Venore.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// Unique project ID
    pub id: Uuid,

    /// Project name
    pub name: String,

    /// Absolute path to the project
    pub path: String,

    /// Detected islands (modules/features)
    pub islands: Vec<Island>,

    /// Metadata
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Project {
    /// Create a new project
    pub fn new(name: String, path: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            path,
            islands: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Add an island to the project
    pub fn add_island(&mut self, island: Island) {
        self.islands.push(island);
        self.updated_at = Utc::now();
    }

    /// Find an island by ID
    pub fn find_island(&self, id: &Uuid) -> Option<&Island> {
        self.islands.iter().find(|i| &i.id == id)
    }
}

// ============================================================================
// ISLAND ENTITY
// ============================================================================

/// Represents an "island" (module, feature, component).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Island {
    /// Unique island ID
    pub id: Uuid,

    /// Island name
    pub name: String,

    /// Path relative to the project
    pub relative_path: String,

    /// Island type
    pub island_type: IslandType,

    /// Nodes contained in this island
    pub nodes: Vec<Node>,

    /// Importance score (0-100)
    pub score: u8,

    /// Metadata
    pub created_at: DateTime<Utc>,
}

impl Island {
    pub fn new(name: String, relative_path: String, island_type: IslandType) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            relative_path,
            island_type,
            nodes: Vec::new(),
            score: 0,
            created_at: Utc::now(),
        }
    }

    pub fn add_node(&mut self, node: Node) {
        self.nodes.push(node);
    }
}

/// Island types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum IslandType {
    /// Full feature
    Feature,
    /// Service
    Service,
    /// UI component
    Component,
    /// Utility
    Utility,
    /// Other
    Other,
}

// ============================================================================
// NODE ENTITY
// ============================================================================

/// Represents a node in the graph (file, class, function).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// Unique node ID
    pub id: Uuid,

    /// Node name
    pub name: String,

    /// Path relative to the project
    pub file_path: String,

    /// Node type
    pub node_type: NodeType,

    /// Outgoing connections (dependencies)
    pub connections: Vec<Connection>,

    /// Position on the 3D canvas (optional)
    pub position: Option<Position>,
}

impl Node {
    pub fn new(name: String, file_path: String, node_type: NodeType) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            file_path,
            node_type,
            connections: Vec::new(),
            position: None,
        }
    }

    pub fn add_connection(&mut self, connection: Connection) {
        self.connections.push(connection);
    }
}

/// Node types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeType {
    File,
    Class,
    Function,
    Component,
    Module,
}

// ============================================================================
// CONNECTION ENTITY
// ============================================================================

/// Represents a connection between two nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    /// Source node ID
    pub from_id: Uuid,

    /// Destination node ID
    pub to_id: Uuid,

    /// Connection type
    pub connection_type: ConnectionType,

    /// Connection weight (1-10)
    pub weight: u8,
}

impl Connection {
    pub fn new(from_id: Uuid, to_id: Uuid, connection_type: ConnectionType) -> Self {
        Self {
            from_id,
            to_id,
            connection_type,
            weight: 1,
        }
    }
}

/// Connection types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConnectionType {
    Import,
    Export,
    Call,
    Dependency,
    Composition,
}

// ============================================================================
// POSITION VALUE OBJECT
// ============================================================================

/// 3D position for rendering
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Position {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn origin() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_project() {
        let project = Project::new("test-project".to_string(), "/path/to/project".to_string());
        assert_eq!(project.name, "test-project");
        assert_eq!(project.islands.len(), 0);
    }

    #[test]
    fn test_add_island_to_project() {
        let mut project = Project::new("test".to_string(), "/path".to_string());
        let island = Island::new("auth".to_string(), "src/auth".to_string(), IslandType::Feature);

        project.add_island(island.clone());
        assert_eq!(project.islands.len(), 1);
        assert!(project.find_island(&island.id).is_some());
    }

    #[test]
    fn test_create_node() {
        let node = Node::new(
            "UserService".to_string(),
            "src/services/user.ts".to_string(),
            NodeType::Class
        );
        assert_eq!(node.name, "UserService");
        assert_eq!(node.connections.len(), 0);
    }
}
