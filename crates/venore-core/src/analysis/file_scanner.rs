//! File scanner for project analysis
//!
//! Recursively scans a directory and collects file information with filtering

use crate::error::{VenoreError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use walkdir::WalkDir;

/// Configuration for directory scanning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanConfig {
    /// Root path of the project to scan
    pub project_path: PathBuf,

    /// File extensions to include (e.g., [".ts", ".tsx", ".rs"])
    pub target_extensions: Vec<String>,

    /// Patterns to ignore (e.g., ["node_modules", "dist", ".git"])
    pub ignore_patterns: Vec<String>,

    /// Maximum file size in kilobytes (files larger than this will be skipped)
    pub max_file_size_kb: u64,
}

/// Information about a scanned file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    /// Absolute path to the file
    pub path: PathBuf,

    /// File name without path
    pub name: String,

    /// File extension (e.g., "ts", "rs")
    pub extension: String,

    /// File size in bytes
    pub size_bytes: u64,

    /// Last modification time
    pub last_modified: SystemTime,
}

/// Result of a directory scan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    /// List of scanned files
    pub files: Vec<FileInfo>,

    /// Total size of all files in bytes
    pub total_size_bytes: u64,

    /// Time taken to scan in milliseconds
    pub scan_duration_ms: u64,
}

/// Scan a directory recursively and return filtered file information
///
/// # Arguments
///
/// * `config` - Configuration specifying what to scan and filter
///
/// # Returns
///
/// Returns `ScanResult` containing all matching files, or an error if:
/// - The project path doesn't exist
/// - The project path is not a directory
/// - IO errors occur during scanning
///
/// # Examples
///
/// ```no_run
/// use std::path::PathBuf;
/// use venore_core::analysis::*;
///
/// let config = ScanConfig {
///     project_path: PathBuf::from("./my-project"),
///     target_extensions: vec![".ts".to_string(), ".tsx".to_string()],
///     ignore_patterns: vec!["node_modules".to_string(), "dist".to_string()],
///     max_file_size_kb: 500,
/// };
///
/// let result = scan_directory(config).unwrap();
/// println!("Found {} files", result.files.len());
/// ```
pub fn scan_directory(config: ScanConfig) -> Result<ScanResult> {
    let start = std::time::Instant::now();

    // Validate that project_path exists
    if !config.project_path.exists() {
        return Err(VenoreError::DirectoryNotFound(
            config.project_path.to_string_lossy().to_string()
        ));
    }

    // Validate that it's a directory
    if !config.project_path.is_dir() {
        return Err(VenoreError::InvalidPath(
            format!("Path is not a directory: {}", config.project_path.display())
        ));
    }

    let mut files = Vec::new();
    let mut total_size_bytes = 0u64;

    // Walk directory recursively
    for entry in WalkDir::new(&config.project_path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !should_ignore(e.path(), &config.ignore_patterns))
    {
        let entry = entry.map_err(|e| VenoreError::Io(e.to_string()))?;

        // Skip directories
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();

        // Filter by extension
        if !has_target_extension(path, &config.target_extensions) {
            continue;
        }

        // Get file metadata
        let metadata = entry.metadata().map_err(|e| VenoreError::Io(e.to_string()))?;
        let size_bytes = metadata.len();

        // Filter by file size
        let size_kb = size_bytes / 1024;
        if size_kb > config.max_file_size_kb {
            continue;
        }

        // Get file name
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| VenoreError::InvalidPath(path.to_string_lossy().to_string()))?
            .to_string();

        // Get extension (without the dot)
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();

        // Get last modified time
        let last_modified = metadata
            .modified()
            .map_err(|e| VenoreError::Io(e.to_string()))?;

        // Add to results
        files.push(FileInfo {
            path: path.to_path_buf(),
            name,
            extension,
            size_bytes,
            last_modified,
        });

        total_size_bytes += size_bytes;
    }

    let scan_duration_ms = start.elapsed().as_millis() as u64;

    Ok(ScanResult {
        files,
        total_size_bytes,
        scan_duration_ms,
    })
}

/// Check if a path should be ignored based on ignore patterns
fn should_ignore(path: &Path, ignore_patterns: &[String]) -> bool {
    let path_str = path.to_string_lossy();

    for pattern in ignore_patterns {
        if path_str.contains(pattern.as_str()) {
            return true;
        }
    }

    false
}

/// Check if a file has one of the target extensions
fn has_target_extension(path: &Path, target_extensions: &[String]) -> bool {
    // If no extensions specified, include all files
    if target_extensions.is_empty() {
        return true;
    }

    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    for target_ext in target_extensions {
        // Handle both ".ts" and "ts" formats
        let target = target_ext.trim_start_matches('.');
        if extension == target {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    fn create_test_project(base: &Path) -> std::io::Result<()> {
        // Create directory structure
        fs::create_dir_all(base.join("src"))?;
        fs::create_dir_all(base.join("node_modules/pkg"))?;
        fs::create_dir_all(base.join("dist"))?;

        // Create TypeScript files
        let mut file1 = fs::File::create(base.join("src/index.ts"))?;
        file1.write_all(b"console.log('Hello');")?;

        let mut file2 = fs::File::create(base.join("src/utils.ts"))?;
        file2.write_all(b"export const add = (a: number, b: number) => a + b;")?;

        // Create files that should be ignored
        let mut file3 = fs::File::create(base.join("node_modules/pkg/index.js"))?;
        file3.write_all(b"module.exports = {};")?;

        let mut file4 = fs::File::create(base.join("dist/bundle.js"))?;
        file4.write_all(b"console.log('bundled');")?;

        // Create other file types
        let mut file5 = fs::File::create(base.join("README.md"))?;
        file5.write_all(b"# Test Project")?;

        Ok(())
    }

    #[test]
    fn test_scan_typescript_project() {
        // Create temporary test directory
        let temp_dir = std::env::temp_dir().join("venore-test-scan");
        let _ = fs::remove_dir_all(&temp_dir); // Clean up if exists
        create_test_project(&temp_dir).unwrap();

        let config = ScanConfig {
            project_path: temp_dir.clone(),
            target_extensions: vec![".ts".to_string()],
            ignore_patterns: vec!["node_modules".to_string(), "dist".to_string()],
            max_file_size_kb: 500,
        };

        let result = scan_directory(config).unwrap();

        // Should find 2 .ts files (index.ts and utils.ts)
        assert_eq!(result.files.len(), 2);

        // Check that we found the right files
        let file_names: Vec<String> = result.files.iter().map(|f| f.name.clone()).collect();
        assert!(file_names.contains(&"index.ts".to_string()));
        assert!(file_names.contains(&"utils.ts".to_string()));

        // Check extensions
        assert!(result.files.iter().all(|f| f.extension == "ts"));

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_ignores_node_modules() {
        let temp_dir = std::env::temp_dir().join("venore-test-ignore");
        let _ = fs::remove_dir_all(&temp_dir);
        create_test_project(&temp_dir).unwrap();

        let config = ScanConfig {
            project_path: temp_dir.clone(),
            target_extensions: vec![".js".to_string(), ".ts".to_string()],
            ignore_patterns: vec!["node_modules".to_string()],
            max_file_size_kb: 500,
        };

        let result = scan_directory(config).unwrap();

        // Should not contain any files from node_modules
        assert!(!result.files.iter().any(|f| {
            f.path.to_string_lossy().contains("node_modules")
        }));

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_ignores_dist() {
        let temp_dir = std::env::temp_dir().join("venore-test-dist");
        let _ = fs::remove_dir_all(&temp_dir);
        create_test_project(&temp_dir).unwrap();

        let config = ScanConfig {
            project_path: temp_dir.clone(),
            target_extensions: vec![".js".to_string()],
            ignore_patterns: vec!["dist".to_string()],
            max_file_size_kb: 500,
        };

        let result = scan_directory(config).unwrap();

        // Should not contain dist/bundle.js
        assert!(!result.files.iter().any(|f| {
            f.path.to_string_lossy().contains("dist")
        }));

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_filters_by_extension() {
        let temp_dir = std::env::temp_dir().join("venore-test-ext");
        let _ = fs::remove_dir_all(&temp_dir);
        create_test_project(&temp_dir).unwrap();

        let config = ScanConfig {
            project_path: temp_dir.clone(),
            target_extensions: vec![".md".to_string()],
            ignore_patterns: vec![],
            max_file_size_kb: 500,
        };

        let result = scan_directory(config).unwrap();

        // Should only find README.md
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].name, "README.md");
        assert_eq!(result.files[0].extension, "md");

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_error_on_nonexistent_path() {
        let config = ScanConfig {
            project_path: PathBuf::from("/this/path/does/not/exist/hopefully"),
            target_extensions: vec![],
            ignore_patterns: vec![],
            max_file_size_kb: 500,
        };

        let result = scan_directory(config);
        assert!(result.is_err());

        match result {
            Err(VenoreError::DirectoryNotFound(_)) => { /* Expected */ },
            _ => panic!("Expected DirectoryNotFound error"),
        }
    }

    #[test]
    fn test_scan_result_has_metrics() {
        let temp_dir = std::env::temp_dir().join("venore-test-metrics");
        let _ = fs::remove_dir_all(&temp_dir);
        create_test_project(&temp_dir).unwrap();

        let config = ScanConfig {
            project_path: temp_dir.clone(),
            target_extensions: vec![".ts".to_string()],
            ignore_patterns: vec!["node_modules".to_string()],
            max_file_size_kb: 500,
        };

        let result = scan_directory(config).unwrap();

        // Should have total size
        assert!(result.total_size_bytes > 0);

        // Should have scan duration (u64 is always >= 0, just check it exists)
        assert!(result.scan_duration_ms < 10000); // Should be < 10 seconds for test

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
