use crate::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;

use super::common::kotlin as kotlin_common;
use super::super::traits::{
    DetectedFramework, ModuleDetectionStrategy, ProjectAnalyzer, ProjectType,
    ProjectTypeDetection,
};

/// Analyzer for Kotlin projects built with Gradle.
///
/// Detects projects with:
/// - `build.gradle.kts` (Kotlin DSL) or `build.gradle` (Groovy DSL)
/// - Bonus signals: `gradlew` wrapper, `settings.gradle[.kts]`
///
/// Note: pure-Java Gradle projects also match this signature today —
/// there is no separate Java-Gradle analyzer yet. We surface this as
/// `KotlinGradle` because Kotlin is the modern default and the JVM
/// toolchain is identical. A future Java-Gradle analyzer would gate on
/// the absence of `.kt`/`.kts` files (out of scope for this pass).
pub struct KotlinGradleAnalyzer;

impl Default for KotlinGradleAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl KotlinGradleAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProjectAnalyzer for KotlinGradleAnalyzer {
    fn name(&self) -> &str {
        "kotlin-gradle"
    }

    fn project_type(&self) -> ProjectType {
        ProjectType::KotlinGradle
    }

    async fn detect(&self, project_root: &Path) -> Result<ProjectTypeDetection> {
        let has_build = project_root.join("build.gradle.kts").exists()
            || project_root.join("build.gradle").exists();

        if !has_build {
            return Ok(ProjectTypeDetection {
                project_type: self.project_type(),
                confidence: 0.0,
                evidence: vec![],
                metadata: HashMap::new(),
                frameworks: Vec::new(),
            });
        }

        let mut confidence: f32 = 0.7;
        let mut evidence = vec!["Found build.gradle(.kts)".to_string()];
        let mut metadata = HashMap::new();

        if kotlin_common::has_gradle_wrapper(project_root) {
            confidence += 0.1;
            evidence.push("Found gradlew wrapper".to_string());
        }

        if kotlin_common::is_multi_module(project_root) {
            confidence += 0.1;
            evidence.push("Found settings.gradle(.kts) — multi-module build".to_string());
            metadata.insert("multi_module".to_string(), "true".to_string());
        }

        // Framework detection from build-script text.
        let mut frameworks: Vec<DetectedFramework> = Vec::new();
        if let Some(text) = kotlin_common::read_build_gradle(project_root) {
            frameworks = kotlin_common::detect_frameworks(&text);
        }

        // Filesystem-level Android sanity check. Picks up projects
        // where the Android plugin is declared in `app/build.gradle`
        // rather than the root build script.
        if kotlin_common::is_android_project(project_root)
            && !frameworks.contains(&DetectedFramework::AndroidApp)
        {
            frameworks.push(DetectedFramework::AndroidApp);
            evidence.push("Found AndroidManifest.xml".to_string());
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
            // Each subproject in a multi-module Gradle build is rooted
            // at its own build.gradle(.kts), so we mark them as module
            // anchors. The detector treats both variants equivalently.
            module_markers: vec![
                "build.gradle.kts".to_string(),
                "build.gradle".to_string(),
            ],
            entry_point_files: vec![
                "src/main/kotlin/Main.kt".to_string(),
                "src/main/kotlin/Application.kt".to_string(),
                "src/main/java/Main.java".to_string(),
            ],
        }
    }

    async fn extract_metadata(&self, project_root: &Path) -> Result<HashMap<String, String>> {
        let mut metadata = HashMap::new();

        if let Some(text) = kotlin_common::read_build_gradle(project_root) {
            let frameworks = kotlin_common::detect_frameworks(&text);
            if !frameworks.is_empty() {
                metadata.insert(
                    "frameworks".to_string(),
                    DetectedFramework::join_display_names(&frameworks),
                );
            }
        }

        if kotlin_common::is_multi_module(project_root) {
            metadata.insert("multi_module".to_string(), "true".to_string());
        }
        if kotlin_common::is_android_project(project_root) {
            metadata.insert("android".to_string(), "true".to_string());
        }

        Ok(metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_build_gradle_kts(dir: &Path, content: &str) {
        fs::write(dir.join("build.gradle.kts"), content).unwrap();
    }

    fn write_build_gradle(dir: &Path, content: &str) {
        fs::write(dir.join("build.gradle"), content).unwrap();
    }

    #[tokio::test]
    async fn test_detect_no_gradle() {
        let temp = TempDir::new().unwrap();
        let analyzer = KotlinGradleAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();
        assert_eq!(detection.confidence, 0.0);
    }

    #[tokio::test]
    async fn test_detect_plain_gradle_kts() {
        let temp = TempDir::new().unwrap();
        write_build_gradle_kts(
            temp.path(),
            r#"
plugins {
    kotlin("jvm") version "1.9.0"
}
"#,
        );

        let analyzer = KotlinGradleAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        assert!(detection.confidence >= 0.7);
        assert!(detection.frameworks.is_empty());
    }

    #[tokio::test]
    async fn test_detect_ktor() {
        let temp = TempDir::new().unwrap();
        write_build_gradle_kts(
            temp.path(),
            r#"
dependencies {
    implementation("io.ktor:ktor-server-core:2.3.0")
    implementation("io.ktor:ktor-server-netty:2.3.0")
}
"#,
        );

        let analyzer = KotlinGradleAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        assert!(detection.frameworks.contains(&DetectedFramework::Ktor));
    }

    #[tokio::test]
    async fn test_detect_spring_boot_plugin() {
        let temp = TempDir::new().unwrap();
        write_build_gradle_kts(
            temp.path(),
            r#"
plugins {
    id("org.springframework.boot") version "3.1.0"
    kotlin("jvm") version "1.9.0"
}
"#,
        );

        let analyzer = KotlinGradleAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        assert!(detection
            .frameworks
            .contains(&DetectedFramework::SpringBoot));
    }

    #[tokio::test]
    async fn test_detect_android_via_plugin() {
        let temp = TempDir::new().unwrap();
        write_build_gradle_kts(
            temp.path(),
            r#"
plugins {
    id("com.android.application")
    kotlin("android")
}
"#,
        );

        let analyzer = KotlinGradleAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        assert!(detection
            .frameworks
            .contains(&DetectedFramework::AndroidApp));
    }

    #[tokio::test]
    async fn test_detect_android_via_manifest_layout() {
        let temp = TempDir::new().unwrap();
        write_build_gradle_kts(temp.path(), "plugins { kotlin(\"jvm\") }\n");
        fs::create_dir_all(temp.path().join("app").join("src").join("main")).unwrap();
        fs::write(
            temp.path()
                .join("app")
                .join("src")
                .join("main")
                .join("AndroidManifest.xml"),
            "<manifest/>",
        )
        .unwrap();

        let analyzer = KotlinGradleAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        assert!(detection
            .frameworks
            .contains(&DetectedFramework::AndroidApp));
    }

    #[tokio::test]
    async fn test_groovy_dsl_supported() {
        let temp = TempDir::new().unwrap();
        write_build_gradle(
            temp.path(),
            r#"
dependencies {
    implementation 'io.ktor:ktor-server-core:2.3.0'
}
"#,
        );

        let analyzer = KotlinGradleAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        assert!(detection.confidence >= 0.7);
        assert!(detection.frameworks.contains(&DetectedFramework::Ktor));
    }

    #[tokio::test]
    async fn test_multi_module_signal() {
        let temp = TempDir::new().unwrap();
        write_build_gradle_kts(temp.path(), "plugins { kotlin(\"jvm\") }\n");
        fs::write(temp.path().join("settings.gradle.kts"), "include(\"app\")").unwrap();

        let analyzer = KotlinGradleAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        assert_eq!(detection.metadata.get("multi_module").unwrap(), "true");
    }
}
