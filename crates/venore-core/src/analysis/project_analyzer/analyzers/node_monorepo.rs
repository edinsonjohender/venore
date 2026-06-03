use crate::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;

use super::common::node as node_common;
use super::super::traits::{
    DetectedFramework, ModuleDetectionStrategy, ProjectAnalyzer, ProjectType,
    ProjectTypeDetection,
};

/// Analyzer for Node.js monorepo projects.
///
/// Detects projects with:
/// - package.json at the root
/// - A workspaces configuration (npm, yarn, pnpm)
/// - Monorepo structure (packages/, apps/, libs/, etc.)
pub struct NodeMonorepoAnalyzer;

impl Default for NodeMonorepoAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeMonorepoAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Detect a common monorepo directory layout.
    fn detect_monorepo_structure(&self, project_root: &Path) -> Vec<String> {
        let common_dirs = ["packages", "apps", "libs", "modules", "services"];

        common_dirs
            .iter()
            .filter(|dir| project_root.join(dir).is_dir())
            .map(|s| s.to_string())
            .collect()
    }
}

#[async_trait]
impl ProjectAnalyzer for NodeMonorepoAnalyzer {
    fn name(&self) -> &str {
        "node-monorepo"
    }

    fn project_type(&self) -> ProjectType {
        ProjectType::NodeMonorepo
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

        let mut confidence = 0.3; // Base confidence for having a package.json
        let mut evidence = vec!["Found package.json".to_string()];
        let mut metadata = HashMap::new();

        // Check workspaces config
        if node_common::has_workspaces_config(&package_json) {
            confidence += 0.4;
            evidence.push("Found workspaces configuration".to_string());
        }

        // Check monorepo structure
        let monorepo_dirs = self.detect_monorepo_structure(project_root);
        if !monorepo_dirs.is_empty() {
            confidence += 0.3;
            evidence.push(format!(
                "Found monorepo directories: {}",
                monorepo_dirs.join(", ")
            ));
            metadata.insert("monorepo_dirs".to_string(), monorepo_dirs.join(","));
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

        // Monorepos sometimes host a Next.js app at the root with
        // packages alongside; surface the router when detected.
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
            // In a monorepo, each package.json marks a module
            module_markers: vec!["package.json".to_string()],

            // Common Node entry points
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

        // Package manager
        if let Some(pm) = node_common::detect_package_manager(project_root) {
            metadata.insert("package_manager".to_string(), pm);
        }

        // Frameworks
        let frameworks = node_common::detect_frameworks(&package_json);
        if !frameworks.is_empty() {
            metadata.insert(
                "frameworks".to_string(),
                DetectedFramework::join_display_names(&frameworks),
            );
        }

        // Monorepo directories
        let monorepo_dirs = self.detect_monorepo_structure(project_root);
        if !monorepo_dirs.is_empty() {
            metadata.insert("monorepo_dirs".to_string(), monorepo_dirs.join(","));
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
    async fn test_detect_no_package_json() {
        let temp_dir = TempDir::new().unwrap();
        let analyzer = NodeMonorepoAnalyzer::new();

        let detection = analyzer.detect(temp_dir.path()).await.unwrap();

        assert_eq!(detection.confidence, 0.0);
        assert!(detection.evidence.is_empty());
    }

    #[tokio::test]
    async fn test_detect_simple_package() {
        let temp_dir = TempDir::new().unwrap();
        create_package_json(temp_dir.path(), r#"{"name": "test"}"#);

        let analyzer = NodeMonorepoAnalyzer::new();
        let detection = analyzer.detect(temp_dir.path()).await.unwrap();

        // Only package.json, no workspaces or structure
        assert_eq!(detection.confidence, 0.3);
        assert_eq!(detection.evidence.len(), 1);
        assert!(detection.evidence[0].contains("package.json"));
    }

    #[tokio::test]
    async fn test_detect_with_workspaces() {
        let temp_dir = TempDir::new().unwrap();
        create_package_json(
            temp_dir.path(),
            r#"{"name": "test", "workspaces": ["packages/*"]}"#,
        );

        let analyzer = NodeMonorepoAnalyzer::new();
        let detection = analyzer.detect(temp_dir.path()).await.unwrap();

        // package.json + workspaces
        assert!((detection.confidence - 0.7).abs() < 0.01); // Approximate comparison
        assert!(detection.evidence.iter().any(|e| e.contains("workspaces")));
    }

    #[tokio::test]
    async fn test_detect_full_monorepo() {
        let temp_dir = TempDir::new().unwrap();
        create_package_json(
            temp_dir.path(),
            r#"{"name": "test", "workspaces": ["packages/*"]}"#,
        );

        // Create the monorepo structure
        fs::create_dir(temp_dir.path().join("packages")).unwrap();
        fs::create_dir(temp_dir.path().join("apps")).unwrap();

        let analyzer = NodeMonorepoAnalyzer::new();
        let detection = analyzer.detect(temp_dir.path()).await.unwrap();

        // package.json + workspaces + structure
        assert_eq!(detection.confidence, 1.0);
        assert!(detection.evidence.iter().any(|e| e.contains("workspaces")));
        assert!(detection
            .evidence
            .iter()
            .any(|e| e.contains("monorepo directories")));
        assert_eq!(detection.metadata.get("monorepo_dirs").unwrap(), "packages,apps");
    }

    #[tokio::test]
    async fn test_detect_package_manager() {
        let temp_dir = TempDir::new().unwrap();
        create_package_json(temp_dir.path(), r#"{"name": "test"}"#);

        // Create pnpm-lock.yaml
        fs::write(temp_dir.path().join("pnpm-lock.yaml"), "").unwrap();

        let analyzer = NodeMonorepoAnalyzer::new();
        let detection = analyzer.detect(temp_dir.path()).await.unwrap();

        assert_eq!(detection.metadata.get("package_manager").unwrap(), "pnpm");
    }

    #[tokio::test]
    async fn test_detect_frameworks() {
        let temp_dir = TempDir::new().unwrap();
        create_package_json(
            temp_dir.path(),
            r#"{"name": "test", "dependencies": {"react": "^18.0.0", "express": "^4.0.0"}}"#,
        );

        let analyzer = NodeMonorepoAnalyzer::new();
        let detection = analyzer.detect(temp_dir.path()).await.unwrap();

        let frameworks = detection.metadata.get("frameworks").unwrap();
        assert!(frameworks.contains("React"));
        assert!(frameworks.contains("Express"));
    }

    #[test]
    fn test_module_detection_strategy() {
        let analyzer = NodeMonorepoAnalyzer::new();
        let strategy = analyzer.module_detection_strategy();

        assert_eq!(strategy.module_markers, vec!["package.json"]);
        assert!(strategy.entry_point_files.contains(&"index.ts".to_string()));
        assert!(strategy.entry_point_files.contains(&"index.js".to_string()));
    }
}
