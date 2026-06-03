//! Helpers shared by Ruby analyzers.
//!
//! `Gemfile` is Ruby DSL, not a structured format. Rather than embed a
//! Ruby evaluator we parse the small subset that matters with a regex:
//! lines of the form `gem 'name'` / `gem "name"` (with or without
//! version constraints / groups). This is the same pragmatic shortcut
//! Bundler-aware tooling like `bundler-audit` uses.

use std::path::Path;

use once_cell::sync::Lazy;
use regex::Regex;

use super::super::super::traits::DetectedFramework;

/// Match a `gem 'name'` (or `gem "name"`) line, capturing the gem name.
/// Allows leading whitespace, trailing comma/version constraints, etc.
static RE_GEM_LINE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*gem\s+['"]([A-Za-z0-9_\-]+)['"]"#).unwrap()
});

/// Read the textual contents of a Gemfile.
pub fn read_gemfile(gemfile_path: &Path) -> Option<String> {
    std::fs::read_to_string(gemfile_path).ok()
}

/// Detect declared frameworks from a Gemfile.
///
/// Reads top-level `gem` declarations and maps known names to
/// `DetectedFramework`. Ignores `gemspec` declarations and `group`
/// blocks — Rails / Sinatra are typically declared at the top level
/// anyway.
pub fn detect_frameworks(gemfile_text: &str) -> Vec<DetectedFramework> {
    let mut frameworks = Vec::new();

    for cap in RE_GEM_LINE.captures_iter(gemfile_text) {
        if let Some(name) = cap.get(1) {
            if let Some(fw) = DetectedFramework::from_dep_name(name.as_str()) {
                frameworks.push(fw);
            }
        }
    }

    frameworks
}

/// Heuristic Rails detection from filesystem layout.
///
/// A Gemfile alone doesn't prove Rails — `rails` may live in a child
/// app's Gemfile only. Rails projects always have `config/application.rb`
/// and `bin/rails`. If either is present, we're confident this is a
/// Rails project regardless of how the dep is declared.
pub fn is_rails_project(project_root: &Path) -> bool {
    project_root.join("config").join("application.rb").exists()
        || project_root.join("bin").join("rails").exists()
}
