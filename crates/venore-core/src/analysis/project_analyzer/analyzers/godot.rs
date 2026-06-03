use crate::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;

use super::super::traits::{
    ModuleDetectionStrategy, ProjectAnalyzer, ProjectType, ProjectTypeDetection,
};

/// Analyzer for Godot game projects.
///
/// Detection: `project.godot` at the root — Godot's canonical
/// descriptor (TOML-like, written by the editor). The file's mere
/// presence is enough; the engine itself uses it as the only required
/// marker.
///
/// Strategy: groups files by top-level folder. Godot projects don't
/// have a module manifest — scenes live in `scenes/`, scripts in
/// `scripts/`, etc. by convention.
pub struct GodotAnalyzer;

impl Default for GodotAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl GodotAnalyzer {
    pub fn new() -> Self {
        Self
    }

    fn read_project_godot(&self, project_root: &Path) -> Option<String> {
        std::fs::read_to_string(project_root.join("project.godot")).ok()
    }

    /// Pull the project's display name from the `[application]`
    /// section of `project.godot`.
    ///
    /// `project.godot` is INI-like (Godot uses its own dialect), so we
    /// scan line-by-line for `config/name="..."` rather than pulling
    /// in a full INI parser for one field.
    fn get_project_name(&self, content: &str) -> Option<String> {
        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("config/name=") {
                return Some(
                    rest.trim()
                        .trim_matches(|c: char| matches!(c, '"' | '\''))
                        .to_string(),
                );
            }
        }
        None
    }

    /// Extract the engine version from `[application]` if present.
    fn get_godot_version(&self, content: &str) -> Option<String> {
        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("config/features=") {
                return Some(
                    rest.trim()
                        .trim_matches(|c: char| matches!(c, '"' | '\'' | '(' | ')'))
                        .split(',')
                        .next()
                        .unwrap_or("")
                        .trim()
                        .trim_matches(|c: char| matches!(c, '"' | '\''))
                        .to_string(),
                );
            }
        }
        None
    }
}

#[async_trait]
impl ProjectAnalyzer for GodotAnalyzer {
    fn name(&self) -> &str {
        "godot"
    }

    fn project_type(&self) -> ProjectType {
        ProjectType::Godot
    }

    async fn detect(&self, project_root: &Path) -> Result<ProjectTypeDetection> {
        if !project_root.join("project.godot").exists() {
            return Ok(ProjectTypeDetection {
                project_type: self.project_type(),
                confidence: 0.0,
                evidence: vec![],
                metadata: HashMap::new(),
                frameworks: Vec::new(),
            });
        }

        // project.godot is a unique-to-Godot marker — no other tool ships
        // a file by that name. Confidence is effectively maxed.
        let mut confidence: f32 = 0.95;
        let mut evidence = vec!["Found project.godot".to_string()];
        let mut metadata = HashMap::new();

        if let Some(content) = self.read_project_godot(project_root) {
            if let Some(name) = self.get_project_name(&content) {
                metadata.insert("project_name".to_string(), name);
            }
            if let Some(version) = self.get_godot_version(&content) {
                metadata.insert("godot_version".to_string(), version);
            }
        }

        // C# Godot projects ship a `.csproj` alongside; surface it so
        // downstream tooling can route to the C# language server too.
        let has_csproj = std::fs::read_dir(project_root)
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .any(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .map(|n| n.ends_with(".csproj"))
                    .unwrap_or(false)
            });
        if has_csproj {
            evidence.push("Found *.csproj — C# Godot project".to_string());
            metadata.insert("language".to_string(), "csharp".to_string());
            confidence = (confidence + 0.05).min(1.0);
        } else {
            metadata.insert("language".to_string(), "gdscript".to_string());
        }

        Ok(ProjectTypeDetection {
            project_type: self.project_type(),
            confidence,
            evidence,
            metadata,
            // Godot itself is the framework — captured by the project
            // type, no separate `DetectedFramework` variant needed.
            frameworks: Vec::new(),
        })
    }

    fn module_detection_strategy(&self) -> ModuleDetectionStrategy {
        ModuleDetectionStrategy {
            // No file-level marker for Godot modules — scripts and
            // scenes are organised by folder convention.
            module_markers: vec![],
            entry_point_files: vec![
                "project.godot".to_string(),
                "main.gd".to_string(),
                "scenes/main.tscn".to_string(),
                "scripts/main.gd".to_string(),
            ],
        }
    }

    async fn extract_metadata(&self, project_root: &Path) -> Result<HashMap<String, String>> {
        let mut metadata = HashMap::new();

        if let Some(content) = self.read_project_godot(project_root) {
            if let Some(name) = self.get_project_name(&content) {
                metadata.insert("project_name".to_string(), name);
            }
            if let Some(version) = self.get_godot_version(&content) {
                metadata.insert("godot_version".to_string(), version);
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

    fn write_project_godot(dir: &Path, content: &str) {
        fs::write(dir.join("project.godot"), content).unwrap();
    }

    #[tokio::test]
    async fn test_detect_no_project_godot() {
        let temp = TempDir::new().unwrap();
        let analyzer = GodotAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();
        assert_eq!(detection.confidence, 0.0);
    }

    #[tokio::test]
    async fn test_detect_minimal_project_godot() {
        let temp = TempDir::new().unwrap();
        write_project_godot(temp.path(), "[application]\nconfig/name=\"Demo\"\n");

        let analyzer = GodotAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        assert!(detection.confidence >= 0.95);
        assert_eq!(detection.metadata.get("project_name").unwrap(), "Demo");
        assert_eq!(detection.metadata.get("language").unwrap(), "gdscript");
    }

    #[tokio::test]
    async fn test_detect_godot_with_features() {
        let temp = TempDir::new().unwrap();
        write_project_godot(
            temp.path(),
            r#"
[application]
config/name="My Game"
config/features=PackedStringArray("4.2", "Forward Plus")
"#,
        );

        let analyzer = GodotAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        assert_eq!(
            detection.metadata.get("project_name").unwrap(),
            "My Game"
        );
        let version = detection.metadata.get("godot_version").unwrap();
        assert!(version.contains("4.2"), "got version: {version}");
    }

    #[tokio::test]
    async fn test_detect_csharp_godot_project() {
        let temp = TempDir::new().unwrap();
        write_project_godot(temp.path(), "[application]\nconfig/name=\"CsDemo\"\n");
        fs::write(temp.path().join("CsDemo.csproj"), "<Project/>").unwrap();

        let analyzer = GodotAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        assert_eq!(detection.metadata.get("language").unwrap(), "csharp");
        assert!(detection.confidence >= 0.95);
    }

    #[test]
    fn test_module_detection_strategy() {
        let analyzer = GodotAnalyzer::new();
        let strategy = analyzer.module_detection_strategy();
        assert!(strategy.module_markers.is_empty());
        assert!(strategy
            .entry_point_files
            .contains(&"project.godot".to_string()));
    }
}
