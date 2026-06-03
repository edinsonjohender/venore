use crate::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;

use super::common::php as php_common;
use super::super::traits::{
    DetectedFramework, ModuleDetectionStrategy, ProjectAnalyzer, ProjectType,
    ProjectTypeDetection,
};

/// Analyzer for PHP projects managed with Composer.
///
/// Detects projects with:
/// - `composer.json` at the root
///
/// Strategy: groups files by PSR-4 autoload roots (`src/`, `app/`,
/// etc.) when declared; otherwise falls back to common entry points.
pub struct PhpComposerAnalyzer;

impl Default for PhpComposerAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl PhpComposerAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProjectAnalyzer for PhpComposerAnalyzer {
    fn name(&self) -> &str {
        "php-composer"
    }

    fn project_type(&self) -> ProjectType {
        ProjectType::PhpComposer
    }

    async fn detect(&self, project_root: &Path) -> Result<ProjectTypeDetection> {
        let composer_json = project_root.join("composer.json");

        if !composer_json.exists() {
            return Ok(ProjectTypeDetection {
                project_type: self.project_type(),
                confidence: 0.0,
                evidence: vec![],
                metadata: HashMap::new(),
                frameworks: Vec::new(),
            });
        }

        let Some(composer) = php_common::parse_composer_json(&composer_json) else {
            return Ok(ProjectTypeDetection {
                project_type: self.project_type(),
                confidence: 0.0,
                evidence: vec!["Found composer.json but failed to parse".to_string()],
                metadata: HashMap::new(),
                frameworks: Vec::new(),
            });
        };

        // The mere presence of a valid composer.json is a strong signal
        // (the only well-supported package manager for PHP today).
        let mut confidence = 0.9;
        let mut evidence = vec!["Found composer.json".to_string()];
        let mut metadata = HashMap::new();

        // Package name → metadata only.
        if let Some(name) = php_common::get_package_name(&composer) {
            metadata.insert("package_name".to_string(), name);
        }

        // PHP version constraint.
        if let Some(php_version) = php_common::get_php_version(&composer) {
            metadata.insert("php_version".to_string(), php_version);
        }

        // PSR-4 autoload roots → drives module detection later.
        let psr4_roots = php_common::get_psr4_roots(&composer);
        if !psr4_roots.is_empty() {
            evidence.push(format!(
                "Found PSR-4 autoload roots: {}",
                psr4_roots.join(", ")
            ));
            metadata.insert("psr4_roots".to_string(), psr4_roots.join(","));
            confidence = (confidence + 0.05_f32).min(1.0);
        }

        // Frameworks → typed list + comma-joined string for legacy
        // consumers reading `metadata["frameworks"]`.
        let frameworks = php_common::detect_frameworks(&composer);
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
            // No file-level marker maps cleanly to "module root" in PHP.
            // PSR-4 autoload paths play that role; module_detector
            // doesn't read JSON, so we leave markers empty here and
            // rely on the entry-point fallback. Future work: project
            // analyzer surfaces psr4 roots via metadata so the detector
            // can group by those folders.
            module_markers: vec![],
            entry_point_files: vec![
                "index.php".to_string(),
                "public/index.php".to_string(),
                "bootstrap/app.php".to_string(),
            ],
        }
    }

    async fn extract_metadata(&self, project_root: &Path) -> Result<HashMap<String, String>> {
        let mut metadata = HashMap::new();

        let composer_json = project_root.join("composer.json");
        if !composer_json.exists() {
            return Ok(metadata);
        }

        if let Some(composer) = php_common::parse_composer_json(&composer_json) {
            if let Some(name) = php_common::get_package_name(&composer) {
                metadata.insert("package_name".to_string(), name);
            }
            if let Some(php_version) = php_common::get_php_version(&composer) {
                metadata.insert("php_version".to_string(), php_version);
            }
            let frameworks = php_common::detect_frameworks(&composer);
            if !frameworks.is_empty() {
                metadata.insert(
                    "frameworks".to_string(),
                    DetectedFramework::join_display_names(&frameworks),
                );
            }
            let psr4_roots = php_common::get_psr4_roots(&composer);
            if !psr4_roots.is_empty() {
                metadata.insert("psr4_roots".to_string(), psr4_roots.join(","));
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

    fn write_composer(dir: &Path, content: &str) {
        fs::write(dir.join("composer.json"), content).unwrap();
    }

    #[tokio::test]
    async fn test_detect_no_composer() {
        let temp_dir = TempDir::new().unwrap();
        let analyzer = PhpComposerAnalyzer::new();

        let detection = analyzer.detect(temp_dir.path()).await.unwrap();
        assert_eq!(detection.confidence, 0.0);
    }

    #[tokio::test]
    async fn test_detect_plain_composer() {
        let temp_dir = TempDir::new().unwrap();
        write_composer(
            temp_dir.path(),
            r#"{
                "name": "acme/widget",
                "require": { "php": "^8.1" }
            }"#,
        );

        let analyzer = PhpComposerAnalyzer::new();
        let detection = analyzer.detect(temp_dir.path()).await.unwrap();

        assert!(detection.confidence >= 0.9);
        assert_eq!(
            detection.metadata.get("package_name").unwrap(),
            "acme/widget"
        );
        assert_eq!(detection.metadata.get("php_version").unwrap(), "^8.1");
        assert!(detection.frameworks.is_empty());
    }

    #[tokio::test]
    async fn test_detect_laravel_project() {
        let temp_dir = TempDir::new().unwrap();
        write_composer(
            temp_dir.path(),
            r#"{
                "name": "acme/app",
                "require": {
                    "php": "^8.1",
                    "laravel/framework": "^10.0"
                },
                "autoload": {
                    "psr-4": { "App\\": "app/" }
                }
            }"#,
        );

        let analyzer = PhpComposerAnalyzer::new();
        let detection = analyzer.detect(temp_dir.path()).await.unwrap();

        assert!(detection.frameworks.contains(&DetectedFramework::Laravel));
        assert!(detection
            .metadata
            .get("frameworks")
            .unwrap()
            .contains("Laravel"));
        assert_eq!(detection.metadata.get("psr4_roots").unwrap(), "app");
    }

    #[tokio::test]
    async fn test_detect_symfony_project() {
        let temp_dir = TempDir::new().unwrap();
        write_composer(
            temp_dir.path(),
            r#"{
                "name": "acme/symfony-app",
                "require": {
                    "php": "^8.1",
                    "symfony/framework-bundle": "^6.3"
                }
            }"#,
        );

        let analyzer = PhpComposerAnalyzer::new();
        let detection = analyzer.detect(temp_dir.path()).await.unwrap();

        assert!(detection.frameworks.contains(&DetectedFramework::Symfony));
    }

    #[test]
    fn test_module_detection_strategy() {
        let analyzer = PhpComposerAnalyzer::new();
        let strategy = analyzer.module_detection_strategy();

        assert!(strategy.module_markers.is_empty());
        assert!(strategy
            .entry_point_files
            .contains(&"index.php".to_string()));
    }
}
