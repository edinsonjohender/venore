//! Layer analysis types
//!
//! Domain types for per-module layer inspection results.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Which aspect of the module this layer represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LayerType {
    Context,
    Tests,
    Documentation,
    Connections,
    Status,
}

impl LayerType {
    /// Parse from the string names used in wizard config.
    pub fn from_config_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "context" => Some(Self::Context),
            "tests" => Some(Self::Tests),
            "documentation" | "docs" => Some(Self::Documentation),
            "connections" => Some(Self::Connections),
            "status" => Some(Self::Status),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Context => "context",
            Self::Tests => "tests",
            Self::Documentation => "documentation",
            Self::Connections => "connections",
            Self::Status => "status",
        }
    }
}

/// How complete this layer is for a given module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LayerStatus {
    Complete,
    Partial,
    Missing,
}

impl LayerStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Missing => "missing",
        }
    }
}

/// Result of analyzing a single layer for a module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerAnalysis {
    pub layer_type: LayerType,
    pub status: LayerStatus,
    /// Evidence/details specific to this layer type.
    pub details: HashMap<String, serde_json::Value>,
}

/// All layer analyses for a single module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleLayerAnalysis {
    pub module_name: String,
    pub module_path: String,
    pub layers: Vec<LayerAnalysis>,
}

/// Dependency/dependent info for the connections layer.
#[derive(Debug, Clone, Default)]
pub struct ModuleConnectionInfo {
    pub dependencies: Vec<String>,
    pub dependents: Vec<String>,
}
