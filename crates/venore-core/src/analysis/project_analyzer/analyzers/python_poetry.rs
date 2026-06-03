use crate::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;

use super::common::python as python_common;
use super::super::traits::{
    DetectedFramework, ModuleDetectionStrategy, ProjectAnalyzer, ProjectType,
    ProjectTypeDetection,
};

/// Analyzer for Python projects managed with Poetry.
///
/// Detects projects with:
/// - pyproject.toml at the root
/// - A [tool.poetry] section
pub struct PythonPoetryAnalyzer;

impl Default for PythonPoetryAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl PythonPoetryAnalyzer {
    pub fn new() -> Self {
        Self
    }

    fn parse_pyproject_toml(&self, pyproject_path: &Path) -> Option<toml::Value> {
        let content = std::fs::read_to_string(pyproject_path).ok()?;
        content.parse::<toml::Value>().ok()
    }

    fn has_poetry_section(&self, toml: &toml::Value) -> bool {
        toml.get("tool")
            .and_then(|t| t.get("poetry"))
            .is_some()
    }

    fn get_python_version(&self, toml: &toml::Value) -> Option<String> {
        toml.get("tool")
            .and_then(|t| t.get("poetry"))
            .and_then(|p| p.get("dependencies"))
            .and_then(|d| d.get("python"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    fn get_package_name(&self, toml: &toml::Value) -> Option<String> {
        toml.get("tool")
            .and_then(|t| t.get("poetry"))
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
            .map(|s| s.to_string())
    }

    fn detect_frameworks(&self, toml: &toml::Value) -> Vec<DetectedFramework> {
        let mut frameworks = Vec::new();

        if let Some(deps) = toml
            .get("tool")
            .and_then(|t| t.get("poetry"))
            .and_then(|p| p.get("dependencies"))
            .and_then(|d| d.as_table())
        {
            for dep_name in deps.keys() {
                if let Some(fw) = DetectedFramework::from_dep_name(dep_name) {
                    frameworks.push(fw);
                }
            }
        }

        frameworks
    }
}

#[async_trait]
impl ProjectAnalyzer for PythonPoetryAnalyzer {
    fn name(&self) -> &str {
        "python-poetry"
    }

    fn project_type(&self) -> ProjectType {
        ProjectType::PythonPoetry
    }

    async fn detect(&self, project_root: &Path) -> Result<ProjectTypeDetection> {
        let pyproject_toml = project_root.join("pyproject.toml");

        if !pyproject_toml.exists() {
            return Ok(ProjectTypeDetection {
                project_type: self.project_type(),
                confidence: 0.0,
                evidence: vec![],
                metadata: HashMap::new(),
                frameworks: Vec::new(),
            });
        }

        let Some(toml) = self.parse_pyproject_toml(&pyproject_toml) else {
            return Ok(ProjectTypeDetection {
                project_type: self.project_type(),
                confidence: 0.0,
                evidence: vec!["Found pyproject.toml but failed to parse".to_string()],
                metadata: HashMap::new(),
                frameworks: Vec::new(),
            });
        };

        let mut confidence = 0.3; // Base confidence
        let mut evidence = vec!["Found pyproject.toml".to_string()];
        let mut metadata = HashMap::new();
        let mut frameworks: Vec<DetectedFramework> = Vec::new();

        // Check for [tool.poetry] section
        if self.has_poetry_section(&toml) {
            confidence += 0.7;
            evidence.push("Found [tool.poetry] section".to_string());

            // Package name
            if let Some(name) = self.get_package_name(&toml) {
                metadata.insert("package_name".to_string(), name);
            }

            // Python version
            if let Some(py_version) = self.get_python_version(&toml) {
                metadata.insert("python_version".to_string(), py_version);
            }

            // Frameworks
            frameworks = self.detect_frameworks(&toml);
        } else {
            // Without [tool.poetry] this may be a different kind of Python project
            evidence.push("pyproject.toml does not have [tool.poetry] section".to_string());
        }

        // Django filesystem fallback. Catches projects that declare
        // dependencies via `requirements.txt` (not parsed here) or
        // omit `django` from the explicit list. `manage.py` is the
        // canonical Django marker.
        if python_common::is_django_project(project_root)
            && !frameworks.contains(&DetectedFramework::Django)
        {
            frameworks.push(DetectedFramework::Django);
            evidence.push("Found manage.py (Django)".to_string());
        }

        if !frameworks.is_empty() {
            metadata.insert(
                "frameworks".to_string(),
                DetectedFramework::join_display_names(&frameworks),
            );
        }

        // Django app inventory — only useful when Django is in play.
        // Drives module-detection conventions downstream so each app
        // surfaces as its own module on the canvas.
        if frameworks.contains(&DetectedFramework::Django) {
            let apps = python_common::detect_django_apps(project_root);
            if !apps.is_empty() {
                metadata.insert("django_apps".to_string(), apps.join(","));
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
            // In Python, __init__.py marks a package/module
            module_markers: vec!["__init__.py".to_string()],

            // Common entry points
            entry_point_files: vec![
                "__init__.py".to_string(),
                "__main__.py".to_string(),
                "main.py".to_string(),
            ],
        }
    }

    async fn extract_metadata(&self, project_root: &Path) -> Result<HashMap<String, String>> {
        let mut metadata = HashMap::new();

        let pyproject_toml = project_root.join("pyproject.toml");
        if !pyproject_toml.exists() {
            return Ok(metadata);
        }

        if let Some(toml) = self.parse_pyproject_toml(&pyproject_toml) {
            if let Some(name) = self.get_package_name(&toml) {
                metadata.insert("package_name".to_string(), name);
            }

            if let Some(py_version) = self.get_python_version(&toml) {
                metadata.insert("python_version".to_string(), py_version);
            }

            let mut frameworks = self.detect_frameworks(&toml);
            if python_common::is_django_project(project_root)
                && !frameworks.contains(&DetectedFramework::Django)
            {
                frameworks.push(DetectedFramework::Django);
            }
            if !frameworks.is_empty() {
                metadata.insert(
                    "frameworks".to_string(),
                    DetectedFramework::join_display_names(&frameworks),
                );
            }
            if frameworks.contains(&DetectedFramework::Django) {
                let apps = python_common::detect_django_apps(project_root);
                if !apps.is_empty() {
                    metadata.insert("django_apps".to_string(), apps.join(","));
                }
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

    fn create_pyproject_toml(dir: &Path, content: &str) {
        fs::write(dir.join("pyproject.toml"), content).unwrap();
    }

    #[tokio::test]
    async fn test_detect_no_pyproject() {
        let temp_dir = TempDir::new().unwrap();
        let analyzer = PythonPoetryAnalyzer::new();

        let detection = analyzer.detect(temp_dir.path()).await.unwrap();

        assert_eq!(detection.confidence, 0.0);
    }

    #[tokio::test]
    async fn test_detect_poetry_project() {
        let temp_dir = TempDir::new().unwrap();
        create_pyproject_toml(
            temp_dir.path(),
            r#"
[tool.poetry]
name = "my-project"
version = "0.1.0"

[tool.poetry.dependencies]
python = "^3.9"
django = "^4.0"
"#,
        );

        let analyzer = PythonPoetryAnalyzer::new();
        let detection = analyzer.detect(temp_dir.path()).await.unwrap();

        assert_eq!(detection.confidence, 1.0);
        assert!(detection
            .evidence
            .iter()
            .any(|e| e.contains("tool.poetry")));
        assert_eq!(
            detection.metadata.get("package_name").unwrap(),
            "my-project"
        );
        assert_eq!(detection.metadata.get("python_version").unwrap(), "^3.9");
        assert!(detection
            .metadata
            .get("frameworks")
            .unwrap()
            .contains("Django"));
    }

    #[tokio::test]
    async fn test_reject_non_poetry() {
        let temp_dir = TempDir::new().unwrap();
        create_pyproject_toml(
            temp_dir.path(),
            r#"
[build-system]
requires = ["setuptools"]
"#,
        );

        let analyzer = PythonPoetryAnalyzer::new();
        let detection = analyzer.detect(temp_dir.path()).await.unwrap();

        // Without [tool.poetry] confidence must stay low
        assert_eq!(detection.confidence, 0.3);
    }

    #[test]
    fn test_module_detection_strategy() {
        let analyzer = PythonPoetryAnalyzer::new();
        let strategy = analyzer.module_detection_strategy();

        assert_eq!(strategy.module_markers, vec!["__init__.py"]);
        assert!(strategy
            .entry_point_files
            .contains(&"__init__.py".to_string()));
        assert!(strategy
            .entry_point_files
            .contains(&"__main__.py".to_string()));
    }

    #[tokio::test]
    async fn test_detect_django_via_manage_py_fallback() {
        // No Poetry, no django in deps — only the canonical Django
        // marker. Should still surface Django via filesystem fallback.
        let temp = TempDir::new().unwrap();
        create_pyproject_toml(
            temp.path(),
            r#"
[tool.poetry]
name = "my-site"
version = "0.1.0"

[tool.poetry.dependencies]
python = "^3.11"
"#,
        );
        fs::write(temp.path().join("manage.py"), "# Django manager\n").unwrap();

        let analyzer = PythonPoetryAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        assert!(detection.frameworks.contains(&DetectedFramework::Django));
        assert!(detection
            .evidence
            .iter()
            .any(|e| e.contains("manage.py")));
    }

    #[tokio::test]
    async fn test_detect_django_apps_at_root() {
        let temp = TempDir::new().unwrap();
        create_pyproject_toml(
            temp.path(),
            r#"
[tool.poetry]
name = "my-site"
version = "0.1.0"

[tool.poetry.dependencies]
python = "^3.11"
django = "^5.0"
"#,
        );
        fs::write(temp.path().join("manage.py"), "# Django\n").unwrap();
        // Two apps: users + blog.
        fs::create_dir_all(temp.path().join("users")).unwrap();
        fs::write(
            temp.path().join("users").join("apps.py"),
            "class UsersConfig: pass\n",
        )
        .unwrap();
        fs::create_dir_all(temp.path().join("blog")).unwrap();
        fs::write(
            temp.path().join("blog").join("apps.py"),
            "class BlogConfig: pass\n",
        )
        .unwrap();

        let analyzer = PythonPoetryAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        let apps = detection.metadata.get("django_apps").unwrap();
        assert!(apps.contains("users"));
        assert!(apps.contains("blog"));
    }

    #[tokio::test]
    async fn test_detect_django_apps_under_project_folder() {
        // Layout typical of `django-admin startproject mysite`:
        // <root>/manage.py
        // <root>/mysite/{settings.py,urls.py,__init__.py}
        // <root>/mysite/users/apps.py
        let temp = TempDir::new().unwrap();
        create_pyproject_toml(
            temp.path(),
            r#"
[tool.poetry]
name = "mysite"
version = "0.1.0"

[tool.poetry.dependencies]
python = "^3.11"
django = "^5.0"
"#,
        );
        fs::write(temp.path().join("manage.py"), "# Django\n").unwrap();
        let project_dir = temp.path().join("mysite");
        fs::create_dir_all(&project_dir).unwrap();
        fs::write(project_dir.join("__init__.py"), "").unwrap();
        fs::write(project_dir.join("settings.py"), "DEBUG = True\n").unwrap();
        fs::write(project_dir.join("urls.py"), "urlpatterns = []\n").unwrap();
        fs::create_dir_all(project_dir.join("users")).unwrap();
        fs::write(
            project_dir.join("users").join("apps.py"),
            "class UsersConfig: pass\n",
        )
        .unwrap();

        let analyzer = PythonPoetryAnalyzer::new();
        let detection = analyzer.detect(temp.path()).await.unwrap();

        let apps = detection.metadata.get("django_apps").unwrap();
        assert!(
            apps.contains("mysite/users"),
            "expected mysite/users in {apps}"
        );
    }
}
