use crate::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;

use super::common::ruby as ruby_common;
use super::super::traits::{
    DetectedFramework, ModuleDetectionStrategy, ProjectAnalyzer, ProjectType,
    ProjectTypeDetection,
};

/// Analyzer for Ruby projects managed with Bundler.
///
/// Detects projects with:
/// - `Gemfile` at the root (apps, scripts, gems with examples)
/// - Optionally `*.gemspec` next to it (published gems)
///
/// Strategy: groups by `lib/` and Rails-style `app/<concept>/` when
/// the project is Rails.
pub struct RubyBundlerAnalyzer;

impl Default for RubyBundlerAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl RubyBundlerAnalyzer {
    pub fn new() -> Self {
        Self
    }

    fn has_gemspec(&self, project_root: &Path) -> bool {
        if let Ok(entries) = std::fs::read_dir(project_root) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".gemspec") {
                        return true;
                    }
                }
            }
        }
        false
    }
}

#[async_trait]
impl ProjectAnalyzer for RubyBundlerAnalyzer {
    fn name(&self) -> &str {
        "ruby-bundler"
    }

    fn project_type(&self) -> ProjectType {
        ProjectType::RubyBundler
    }

    async fn detect(&self, project_root: &Path) -> Result<ProjectTypeDetection> {
        let gemfile = project_root.join("Gemfile");

        if !gemfile.exists() {
            return Ok(ProjectTypeDetection {
                project_type: self.project_type(),
                confidence: 0.0,
                evidence: vec![],
                metadata: HashMap::new(),
                frameworks: Vec::new(),
            });
        }

        let mut confidence: f32 = 0.85;
        let mut evidence = vec!["Found Gemfile".to_string()];
        let mut metadata = HashMap::new();

        if project_root.join("Gemfile.lock").exists() {
            confidence += 0.05;
            evidence.push("Found Gemfile.lock".to_string());
        }

        if self.has_gemspec(project_root) {
            evidence.push("Found *.gemspec (library project)".to_string());
            metadata.insert("library".to_string(), "true".to_string());
        }

        // Framework detection — Gemfile is Ruby DSL, parsed via regex.
        let mut frameworks: Vec<DetectedFramework> = Vec::new();
        if let Some(text) = ruby_common::read_gemfile(&gemfile) {
            frameworks = ruby_common::detect_frameworks(&text);
        }

        // Filesystem-level Rails sanity check — picks up projects that
        // declare `rails` only in a child Gemfile or use path-based
        // dependencies. We add Rails to the list even if the gem line
        // wasn't matched.
        if ruby_common::is_rails_project(project_root)
            && !frameworks.contains(&DetectedFramework::Rails)
        {
            frameworks.push(DetectedFramework::Rails);
            evidence.push("Found Rails layout (config/application.rb or bin/rails)".to_string());
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
            // No file-level marker for Ruby modules. Group via folder
            // heuristics in the module detector (e.g. `lib/`, `app/`).
            module_markers: vec![],
            entry_point_files: vec![
                "lib/main.rb".to_string(),
                "config/application.rb".to_string(),
                "config.ru".to_string(),
                "Rakefile".to_string(),
            ],
        }
    }

    async fn extract_metadata(&self, project_root: &Path) -> Result<HashMap<String, String>> {
        let mut metadata = HashMap::new();

        let gemfile = project_root.join("Gemfile");
        if !gemfile.exists() {
            return Ok(metadata);
        }

        if let Some(text) = ruby_common::read_gemfile(&gemfile) {
            let frameworks = ruby_common::detect_frameworks(&text);
            if !frameworks.is_empty() {
                metadata.insert(
                    "frameworks".to_string(),
                    DetectedFramework::join_display_names(&frameworks),
                );
            }
        }

        if ruby_common::is_rails_project(project_root) {
            metadata.insert("rails_layout".to_string(), "true".to_string());
        }
        if self.has_gemspec(project_root) {
            metadata.insert("library".to_string(), "true".to_string());
        }

        Ok(metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_gemfile(dir: &Path, content: &str) {
        fs::write(dir.join("Gemfile"), content).unwrap();
    }

    #[tokio::test]
    async fn test_detect_no_gemfile() {
        let temp_dir = TempDir::new().unwrap();
        let analyzer = RubyBundlerAnalyzer::new();
        let detection = analyzer.detect(temp_dir.path()).await.unwrap();
        assert_eq!(detection.confidence, 0.0);
    }

    #[tokio::test]
    async fn test_detect_plain_gemfile() {
        let temp_dir = TempDir::new().unwrap();
        write_gemfile(
            temp_dir.path(),
            r#"source 'https://rubygems.org'
gem 'json'
gem 'rake'
"#,
        );

        let analyzer = RubyBundlerAnalyzer::new();
        let detection = analyzer.detect(temp_dir.path()).await.unwrap();

        assert!(detection.confidence >= 0.85);
        assert!(detection.frameworks.is_empty());
    }

    #[tokio::test]
    async fn test_detect_rails_via_gem_line() {
        let temp_dir = TempDir::new().unwrap();
        write_gemfile(
            temp_dir.path(),
            r#"source 'https://rubygems.org'
gem 'rails', '~> 7.0'
gem 'puma'
"#,
        );

        let analyzer = RubyBundlerAnalyzer::new();
        let detection = analyzer.detect(temp_dir.path()).await.unwrap();

        assert!(detection.frameworks.contains(&DetectedFramework::Rails));
        assert!(detection
            .metadata
            .get("frameworks")
            .unwrap()
            .contains("Rails"));
    }

    #[tokio::test]
    async fn test_detect_rails_via_layout_only() {
        let temp_dir = TempDir::new().unwrap();
        write_gemfile(temp_dir.path(), "source 'https://rubygems.org'\n");
        fs::create_dir_all(temp_dir.path().join("config")).unwrap();
        fs::write(
            temp_dir.path().join("config").join("application.rb"),
            "module App; end",
        )
        .unwrap();

        let analyzer = RubyBundlerAnalyzer::new();
        let detection = analyzer.detect(temp_dir.path()).await.unwrap();

        assert!(detection.frameworks.contains(&DetectedFramework::Rails));
    }

    #[tokio::test]
    async fn test_detect_sinatra() {
        let temp_dir = TempDir::new().unwrap();
        write_gemfile(
            temp_dir.path(),
            r#"source 'https://rubygems.org'
gem 'sinatra'
"#,
        );

        let analyzer = RubyBundlerAnalyzer::new();
        let detection = analyzer.detect(temp_dir.path()).await.unwrap();

        assert!(detection.frameworks.contains(&DetectedFramework::Sinatra));
    }

    #[tokio::test]
    async fn test_detect_gemspec_library() {
        let temp_dir = TempDir::new().unwrap();
        write_gemfile(temp_dir.path(), "gemspec\n");
        fs::write(temp_dir.path().join("my_gem.gemspec"), "").unwrap();

        let analyzer = RubyBundlerAnalyzer::new();
        let detection = analyzer.detect(temp_dir.path()).await.unwrap();

        assert_eq!(detection.metadata.get("library").unwrap(), "true");
    }
}
