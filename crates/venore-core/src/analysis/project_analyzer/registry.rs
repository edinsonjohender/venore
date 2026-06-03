use crate::Result;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::Path;

use super::analyzers::{
    CCppProjectAnalyzer, DotnetProjectAnalyzer, GodotAnalyzer, KotlinGradleAnalyzer,
    NodeMonorepoAnalyzer, NodeSinglePackageAnalyzer, PhpComposerAnalyzer,
    PythonPoetryAnalyzer, RubyBundlerAnalyzer, RustSingleCrateAnalyzer,
    RustWorkspaceAnalyzer,
};
use super::traits::{ProjectAnalyzer, ProjectType, ProjectTypeDetection};

/// Global registry of project analyzers.
pub struct AnalyzerRegistry {
    analyzers: HashMap<ProjectType, Box<dyn ProjectAnalyzer>>,
}

impl AnalyzerRegistry {
    /// Build a new registry and register every built-in analyzer.
    fn new() -> Self {
        let mut analyzers: HashMap<ProjectType, Box<dyn ProjectAnalyzer>> = HashMap::new();

        // Register built-in analyzers
        analyzers.insert(
            ProjectType::NodeMonorepo,
            Box::new(NodeMonorepoAnalyzer::new()),
        );
        analyzers.insert(
            ProjectType::NodeSinglePackage,
            Box::new(NodeSinglePackageAnalyzer::new()),
        );
        analyzers.insert(
            ProjectType::RustWorkspace,
            Box::new(RustWorkspaceAnalyzer::new()),
        );
        analyzers.insert(
            ProjectType::RustSingleCrate,
            Box::new(RustSingleCrateAnalyzer::new()),
        );
        analyzers.insert(
            ProjectType::PythonPoetry,
            Box::new(PythonPoetryAnalyzer::new()),
        );
        analyzers.insert(
            ProjectType::PhpComposer,
            Box::new(PhpComposerAnalyzer::new()),
        );
        analyzers.insert(
            ProjectType::RubyBundler,
            Box::new(RubyBundlerAnalyzer::new()),
        );
        analyzers.insert(
            ProjectType::KotlinGradle,
            Box::new(KotlinGradleAnalyzer::new()),
        );
        analyzers.insert(
            ProjectType::CCppProject,
            Box::new(CCppProjectAnalyzer::new()),
        );
        analyzers.insert(ProjectType::Godot, Box::new(GodotAnalyzer::new()));
        analyzers.insert(
            ProjectType::DotnetProject,
            Box::new(DotnetProjectAnalyzer::new()),
        );

        Self { analyzers }
    }

    /// Look up an analyzer by project type.
    pub fn get(&self, project_type: &ProjectType) -> Option<&dyn ProjectAnalyzer> {
        self.analyzers.get(project_type).map(|b| b.as_ref())
    }

    /// List every registered analyzer.
    pub fn list_all(&self) -> Vec<&dyn ProjectAnalyzer> {
        self.analyzers.values().map(|b| b.as_ref()).collect()
    }

    /// Auto-detect the project type.
    /// Runs every analyzer in parallel and returns the highest-confidence detection.
    pub async fn auto_detect(&self, project_root: &Path) -> Result<ProjectTypeDetection> {
        let mut detections = Vec::new();

        // Run every analyzer
        for analyzer in self.list_all() {
            if let Ok(detection) = analyzer.detect(project_root).await {
                if detection.confidence > 0.0 {
                    detections.push(detection);
                }
            }
        }

        // Sort by confidence, descending
        detections.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Return the highest-confidence match, or Unknown if there is none
        Ok(detections.into_iter().next().unwrap_or_else(|| {
            ProjectTypeDetection {
                project_type: ProjectType::Unknown,
                confidence: 1.0,
                evidence: vec!["No specific project type detected".to_string()],
                metadata: HashMap::new(),
                frameworks: Vec::new(),
            }
        }))
    }
}

/// Global registry instance (lazy init).
static REGISTRY: Lazy<AnalyzerRegistry> = Lazy::new(AnalyzerRegistry::new);

/// Access the global registry.
pub fn registry() -> &'static AnalyzerRegistry {
    &REGISTRY
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_initialization() {
        let reg = registry();
        let known = ProjectType::all_known();

        // Every concrete project type (= everything except Unknown) must
        // have an analyzer registered. Iterating over all_known() means
        // adding a new ProjectType variant without registering an
        // analyzer for it fails this test deterministically.
        for project_type in known {
            assert!(
                reg.get(project_type).is_some(),
                "missing analyzer for {:?}",
                project_type
            );
        }

        // Sanity: the analyzer count equals the number of known types
        // (we don't expect orphan registrations).
        assert_eq!(reg.list_all().len(), known.len());
    }

    #[tokio::test]
    async fn test_auto_detect_unknown() {
        let reg = registry();
        // Isolated TempDir, not std::env::temp_dir(): the latter is shared
        // and can have stray markers (package.json, Cargo.toml) that a real
        // analyzer would pick up.
        let temp_dir = tempfile::TempDir::new().unwrap();

        let detection = reg.auto_detect(temp_dir.path()).await.unwrap();

        // With no matching analyzer, should return Unknown
        assert_eq!(detection.project_type, ProjectType::Unknown);
        assert_eq!(detection.confidence, 1.0);
    }
}
