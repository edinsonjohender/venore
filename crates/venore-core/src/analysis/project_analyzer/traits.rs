use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Project types supported by the pluggable analyzer system.
///
/// Each type represents a distinct project layout with its own
/// module-detection strategy.
///
/// # Examples
///
/// ```
/// use venore_core::analysis::project_analyzer::traits::ProjectType;
///
/// let project_type = ProjectType::NodeMonorepo;
/// assert_eq!(project_type.display_name(), "Node.js Monorepo");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProjectType {
    /// Node.js monorepo with workspaces (npm, yarn, or pnpm).
    ///
    /// Detected via:
    /// - `package.json` at the root with a `workspaces` field
    /// - Monorepo structure (`packages/`, `apps/`, etc.)
    ///
    /// Strategy: uses `package.json` as the module marker.
    NodeMonorepo,

    /// Node.js single package without workspaces.
    ///
    /// Detected via:
    /// - `package.json` at the root WITHOUT a `workspaces` field
    ///
    /// Strategy: uses traditional entry points.
    NodeSinglePackage,

    /// Rust workspace with multiple crates.
    ///
    /// Detected via:
    /// - `Cargo.toml` at the root with a `[workspace]` section
    /// - `members` field listing the crates
    ///
    /// Strategy: uses `Cargo.toml` as the module marker.
    RustWorkspace,

    /// Single Rust crate without a workspace.
    ///
    /// Detected via:
    /// - `Cargo.toml` at the root WITHOUT a `[workspace]` section
    ///
    /// Strategy: uses traditional entry points.
    RustSingleCrate,

    /// Python project managed by Poetry.
    ///
    /// Detected via:
    /// - `pyproject.toml` with a `[tool.poetry]` section
    ///
    /// Strategy: uses `__init__.py` as the module marker.
    PythonPoetry,

    /// PHP project managed by Composer.
    ///
    /// Detected via:
    /// - `composer.json` at the root
    ///
    /// Strategy: groups by Composer packages and PSR-4 autoload roots
    /// (`src/`, `app/`).
    PhpComposer,

    /// Ruby project managed by Bundler.
    ///
    /// Detected via:
    /// - `Gemfile` at the root (with or without `*.gemspec` next to it)
    ///
    /// Strategy: groups by `lib/` and Rails-style `app/<concept>/` if
    /// the project is Rails.
    RubyBundler,

    /// Kotlin project built with Gradle (single-module or multi-module).
    ///
    /// Detected via:
    /// - `build.gradle.kts` or `build.gradle` at the root
    /// - `settings.gradle.kts` / `settings.gradle` (multi-module)
    ///
    /// Strategy: each `build.gradle.kts` marks a module (Gradle
    /// subproject); falls back to `src/main/kotlin/` entry points.
    KotlinGradle,

    /// C or C++ project.
    ///
    /// Detected via the presence of any common C/C++ build-system
    /// marker:
    /// - `CMakeLists.txt` (CMake — modern default)
    /// - `meson.build` (Meson)
    /// - `Makefile` (raw Make)
    /// - `conanfile.{txt,py}` (Conan package manager)
    /// - `vcpkg.json` (vcpkg)
    /// - `xmake.lua` (xmake)
    ///
    /// The detected build system is reported in metadata. Strategy:
    /// group by top-level folders (`src/`, `include/`, `lib/`, …).
    CCppProject,

    /// Godot game project.
    ///
    /// Detected via `project.godot` at the root (Godot's canonical
    /// project descriptor). Strategy: groups by top-level folders
    /// (Godot projects typically use `scripts/`, `scenes/`, `assets/`).
    Godot,

    /// .NET / C# project.
    ///
    /// Detected via `*.csproj` or `*.sln` at the root. Covers Unity
    /// games, ASP.NET apps, console apps, libraries — the analyzer
    /// surfaces specifics (Unity / ASP.NET / library) in metadata and
    /// frameworks.
    ///
    /// Strategy: each `.csproj` marks a project (= module), mirroring
    /// the Rust workspace convention.
    DotnetProject,

    /// Unknown type — falls back to the entry-point algorithm.
    ///
    /// Used when no analyzer detects the project type.
    /// Maintains 100% backward compatibility with the previous algorithm.
    Unknown,
}

impl ProjectType {
    /// Human-readable name for the project type.
    pub fn display_name(&self) -> &str {
        match self {
            Self::NodeMonorepo => "Node.js Monorepo",
            Self::NodeSinglePackage => "Node.js Single Package",
            Self::RustWorkspace => "Rust Workspace",
            Self::RustSingleCrate => "Rust Single Crate",
            Self::PythonPoetry => "Python (Poetry)",
            Self::PhpComposer => "PHP (Composer)",
            Self::RubyBundler => "Ruby (Bundler)",
            Self::KotlinGradle => "Kotlin (Gradle)",
            Self::CCppProject => "C / C++",
            Self::Godot => "Godot",
            Self::DotnetProject => ".NET / C#",
            Self::Unknown => "Unknown",
        }
    }

    /// Every concrete project type, excluding `Unknown`.
    ///
    /// Used by the registry to validate that every detectable type has an
    /// analyzer registered. The internal `match` is an exhaustiveness
    /// witness: when a new variant is added to `ProjectType`, this fn
    /// fails to compile until the returned slice is updated, which forces
    /// the developer to also register an analyzer for the new type.
    pub fn all_known() -> &'static [ProjectType] {
        #[allow(dead_code)]
        fn exhaustiveness_witness(t: ProjectType) {
            match t {
                ProjectType::NodeMonorepo => (),
                ProjectType::NodeSinglePackage => (),
                ProjectType::RustWorkspace => (),
                ProjectType::RustSingleCrate => (),
                ProjectType::PythonPoetry => (),
                ProjectType::PhpComposer => (),
                ProjectType::RubyBundler => (),
                ProjectType::KotlinGradle => (),
                ProjectType::CCppProject => (),
                ProjectType::Godot => (),
                ProjectType::DotnetProject => (),
                ProjectType::Unknown => (),
            }
        }
        &[
            ProjectType::NodeMonorepo,
            ProjectType::NodeSinglePackage,
            ProjectType::RustWorkspace,
            ProjectType::RustSingleCrate,
            ProjectType::PythonPoetry,
            ProjectType::PhpComposer,
            ProjectType::RubyBundler,
            ProjectType::KotlinGradle,
            ProjectType::CCppProject,
            ProjectType::Godot,
            ProjectType::DotnetProject,
        ]
    }
}

/// Result of a project-type detection.
///
/// Holds the detected type, confidence level, evidence, and metadata.
///
/// # Fields
///
/// * `project_type` - the detected project type (see [`ProjectType`])
/// * `confidence` - confidence level from 0.0 to 1.0
///   - 0.0 = not detected
///   - < 0.8 = medium confidence (the user is asked to confirm)
///   - >= 0.8 = high confidence (auto-confirmed)
///   - 1.0 = full confidence
/// * `evidence` - list of evidence found (files, configurations)
/// * `metadata` - additional information (versions, frameworks, etc.)
///
/// # Examples
///
/// ```
/// use venore_core::analysis::project_analyzer::traits::{ProjectType, ProjectTypeDetection};
/// use std::collections::HashMap;
///
/// let detection = ProjectTypeDetection {
///     project_type: ProjectType::NodeMonorepo,
///     confidence: 0.9,
///     evidence: vec!["Found package.json".to_string()],
///     metadata: HashMap::from([
///         ("framework".to_string(), "React".to_string()),
///     ]),
///     frameworks: vec![],
/// };
///
/// assert!(detection.confidence >= 0.8); // Auto-confirms
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTypeDetection {
    /// Detected project type
    pub project_type: ProjectType,

    /// Confidence level (0.0 - 1.0)
    pub confidence: f32,

    /// Evidence found during detection
    pub evidence: Vec<String>,

    /// Type-specific additional metadata as untyped key/value pairs.
    ///
    /// Kept for backwards compatibility with the UI / DTOs that read
    /// `metadata.get("frameworks")` directly. New consumers should
    /// prefer the typed `frameworks` field below.
    pub metadata: HashMap<String, String>,

    /// Typed list of frameworks detected on top of the host language.
    ///
    /// Examples: `[NextJs, React]` for a Next.js app, `[Django]` for
    /// a Django project, `[Tauri]` for a Tauri Rust crate. Empty when
    /// no framework is detected or the host language has no concept of
    /// framework. The same set is also rendered into the comma-joined
    /// `metadata["frameworks"]` string for legacy consumers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frameworks: Vec<DetectedFramework>,
}

/// Typed enumeration of frameworks Venore can detect on top of a host
/// language's analyzer.
///
/// Adding a variant means: a string match in the relevant analyzer
/// (`analyzers/common/node.rs::detect_frameworks`, `python_poetry.rs`,
/// future `rust::detect_frameworks` for Tauri, etc.) pushes it onto
/// `ProjectTypeDetection.frameworks`. Display name and serialization
/// key live with the enum so consumers (UI, module detector) don't
/// have to keep their own mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DetectedFramework {
    // Node ecosystem
    React,
    NextJs,
    VueJs,
    Svelte,
    Express,
    NestJs,
    // Python ecosystem
    Django,
    Flask,
    FastApi,
    Streamlit,
    // PHP ecosystem
    Laravel,
    Symfony,
    // Ruby ecosystem
    Rails,
    Sinatra,
    // Kotlin ecosystem
    Ktor,
    SpringBoot,
    AndroidApp,
    // C++ ecosystem
    Qt,
    // Rust ecosystem
    Tauri,
    // .NET ecosystem
    Unity,
    AspNet,
}

impl DetectedFramework {
    /// Human-readable label for UI rendering.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::React => "React",
            Self::NextJs => "Next.js",
            Self::VueJs => "Vue.js",
            Self::Svelte => "Svelte",
            Self::Express => "Express",
            Self::NestJs => "NestJS",
            Self::Django => "Django",
            Self::Flask => "Flask",
            Self::FastApi => "FastAPI",
            Self::Streamlit => "Streamlit",
            Self::Laravel => "Laravel",
            Self::Symfony => "Symfony",
            Self::Rails => "Ruby on Rails",
            Self::Sinatra => "Sinatra",
            Self::Ktor => "Ktor",
            Self::SpringBoot => "Spring Boot",
            Self::AndroidApp => "Android",
            Self::Qt => "Qt",
            Self::Tauri => "Tauri",
            Self::Unity => "Unity",
            Self::AspNet => "ASP.NET",
        }
    }

    /// Map a package / dependency name to its framework variant.
    ///
    /// Returns `None` for unknown names so callers can keep iterating
    /// over package lists without special-casing each ecosystem.
    pub fn from_dep_name(name: &str) -> Option<Self> {
        match name {
            "react" => Some(Self::React),
            "next" => Some(Self::NextJs),
            "vue" => Some(Self::VueJs),
            "svelte" => Some(Self::Svelte),
            "express" => Some(Self::Express),
            "@nestjs/core" => Some(Self::NestJs),
            "django" => Some(Self::Django),
            "flask" => Some(Self::Flask),
            "fastapi" => Some(Self::FastApi),
            "streamlit" => Some(Self::Streamlit),
            "laravel/framework" => Some(Self::Laravel),
            "symfony/symfony" | "symfony/framework-bundle" => Some(Self::Symfony),
            "rails" => Some(Self::Rails),
            "sinatra" => Some(Self::Sinatra),
            "tauri" => Some(Self::Tauri),
            _ => None,
        }
    }

    /// Render a list of frameworks as the comma-joined string the legacy
    /// `metadata["frameworks"]` consumers expect.
    pub fn join_display_names(frameworks: &[Self]) -> String {
        frameworks
            .iter()
            .map(|f| f.display_name())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Module-detection strategy for a project type.
///
/// Defines how files are grouped into logical modules based on the project type.
///
/// # Concepts
///
/// ## Module markers
/// Files that define the root of a module. When one of these files is found,
/// EVERY file in that directory (and subdirectories) belongs to the same module.
///
/// Example: in a Node monorepo, `package.json` is a module marker.
/// - `packages/math/package.json` → module "math" with EVERY file under `packages/math/`
///
/// ## Entry points
/// Common entry files. Used as a fallback when no module markers exist,
/// or to detect the entry point of a module.
///
/// Example: `index.ts`, `main.go`, `lib.rs`
///
/// # Examples
///
/// ```
/// use venore_core::analysis::project_analyzer::traits::ModuleDetectionStrategy;
///
/// // Node Monorepo strategy
/// let strategy = ModuleDetectionStrategy {
///     module_markers: vec!["package.json".to_string()],
///     entry_point_files: vec!["index.ts".to_string(), "index.js".to_string()],
/// };
///
/// // Rust Workspace strategy
/// let strategy = ModuleDetectionStrategy {
///     module_markers: vec!["Cargo.toml".to_string()],
///     entry_point_files: vec!["lib.rs".to_string(), "main.rs".to_string()],
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDetectionStrategy {
    /// Files that mark a module (e.g. ["package.json", "Cargo.toml"])
    pub module_markers: Vec<String>,

    /// Files that mark entry points (e.g. ["index.ts", "lib.rs"])
    pub entry_point_files: Vec<String>,
}

/// Trait implemented by project-type analyzers.
///
/// Every pluggable analyzer must implement this trait. The system runs all
/// registered analyzers in parallel and selects the one with the highest
/// confidence.
///
/// # Implementation requirements
///
/// * Must be `Send + Sync` for concurrent execution
/// * `detect()` MUST be fast (< 100ms) — only inspect config files
/// * Return confidence 0.0 if the analyzer does not apply (NOT an error)
/// * Use `#[async_trait]` for async methods
///
/// # Lifecycle
///
/// 1. **Registration**: the analyzer is registered in [`AnalyzerRegistry`](super::registry::AnalyzerRegistry)
/// 2. **Auto-detection**: `detect()` is called on every analyzer
/// 3. **Selection**: the highest-`confidence` detection wins
/// 4. **Strategy**: `module_detection_strategy()` configures the module detector
/// 5. **Metadata**: `extract_metadata()` fetches additional info for the user
///
/// # Examples
///
/// ```
/// use venore_core::analysis::project_analyzer::traits::*;
/// use venore_core::Result;
/// use async_trait::async_trait;
/// use std::collections::HashMap;
/// use std::path::Path;
///
/// pub struct MyCustomAnalyzer;
///
/// #[async_trait]
/// impl ProjectAnalyzer for MyCustomAnalyzer {
///     fn name(&self) -> &str {
///         "my-custom"
///     }
///
///     fn project_type(&self) -> ProjectType {
///         ProjectType::Unknown
///     }
///
///     async fn detect(&self, project_root: &Path) -> Result<ProjectTypeDetection> {
///         // Inspect config files
///         if !project_root.join("my-config.yml").exists() {
///             return Ok(ProjectTypeDetection {
///                 project_type: self.project_type(),
///                 confidence: 0.0,  // Not an error — just confidence 0
///                 evidence: vec![],
///                 metadata: HashMap::new(),
///                 frameworks: vec![],
///             });
///         }
///
///         Ok(ProjectTypeDetection {
///             project_type: self.project_type(),
///             confidence: 0.9,
///             evidence: vec!["Found my-config.yml".to_string()],
///             metadata: HashMap::new(),
///             frameworks: vec![],
///         })
///     }
///
///     fn module_detection_strategy(&self) -> ModuleDetectionStrategy {
///         ModuleDetectionStrategy {
///             module_markers: vec!["my-config.yml".to_string()],
///             entry_point_files: vec!["main.ext".to_string()],
///         }
///     }
///
///     async fn extract_metadata(&self, _project_root: &Path) -> Result<HashMap<String, String>> {
///         Ok(HashMap::new())
///     }
/// }
/// ```
///
/// # See also
///
/// * [`AnalyzerRegistry`](super::registry::AnalyzerRegistry) - global analyzer registry
/// * [`factory::detect_project_type`](super::factory::detect_project_type) - auto-detection
#[async_trait]
pub trait ProjectAnalyzer: Send + Sync {
    /// Identifier name of the analyzer.
    ///
    /// Must be unique, in kebab-case. Used for debugging and logs.
    ///
    /// # Examples
    ///
    /// * "node-monorepo"
    /// * "rust-workspace"
    /// * "python-poetry"
    fn name(&self) -> &str;

    /// Project type this analyzer detects.
    ///
    /// Must match a variant of [`ProjectType`].
    fn project_type(&self) -> ProjectType;

    /// Detect whether this analyzer applies to the given project.
    ///
    /// # Performance requirements
    ///
    /// **MUST** be fast (< 100ms). Only inspect configuration files; do not
    /// scan the whole project.
    ///
    /// # Error handling
    ///
    /// If the analyzer does not apply, return confidence 0.0 — NOT an error:
    ///
    /// ```ignore
    /// if !applies_to_project {
    ///     return Ok(ProjectTypeDetection {
    ///         confidence: 0.0,
    ///         // ...
    ///     });
    /// }
    /// ```
    ///
    /// # Confidence scoring
    ///
    /// * 0.0 = does not apply
    /// * 0.3 = weak evidence (1 config file)
    /// * 0.7 = medium evidence (config + structure)
    /// * 1.0 = strong evidence (everything detected)
    ///
    /// # Parameters
    ///
    /// * `project_root` - root directory of the project to analyze
    ///
    /// # Returns
    ///
    /// [`ProjectTypeDetection`] with type, confidence, evidence, and metadata
    async fn detect(&self, project_root: &Path) -> Result<ProjectTypeDetection>;

    /// Module-detection strategy for this project type.
    ///
    /// The strategy defines:
    /// * **Module markers**: files that mark a module
    /// * **Entry points**: common entry files
    ///
    /// See [`ModuleDetectionStrategy`] for details.
    fn module_detection_strategy(&self) -> ModuleDetectionStrategy;

    /// Extract additional project metadata.
    ///
    /// Called after `detect()` to fetch additional information shown to
    /// the user (frameworks, versions, etc.).
    ///
    /// # Metadata examples
    ///
    /// * `"framework"` → `"React"`, `"Express"`, `"Gin"`
    /// * `"package_manager"` → `"npm"`, `"yarn"`, `"pnpm"`
    /// * `"go_version"` → `"1.21"`
    /// * `"rust_edition"` → `"2021"`
    ///
    /// # Parameters
    ///
    /// * `project_root` - root directory of the project
    ///
    /// # Returns
    ///
    /// HashMap with metadata key-value pairs
    async fn extract_metadata(&self, project_root: &Path) -> Result<HashMap<String, String>>;
}
