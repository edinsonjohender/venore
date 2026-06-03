//! End-to-End integration tests for the pluggable analyzer system
//!
//! These tests verify that the complete system works as expected:
//! 1. Project type detection
//! 2. Strategy generation
//! 3. Module detection with strategy
//! 4. Correct module grouping (solving the packages/math/ problem)

use std::fs;
use std::path::Path;
use tempfile::TempDir;
use venore_core::analysis::{
    file_scanner::{self, FileInfo},
    module_detector::{self, DetectorConfig},
    project_analyzer::{factory, traits::ProjectType},
};

/// Helper to create a file with content in a temp directory
fn create_file(base: &Path, relative_path: &str, content: &str) {
    let full_path = base.join(relative_path);

    // Create parent directories if needed
    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }

    fs::write(full_path, content).unwrap();
}

/// Helper to create a FileInfo from a path
fn create_file_info(base: &Path, relative_path: &str) -> FileInfo {
    let full_path = base.join(relative_path);
    let extension = full_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_string();
    let name = full_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    FileInfo {
        path: full_path,
        name,
        extension,
        size_bytes: 100,
        last_modified: std::time::SystemTime::now(),
    }
}

#[tokio::test]
async fn test_e2e_node_monorepo_excalidraw_style() {
    // This is THE KEY TEST that verifies the original problem is solved
    //
    // Problem: packages/math/ with package.json but NO index.ts at root
    // Before: Would create 2 modules (math with 1 file, src with 15 files)
    // After: Should create 1 module (math with ALL 16 files)

    let temp = TempDir::new().unwrap();

    // Create root package.json with workspaces
    create_file(
        temp.path(),
        "package.json",
        r#"{
            "name": "excalidraw-mock",
            "private": true,
            "workspaces": ["packages/*"]
        }"#,
    );

    // Create packages/math/ structure
    create_file(
        temp.path(),
        "packages/math/package.json",
        r#"{
            "name": "@excalidraw/math",
            "version": "1.0.0"
        }"#,
    );

    create_file(temp.path(), "packages/math/global.d.ts", "declare global {}");

    // NO index.ts at packages/math/ level!
    // Entry point is inside src/
    create_file(temp.path(), "packages/math/src/index.ts", "export * from './angle';");
    create_file(temp.path(), "packages/math/src/angle.ts", "export class Angle {}");
    create_file(temp.path(), "packages/math/src/curve.ts", "export class Curve {}");
    create_file(temp.path(), "packages/math/src/point.ts", "export class Point {}");
    create_file(temp.path(), "packages/math/src/line.ts", "export class Line {}");

    // Step 1: Detect project type
    let detection = factory::detect_project_type(temp.path()).await.unwrap();

    assert_eq!(detection.project_type, ProjectType::NodeMonorepo);
    assert!(
        detection.confidence >= 0.7,
        "Confidence should be >= 70%, got: {}",
        detection.confidence
    );
    assert!(!detection.evidence.is_empty());

    // Step 2: Get strategy from analyzer
    let analyzer = factory::get_analyzer(ProjectType::NodeMonorepo).unwrap();
    let strategy = analyzer.module_detection_strategy();

    assert_eq!(strategy.module_markers, vec!["package.json"]);

    // Step 3: Create FileInfo list (simulate file scanner)
    let files = vec![
        create_file_info(temp.path(), "packages/math/package.json"),
        create_file_info(temp.path(), "packages/math/global.d.ts"),
        create_file_info(temp.path(), "packages/math/src/index.ts"),
        create_file_info(temp.path(), "packages/math/src/angle.ts"),
        create_file_info(temp.path(), "packages/math/src/curve.ts"),
        create_file_info(temp.path(), "packages/math/src/point.ts"),
        create_file_info(temp.path(), "packages/math/src/line.ts"),
    ];

    // Step 4: Detect modules WITH STRATEGY
    let detector_config = DetectorConfig {
        files: files.clone(),
        parse_results: vec![],
        project_root: temp.path().to_path_buf(),
        detection_strategy: Some(strategy),
    };

    let result = module_detector::detect_modules(detector_config).unwrap();

    // ✅ CRITICAL VERIFICATION: Should detect exactly 1 module, not 2!
    assert_eq!(
        result.modules.len(),
        1,
        "Should detect exactly 1 module (math), got {} modules: {:?}",
        result.modules.len(),
        result.modules.iter().map(|m| &m.name).collect::<Vec<_>>()
    );

    let math_module = &result.modules[0];
    assert_eq!(math_module.name, "math");

    // Should contain ALL 7 files
    assert_eq!(
        math_module.files.len(),
        7,
        "Math module should contain ALL 7 files (package.json + global.d.ts + 5 src files), got: {}",
        math_module.files.len()
    );

    // Verify all expected files are included
    let file_names: Vec<String> = math_module
        .files
        .iter()
        .filter_map(|f| f.file_name())
        .filter_map(|n| n.to_str())
        .map(|s| s.to_string())
        .collect();

    assert!(file_names.contains(&"package.json".to_string()));
    assert!(file_names.contains(&"global.d.ts".to_string()));
    assert!(file_names.contains(&"index.ts".to_string()));
    assert!(file_names.contains(&"angle.ts".to_string()));
    assert!(file_names.contains(&"curve.ts".to_string()));
    assert!(file_names.contains(&"point.ts".to_string()));
    assert!(file_names.contains(&"line.ts".to_string()));
}

#[tokio::test]
async fn test_e2e_rust_workspace() {
    let temp = TempDir::new().unwrap();

    // Create root Cargo.toml with workspace
    create_file(
        temp.path(),
        "Cargo.toml",
        r#"[workspace]
members = ["crates/core", "crates/cli"]
"#,
    );

    // Create crates/core
    create_file(
        temp.path(),
        "crates/core/Cargo.toml",
        r#"[package]
name = "my-core"
version = "0.1.0"
"#,
    );
    create_file(temp.path(), "crates/core/src/lib.rs", "pub fn hello() {}");
    create_file(temp.path(), "crates/core/src/utils.rs", "pub fn util() {}");

    // Create crates/cli
    create_file(
        temp.path(),
        "crates/cli/Cargo.toml",
        r#"[package]
name = "my-cli"
version = "0.1.0"
"#,
    );
    create_file(temp.path(), "crates/cli/src/main.rs", "fn main() {}");

    // Step 1: Detect project type
    let detection = factory::detect_project_type(temp.path()).await.unwrap();

    assert_eq!(detection.project_type, ProjectType::RustWorkspace);
    assert!(detection.confidence >= 0.8);

    // Step 2: Get strategy
    let analyzer = factory::get_analyzer(ProjectType::RustWorkspace).unwrap();
    let strategy = analyzer.module_detection_strategy();

    assert_eq!(strategy.module_markers, vec!["Cargo.toml"]);

    // Step 3: Create FileInfo list
    let files = vec![
        create_file_info(temp.path(), "crates/core/Cargo.toml"),
        create_file_info(temp.path(), "crates/core/src/lib.rs"),
        create_file_info(temp.path(), "crates/core/src/utils.rs"),
        create_file_info(temp.path(), "crates/cli/Cargo.toml"),
        create_file_info(temp.path(), "crates/cli/src/main.rs"),
    ];

    // Step 4: Detect modules WITH STRATEGY
    let detector_config = DetectorConfig {
        files: files.clone(),
        parse_results: vec![],
        project_root: temp.path().to_path_buf(),
        detection_strategy: Some(strategy),
    };

    let result = module_detector::detect_modules(detector_config).unwrap();

    // Should detect 2 modules: core and cli
    assert_eq!(result.modules.len(), 2);

    let module_names: Vec<String> = result.modules.iter().map(|m| m.name.clone()).collect();
    assert!(module_names.contains(&"core".to_string()));
    assert!(module_names.contains(&"cli".to_string()));

    // Core should have 3 files (Cargo.toml + lib.rs + utils.rs)
    let core_module = result
        .modules
        .iter()
        .find(|m| m.name == "core")
        .expect("core module should exist");
    assert_eq!(core_module.files.len(), 3);

    // CLI should have 2 files (Cargo.toml + main.rs)
    let cli_module = result
        .modules
        .iter()
        .find(|m| m.name == "cli")
        .expect("cli module should exist");
    assert_eq!(cli_module.files.len(), 2);
}

#[tokio::test]
async fn test_e2e_backward_compatibility_unknown_project() {
    // This test verifies backward compatibility:
    // When no project type is detected (Unknown), the system should
    // fall back to the original entry-point algorithm

    let temp = TempDir::new().unwrap();

    // Create files WITHOUT any project markers (no package.json, no Cargo.toml)
    // This should result in Unknown project type
    create_file(temp.path(), "src/auth/index.ts", "export class Auth {}");
    create_file(temp.path(), "src/auth/service.ts", "export class Service {}");
    create_file(temp.path(), "src/users/mod.rs", "pub fn users() {}");
    create_file(temp.path(), "src/users/model.rs", "pub struct User {}");

    // Step 1: Detect project type
    let detection = factory::detect_project_type(temp.path()).await.unwrap();

    assert_eq!(detection.project_type, ProjectType::Unknown);
    assert_eq!(detection.confidence, 1.0); // Unknown always has 100% confidence

    // Step 2: Try to get analyzer for Unknown (should fail)
    let analyzer_result = factory::get_analyzer(ProjectType::Unknown);
    assert!(analyzer_result.is_err(), "Unknown should not have an analyzer");

    // Step 3: Create FileInfo list
    let files = vec![
        create_file_info(temp.path(), "src/auth/index.ts"),
        create_file_info(temp.path(), "src/auth/service.ts"),
        create_file_info(temp.path(), "src/users/mod.rs"),
        create_file_info(temp.path(), "src/users/model.rs"),
    ];

    // Step 4: Detect modules WITHOUT STRATEGY (fallback to old algorithm)
    let detector_config = DetectorConfig {
        files: files.clone(),
        parse_results: vec![],
        project_root: temp.path().to_path_buf(),
        detection_strategy: None, // No strategy = use old algorithm
    };

    let result = module_detector::detect_modules(detector_config).unwrap();

    // Should detect 2 modules using entry points (index.ts and mod.rs)
    assert_eq!(result.modules.len(), 2);

    let module_names: Vec<String> = result.modules.iter().map(|m| m.name.clone()).collect();
    assert!(module_names.contains(&"auth".to_string()));
    assert!(module_names.contains(&"users".to_string()));

    // This verifies backward compatibility: same behavior as before
}

#[tokio::test]
async fn test_e2e_multiple_packages_in_monorepo() {
    // Test with multiple packages to ensure each is detected separately
    let temp = TempDir::new().unwrap();

    create_file(
        temp.path(),
        "package.json",
        r#"{ "workspaces": ["packages/*"] }"#,
    );

    // Package 1: math
    create_file(temp.path(), "packages/math/package.json", r#"{"name": "math"}"#);
    create_file(temp.path(), "packages/math/src/index.ts", "export class Math {}");
    create_file(temp.path(), "packages/math/src/utils.ts", "export const PI = 3.14;");

    // Package 2: utils
    create_file(temp.path(), "packages/utils/package.json", r#"{"name": "utils"}"#);
    create_file(temp.path(), "packages/utils/src/string.ts", "export const trim = () => {};");

    // Package 3: core
    create_file(temp.path(), "packages/core/package.json", r#"{"name": "core"}"#);
    create_file(temp.path(), "packages/core/index.ts", "export class Core {}");

    // Detect project type
    let detection = factory::detect_project_type(temp.path()).await.unwrap();
    assert_eq!(detection.project_type, ProjectType::NodeMonorepo);

    // Get strategy
    let analyzer = factory::get_analyzer(ProjectType::NodeMonorepo).unwrap();
    let strategy = analyzer.module_detection_strategy();

    // Create FileInfo list
    let files = vec![
        create_file_info(temp.path(), "packages/math/package.json"),
        create_file_info(temp.path(), "packages/math/src/index.ts"),
        create_file_info(temp.path(), "packages/math/src/utils.ts"),
        create_file_info(temp.path(), "packages/utils/package.json"),
        create_file_info(temp.path(), "packages/utils/src/string.ts"),
        create_file_info(temp.path(), "packages/core/package.json"),
        create_file_info(temp.path(), "packages/core/index.ts"),
    ];

    // Detect modules
    let detector_config = DetectorConfig {
        files: files.clone(),
        parse_results: vec![],
        project_root: temp.path().to_path_buf(),
        detection_strategy: Some(strategy),
    };

    let result = module_detector::detect_modules(detector_config).unwrap();

    // Should detect 3 modules
    assert_eq!(result.modules.len(), 3);

    let module_names: Vec<String> = result.modules.iter().map(|m| m.name.clone()).collect();
    assert!(module_names.contains(&"math".to_string()));
    assert!(module_names.contains(&"utils".to_string()));
    assert!(module_names.contains(&"core".to_string()));

    // Verify file counts
    let math_module = result.modules.iter().find(|m| m.name == "math").unwrap();
    assert_eq!(math_module.files.len(), 3); // package.json + 2 src files

    let utils_module = result.modules.iter().find(|m| m.name == "utils").unwrap();
    assert_eq!(utils_module.files.len(), 2); // package.json + 1 src file

    let core_module = result.modules.iter().find(|m| m.name == "core").unwrap();
    assert_eq!(core_module.files.len(), 2); // package.json + index.ts
}

#[tokio::test]
async fn test_e2e_performance_detection_is_fast() {
    // Verify that project type detection is fast (< 100ms)
    let temp = TempDir::new().unwrap();

    // Create a Node monorepo structure
    create_file(
        temp.path(),
        "package.json",
        r#"{ "workspaces": ["packages/*"] }"#,
    );
    fs::create_dir_all(temp.path().join("packages")).unwrap();
    create_file(temp.path(), "pnpm-lock.yaml", "");

    let start = std::time::Instant::now();
    let _detection = factory::detect_project_type(temp.path()).await.unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() < 100,
        "Detection should be < 100ms, took: {}ms",
        elapsed.as_millis()
    );
}

#[tokio::test]
async fn test_e2e_with_real_file_scanner() {
    // This test uses the actual file_scanner instead of mock FileInfo
    // to verify the complete pipeline works

    let temp = TempDir::new().unwrap();

    // Create a small Node monorepo
    create_file(
        temp.path(),
        "package.json",
        r#"{ "workspaces": ["packages/*"] }"#,
    );
    create_file(temp.path(), "packages/math/package.json", r#"{"name": "math"}"#);
    create_file(temp.path(), "packages/math/index.ts", "export class Math {}");
    create_file(temp.path(), "packages/math/utils.ts", "export const PI = 3.14;");

    // Step 1: Detect project type
    let detection = factory::detect_project_type(temp.path()).await.unwrap();
    assert_eq!(detection.project_type, ProjectType::NodeMonorepo);

    // Step 2: Get strategy
    let analyzer = factory::get_analyzer(ProjectType::NodeMonorepo).unwrap();
    let strategy = analyzer.module_detection_strategy();

    // Step 3: Use REAL file scanner
    let scan_config = file_scanner::ScanConfig {
        project_path: temp.path().to_path_buf(),
        target_extensions: vec!["ts".into(), "json".into()],
        ignore_patterns: vec![],
        max_file_size_kb: 1024,
    };

    let scan_result = file_scanner::scan_directory(scan_config).unwrap();

    // Should have found 4 files
    assert!(scan_result.files.len() >= 4);

    // Step 4: Detect modules
    let detector_config = DetectorConfig {
        files: scan_result.files,
        parse_results: vec![],
        project_root: temp.path().to_path_buf(),
        detection_strategy: Some(strategy),
    };

    let result = module_detector::detect_modules(detector_config).unwrap();

    // Should detect 1 module: math
    assert_eq!(result.modules.len(), 1);
    assert_eq!(result.modules[0].name, "math");

    // Math module should have 3 files (package.json + index.ts + utils.ts)
    assert_eq!(result.modules[0].files.len(), 3);
}
