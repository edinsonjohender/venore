use crate::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;

use super::common::rust as rust_common;
use super::super::traits::{
    DetectedFramework, ModuleDetectionStrategy, ProjectAnalyzer, ProjectType,
    ProjectTypeDetection,
};

/// Analyzer for single-crate Rust projects.
///
/// Detects projects with:
/// - Cargo.toml at the root
/// - No [workspace] section
pub struct RustSingleCrateAnalyzer;

impl Default for RustSingleCrateAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl RustSingleCrateAnalyzer {
    pub fn new() -> Self {
        Self
    }

    fn parse_cargo_toml(&self, cargo_toml_path: &Path) -> Option<toml::Value> {
        let content = std::fs::read_to_string(cargo_toml_path).ok()?;
        content.parse::<toml::Value>().ok()
    }

    fn has_workspace_section(&self, toml: &toml::Value) -> bool {
        toml.get("workspace").is_some()
    }

    fn get_rust_edition(&self, toml: &toml::Value) -> Option<String> {
        toml.get("package")
            .and_then(|p| p.get("edition"))
            .and_then(|e| e.as_str())
            .map(|s| s.to_string())
    }

    fn get_package_name(&self, toml: &toml::Value) -> Option<String> {
        toml.get("package")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
            .map(|s| s.to_string())
    }
}

#[async_trait]
impl ProjectAnalyzer for RustSingleCrateAnalyzer {
    fn name(&self) -> &str {
        "rust-single-crate"
    }

    fn project_type(&self) -> ProjectType {
        ProjectType::RustSingleCrate
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

        let mut confidence = 0.5; // Base confidence
        let mut evidence = vec!["Found Cargo.toml".to_string()];
        let mut metadata = HashMap::new();

        // If a [workspace] section is present, it's not a single crate
        if self.has_workspace_section(&toml) {
            return Ok(ProjectTypeDetection {
                project_type: self.project_type(),
                confidence: 0.0,
                evidence: vec!["Has [workspace] section (not a single crate)".to_string()],
                metadata,
                frameworks: Vec::new(),
            });
        }

        // Verify it has a [package] section
        if toml.get("package").is_some() {
            confidence += 0.3;
            evidence.push("Found [package] section".to_string());

            // Package name
            if let Some(name) = self.get_package_name(&toml) {
                metadata.insert("package_name".to_string(), name);
            }

            // Rust edition
            if let Some(edition) = self.get_rust_edition(&toml) {
                metadata.insert("rust_edition".to_string(), edition);
            }
        }

        // Framework detection (Tauri today). Two signals:
        // - `tauri` declared in [dependencies]
        // - `src-tauri/tauri.conf.json` next to this crate (catches the
        //   case where this crate IS the src-tauri crate of a Tauri app).
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
            // Single crate doesn't use module_markers (uses entry points)
            module_markers: vec![],

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
            if let Some(name) = self.get_package_name(&toml) {
                metadata.insert("package_name".to_string(), name);
            }

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
    async fn test_detect_single_crate() {
        let temp_dir = TempDir::new().unwrap();
        create_cargo_toml(
            temp_dir.path(),
            r#"
[package]
name = "my-crate"
version = "0.1.0"
edition = "2021"
"#,
        );

        let analyzer = RustSingleCrateAnalyzer::new();
        let detection = analyzer.detect(temp_dir.path()).await.unwrap();

        assert_eq!(detection.confidence, 0.8);
        assert!(detection.evidence.iter().any(|e| e.contains("package")));
        assert_eq!(
            detection.metadata.get("package_name").unwrap(),
            "my-crate"
        );
        assert_eq!(detection.metadata.get("rust_edition").unwrap(), "2021");
    }

    #[tokio::test]
    async fn test_reject_workspace() {
        let temp_dir = TempDir::new().unwrap();
        create_cargo_toml(
            temp_dir.path(),
            r#"
[workspace]
members = ["crates/core"]
"#,
        );

        let analyzer = RustSingleCrateAnalyzer::new();
        let detection = analyzer.detect(temp_dir.path()).await.unwrap();

        // Must reject workspaces
        assert_eq!(detection.confidence, 0.0);
    }

    #[test]
    fn test_module_strategy_no_markers() {
        let analyzer = RustSingleCrateAnalyzer::new();
        let strategy = analyzer.module_detection_strategy();

        // Single crate doesn't use module_markers
        assert!(strategy.module_markers.is_empty());
        assert!(!strategy.entry_point_files.is_empty());
    }

    #[tokio::test]
    async fn test_detect_tauri_via_dependency() {
        let temp = TempDir::new().unwrap();
        create_cargo_toml(
            temp.path(),
            r#"
[package]
name = "my-app"
version = "0.1.0"
edition = "2021"

[dependencies]
tauri = "2.0"
serde = "1"
"#,
        );

        let analyzer = RustSingleCrateAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        assert!(detection.frameworks.contains(&DetectedFramework::Tauri));
        assert!(detection
            .metadata
            .get("frameworks")
            .unwrap()
            .contains("Tauri"));
    }

    #[tokio::test]
    async fn test_detect_tauri_via_layout_fallback() {
        let temp = TempDir::new().unwrap();
        create_cargo_toml(
            temp.path(),
            r#"
[package]
name = "src-tauri"
version = "0.1.0"
edition = "2021"
"#,
        );
        fs::create_dir_all(temp.path().join("src-tauri")).unwrap();
        fs::write(
            temp.path().join("src-tauri").join("tauri.conf.json"),
            "{}",
        )
        .unwrap();

        let analyzer = RustSingleCrateAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        assert!(detection.frameworks.contains(&DetectedFramework::Tauri));
    }
}
