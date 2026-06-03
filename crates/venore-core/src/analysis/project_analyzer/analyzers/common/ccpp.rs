//! Helpers shared by C/C++ analyzers.
//!
//! C/C++ has no canonical build manifest — projects ship with one of
//! many possible markers (CMake, Meson, Make, Conan, vcpkg, xmake).
//! This module surfaces detection helpers for each.

use std::path::Path;

use once_cell::sync::Lazy;
use regex::Regex;

use super::super::super::traits::DetectedFramework;

/// `find_package(Qt5 ...)` or `find_package(Qt6 ...)` in CMakeLists.txt.
static RE_CMAKE_QT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)find_package\s*\(\s*Qt[56]\b").unwrap()
});

/// Identified build system for a C/C++ project. Order roughly reflects
/// modern preference: CMake → Meson → vcpkg → conan → xmake → Make.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildSystem {
    CMake,
    Meson,
    Conan,
    Vcpkg,
    Xmake,
    Make,
}

impl BuildSystem {
    pub fn name(&self) -> &'static str {
        match self {
            Self::CMake => "CMake",
            Self::Meson => "Meson",
            Self::Conan => "Conan",
            Self::Vcpkg => "vcpkg",
            Self::Xmake => "xmake",
            Self::Make => "Make",
        }
    }

    /// How strongly does this marker prove "this is a C/C++ project"?
    /// CMake / Meson / xmake almost always mean C/C++; bare Makefiles
    /// are common in many other ecosystems too, so we weight them
    /// lower.
    pub fn base_confidence(&self) -> f32 {
        match self {
            Self::CMake | Self::Meson | Self::Xmake => 0.9,
            Self::Conan | Self::Vcpkg => 0.85,
            Self::Make => 0.5,
        }
    }
}

/// Detect every build system marker present at the project root.
/// Returns multiple if several are mixed (common in older projects
/// that ship both CMake and Make).
pub fn detect_build_systems(project_root: &Path) -> Vec<BuildSystem> {
    let mut out = Vec::new();
    if project_root.join("CMakeLists.txt").exists() {
        out.push(BuildSystem::CMake);
    }
    if project_root.join("meson.build").exists() {
        out.push(BuildSystem::Meson);
    }
    if project_root.join("conanfile.txt").exists()
        || project_root.join("conanfile.py").exists()
    {
        out.push(BuildSystem::Conan);
    }
    if project_root.join("vcpkg.json").exists() {
        out.push(BuildSystem::Vcpkg);
    }
    if project_root.join("xmake.lua").exists() {
        out.push(BuildSystem::Xmake);
    }
    if project_root.join("Makefile").exists()
        || project_root.join("GNUmakefile").exists()
    {
        out.push(BuildSystem::Make);
    }
    out
}

/// Detect C/C++ frameworks from a CMakeLists.txt textual content.
///
/// Today this catches Qt (Qt5/Qt6 via `find_package`). Future
/// additions slot in here without changing the analyzer's surface.
pub fn detect_frameworks_from_cmake(cmake_text: &str) -> Vec<DetectedFramework> {
    let mut frameworks = Vec::new();
    if RE_CMAKE_QT.is_match(cmake_text) {
        frameworks.push(DetectedFramework::Qt);
    }
    frameworks
}

/// Read `CMakeLists.txt` content if it exists at the project root.
pub fn read_cmakelists(project_root: &Path) -> Option<String> {
    std::fs::read_to_string(project_root.join("CMakeLists.txt")).ok()
}
