//! Types for Context Writer V2
//!
//! Comprehensive type definitions for the .context.md V2 schema.
//! Includes identity, architecture, operations, quality, versioning, and agent context.

use serde::{Deserialize, Serialize};

// ============================================================================
// IDENTITY
// ============================================================================

/// Module identity information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleIdentity {
    pub name: String,
    pub module_type: ModuleType,
    pub status: ModuleStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub tags: Vec<String>,
}

/// Type of module
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModuleType {
    Component,
    Service,
    Store,
    Util,
    Feature,
    Config,
    Hook,
}

impl std::fmt::Display for ModuleType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModuleType::Component => write!(f, "component"),
            ModuleType::Service => write!(f, "service"),
            ModuleType::Store => write!(f, "store"),
            ModuleType::Util => write!(f, "util"),
            ModuleType::Feature => write!(f, "feature"),
            ModuleType::Config => write!(f, "config"),
            ModuleType::Hook => write!(f, "hook"),
        }
    }
}

/// Development status of module
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModuleStatus {
    Stable,
    InProgress,
    Deprecated,
    Critical,
    New,
}

impl std::fmt::Display for ModuleStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModuleStatus::Stable => write!(f, "stable"),
            ModuleStatus::InProgress => write!(f, "in-progress"),
            ModuleStatus::Deprecated => write!(f, "deprecated"),
            ModuleStatus::Critical => write!(f, "critical"),
            ModuleStatus::New => write!(f, "new"),
        }
    }
}

// ============================================================================
// ARCHITECTURE
// ============================================================================

/// Documentation/completeness layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    #[serde(rename = "type")]
    pub layer_type: LayerType,
    pub status: LayerStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coverage: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// Type of documentation layer
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LayerType {
    Context,
    Tests,
    Docs,
    Integration,
    Security,
}

impl std::fmt::Display for LayerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayerType::Context => write!(f, "context"),
            LayerType::Tests => write!(f, "tests"),
            LayerType::Docs => write!(f, "docs"),
            LayerType::Integration => write!(f, "integration"),
            LayerType::Security => write!(f, "security"),
        }
    }
}

/// Completeness status of layer
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LayerStatus {
    Complete,
    Partial,
    InProgress,
    Missing,
}

impl std::fmt::Display for LayerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayerStatus::Complete => write!(f, "complete"),
            LayerStatus::Partial => write!(f, "partial"),
            LayerStatus::InProgress => write!(f, "in-progress"),
            LayerStatus::Missing => write!(f, "missing"),
        }
    }
}

/// Connection to another module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub to: String,
    #[serde(rename = "type")]
    pub connection_type: ConnectionType,
    pub description: String,
    pub critical: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_flow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_contract: Option<Vec<String>>,
}

/// Type of module connection
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionType {
    Consumes,
    Provides,
    Syncs,
    Triggers,
}

impl std::fmt::Display for ConnectionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionType::Consumes => write!(f, "consumes"),
            ConnectionType::Provides => write!(f, "provides"),
            ConnectionType::Syncs => write!(f, "syncs"),
            ConnectionType::Triggers => write!(f, "triggers"),
        }
    }
}

/// Module dependencies
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct Dependencies {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub internal: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub external: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub optional: Vec<String>,
}


// ============================================================================
// OPERATIONS (NOT CURRENTLY USED)
// ============================================================================
// NOTE: These types are defined but NOT used in ContextMetadata.
// They require manual input or external tools to populate.
// Kept for future use when we implement manual editing or Git integration.

/// Deployment and operational information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Deployment {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub environment_vars: Vec<EnvVar>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dev_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub performance: Option<PerformanceTarget>,
}

/// Environment variable configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct EnvVar {
    pub name: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,
}

/// Performance targets/expectations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct PerformanceTarget {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_latency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_limit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle_size: Option<String>,
}

// ============================================================================
// QUALITY (NOT CURRENTLY USED)
// ============================================================================
// NOTE: These types are defined but NOT used in ContextMetadata.
// They require external tools (Jest, ESLint, etc.) to populate.
// Kept for future use when we implement external tool integrations.

/// Quality metrics and audit information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Quality {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_coverage: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_coverage: Option<u8>,
    pub complexity_score: ComplexityScore,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub performance_grade: Option<char>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_audit: Option<SecurityAudit>,
}

/// Code complexity assessment
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[derive(Default)]
pub enum ComplexityScore {
    #[default]
    Low,
    Medium,
    High,
    VeryHigh,
}

impl std::fmt::Display for ComplexityScore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComplexityScore::Low => write!(f, "Low"),
            ComplexityScore::Medium => write!(f, "Medium"),
            ComplexityScore::High => write!(f, "High"),
            ComplexityScore::VeryHigh => write!(f, "Very High"),
        }
    }
}


/// Security audit information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct SecurityAudit {
    pub last_reviewed: String,
    pub status: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub known_issues: Vec<String>,
}

// ============================================================================
// VERSIONING (NOT CURRENTLY USED)
// ============================================================================
// NOTE: These types are defined but NOT used in ContextMetadata.
// They require Git history integration to populate.
// Kept for future use when we implement Git integration.

/// Version and change management
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Versioning {
    pub current_version: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub breaking_changes: Vec<BreakingChange>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub deprecations: Vec<Deprecation>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub roadmap: Vec<RoadmapItem>,
}

/// Breaking change documentation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct BreakingChange {
    pub version: String,
    pub date: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub migration_guide: Option<String>,
}

/// Deprecation warning
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Deprecation {
    pub feature: String,
    pub deprecated_in: String,
    pub removed_in: String,
    pub alternative: String,
}

/// Roadmap item
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct RoadmapItem {
    pub feature: String,
    pub priority: Priority,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eta: Option<String>,
}

/// Priority level
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Priority::Low => write!(f, "low"),
            Priority::Medium => write!(f, "medium"),
            Priority::High => write!(f, "high"),
            Priority::Critical => write!(f, "critical"),
        }
    }
}

// ============================================================================
// AGENT CONTEXT
// ============================================================================

/// Context for AI agents (Claude, OpenCode, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentContext {
    pub scope: AgentScope,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub safe_operations: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub risky_operations: Vec<String>,
    pub complexity_level: ComplexityLevel,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub suggested_agents: Vec<SuggestedAgent>,
}

/// Scope of operations allowed for agent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentScope {
    Exploration,
    Modification,
    Both,
}

impl std::fmt::Display for AgentScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentScope::Exploration => write!(f, "exploration"),
            AgentScope::Modification => write!(f, "modification"),
            AgentScope::Both => write!(f, "both"),
        }
    }
}

/// Complexity level for agent planning
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[derive(Default)]
pub enum ComplexityLevel {
    #[default]
    Low,
    Medium,
    High,
    VeryHigh,
}

impl std::fmt::Display for ComplexityLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComplexityLevel::Low => write!(f, "low"),
            ComplexityLevel::Medium => write!(f, "medium"),
            ComplexityLevel::High => write!(f, "high"),
            ComplexityLevel::VeryHigh => write!(f, "very-high"),
        }
    }
}


/// Suggested agent and tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedAgent {
    #[serde(rename = "type")]
    pub agent_type: String,
    pub tasks: Vec<String>,
}

// ============================================================================
// GENERATION CONFIGURATION
// ============================================================================

/// Depth level for context generation (from V1)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum DepthLevel {
    /// Minimal: ~500-800 tokens, no code snippets
    Minimal,
    /// Normal: ~1.5-2K tokens, 1 code snippet (10 lines) [DEFAULT]
    #[default]
    Normal,
    /// Detailed: ~3-4K tokens, 3 code snippets (300 chars)
    Detailed,
    /// Expert: ~5-8K tokens, 5 code snippets (500 chars)
    Expert,
}


impl std::fmt::Display for DepthLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DepthLevel::Minimal => write!(f, "minimal"),
            DepthLevel::Normal => write!(f, "normal"),
            DepthLevel::Detailed => write!(f, "detailed"),
            DepthLevel::Expert => write!(f, "expert"),
        }
    }
}

impl DepthLevel {
    /// Returns the max_tokens limit for LLM requests based on depth level.
    pub fn max_tokens(&self) -> u32 {
        match self {
            DepthLevel::Minimal => 1500,
            DepthLevel::Normal => 4000,
            DepthLevel::Detailed => 6000,
            DepthLevel::Expert => 10000,
        }
    }
}

/// Configuration for context generation
#[derive(Debug, Clone)]
pub struct GenerationConfig {
    /// Depth level for analysis
    pub depth_level: DepthLevel,
    /// Layers to generate (future: will allow selecting specific layers)
    pub layers_to_generate: Vec<LayerType>,
    /// File/folder exclusions (e.g., "node_modules/", "*.test.ts")
    pub exclusions: Vec<String>,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            depth_level: DepthLevel::Normal,
            layers_to_generate: vec![LayerType::Context], // Only context for now
            exclusions: vec![
                "node_modules/".to_string(),
                ".git/".to_string(),
                "dist/".to_string(),
                "build/".to_string(),
            ],
        }
    }
}

// ============================================================================
// GENERATION METADATA
// ============================================================================

/// Metadata about context generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationMetadata {
    pub analyzed_at: String,
    pub agent: String,
    pub model: String,
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_hash: Option<String>,
    pub stale: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_used: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_time_ms: Option<u64>,
}

// ============================================================================
// MASTER TYPE
// ============================================================================

/// Complete context metadata for V2 schema
///
/// NOTE: Removed fields (not auto-generated by LLM):
/// - `deployment` - Requires manual configuration (env vars, commands, etc.)
/// - `quality` - Requires external tools (Jest, ESLint, etc.)
/// - `versioning` - Requires Git history integration
///
/// These can be re-added in future versions when we implement:
/// - Manual metadata editing UI
/// - Git integration for changelog
/// - External tool integrations for metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextMetadata {
    pub identity: ModuleIdentity,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub layers: Vec<Layer>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub connections: Vec<Connection>,
    pub dependencies: Dependencies,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_context: Option<AgentContext>,
    pub generation: GenerationMetadata,
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_identity_creation() {
        let identity = ModuleIdentity {
            name: "TestModule".to_string(),
            module_type: ModuleType::Component,
            status: ModuleStatus::Stable,
            owner: Some("@team".to_string()),
            tags: vec!["ui".to_string(), "react".to_string()],
        };

        assert_eq!(identity.name, "TestModule");
        assert_eq!(identity.module_type, ModuleType::Component);
        assert_eq!(identity.status, ModuleStatus::Stable);
    }

    #[test]
    fn test_module_type_display() {
        assert_eq!(ModuleType::Component.to_string(), "component");
        assert_eq!(ModuleType::Service.to_string(), "service");
        assert_eq!(ModuleType::Store.to_string(), "store");
    }

    #[test]
    fn test_module_status_display() {
        assert_eq!(ModuleStatus::Stable.to_string(), "stable");
        assert_eq!(ModuleStatus::InProgress.to_string(), "in-progress");
        assert_eq!(ModuleStatus::Deprecated.to_string(), "deprecated");
    }

    #[test]
    fn test_layer_serialization() {
        let layer = Layer {
            layer_type: LayerType::Context,
            status: LayerStatus::Complete,
            coverage: Some(95),
            notes: None,
        };

        let json = serde_json::to_string(&layer).unwrap();
        assert!(json.contains("context"));
        assert!(json.contains("complete"));
        assert!(json.contains("95"));
    }

    #[test]
    fn test_connection_creation() {
        let conn = Connection {
            to: "OtherModule".to_string(),
            connection_type: ConnectionType::Consumes,
            description: "Reads data from OtherModule".to_string(),
            critical: true,
            data_flow: Some("request→validate→response".to_string()),
            api_contract: Some(vec!["getData(): Promise<Data>".to_string()]),
        };

        assert_eq!(conn.to, "OtherModule");
        assert_eq!(conn.connection_type, ConnectionType::Consumes);
        assert!(conn.critical);
    }

    #[test]
    fn test_dependencies_default() {
        let deps = Dependencies::default();
        assert!(deps.internal.is_empty());
        assert!(deps.external.is_empty());
        assert!(deps.optional.is_empty());
    }

    #[test]
    fn test_complexity_score_default() {
        let score = ComplexityScore::default();
        assert_eq!(score, ComplexityScore::Low);
        assert_eq!(score.to_string(), "Low");
    }

    #[test]
    fn test_complexity_level_default() {
        let level = ComplexityLevel::default();
        assert_eq!(level, ComplexityLevel::Low);
        assert_eq!(level.to_string(), "low");
    }

    #[test]
    fn test_generation_metadata_creation() {
        let metadata = GenerationMetadata {
            analyzed_at: "2026-01-21T20:00:00Z".to_string(),
            agent: "venore-context-agent-v3".to_string(),
            model: "claude-sonnet-4-5".to_string(),
            provider: "anthropic".to_string(),
            code_hash: Some("abc123".to_string()),
            stale: false,
            tokens_used: Some(3200),
            generation_time_ms: Some(4500),
        };

        assert_eq!(metadata.agent, "venore-context-agent-v3");
        assert_eq!(metadata.tokens_used, Some(3200));
        assert!(!metadata.stale);
    }

    #[test]
    fn test_context_metadata_minimal() {
        let metadata = ContextMetadata {
            identity: ModuleIdentity {
                name: "Test".to_string(),
                module_type: ModuleType::Component,
                status: ModuleStatus::New,
                owner: None,
                tags: vec![],
            },
            layers: vec![],
            connections: vec![],
            dependencies: Dependencies::default(),
            agent_context: None,
            generation: GenerationMetadata {
                analyzed_at: "2026-01-21T20:00:00Z".to_string(),
                agent: "test-agent".to_string(),
                model: "test-model".to_string(),
                provider: "test".to_string(),
                code_hash: None,
                stale: false,
                tokens_used: None,
                generation_time_ms: None,
            },
        };

        assert_eq!(metadata.identity.name, "Test");
        assert!(metadata.layers.is_empty());
        assert!(metadata.agent_context.is_none());
    }

    #[test]
    fn test_context_metadata_serialization() {
        let metadata = ContextMetadata {
            identity: ModuleIdentity {
                name: "Test".to_string(),
                module_type: ModuleType::Component,
                status: ModuleStatus::Stable,
                owner: None,
                tags: vec!["ui".to_string()],
            },
            layers: vec![],
            connections: vec![],
            dependencies: Dependencies::default(),
            agent_context: None,
            generation: GenerationMetadata {
                analyzed_at: "2026-01-21T20:00:00Z".to_string(),
                agent: "test".to_string(),
                model: "test".to_string(),
                provider: "test".to_string(),
                code_hash: None,
                stale: false,
                tokens_used: None,
                generation_time_ms: None,
            },
        };

        // Should serialize without errors
        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("Test"));
        assert!(json.contains("component"));
        assert!(json.contains("stable"));
    }

    #[test]
    fn test_versioning_with_breaking_changes() {
        let versioning = Versioning {
            current_version: "2.0.0".to_string(),
            breaking_changes: vec![BreakingChange {
                version: "2.0.0".to_string(),
                date: "2026-01-15".to_string(),
                description: "Changed API".to_string(),
                migration_guide: Some("See MIGRATION.md".to_string()),
            }],
            deprecations: vec![],
            roadmap: vec![],
        };

        assert_eq!(versioning.current_version, "2.0.0");
        assert_eq!(versioning.breaking_changes.len(), 1);
        assert_eq!(versioning.breaking_changes[0].version, "2.0.0");
    }

    #[test]
    fn test_agent_context_with_operations() {
        let agent_context = AgentContext {
            scope: AgentScope::Both,
            safe_operations: vec!["Read data".to_string(), "Analyze structure".to_string()],
            risky_operations: vec!["Modify schema".to_string()],
            complexity_level: ComplexityLevel::Medium,
            suggested_agents: vec![SuggestedAgent {
                agent_type: "plan".to_string(),
                tasks: vec!["Review architecture".to_string()],
            }],
        };

        assert_eq!(agent_context.scope, AgentScope::Both);
        assert_eq!(agent_context.safe_operations.len(), 2);
        assert_eq!(agent_context.risky_operations.len(), 1);
        assert_eq!(agent_context.complexity_level, ComplexityLevel::Medium);
    }

    #[test]
    fn test_depth_level_max_tokens() {
        assert_eq!(DepthLevel::Minimal.max_tokens(), 1500);
        assert_eq!(DepthLevel::Normal.max_tokens(), 4000);
        assert_eq!(DepthLevel::Detailed.max_tokens(), 6000);
        assert_eq!(DepthLevel::Expert.max_tokens(), 10000);
    }
}
