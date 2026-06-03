//! Frontmatter Builder for Context V2
//!
//! Generates YAML frontmatter from ContextMetadata following the V2 schema.

use crate::context::types::*;
use crate::error::Result;

/// Builder for YAML frontmatter
pub struct FrontmatterBuilder;

impl FrontmatterBuilder {
    /// Build complete YAML frontmatter from metadata
    pub fn build(metadata: &ContextMetadata) -> Result<String> {
        let mut yaml = String::new();

        yaml.push_str("---\n");

        // Identity section
        Self::add_identity(&mut yaml, &metadata.identity);

        // Architecture section
        if !metadata.layers.is_empty() {
            Self::add_layers(&mut yaml, &metadata.layers);
        }

        if !metadata.connections.is_empty() {
            Self::add_connections(&mut yaml, &metadata.connections);
        }

        Self::add_dependencies(&mut yaml, &metadata.dependencies);

        // NOTE: Operations, Quality, and Versioning sections removed
        // These fields are no longer in ContextMetadata.
        // See types.rs for explanation.

        // Agent context section
        if let Some(agent_context) = &metadata.agent_context {
            Self::add_agent_context(&mut yaml, agent_context);
        }

        // Generation metadata
        Self::add_generation(&mut yaml, &metadata.generation);

        yaml.push_str("---\n");

        Ok(yaml)
    }

    // ========================================================================
    // IDENTITY
    // ========================================================================

    fn add_identity(yaml: &mut String, identity: &ModuleIdentity) {
        yaml.push_str(&format!("name: \"{}\"\n", Self::escape_yaml(&identity.name)));
        yaml.push_str(&format!("type: {}\n", identity.module_type));
        yaml.push_str(&format!("status: {}\n", identity.status));

        if let Some(owner) = &identity.owner {
            yaml.push_str(&format!("owner: \"{}\"\n", Self::escape_yaml(owner)));
        }

        if !identity.tags.is_empty() {
            yaml.push_str("tags: [");
            yaml.push_str(
                &identity
                    .tags
                    .iter()
                    .map(|tag| tag.to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            yaml.push_str("]\n");
        }

        yaml.push('\n');
    }

    // ========================================================================
    // ARCHITECTURE
    // ========================================================================

    fn add_layers(yaml: &mut String, layers: &[Layer]) {
        yaml.push_str("layers:\n");
        for layer in layers {
            yaml.push_str(&format!("  - type: {}\n", layer.layer_type));
            yaml.push_str(&format!("    status: {}\n", layer.status));

            if let Some(coverage) = layer.coverage {
                yaml.push_str(&format!("    coverage: {}%\n", coverage));
            }

            if let Some(notes) = &layer.notes {
                yaml.push_str(&format!("    notes: \"{}\"\n", Self::escape_yaml(notes)));
            }
        }
        yaml.push('\n');
    }

    fn add_connections(yaml: &mut String, connections: &[Connection]) {
        yaml.push_str("connections:\n");
        for conn in connections {
            yaml.push_str(&format!("  - to: \"{}\"\n", Self::escape_yaml(&conn.to)));
            yaml.push_str(&format!("    type: {}\n", conn.connection_type));
            yaml.push_str(&format!(
                "    description: \"{}\"\n",
                Self::escape_yaml(&conn.description)
            ));
            yaml.push_str(&format!("    critical: {}\n", conn.critical));

            if let Some(data_flow) = &conn.data_flow {
                yaml.push_str(&format!(
                    "    data_flow: \"{}\"\n",
                    Self::escape_yaml(data_flow)
                ));
            }

            if let Some(api_contract) = &conn.api_contract {
                if !api_contract.is_empty() {
                    yaml.push_str("    api_contract:\n");
                    for method in api_contract {
                        yaml.push_str(&format!("      - \"{}\"\n", Self::escape_yaml(method)));
                    }
                }
            }
        }
        yaml.push('\n');
    }

    fn add_dependencies(yaml: &mut String, deps: &Dependencies) {
        if deps.internal.is_empty() && deps.external.is_empty() && deps.optional.is_empty() {
            return;
        }

        yaml.push_str("dependencies:\n");

        if !deps.internal.is_empty() {
            yaml.push_str("  internal:\n");
            for dep in &deps.internal {
                yaml.push_str(&format!("    - \"{}\"\n", Self::escape_yaml(dep)));
            }
        }

        if !deps.external.is_empty() {
            yaml.push_str("  external:\n");
            for dep in &deps.external {
                yaml.push_str(&format!("    - \"{}\"\n", Self::escape_yaml(dep)));
            }
        }

        if !deps.optional.is_empty() {
            yaml.push_str("  optional:\n");
            for dep in &deps.optional {
                yaml.push_str(&format!("    - \"{}\"\n", Self::escape_yaml(dep)));
            }
        }

        yaml.push('\n');
    }

    // ========================================================================
    // OPERATIONS
    // ========================================================================

    #[allow(dead_code)]
    fn add_deployment(yaml: &mut String, deployment: &Deployment) {
        yaml.push_str("deployment:\n");

        if let Some(runtime) = &deployment.runtime {
            yaml.push_str(&format!("  runtime: \"{}\"\n", Self::escape_yaml(runtime)));
        }

        if !deployment.environment_vars.is_empty() {
            yaml.push_str("  environment_vars:\n");
            for env_var in &deployment.environment_vars {
                yaml.push_str(&format!("    - name: \"{}\"\n", Self::escape_yaml(&env_var.name)));
                yaml.push_str(&format!("      required: {}\n", env_var.required));

                if let Some(default) = &env_var.default {
                    yaml.push_str(&format!("      default: \"{}\"\n", Self::escape_yaml(default)));
                }

                if let Some(example) = &env_var.example {
                    yaml.push_str(&format!("      example: \"{}\"\n", Self::escape_yaml(example)));
                }
            }
        }

        if let Some(build_cmd) = &deployment.build_command {
            yaml.push_str(&format!(
                "  build_command: \"{}\"\n",
                Self::escape_yaml(build_cmd)
            ));
        }

        if let Some(dev_cmd) = &deployment.dev_command {
            yaml.push_str(&format!("  dev_command: \"{}\"\n", Self::escape_yaml(dev_cmd)));
        }

        if let Some(performance) = &deployment.performance {
            yaml.push_str("  performance:\n");

            if let Some(latency) = &performance.target_latency {
                yaml.push_str(&format!(
                    "    target_latency: \"{}\"\n",
                    Self::escape_yaml(latency)
                ));
            }

            if let Some(memory) = &performance.memory_limit {
                yaml.push_str(&format!(
                    "    memory_limit: \"{}\"\n",
                    Self::escape_yaml(memory)
                ));
            }

            if let Some(bundle) = &performance.bundle_size {
                yaml.push_str(&format!(
                    "    bundle_size: \"{}\"\n",
                    Self::escape_yaml(bundle)
                ));
            }
        }

        yaml.push('\n');
    }

    // ========================================================================
    // QUALITY
    // ========================================================================

    #[allow(dead_code)]
    fn add_quality(yaml: &mut String, quality: &Quality) {
        yaml.push_str("quality:\n");

        if let Some(test_coverage) = quality.test_coverage {
            yaml.push_str(&format!("  test_coverage: {}%\n", test_coverage));
        }

        if let Some(type_coverage) = quality.type_coverage {
            yaml.push_str(&format!("  type_coverage: {}%\n", type_coverage));
        }

        yaml.push_str(&format!("  complexity_score: \"{}\"\n", quality.complexity_score));

        if let Some(grade) = quality.performance_grade {
            yaml.push_str(&format!("  performance_grade: \"{}\"\n", grade));
        }

        if let Some(audit) = &quality.security_audit {
            yaml.push_str("  security_audit:\n");
            yaml.push_str(&format!(
                "    last_reviewed: \"{}\"\n",
                Self::escape_yaml(&audit.last_reviewed)
            ));
            yaml.push_str(&format!("    status: \"{}\"\n", Self::escape_yaml(&audit.status)));

            if !audit.known_issues.is_empty() {
                yaml.push_str("    known_issues:\n");
                for issue in &audit.known_issues {
                    yaml.push_str(&format!("      - \"{}\"\n", Self::escape_yaml(issue)));
                }
            }
        }

        yaml.push('\n');
    }

    // ========================================================================
    // VERSIONING
    // ========================================================================

    #[allow(dead_code)]
    fn add_versioning(yaml: &mut String, versioning: &Versioning) {
        yaml.push_str("versioning:\n");
        yaml.push_str(&format!(
            "  current_version: \"{}\"\n",
            Self::escape_yaml(&versioning.current_version)
        ));

        if !versioning.breaking_changes.is_empty() {
            yaml.push_str("  breaking_changes:\n");
            for change in &versioning.breaking_changes {
                yaml.push_str(&format!(
                    "    - version: \"{}\"\n",
                    Self::escape_yaml(&change.version)
                ));
                yaml.push_str(&format!("      date: \"{}\"\n", Self::escape_yaml(&change.date)));
                yaml.push_str(&format!(
                    "      description: \"{}\"\n",
                    Self::escape_yaml(&change.description)
                ));

                if let Some(migration) = &change.migration_guide {
                    yaml.push_str(&format!(
                        "      migration_guide: \"{}\"\n",
                        Self::escape_yaml(migration)
                    ));
                }
            }
        }

        if !versioning.deprecations.is_empty() {
            yaml.push_str("  deprecations:\n");
            for deprecation in &versioning.deprecations {
                yaml.push_str(&format!(
                    "    - feature: \"{}\"\n",
                    Self::escape_yaml(&deprecation.feature)
                ));
                yaml.push_str(&format!(
                    "      deprecated_in: \"{}\"\n",
                    Self::escape_yaml(&deprecation.deprecated_in)
                ));
                yaml.push_str(&format!(
                    "      removed_in: \"{}\"\n",
                    Self::escape_yaml(&deprecation.removed_in)
                ));
                yaml.push_str(&format!(
                    "      alternative: \"{}\"\n",
                    Self::escape_yaml(&deprecation.alternative)
                ));
            }
        }

        if !versioning.roadmap.is_empty() {
            yaml.push_str("  roadmap:\n");
            for item in &versioning.roadmap {
                yaml.push_str(&format!(
                    "    - feature: \"{}\"\n",
                    Self::escape_yaml(&item.feature)
                ));
                yaml.push_str(&format!("      priority: {}\n", item.priority));

                if let Some(eta) = &item.eta {
                    yaml.push_str(&format!("      eta: \"{}\"\n", Self::escape_yaml(eta)));
                }
            }
        }

        yaml.push('\n');
    }

    // ========================================================================
    // AGENT CONTEXT
    // ========================================================================

    fn add_agent_context(yaml: &mut String, context: &AgentContext) {
        yaml.push_str("agent_context:\n");
        yaml.push_str(&format!("  scope: {}\n", context.scope));

        if !context.safe_operations.is_empty() {
            yaml.push_str("  safe_operations:\n");
            for op in &context.safe_operations {
                yaml.push_str(&format!("    - \"{}\"\n", Self::escape_yaml(op)));
            }
        }

        if !context.risky_operations.is_empty() {
            yaml.push_str("  risky_operations:\n");
            for op in &context.risky_operations {
                yaml.push_str(&format!("    - \"{}\"\n", Self::escape_yaml(op)));
            }
        }

        yaml.push_str(&format!("  complexity_level: {}\n", context.complexity_level));

        if !context.suggested_agents.is_empty() {
            yaml.push_str("  suggested_agents:\n");
            for agent in &context.suggested_agents {
                yaml.push_str(&format!(
                    "    - type: \"{}\"\n",
                    Self::escape_yaml(&agent.agent_type)
                ));
                yaml.push_str("      tasks:\n");
                for task in &agent.tasks {
                    yaml.push_str(&format!("        - \"{}\"\n", Self::escape_yaml(task)));
                }
            }
        }

        yaml.push('\n');
    }

    // ========================================================================
    // GENERATION
    // ========================================================================

    fn add_generation(yaml: &mut String, generation: &GenerationMetadata) {
        yaml.push_str("analyzed:\n");
        yaml.push_str(&format!(
            "  at: \"{}\"\n",
            Self::escape_yaml(&generation.analyzed_at)
        ));
        yaml.push_str(&format!("  agent: \"{}\"\n", Self::escape_yaml(&generation.agent)));
        yaml.push_str(&format!("  model: \"{}\"\n", Self::escape_yaml(&generation.model)));
        yaml.push_str(&format!(
            "  provider: \"{}\"\n",
            Self::escape_yaml(&generation.provider)
        ));

        if let Some(code_hash) = &generation.code_hash {
            yaml.push_str(&format!("  codeHash: \"{}\"\n", Self::escape_yaml(code_hash)));
        }

        yaml.push_str(&format!("  stale: {}\n", generation.stale));

        if let Some(tokens) = generation.tokens_used {
            yaml.push_str(&format!("  tokensUsed: {}\n", tokens));
        }

        if let Some(time_ms) = generation.generation_time_ms {
            yaml.push_str(&format!("  generation_time_ms: {}\n", time_ms));
        }
    }

    // ========================================================================
    // UTILITIES
    // ========================================================================

    /// Escape special characters in YAML strings
    fn escape_yaml(s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
    }

    // ========================================================================
    // READING FRONTMATTER
    // ========================================================================

    /// Extract frontmatter YAML from a .context.md file
    ///
    /// Returns the YAML content between `---` delimiters (without the delimiters).
    pub fn extract_frontmatter(content: &str) -> Option<String> {
        let lines: Vec<&str> = content.lines().collect();

        // Find first ---
        let start = lines.iter().position(|line| line.trim() == "---")?;

        // Find second ---
        let end = lines.iter().skip(start + 1).position(|line| line.trim() == "---")?;

        // Extract YAML content (between the two ---)
        let yaml_lines = &lines[start + 1..start + 1 + end];
        Some(yaml_lines.join("\n"))
    }

    /// Read code hash from frontmatter YAML
    ///
    /// Parses the YAML and extracts the `analyzed.codeHash` field.
    pub fn read_code_hash(frontmatter_yaml: &str) -> Option<String> {
        use serde_yaml::Value;

        let yaml: Value = serde_yaml::from_str(frontmatter_yaml).ok()?;

        // Navigate to analyzed.codeHash
        yaml.get("analyzed")?
            .get("codeHash")?
            .as_str()
            .map(|s| s.to_string())
    }

    /// Read code hash from .context.md file path
    ///
    /// Convenience function that reads file, extracts frontmatter, and returns hash.
    pub fn read_code_hash_from_file(context_path: &std::path::Path) -> Result<Option<String>> {
        use std::fs;

        let content = fs::read_to_string(context_path)
            .map_err(|e| crate::error::VenoreError::FileReadError(
                format!("{}: {}", context_path.display(), e)
            ))?;

        let frontmatter = match Self::extract_frontmatter(&content) {
            Some(fm) => fm,
            None => return Ok(None),
        };

        Ok(Self::read_code_hash(&frontmatter))
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_minimal_metadata() -> ContextMetadata {
        ContextMetadata {
            identity: ModuleIdentity {
                name: "TestModule".to_string(),
                module_type: ModuleType::Component,
                status: ModuleStatus::Stable,
                owner: None,
                tags: vec!["test".to_string()],
            },
            layers: vec![],
            connections: vec![],
            dependencies: Dependencies::default(),
            agent_context: None,
            generation: GenerationMetadata {
                analyzed_at: Utc::now().to_rfc3339(),
                agent: "test-agent".to_string(),
                model: "test-model".to_string(),
                provider: "test".to_string(),
                code_hash: None,
                stale: false,
                tokens_used: None,
                generation_time_ms: None,
            },
        }
    }

    #[test]
    fn test_build_minimal_frontmatter() {
        let metadata = create_minimal_metadata();
        let yaml = FrontmatterBuilder::build(&metadata).unwrap();

        assert!(yaml.starts_with("---\n"));
        assert!(yaml.ends_with("---\n"));
        assert!(yaml.contains("name: \"TestModule\""));
        assert!(yaml.contains("type: component"));
        assert!(yaml.contains("status: stable"));
        assert!(yaml.contains("tags: [test]"));
    }

    #[test]
    fn test_frontmatter_with_layers() {
        let mut metadata = create_minimal_metadata();
        metadata.layers = vec![
            Layer {
                layer_type: LayerType::Context,
                status: LayerStatus::Complete,
                coverage: Some(100),
                notes: None,
            },
            Layer {
                layer_type: LayerType::Tests,
                status: LayerStatus::Partial,
                coverage: Some(68),
                notes: Some("Missing integration tests".to_string()),
            },
        ];

        let yaml = FrontmatterBuilder::build(&metadata).unwrap();

        assert!(yaml.contains("layers:"));
        assert!(yaml.contains("type: context"));
        assert!(yaml.contains("status: complete"));
        assert!(yaml.contains("coverage: 100%"));
        assert!(yaml.contains("type: tests"));
        assert!(yaml.contains("coverage: 68%"));
        assert!(yaml.contains("notes: \"Missing integration tests\""));
    }

    #[test]
    fn test_frontmatter_with_connections() {
        let mut metadata = create_minimal_metadata();
        metadata.connections = vec![Connection {
            to: "OtherModule".to_string(),
            connection_type: ConnectionType::Consumes,
            description: "Reads data".to_string(),
            critical: true,
            data_flow: Some("request→response".to_string()),
            api_contract: Some(vec!["getData(): Promise<Data>".to_string()]),
        }];

        let yaml = FrontmatterBuilder::build(&metadata).unwrap();

        assert!(yaml.contains("connections:"));
        assert!(yaml.contains("to: \"OtherModule\""));
        assert!(yaml.contains("type: consumes"));
        assert!(yaml.contains("description: \"Reads data\""));
        assert!(yaml.contains("critical: true"));
        assert!(yaml.contains("data_flow: \"request→response\""));
        assert!(yaml.contains("api_contract:"));
    }

    #[test]
    fn test_frontmatter_with_dependencies() {
        let mut metadata = create_minimal_metadata();
        metadata.dependencies = Dependencies {
            internal: vec!["@/stores/workspace".to_string()],
            external: vec!["react@^18.0.0".to_string(), "three@^0.150.0".to_string()],
            optional: vec!["framer-motion@^10.0.0".to_string()],
        };

        let yaml = FrontmatterBuilder::build(&metadata).unwrap();

        assert!(yaml.contains("dependencies:"));
        assert!(yaml.contains("internal:"));
        assert!(yaml.contains("@/stores/workspace"));
        assert!(yaml.contains("external:"));
        assert!(yaml.contains("react@^18.0.0"));
        assert!(yaml.contains("optional:"));
        assert!(yaml.contains("framer-motion@^10.0.0"));
    }

    // NOTE: Tests disabled - deployment, quality, versioning removed from ContextMetadata
    // These fields require manual input or external tools to populate.
    // Re-enable when we implement:
    // - Manual metadata editing UI
    // - Git integration for changelog
    // - External tool integrations for metrics

    // #[test]
    // fn test_frontmatter_with_deployment() { ... }

    // #[test]
    // fn test_frontmatter_with_quality() { ... }

    // #[test]
    // fn test_frontmatter_with_versioning() { ... }

    #[test]
    fn test_frontmatter_with_agent_context() {
        let mut metadata = create_minimal_metadata();
        metadata.agent_context = Some(AgentContext {
            scope: AgentScope::Both,
            safe_operations: vec!["Read data".to_string()],
            risky_operations: vec!["Modify schema".to_string()],
            complexity_level: ComplexityLevel::Medium,
            suggested_agents: vec![SuggestedAgent {
                agent_type: "plan".to_string(),
                tasks: vec!["Review architecture".to_string()],
            }],
        });

        let yaml = FrontmatterBuilder::build(&metadata).unwrap();

        assert!(yaml.contains("agent_context:"));
        assert!(yaml.contains("scope: both"));
        assert!(yaml.contains("safe_operations:"));
        assert!(yaml.contains("risky_operations:"));
        assert!(yaml.contains("complexity_level: medium"));
        assert!(yaml.contains("suggested_agents:"));
    }

    #[test]
    fn test_yaml_escape() {
        assert_eq!(FrontmatterBuilder::escape_yaml("normal text"), "normal text");
        assert_eq!(
            FrontmatterBuilder::escape_yaml("text with \"quotes\""),
            "text with \\\"quotes\\\""
        );
        assert_eq!(
            FrontmatterBuilder::escape_yaml("text with \\ backslash"),
            "text with \\\\ backslash"
        );
        assert_eq!(
            FrontmatterBuilder::escape_yaml("text with\nnewline"),
            "text with\\nnewline"
        );
    }

    #[test]
    fn test_generation_metadata() {
        let metadata = create_minimal_metadata();
        let yaml = FrontmatterBuilder::build(&metadata).unwrap();

        assert!(yaml.contains("analyzed:"));
        assert!(yaml.contains("agent: \"test-agent\""));
        assert!(yaml.contains("model: \"test-model\""));
        assert!(yaml.contains("provider: \"test\""));
        assert!(yaml.contains("stale: false"));
    }
}
