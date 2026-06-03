use crate::{Result, VenoreError};
use std::path::Path;

use super::registry::registry;
use super::traits::{ProjectType, ProjectTypeDetection};

/// Auto-detect the project type.
///
/// Runs every registered analyzer in parallel and returns the highest-confidence detection.
/// This is the main entry point of the system's public API.
///
/// # How it works
///
/// 1. Pull every analyzer from the registry
/// 2. Run `detect()` on each one in parallel
/// 3. Filter out detections with confidence == 0.0
/// 4. Sort by confidence, descending
/// 5. Return the top match, or `Unknown` if none applied
///
/// # Performance
///
/// Detection is fast (typically < 100ms) because analyzers only inspect
/// configuration files, not the full project tree.
///
/// # Parameters
///
/// * `project_root` - path to the root of the project to analyze
///
/// # Returns
///
/// [`ProjectTypeDetection`] with the detected type, confidence, evidence, and metadata.
/// If no analyzer detects the project, returns [`ProjectType::Unknown`] with confidence 1.0.
///
/// # Examples
///
/// ```no_run
/// use venore_core::analysis::project_analyzer::factory;
/// use std::path::Path;
///
/// # async fn example() {
/// let project_path = Path::new("./my-project");
/// let detection = factory::detect_project_type(project_path).await.unwrap();
///
/// println!("Type: {:?}", detection.project_type);
/// println!("Confidence: {:.0}%", detection.confidence * 100.0);
///
/// if detection.confidence >= 0.8 {
///     println!("High confidence - auto-confirming");
/// }
/// # }
/// ```
///
/// # See also
///
/// * [`ProjectType`](super::traits::ProjectType) - supported types
/// * [`get_analyzer`] - retrieve a specific analyzer by type
pub async fn detect_project_type(project_root: &Path) -> Result<ProjectTypeDetection> {
    registry().auto_detect(project_root).await
}

/// Get a specific analyzer by project type.
///
/// Looks up the registry for an analyzer registered for the given type.
/// Useful to fetch the detection strategy after confirming the type.
///
/// # Parameters
///
/// * `project_type` - desired analyzer's project type
///
/// # Returns
///
/// Static reference to the analyzer, or an error if it isn't registered.
///
/// # Errors
///
/// Returns an error when:
/// * The type has no registered analyzer (e.g. [`ProjectType::Unknown`])
/// * The type is invalid
///
/// # Examples
///
/// ```
/// use venore_core::analysis::project_analyzer::{factory, traits::ProjectType};
///
/// // Fetch the analyzer for a Node Monorepo
/// let analyzer = factory::get_analyzer(ProjectType::NodeMonorepo).unwrap();
/// let strategy = analyzer.module_detection_strategy();
///
/// assert_eq!(strategy.module_markers, vec!["package.json"]);
/// ```
///
/// ```
/// use venore_core::analysis::project_analyzer::{factory, traits::ProjectType};
///
/// // Unknown has no registered analyzer
/// let result = factory::get_analyzer(ProjectType::Unknown);
/// assert!(result.is_err());
/// ```
///
/// # See also
///
/// * [`detect_project_type`] - auto-detect the project type
/// * [`ProjectAnalyzer::module_detection_strategy`](super::traits::ProjectAnalyzer::module_detection_strategy) - fetch the detection strategy
pub fn get_analyzer(project_type: ProjectType) -> Result<&'static dyn super::traits::ProjectAnalyzer> {
    registry()
        .get(&project_type)
        .ok_or_else(|| {
            VenoreError::AnalysisError(format!(
                "No analyzer found for project type: {:?}",
                project_type
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_detect_project_type_unknown() {
        // Use an isolated empty TempDir, not std::env::temp_dir(), so a stray
        // package.json / Cargo.toml left in the system temp by another test
        // or tool doesn't trip a real analyzer.
        let temp_dir = tempfile::TempDir::new().unwrap();
        let detection = detect_project_type(temp_dir.path()).await.unwrap();

        // With no matching analyzer, should return Unknown
        assert_eq!(detection.project_type, ProjectType::Unknown);
    }

    #[test]
    fn test_get_analyzer_success() {
        let result = get_analyzer(ProjectType::NodeMonorepo);

        // With analyzers now registered, this should succeed
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name(), "node-monorepo");
    }

    #[test]
    fn test_get_analyzer_unknown() {
        let result = get_analyzer(ProjectType::Unknown);

        // Unknown is not registered as an analyzer
        assert!(result.is_err());
    }
}
