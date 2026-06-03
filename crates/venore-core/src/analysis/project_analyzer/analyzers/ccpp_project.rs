use crate::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;

use super::common::ccpp as ccpp_common;
use super::super::traits::{
    DetectedFramework, ModuleDetectionStrategy, ProjectAnalyzer, ProjectType,
    ProjectTypeDetection,
};

/// Analyzer for C and C++ projects.
///
/// Detects the project via any well-known build-system marker:
/// CMake, Meson, Make, Conan, vcpkg, or xmake. The marker is reported
/// in metadata as `build_system`.
///
/// We do NOT distinguish C vs C++ at the project level — both fall
/// under the same `ProjectType::CCppProject`. The `Language` enum
/// already differentiates per-file (`Language::C` for `.c`,
/// `Language::Cpp` for `.cpp`/`.cc`/`.h`/...).
pub struct CCppProjectAnalyzer;

impl Default for CCppProjectAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl CCppProjectAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProjectAnalyzer for CCppProjectAnalyzer {
    fn name(&self) -> &str {
        "c-cpp-project"
    }

    fn project_type(&self) -> ProjectType {
        ProjectType::CCppProject
    }

    async fn detect(&self, project_root: &Path) -> Result<ProjectTypeDetection> {
        let systems = ccpp_common::detect_build_systems(project_root);

        if systems.is_empty() {
            return Ok(ProjectTypeDetection {
                project_type: self.project_type(),
                confidence: 0.0,
                evidence: vec![],
                metadata: HashMap::new(),
                frameworks: Vec::new(),
            });
        }

        // Confidence == best marker's base. A bare Makefile alone
        // stays at 0.5 because Makefiles also appear in many
        // non-C/C++ projects.
        let confidence = systems
            .iter()
            .map(|s| s.base_confidence())
            .fold(0.0_f32, f32::max);

        let names: Vec<&'static str> = systems.iter().map(|s| s.name()).collect();
        let mut evidence = vec![format!("Found build system: {}", names.join(", "))];
        let mut metadata = HashMap::new();
        metadata.insert("build_system".to_string(), names.join(","));

        // Framework detection via CMakeLists.txt contents (Qt today).
        let mut frameworks: Vec<DetectedFramework> = Vec::new();
        if systems.contains(&ccpp_common::BuildSystem::CMake) {
            if let Some(cmake) = ccpp_common::read_cmakelists(project_root) {
                frameworks = ccpp_common::detect_frameworks_from_cmake(&cmake);
            }
        }

        if !frameworks.is_empty() {
            evidence.push(format!(
                "Detected frameworks: {}",
                DetectedFramework::join_display_names(&frameworks)
            ));
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
            // CMake subdirectories typically each carry their own
            // CMakeLists.txt — that's the closest C/C++ has to a
            // module marker.
            module_markers: vec!["CMakeLists.txt".to_string()],
            entry_point_files: vec![
                "src/main.c".to_string(),
                "src/main.cpp".to_string(),
                "src/main.cc".to_string(),
                "main.c".to_string(),
                "main.cpp".to_string(),
            ],
        }
    }

    async fn extract_metadata(&self, project_root: &Path) -> Result<HashMap<String, String>> {
        let mut metadata = HashMap::new();
        let systems = ccpp_common::detect_build_systems(project_root);
        if !systems.is_empty() {
            let names: Vec<&'static str> = systems.iter().map(|s| s.name()).collect();
            metadata.insert("build_system".to_string(), names.join(","));
        }

        if let Some(cmake) = ccpp_common::read_cmakelists(project_root) {
            let frameworks = ccpp_common::detect_frameworks_from_cmake(&cmake);
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

    #[tokio::test]
    async fn test_detect_no_marker() {
        let temp = TempDir::new().unwrap();
        let analyzer = CCppProjectAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();
        assert_eq!(detection.confidence, 0.0);
    }

    #[tokio::test]
    async fn test_detect_cmake() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("CMakeLists.txt"),
            "cmake_minimum_required(VERSION 3.20)\nproject(demo C CXX)\n",
        )
        .unwrap();

        let analyzer = CCppProjectAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        assert!(detection.confidence >= 0.9);
        assert_eq!(detection.metadata.get("build_system").unwrap(), "CMake");
    }

    #[tokio::test]
    async fn test_detect_makefile_only_lower_confidence() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("Makefile"), "all:\n\techo hi\n").unwrap();

        let analyzer = CCppProjectAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        // Makefiles alone get 0.5 because they're common in many
        // ecosystems beyond C/C++.
        assert!((detection.confidence - 0.5).abs() < 1e-4);
    }

    #[tokio::test]
    async fn test_detect_meson() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("meson.build"),
            "project('demo', 'cpp')\n",
        )
        .unwrap();

        let analyzer = CCppProjectAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        assert!(detection.confidence >= 0.9);
        assert_eq!(detection.metadata.get("build_system").unwrap(), "Meson");
    }

    #[tokio::test]
    async fn test_detect_vcpkg() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("vcpkg.json"), r#"{"name":"demo"}"#).unwrap();

        let analyzer = CCppProjectAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        assert!(detection.confidence >= 0.85);
        assert_eq!(detection.metadata.get("build_system").unwrap(), "vcpkg");
    }

    #[tokio::test]
    async fn test_detect_mixed_markers() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("CMakeLists.txt"),
            "project(demo)\n",
        )
        .unwrap();
        fs::write(temp.path().join("conanfile.txt"), "[requires]\nfmt/9.1.0\n").unwrap();

        let analyzer = CCppProjectAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        let bs = detection.metadata.get("build_system").unwrap();
        assert!(bs.contains("CMake"));
        assert!(bs.contains("Conan"));
    }

    #[tokio::test]
    async fn test_detect_qt_framework() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("CMakeLists.txt"),
            r#"
cmake_minimum_required(VERSION 3.20)
project(qtdemo)
find_package(Qt6 REQUIRED COMPONENTS Widgets)
"#,
        )
        .unwrap();

        let analyzer = CCppProjectAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        assert!(detection.frameworks.contains(&DetectedFramework::Qt));
    }
}
