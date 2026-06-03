use crate::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;

use super::common::dotnet as dotnet_common;
use super::super::traits::{
    DetectedFramework, ModuleDetectionStrategy, ProjectAnalyzer, ProjectType,
    ProjectTypeDetection,
};

/// Analyzer for .NET / C# projects.
///
/// Detection signals:
/// - `*.csproj` at the root — single-project layout (most apps).
/// - `*.sln` at the root — solution containing multiple projects.
/// - `ProjectSettings/ProjectVersion.txt` + `Assets/` — Unity (frameworks
///   surface as `Unity`, no separate `ProjectType` variant — Unity is
///   still a .NET project under the hood).
///
/// Strategy: every `.csproj` marks a module (same convention as Rust
/// workspace).
pub struct DotnetProjectAnalyzer;

impl Default for DotnetProjectAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl DotnetProjectAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProjectAnalyzer for DotnetProjectAnalyzer {
    fn name(&self) -> &str {
        "dotnet-project"
    }

    fn project_type(&self) -> ProjectType {
        ProjectType::DotnetProject
    }

    async fn detect(&self, project_root: &Path) -> Result<ProjectTypeDetection> {
        let marker = dotnet_common::find_dotnet_marker(project_root);
        let unity = dotnet_common::is_unity_project(project_root);

        // No `.csproj` / `.sln` AND no Unity layout — not a .NET project.
        if marker.is_none() && !unity {
            return Ok(ProjectTypeDetection {
                project_type: self.project_type(),
                confidence: 0.0,
                evidence: vec![],
                metadata: HashMap::new(),
                frameworks: Vec::new(),
            });
        }

        let mut confidence: f32 = 0.0;
        let mut evidence = vec![];
        let mut metadata = HashMap::new();
        let mut frameworks: Vec<DetectedFramework> = Vec::new();

        if let Some((file, kind)) = &marker {
            confidence = 0.9;
            evidence.push(format!("Found {file}"));
            metadata.insert("marker".to_string(), kind.to_string());

            // Inspect the .csproj for SDK style and framework deps.
            if let Some(text) = dotnet_common::read_first_csproj(project_root) {
                if let Some(sdk) = dotnet_common::get_sdk(&text) {
                    metadata.insert("sdk".to_string(), sdk);
                }
                frameworks.extend(dotnet_common::detect_frameworks_from_csproj(&text));
            }
        }

        if unity {
            // Unity layout is a strong signal even without a clean
            // `.csproj` (Unity rewrites them on import). Bump
            // confidence to max and surface the engine version.
            confidence = (confidence + 0.05).max(0.95);
            evidence.push(
                "Found ProjectSettings/ProjectVersion.txt + Assets/ (Unity)".to_string(),
            );
            if !frameworks.contains(&DetectedFramework::Unity) {
                frameworks.push(DetectedFramework::Unity);
            }
            if let Ok(version_file) = std::fs::read_to_string(
                project_root
                    .join("ProjectSettings")
                    .join("ProjectVersion.txt"),
            ) {
                for line in version_file.lines() {
                    if let Some(rest) = line.trim().strip_prefix("m_EditorVersion:") {
                        metadata.insert(
                            "unity_version".to_string(),
                            rest.trim().to_string(),
                        );
                        break;
                    }
                }
            }
        }

        if !frameworks.is_empty() {
            metadata.insert(
                "frameworks".to_string(),
                DetectedFramework::join_display_names(&frameworks),
            );
        }

        Ok(ProjectTypeDetection {
            project_type: self.project_type(),
            confidence: confidence.min(1.0),
            evidence,
            metadata,
            frameworks,
        })
    }

    fn module_detection_strategy(&self) -> ModuleDetectionStrategy {
        ModuleDetectionStrategy {
            module_markers: vec!["*.csproj".to_string()],
            entry_point_files: vec![
                "Program.cs".to_string(),
                "Startup.cs".to_string(),
            ],
        }
    }

    async fn extract_metadata(&self, project_root: &Path) -> Result<HashMap<String, String>> {
        let mut metadata = HashMap::new();

        if let Some(text) = dotnet_common::read_first_csproj(project_root) {
            if let Some(sdk) = dotnet_common::get_sdk(&text) {
                metadata.insert("sdk".to_string(), sdk);
            }
            let mut frameworks = dotnet_common::detect_frameworks_from_csproj(&text);
            if dotnet_common::is_unity_project(project_root)
                && !frameworks.contains(&DetectedFramework::Unity)
            {
                frameworks.push(DetectedFramework::Unity);
            }
            if !frameworks.is_empty() {
                metadata.insert(
                    "frameworks".to_string(),
                    DetectedFramework::join_display_names(&frameworks),
                );
            }
        } else if dotnet_common::is_unity_project(project_root) {
            metadata.insert("frameworks".to_string(), "Unity".to_string());
        }

        Ok(metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_csproj(dir: &Path, name: &str, content: &str) {
        fs::write(dir.join(name), content).unwrap();
    }

    #[tokio::test]
    async fn test_detect_nothing() {
        let temp = TempDir::new().unwrap();
        let analyzer = DotnetProjectAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();
        assert_eq!(detection.confidence, 0.0);
    }

    #[tokio::test]
    async fn test_detect_plain_csproj() {
        let temp = TempDir::new().unwrap();
        write_csproj(
            temp.path(),
            "MyApp.csproj",
            r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <TargetFramework>net8.0</TargetFramework>
  </PropertyGroup>
</Project>"#,
        );

        let analyzer = DotnetProjectAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        assert!(detection.confidence >= 0.9);
        assert_eq!(detection.metadata.get("marker").unwrap(), "csproj");
        assert_eq!(
            detection.metadata.get("sdk").unwrap(),
            "Microsoft.NET.Sdk"
        );
        assert!(detection.frameworks.is_empty());
    }

    #[tokio::test]
    async fn test_detect_aspnet_via_web_sdk() {
        let temp = TempDir::new().unwrap();
        write_csproj(
            temp.path(),
            "WebApp.csproj",
            r#"<Project Sdk="Microsoft.NET.Sdk.Web">
  <PropertyGroup><TargetFramework>net8.0</TargetFramework></PropertyGroup>
</Project>"#,
        );

        let analyzer = DotnetProjectAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        assert!(detection.frameworks.contains(&DetectedFramework::AspNet));
    }

    #[tokio::test]
    async fn test_detect_aspnet_via_package_reference() {
        // Legacy target without the Web SDK still pulls in
        // Microsoft.AspNetCore.* — detect via deps.
        let temp = TempDir::new().unwrap();
        write_csproj(
            temp.path(),
            "App.csproj",
            r#"<Project Sdk="Microsoft.NET.Sdk">
  <ItemGroup>
    <PackageReference Include="Microsoft.AspNetCore.App" Version="2.2" />
  </ItemGroup>
</Project>"#,
        );

        let analyzer = DotnetProjectAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        assert!(detection.frameworks.contains(&DetectedFramework::AspNet));
    }

    #[tokio::test]
    async fn test_detect_unity_via_layout() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join("Assets")).unwrap();
        fs::create_dir_all(temp.path().join("ProjectSettings")).unwrap();
        fs::write(
            temp.path()
                .join("ProjectSettings")
                .join("ProjectVersion.txt"),
            "m_EditorVersion: 2022.3.10f1\nm_EditorVersionWithRevision: 2022.3.10f1 (b94e5b89e5e5)\n",
        )
        .unwrap();

        let analyzer = DotnetProjectAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        assert!(detection.confidence >= 0.95);
        assert!(detection.frameworks.contains(&DetectedFramework::Unity));
        assert_eq!(
            detection.metadata.get("unity_version").unwrap(),
            "2022.3.10f1"
        );
    }

    #[tokio::test]
    async fn test_detect_sln_alone() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("MySolution.sln"), "Microsoft Visual Studio Solution File\n").unwrap();

        let analyzer = DotnetProjectAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        assert!(detection.confidence >= 0.9);
        assert_eq!(detection.metadata.get("marker").unwrap(), "sln");
    }

    #[test]
    fn test_module_detection_strategy() {
        let analyzer = DotnetProjectAnalyzer::new();
        let strategy = analyzer.module_detection_strategy();
        assert!(strategy.module_markers.contains(&"*.csproj".to_string()));
        assert!(strategy
            .entry_point_files
            .contains(&"Program.cs".to_string()));
    }
}
