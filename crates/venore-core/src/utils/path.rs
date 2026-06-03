//! Path utilities for cross-platform path handling

use std::path::{Path, PathBuf};

/// Normalize path separators to forward slash (cross-platform)
///
/// # Examples
/// ```
/// use venore_core::utils::normalize_path;
///
/// assert_eq!(normalize_path("C:\\Users\\John"), "C:/Users/John");
/// assert_eq!(normalize_path("already/normalized"), "already/normalized");
/// ```
pub fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

/// Get file name from path (with or without extension)
///
/// # Examples
/// ```
/// use std::path::Path;
/// use venore_core::utils::get_file_name;
///
/// let path = Path::new("/project/src/main.rs");
/// assert_eq!(get_file_name(path, true), Some("main.rs".to_string()));
/// assert_eq!(get_file_name(path, false), Some("main".to_string()));
/// ```
pub fn get_file_name(path: &Path, include_extension: bool) -> Option<String> {
    if include_extension {
        path.file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
    } else {
        path.file_stem()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
    }
}

/// Get file extension
///
/// # Examples
/// ```
/// use std::path::Path;
/// use venore_core::utils::get_file_extension;
///
/// let path = Path::new("/project/src/main.rs");
/// assert_eq!(get_file_extension(path), Some("rs".to_string()));
/// ```
pub fn get_file_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_string())
}

/// Get project name from path (last component)
///
/// # Examples
/// ```
/// use std::path::Path;
/// use venore_core::utils::get_project_name_from_path;
///
/// let path = Path::new("/home/user/my-project");
/// assert_eq!(get_project_name_from_path(path), Some("my-project".to_string()));
/// ```
pub fn get_project_name_from_path(path: &Path) -> Option<String> {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
}

/// Get parent folder name
///
/// # Examples
/// ```
/// use std::path::Path;
/// use venore_core::utils::get_parent_folder_name;
///
/// let path = Path::new("/home/user/my-project");
/// assert_eq!(get_parent_folder_name(path), Some("user".to_string()));
/// ```
pub fn get_parent_folder_name(path: &Path) -> Option<String> {
    path.parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
}

/// Check if path is absolute
///
/// # Examples
/// ```
/// use std::path::Path;
/// use venore_core::utils::is_absolute_path;
///
/// // On Unix systems
/// #[cfg(unix)]
/// assert!(is_absolute_path(Path::new("/home/user")));
///
/// // On Windows systems
/// #[cfg(windows)]
/// assert!(is_absolute_path(Path::new("C:\\Users\\John")));
///
/// // Relative path on any system
/// assert!(!is_absolute_path(Path::new("relative/path")));
/// ```
pub fn is_absolute_path(path: &Path) -> bool {
    path.is_absolute()
}

/// Join path segments (cross-platform)
///
/// # Examples
/// ```
/// use std::path::Path;
/// use venore_core::utils::join_path;
///
/// let base = Path::new("/home/user");
/// let joined = join_path(base, "project");
/// assert!(joined.to_string_lossy().contains("project"));
/// ```
pub fn join_path(base: &Path, segment: &str) -> PathBuf {
    base.join(segment)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path("C:\\Users\\John"), "C:/Users/John");
        assert_eq!(normalize_path("already/normalized"), "already/normalized");
        assert_eq!(normalize_path("mixed\\and/slashes"), "mixed/and/slashes");
    }

    #[test]
    fn test_get_file_name() {
        let path = Path::new("/project/src/main.rs");
        assert_eq!(get_file_name(path, true), Some("main.rs".to_string()));
        assert_eq!(get_file_name(path, false), Some("main".to_string()));

        let no_ext = Path::new("/project/README");
        assert_eq!(get_file_name(no_ext, true), Some("README".to_string()));
        assert_eq!(get_file_name(no_ext, false), Some("README".to_string()));
    }

    #[test]
    fn test_get_file_extension() {
        let path = Path::new("/project/src/main.rs");
        assert_eq!(get_file_extension(path), Some("rs".to_string()));

        let no_ext = Path::new("/project/README");
        assert_eq!(get_file_extension(no_ext), None);

        let double_ext = Path::new("/project/archive.tar.gz");
        assert_eq!(get_file_extension(double_ext), Some("gz".to_string()));
    }

    #[test]
    fn test_get_project_name() {
        let path = Path::new("/home/user/my-project");
        assert_eq!(get_project_name_from_path(path), Some("my-project".to_string()));

        // Backslash separators are only recognized on Windows; on Unix the
        // whole string is a single component, so this case is Windows-only.
        #[cfg(windows)]
        {
            let windows_path = Path::new("C:\\Users\\John\\my-project");
            assert_eq!(get_project_name_from_path(windows_path), Some("my-project".to_string()));
        }
    }

    #[test]
    fn test_get_parent_folder_name() {
        let path = Path::new("/home/user/my-project");
        assert_eq!(get_parent_folder_name(path), Some("user".to_string()));

        let root = Path::new("/");
        assert_eq!(get_parent_folder_name(root), None);
    }

    #[test]
    fn test_is_absolute_path() {
        #[cfg(unix)]
        {
            assert!(is_absolute_path(Path::new("/home/user")));
        }

        #[cfg(windows)]
        {
            assert!(is_absolute_path(Path::new("C:\\Users\\John")));
        }

        assert!(!is_absolute_path(Path::new("relative/path")));
    }

    #[test]
    fn test_join_path() {
        let base = Path::new("/home/user");
        let joined = join_path(base, "project");
        assert_eq!(joined, PathBuf::from("/home/user/project"));

        let joined_nested = join_path(&joined, "src");
        assert_eq!(joined_nested, PathBuf::from("/home/user/project/src"));
    }
}
