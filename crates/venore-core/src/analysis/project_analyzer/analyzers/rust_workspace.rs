use crate::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;

use super::common::rust as rust_common;
use super::super::traits::{
    DetectedFramework, ModuleDetectionStrategy, ProjectAnalyzer, ProjectType,
    ProjectTypeDetection,
};

/// Analyzer for Rust workspace projects.
///
/// Detects projects with:
/// - Cargo.toml at the root
/// - A [workspace] section with members
pub struct RustWorkspaceAnalyzer;

impl Default for RustWorkspaceAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl RustWorkspaceAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Read and parse Cargo.toml.
    fn parse_cargo_toml(&self, cargo_toml_path: &Path) -> Option<toml::Value> {
        let content = std::fs::read_to_string(cargo_toml_path).ok()?;
        content.parse::<toml::Value>().ok()
    }

    /// Check whether Cargo.toml declares a [workspace] section.
    fn has_workspace_section(&self, toml: &toml::Value) -> bool {
        toml.get("workspace").is_some()
    }

    /// Extract the workspace members.
    fn get_workspace_members(&self, toml: &toml::Value) -> Vec<String> {
        toml.get("workspace")
            .and_then(|w| w.get("members"))
            .and_then(|m| m.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Extract the Rust edition.
    fn get_rust_edition(&self, toml: &toml::Value) -> Option<String> {
        toml.get("package")
            .and_then(|p| p.get("edition"))
            .and_then(|e| e.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                // Some workspaces define edition under [workspace]
                toml.get("workspace")
                    .and_then(|w| w.get("package"))
                    .and_then(|p| p.get("edition"))
                    .and_then(|e| e.as_str())
                    .map(|s| s.to_string())
            })
    }
}

#[async_trait]
impl ProjectAnalyzer for RustWorkspaceAnalyzer {
    fn name(&self) -> &str {
        "rust-workspace"
    }

    fn project_type(&self) -> ProjectType {
        ProjectType::RustWorkspace
    }

    async fn detect(&self, project_root: &Path) -> Result<ProjectTypeDetection> {
        let cargo_toml = project_root.join("Cargo.toml");

        if !cargo_toml.exists() {
            return Ok(ProjectTypeDetection {
                project_type: self.project_type(),
                confidence: 0.0,
                evidence: vec![],
                metadata: HashMap::new(),
                frameworks: Vec::new(),
            });
        }

        let Some(toml) = self.parse_cargo_toml(&cargo_toml) else {
            return Ok(ProjectTypeDetection {
                project_type: self.project_type(),
                confidence: 0.0,
                evidence: vec!["Found Cargo.toml but failed to parse".to_string()],
                metadata: HashMap::new(),
                frameworks: Vec::new(),
            });
        };

        let mut confidence = 0.3; // Base confidence
        let mut evidence = vec!["Found Cargo.toml".to_string()];
        let mut metadata = HashMap::new();

        // Check workspace section
        if self.has_workspace_section(&toml) {
            confidence += 0.5;
            evidence.push("Found [workspace] section".to_string());

            // Extract members
            let members = self.get_workspace_members(&toml);
            if !members.is_empty() {
                confidence += 0.2;
                evidence.push(format!("Found {} workspace members", members.len()));
                metadata.insert("members".to_string(), members.join(","));
                metadata.insert("member_count".to_string(), members.len().to_string());
            }
        } else {
            // Without [workspace] it isn't a workspace
            return Ok(ProjectTypeDetection {
                project_type: self.project_type(),
                confidence: 0.0,
                evidence: vec!["Cargo.toml does not have [workspace] section".to_string()],
                metadata,
                frameworks: Vec::new(),
            });
        }

        // Extract Rust edition
        if let Some(edition) = self.get_rust_edition(&toml) {
            metadata.insert("rust_edition".to_string(), edition);
        }

        // Framework detection. Workspaces declare deps either at the
        // workspace level (`[workspace.dependencies]`) or per-crate;
        // `rust_common::detect_frameworks` looks at both. The
        // `src-tauri/` layout heuristic catches Tauri apps whose
        // workspace root doesn't directly declare `tauri` because the
        // dep lives in the `src-tauri` member crate.
        let mut frameworks = rust_common::detect_frameworks(&toml);
        if rust_common::has_tauri_layout(project_root)
            && !frameworks.contains(&DetectedFramework::Tauri)
        {
            frameworks.push(DetectedFramework::Tauri);
        }
        if !frameworks.is_empty() {
            metadata.insert(
                "frameworks".to_string(),
                DetectedFramework::join_display_names(&frameworks),
            );
        }

        Ok(ProjectTypeDetection {
            project_type: self.project_type(),
            confidence,
            evidence,
            metadata,
            frameworks,
        })
    }

    fn module_detection_strategy(&self) -> ModuleDetectionStrategy {
        ModuleDetectionStrategy {
            // In a Rust workspace, each Cargo.toml marks a crate/module
            module_markers: vec!["Cargo.toml".to_string()],

            // Common Rust entry points
            entry_point_files: vec![
                "lib.rs".to_string(),
                "main.rs".to_string(),
                "mod.rs".to_string(),
                "src/lib.rs".to_string(),
                "src/main.rs".to_string(),
            ],
        }
    }

    async fn extract_metadata(&self, project_root: &Path) -> Result<HashMap<String, String>> {
        let mut metadata = HashMap::new();

        let cargo_toml = project_root.join("Cargo.toml");
        if !cargo_toml.exists() {
            return Ok(metadata);
        }

        if let Some(toml) = self.parse_cargo_toml(&cargo_toml) {
            // Members
            let members = self.get_workspace_members(&toml);
            if !members.is_empty() {
                metadata.insert("members".to_string(), members.join(","));
                metadata.insert("member_count".to_string(), members.len().to_string());
            }

            // Rust edition
            if let Some(edition) = self.get_rust_edition(&toml) {
                metadata.insert("rust_edition".to_string(), edition);
            }

            let mut frameworks = rust_common::detect_frameworks(&toml);
            if rust_common::has_tauri_layout(project_root)
                && !frameworks.contains(&DetectedFramework::Tauri)
            {
                frameworks.push(DetectedFramework::Tauri);
            }
            if !frameworks.is_empty() {
                metadata.insert(
                    "frameworks".to_string(),
                    DetectedFramework::join_display_names(&frameworks),
                );
            }
        }

        Ok(metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_cargo_toml(dir: &Path, content: &str) {
        fs::write(dir.join("Cargo.toml"), content).unwrap();
    }

    #[tokio::test]
    async fn test_detect_no_cargo_toml() {
        let temp_dir = TempDir::new().unwrap();
        let analyzer = RustWorkspaceAnalyzer::new();

        let detection = analyzer.detect(temp_dir.path()).await.unwrap();

        assert_eq!(detection.confidence, 0.0);
    }

    #[tokio::test]
    async fn test_detect_workspace() {
        let temp_dir = TempDir::new().unwrap();
        create_cargo_toml(
            temp_dir.path(),
            r#"
[workspace]
members = ["crates/core", "crates/cli"]

[workspace.package]
edition = "2021"
"#,
        );

        let analyzer = RustWorkspaceAnalyzer::new();
        let detection = analyzer.detect(temp_dir.path()).await.unwrap();

        // Cargo.toml + [workspace] + members
        assert_eq!(detection.confidence, 1.0);
        assert!(detection.evidence.iter().any(|e| e.contains("workspace")));
        assert!(detection.evidence.iter().any(|e| e.contains("2 workspace members")));
        assert_eq!(
            detection.metadata.get("members").unwrap(),
            "crates/core,crates/cli"
        );
        assert_eq!(detection.metadata.get("rust_edition").unwrap(), "2021");
    }

    #[tokio::test]
    async fn test_reject_non_workspace() {
        let temp_dir = TempDir::new().unwrap();
        create_cargo_toml(
            temp_dir.path(),
            r#"
[package]
name = "test"
version = "0.1.0"
edition = "2021"
"#,
        );

        let analyzer = RustWorkspaceAnalyzer::new();
        let detection = analyzer.detect(temp_dir.path()).await.unwrap();

        // Without [workspace] the analyzer must reject
        assert_eq!(detection.confidence, 0.0);
    }

    #[test]
    fn test_module_detection_strategy() {
        let analyzer = RustWorkspaceAnalyzer::new();
        let strategy = analyzer.module_detection_strategy();

        assert_eq!(strategy.module_markers, vec!["Cargo.toml"]);
        assert!(strategy.entry_point_files.contains(&"lib.rs".to_string()));
        assert!(strategy.entry_point_files.contains(&"main.rs".to_string()));
    }

    #[tokio::test]
    async fn test_detect_tauri_in_workspace() {
        let temp = TempDir::new().unwrap();
        // A workspace whose `src-tauri` member is one of the crates.
        create_cargo_toml(
            temp.path(),
            r#"
[workspace]
members = ["src-tauri", "shared"]

[workspace.dependencies]
tauri = "2.0"
serde = "1"
"#,
        );
        // Layout helper kicks in too.
        std::fs::create_dir_all(temp.path().join("src-tauri")).unwrap();
        std::fs::write(
            temp.path().join("src-tauri").join("tauri.conf.json"),
            "{}",
        )
        .unwrap();

        let analyzer = RustWorkspaceAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        assert!(detection.frameworks.contains(&DetectedFramework::Tauri));
        assert!(detection
            .metadata
            .get("frameworks")
            .unwrap()
            .contains("Tauri"));
    }
}
