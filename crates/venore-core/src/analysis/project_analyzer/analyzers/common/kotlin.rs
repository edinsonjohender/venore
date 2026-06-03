//! Helpers shared by Kotlin / Gradle analyzers.
//!
//! Gradle build files come in two flavors:
//!   - `build.gradle.kts` — Kotlin DSL (preferred for new projects)
//!   - `build.gradle` — Groovy DSL (legacy / Android template default)
//!
//! Both are real source code, not structured config. Rather than embed
//! a Groovy/Kotlin evaluator we run regex against the textual form to
//! pick out the well-known dep and plugin IDs that identify the
//! framework family. This is the same pragmatic shortcut Detekt /
//! Gradle Doctor use.

use std::path::Path;

use once_cell::sync::Lazy;
use regex::Regex;

use super::super::super::traits::DetectedFramework;

/// Match `dependencies { ... "group:artifact:version" }` lines.
/// Captures the group:artifact pair.
static RE_GRADLE_DEP: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"["']([a-zA-Z0-9._-]+:[a-zA-Z0-9._-]+)(?::[^"']*)?["']"#).unwrap()
});

/// Match Gradle plugin IDs declared in `plugins { id("...") }` blocks.
static RE_GRADLE_PLUGIN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"id\s*\(?\s*["']([a-zA-Z0-9._-]+)["']\s*\)?"#).unwrap()
});

/// Read either `build.gradle.kts` or `build.gradle` if present at
/// `project_root`. Returns `None` when neither file exists.
pub fn read_build_gradle(project_root: &Path) -> Option<String> {
    let kts = project_root.join("build.gradle.kts");
    if kts.exists() {
        return std::fs::read_to_string(&kts).ok();
    }
    let groovy = project_root.join("build.gradle");
    if groovy.exists() {
        return std::fs::read_to_string(&groovy).ok();
    }
    None
}

/// Detect frameworks from a Gradle build script's textual content.
///
/// Looks at both dependency coordinates (`group:artifact:version`) and
/// plugin IDs — Android in particular is usually declared as a plugin
/// (`id("com.android.application")`) rather than a dep.
pub fn detect_frameworks(gradle_text: &str) -> Vec<DetectedFramework> {
    let mut frameworks = Vec::new();

    // Dependency-string scan. We match `group:artifact` pairs and check
    // for known prefixes (Ktor uses `io.ktor:ktor-*`, Spring Boot uses
    // `org.springframework.boot:*`, etc.).
    for cap in RE_GRADLE_DEP.captures_iter(gradle_text) {
        let Some(coord) = cap.get(1) else { continue };
        let coord = coord.as_str();

        if coord.starts_with("io.ktor:")
            && !frameworks.contains(&DetectedFramework::Ktor)
        {
            frameworks.push(DetectedFramework::Ktor);
        }
        if (coord.starts_with("org.springframework.boot:")
            || coord == "org.springframework.boot")
            && !frameworks.contains(&DetectedFramework::SpringBoot)
        {
            frameworks.push(DetectedFramework::SpringBoot);
        }
    }

    // Plugin-id scan. Android Gradle plugin is the canonical Android
    // signal — apps use `com.android.application`, libraries use
    // `com.android.library`. Spring Boot also ships a plugin.
    for cap in RE_GRADLE_PLUGIN.captures_iter(gradle_text) {
        let Some(plugin) = cap.get(1) else { continue };
        let plugin = plugin.as_str();

        if (plugin == "com.android.application" || plugin == "com.android.library")
            && !frameworks.contains(&DetectedFramework::AndroidApp)
        {
            frameworks.push(DetectedFramework::AndroidApp);
        }
        if plugin == "org.springframework.boot"
            && !frameworks.contains(&DetectedFramework::SpringBoot)
        {
            frameworks.push(DetectedFramework::SpringBoot);
        }
    }

    frameworks
}

/// Filesystem-level Android sanity check.
///
/// Android projects always ship an `AndroidManifest.xml` (either at
/// `<root>/AndroidManifest.xml` for older layouts or under
/// `app/src/main/AndroidManifest.xml` for the standard Gradle
/// template). This catches projects that declare the Android plugin
/// only in a subproject's build script.
pub fn is_android_project(project_root: &Path) -> bool {
    if project_root.join("AndroidManifest.xml").exists() {
        return true;
    }
    project_root
        .join("app")
        .join("src")
        .join("main")
        .join("AndroidManifest.xml")
        .exists()
}

/// Does this project use a Gradle wrapper? `gradlew` (UNIX) or
/// `gradlew.bat` (Windows) raises confidence that this is a real
/// Gradle project, not just a stray `build.gradle` left in a repo.
pub fn has_gradle_wrapper(project_root: &Path) -> bool {
    project_root.join("gradlew").exists() || project_root.join("gradlew.bat").exists()
}

/// Is this a multi-module Gradle build? Signaled by a settings file.
pub fn is_multi_module(project_root: &Path) -> bool {
    project_root.join("settings.gradle.kts").exists()
        || project_root.join("settings.gradle").exists()
}
