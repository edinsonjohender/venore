//! Shared staleness detection utilities
//!
//! Constants and helpers for comparing source file mtimes against `.context.md`.
//! Used by both `dashboard` and `layers` modules.

use std::path::Path;
use std::time::SystemTime;

use walkdir::WalkDir;

/// Source file extensions to consider for staleness checks.
pub const SOURCE_EXTENSIONS: &[&str] = &[
    "ts", "tsx", "js", "jsx", "mjs", "cjs",
    "rs", "py", "go", "java", "kt", "swift",
    "c", "cpp", "h", "hpp", "cs",
    "rb", "php", "vue", "svelte",
];

/// Directories to skip when scanning for source files.
pub const SKIP_DIRS: &[&str] = &[
    "node_modules", "target", "dist", "build", ".git",
    ".next", ".nuxt", "__pycache__", ".venore",
];

/// Walk the module directory and find the most recent mtime of any source file.
pub fn find_max_source_mtime(dir: &Path) -> Option<SystemTime> {
    if !dir.exists() {
        return None;
    }

    let mut max_mtime: Option<SystemTime> = None;

    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| {
            // Skip hidden dirs and known non-source dirs
            if e.file_type().is_dir() {
                let name = e.file_name().to_string_lossy();
                if name.starts_with('.') || SKIP_DIRS.contains(&name.as_ref()) {
                    return false;
                }
            }
            true
        })
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }

        // Check extension
        let path = entry.path();
        let ext = match path.extension().and_then(|e| e.to_str()) {
            Some(e) => e,
            None => continue,
        };

        if !SOURCE_EXTENSIONS.contains(&ext) {
            continue;
        }

        // Skip .context.md itself (it's not a source file)
        if path.file_name().and_then(|n| n.to_str()) == Some(".context.md") {
            continue;
        }

        if let Some(mtime) = entry.metadata().ok().and_then(|m| m.modified().ok()) {
            max_mtime = Some(match max_mtime {
                Some(current) if mtime > current => mtime,
                Some(current) => current,
                None => mtime,
            });
        }
    }

    max_mtime
}
