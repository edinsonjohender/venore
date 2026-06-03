use crate::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;

use super::common::node as node_common;
use super::super::traits::{
    DetectedFramework, ModuleDetectionStrategy, ProjectAnalyzer, ProjectType,
    ProjectTypeDetection,
};

/// Analyzer for single-package Node.js projects.
///
/// Detects projects with:
/// - package.json at the root
/// - No workspaces configuration
/// - Single-project structure (not a monorepo)
pub struct NodeSinglePackageAnalyzer;

impl Default for NodeSinglePackageAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeSinglePackageAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProjectAnalyzer for NodeSinglePackageAnalyzer {
    fn name(&self) -> &str {
        "node-single-package"
    }

    fn project_type(&self) -> ProjectType {
        ProjectType::NodeSinglePackage
    }

    async fn detect(&self, project_root: &Path) -> Result<ProjectTypeDetection> {
        let package_json = project_root.join("package.json");

        if !package_json.exists() {
            return Ok(ProjectTypeDetection {
                project_type: self.project_type(),
                confidence: 0.0,
                evidence: vec![],
                metadata: HashMap::new(),
                frameworks: Vec::new(),
            });
        }

        let mut confidence = 0.5; // Base confidence
        let mut evidence = vec!["Found package.json".to_string()];
        let mut metadata = HashMap::new();

        // If workspaces are declared, this is not a single package
        if node_common::has_workspaces_config(&package_json) {
            confidence = 0.0;
            evidence.push("Has workspaces configuration (not a single package)".to_string());
            return Ok(ProjectTypeDetection {
                project_type: self.project_type(),
                confidence,
                evidence,
                metadata,
                frameworks: Vec::new(),
            });
        }

        // Higher confidence if no monorepo directories are present
        let monorepo_dirs = ["packages", "apps", "libs"];
        let has_monorepo_structure = monorepo_dirs
            .iter()
            .any(|dir| project_root.join(dir).is_dir());

        if !has_monorepo_structure {
            confidence += 0.3;
            evidence.push("No monorepo structure detected".to_string());
        }

        // Detect package manager
        if let Some(pm) = node_common::detect_package_manager(project_root) {
            metadata.insert("package_manager".to_string(), pm);
        }

        // Detect frameworks
        let frameworks = node_common::detect_frameworks(&package_json);
        if !frameworks.is_empty() {
            metadata.insert(
                "frameworks".to_string(),
                DetectedFramework::join_display_names(&frameworks),
            );
        }

        // Next.js router style — only emitted when Next.js is in
        // play. Drives module-detection conventions downstream
        // (App Router groups by `app/<segment>/`, Pages Router by
        // `pages/<segment>/`).
        if frameworks.contains(&DetectedFramework::NextJs) {
            if let Some(router) = node_common::detect_next_router(project_root) {
                metadata.insert("next_router".to_string(), router);
            }
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
            // For a single package, the root is the only module.
            // Sub-modules may still exist in subdirectories.
            module_markers: vec![],

            // Common entry points
            entry_point_files: vec![
                "index.ts".to_string(),
                "index.tsx".to_string(),
                "index.js".to_string(),
                "index.jsx".to_string(),
                "src/index.ts".to_string(),
                "src/index.js".to_string(),
            ],
        }
    }

    async fn extract_metadata(&self, project_root: &Path) -> Result<HashMap<String, String>> {
        let mut metadata = HashMap::new();

        let package_json = project_root.join("package.json");
        if !package_json.exists() {
            return Ok(metadata);
        }

        if let Some(pm) = node_common::detect_package_manager(project_root) {
            metadata.insert("package_manager".to_string(), pm);
        }

        let frameworks = node_common::detect_frameworks(&package_json);
        if !frameworks.is_empty() {
            metadata.insert(
                "frameworks".to_string(),
                DetectedFramework::join_display_names(&frameworks),
            );
        }

        Ok(metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_package_json(dir: &Path, content: &str) {
        fs::write(dir.join("package.json"), content).unwrap();
    }

    #[tokio::test]
    async fn test_detect_single_package() {
        let temp_dir = TempDir::new().unwrap();
        create_package_json(temp_dir.path(), r#"{"name": "test"}"#);

        let analyzer = NodeSinglePackageAnalyzer::new();
        let detection = analyzer.detect(temp_dir.path()).await.unwrap();

        // package.json without workspaces or monorepo structure
        assert_eq!(detection.confidence, 0.8);
        assert!(detection.evidence.iter().any(|e| e.contains("package.json")));
        assert!(detection
            .evidence
            .iter()
            .any(|e| e.contains("No monorepo structure")));
    }

    #[tokio::test]
    async fn test_reject_workspaces() {
        let temp_dir = TempDir::new().unwrap();
        create_package_json(
            temp_dir.path(),
            r#"{"name": "test", "workspaces": ["packages/*"]}"#,
        );

        let analyzer = NodeSinglePackageAnalyzer::new();
        let detection = analyzer.detect(temp_dir.path()).await.unwrap();

        // Must reject projects with workspaces
        assert_eq!(detection.confidence, 0.0);
    }

    #[tokio::test]
    async fn test_module_strategy_no_markers() {
        let analyzer = NodeSinglePackageAnalyzer::new();
        let strategy = analyzer.module_detection_strategy();

        // Single package doesn't use module_markers (uses entry points)
        assert!(strategy.module_markers.is_empty());
        assert!(!strategy.entry_point_files.is_empty());
    }

    #[tokio::test]
    async fn test_detect_nextjs_app_router() {
        let temp = TempDir::new().unwrap();
        create_package_json(
            temp.path(),
            r#"{"name": "site", "dependencies": {"next": "^14.0.0", "react": "^18.0.0"}}"#,
        );
        fs::create_dir_all(temp.path().join("app")).unwrap();

        let analyzer = NodeSinglePackageAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        assert!(detection.frameworks.contains(&DetectedFramework::NextJs));
        assert_eq!(detection.metadata.get("next_router").unwrap(), "app");
    }

    #[tokio::test]
    async fn test_detect_nextjs_pages_router() {
        let temp = TempDir::new().unwrap();
        create_package_json(
            temp.path(),
            r#"{"name": "site", "dependencies": {"next": "^12.0.0", "react": "^18.0.0"}}"#,
        );
        fs::create_dir_all(temp.path().join("pages")).unwrap();

        let analyzer = NodeSinglePackageAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        assert!(detection.frameworks.contains(&DetectedFramework::NextJs));
        assert_eq!(detection.metadata.get("next_router").unwrap(), "pages");
    }

    #[tokio::test]
    async fn test_detect_nextjs_both_routers_during_migration() {
        let temp = TempDir::new().unwrap();
        create_package_json(
            temp.path(),
            r#"{"name": "site", "dependencies": {"next": "^14.0.0", "react": "^18.0.0"}}"#,
        );
        fs::create_dir_all(temp.path().join("app")).unwrap();
        fs::create_dir_all(temp.path().join("pages")).unwrap();

        let analyzer = NodeSinglePackageAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        assert_eq!(
            detection.metadata.get("next_router").unwrap(),
            "app+pages"
        );
    }

    #[tokio::test]
    async fn test_detect_nextjs_via_config_fallback() {
        // Project where `next` dep is hoisted to a monorepo root and
        // therefore absent from this package.json. The `next.config.mjs`
        // fallback should still surface Next.js.
        let temp = TempDir::new().unwrap();
        create_package_json(
            temp.path(),
            r#"{"name": "site", "dependencies": {"react": "^18.0.0"}}"#,
        );
        fs::write(
            temp.path().join("next.config.mjs"),
            "export default {};",
        )
        .unwrap();
        fs::create_dir_all(temp.path().join("src").join("app")).unwrap();

        let analyzer = NodeSinglePackageAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        assert!(detection.frameworks.contains(&DetectedFramework::NextJs));
        // Router detection also handles `src/app` layout.
        assert_eq!(detection.metadata.get("next_router").unwrap(), "app");
    }
}
