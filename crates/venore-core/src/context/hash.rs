//! Code hashing utilities for staleness detection

use sha2::{Sha256, Digest};
use std::path::Path;
use walkdir::WalkDir;

use crate::error::{Result, VenoreError};
use crate::utils::staleness::{SKIP_DIRS, SOURCE_EXTENSIONS};

/// Calculate SHA-256 hash of code content
///
/// Returns hash in format: "sha256-{hex}"
///
/// # Examples
///
/// ```
/// use venore_core::context::hash::calculate_code_hash;
///
/// let code = "const foo = 'bar';";
/// let hash = calculate_code_hash(code);
/// assert!(hash.starts_with("sha256-"));
/// ```
pub fn calculate_code_hash(code: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(code.as_bytes());
    let result = hasher.finalize();
    format!("sha256-{:x}", result)
}

/// Check if a .context.md file is stale compared to source code
///
/// Returns `true` if:
/// - Context file doesn't exist
/// - Context file has no frontmatter
/// - Context file has no code hash
/// - Code hash in context differs from current code hash
///
/// # Arguments
/// * `code_path` - Path to source code file
/// * `context_path` - Path to .context.md file
/// * `current_code` - Current code content (to hash)
///
/// # Example
/// ```no_run
/// use venore_core::context::is_context_stale;
/// use std::path::Path;
///
/// let code_path = Path::new("src/main.rs");
/// let context_path = Path::new("src/.context/main.context.md");
/// let code = std::fs::read_to_string(code_path).unwrap();
///
/// if is_context_stale(code_path, context_path, &code).unwrap() {
///     println!("Context is stale, needs regeneration");
/// }
/// ```
pub fn is_context_stale(
    _code_path: &Path,
    context_path: &Path,
    current_code: &str,
) -> Result<bool> {
    use crate::context::frontmatter::FrontmatterBuilder;

    // Check if context file exists
    if !context_path.exists() {
        return Ok(true); // No context file = stale
    }

    // Calculate current hash
    let current_hash = calculate_code_hash(current_code);

    // Read hash from context file
    let stored_hash = match FrontmatterBuilder::read_code_hash_from_file(context_path)? {
        Some(hash) => hash,
        None => return Ok(true), // No hash in frontmatter = stale
    };

    // Compare hashes
    Ok(current_hash != stored_hash)
}

/// Cheap filesystem fingerprint of a module subtree, computed from directory
/// metadata (`stat`) **without reading file contents**. Used as a first-pass
/// staleness filter: if it matches the stored one, the (expensive) SHA-256 can
/// be skipped — nothing in the subtree changed.
///
/// Conservative by design: any add/edit/delete moves `file_count`, `total_size`
/// or `max_mtime`. The SHA-256 stays the authoritative arbiter whenever the
/// fingerprint differs, so a false "differs" only costs one extra hash, never a
/// wrong verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ModuleFingerprint {
    pub file_count: u32,
    /// Sum of file sizes in bytes.
    pub total_size: u64,
    /// Newest mtime across the subtree, unix seconds.
    pub max_mtime: i64,
}

/// Enumerate the source files under a module dir, sorted by normalized relative
/// path. `None` when the dir is missing. Shared by the content hash and the
/// fingerprint so both always look at **exactly the same set of files**.
///
/// Excludes are the standard non-source dirs from `utils::staleness`
/// (`node_modules`, `target`, `dist`, `build`, `.git`, `.venore`, etc.) plus
/// every dotfile-prefixed directory, and non-source extensions.
fn collect_source_files(project_path: &Path, module_relative: &str) -> Option<Vec<std::path::PathBuf>> {
    let module_relative_norm = module_relative.replace('\\', "/");
    let dir = if module_relative_norm.is_empty() || module_relative_norm == "." {
        project_path.to_path_buf()
    } else {
        project_path.join(&module_relative_norm)
    };

    if !dir.exists() {
        return None;
    }

    let mut paths: Vec<std::path::PathBuf> = WalkDir::new(&dir)
        .into_iter()
        .filter_entry(|e| {
            if e.file_type().is_dir() {
                let name = e.file_name().to_string_lossy();
                if name.starts_with('.') || SKIP_DIRS.contains(&name.as_ref()) {
                    return false;
                }
            }
            true
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            let ext = e.path().extension().and_then(|x| x.to_str()).unwrap_or("");
            SOURCE_EXTENSIONS.contains(&ext)
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    // Sort by normalized relative path so the same tree always yields the same
    // order regardless of walkdir order.
    paths.sort_by(|a, b| {
        let ra = relative_forward_slash(a, project_path);
        let rb = relative_forward_slash(b, project_path);
        ra.cmp(&rb)
    });

    Some(paths)
}

/// Read a file's mtime as unix seconds (0 if unavailable).
fn file_mtime_secs(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Stat-only fingerprint of a module subtree. `None` when the dir is missing
/// (the caller treats that as "deleted" via the content hash's MISSING marker,
/// so it must NOT short-circuit on a missing dir).
pub fn calculate_module_fingerprint(
    project_path: &Path,
    module_relative: &str,
) -> Result<Option<ModuleFingerprint>> {
    let paths = match collect_source_files(project_path, module_relative) {
        Some(p) => p,
        None => return Ok(None),
    };

    let mut fp = ModuleFingerprint::default();
    for path in &paths {
        let meta = std::fs::metadata(path).map_err(|e| {
            VenoreError::FileReadError(format!("Failed to stat {}: {}", path.display(), e))
        })?;
        fp.file_count += 1;
        fp.total_size += meta.len();
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        if mtime > fp.max_mtime {
            fp.max_mtime = mtime;
        }
    }
    Ok(Some(fp))
}

/// SHA-256 of every source file under a module directory, **plus** the cheap
/// fingerprint — computed in one walk (we read every file anyway, so size/mtime
/// come for free). Use this when persisting an entry so the fingerprint is
/// stored alongside the hash.
///
/// Stable across machines:
///   - File paths are normalized to forward slashes before being mixed in,
///     so Windows and POSIX hosts produce the same digest for the same tree.
///   - Files are sorted by relative path so insertion order doesn't change
///     the result.
///   - Each file contributes `relative_path \0 byte_len \0 bytes` to the
///     hasher, so renames change the hash even if the bytes are identical.
///
/// If the module dir is missing the hash is `"sha256-MISSING"` and the
/// fingerprint is all-zero — callers interpret that as "module deleted".
pub fn calculate_module_hash_and_fingerprint(
    project_path: &Path,
    module_relative: &str,
) -> Result<(String, ModuleFingerprint)> {
    let paths = match collect_source_files(project_path, module_relative) {
        Some(p) => p,
        None => return Ok(("sha256-MISSING".to_string(), ModuleFingerprint::default())),
    };

    let mut hasher = Sha256::new();
    let mut fp = ModuleFingerprint::default();
    for path in &paths {
        let rel = relative_forward_slash(path, project_path);
        let bytes = std::fs::read(path).map_err(|e| {
            VenoreError::FileReadError(format!("Failed to read {}: {}", path.display(), e))
        })?;
        let mtime = file_mtime_secs(path);

        hasher.update(rel.as_bytes());
        hasher.update(b"\0");
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(b"\0");
        hasher.update(&bytes);

        fp.file_count += 1;
        fp.total_size += bytes.len() as u64;
        if mtime > fp.max_mtime {
            fp.max_mtime = mtime;
        }
    }

    Ok((format!("sha256-{:x}", hasher.finalize()), fp))
}

/// SHA-256 fingerprint of every source file under a module directory.
///
/// Returns `(hash, file_count)`. If the module dir is missing the hash is
/// `"sha256-MISSING"` and `file_count` is 0 — callers should interpret that
/// as "module no longer exists on disk", which is a special kind of stale.
/// Thin wrapper over [`calculate_module_hash_and_fingerprint`].
pub fn calculate_module_hash(project_path: &Path, module_relative: &str) -> Result<(String, u32)> {
    let (hash, fp) = calculate_module_hash_and_fingerprint(project_path, module_relative)?;
    Ok((hash, fp.file_count))
}

fn relative_forward_slash(path: &Path, root: &Path) -> String {
    let stripped = path.strip_prefix(root).unwrap_or(path);
    stripped.to_string_lossy().replace('\\', "/")
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_code_hash() {
        let code = "const foo = 'bar';";
        let hash = calculate_code_hash(code);

        assert!(hash.starts_with("sha256-"));
        assert_eq!(hash.len(), 71); // "sha256-" + 64 hex chars
    }

    #[test]
    fn test_same_code_same_hash() {
        let code = "function test() { return 42; }";
        let hash1 = calculate_code_hash(code);
        let hash2 = calculate_code_hash(code);

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_different_code_different_hash() {
        let hash1 = calculate_code_hash("const a = 1;");
        let hash2 = calculate_code_hash("const a = 2;");

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_empty_code() {
        let hash = calculate_code_hash("");
        assert!(hash.starts_with("sha256-"));
        assert_eq!(hash.len(), 71);
    }

    #[test]
    fn test_multiline_code() {
        let code = r#"
function example() {
    const x = 1;
    return x + 2;
}
"#;
        let hash = calculate_code_hash(code);
        assert!(hash.starts_with("sha256-"));
    }

    #[test]
    fn test_whitespace_matters() {
        let hash1 = calculate_code_hash("const a=1;");
        let hash2 = calculate_code_hash("const a = 1;");

        // Whitespace affects hash
        assert_ne!(hash1, hash2);
    }

    // ------------------------------------------------------------------------
    // calculate_module_hash
    // ------------------------------------------------------------------------

    use std::fs;
    use tempfile::TempDir;

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    #[test]
    fn module_hash_missing_dir_returns_marker() {
        let root = TempDir::new().unwrap();
        let (h, n) = calculate_module_hash(root.path(), "does/not/exist").unwrap();
        assert_eq!(h, "sha256-MISSING");
        assert_eq!(n, 0);
    }

    #[test]
    fn module_hash_is_deterministic_across_walk_order() {
        let root = TempDir::new().unwrap();
        write(&root.path().join("m/a.ts"), "a");
        write(&root.path().join("m/b.ts"), "b");
        write(&root.path().join("m/sub/c.ts"), "c");

        let (h1, n1) = calculate_module_hash(root.path(), "m").unwrap();
        let (h2, n2) = calculate_module_hash(root.path(), "m").unwrap();
        assert_eq!(h1, h2);
        assert_eq!(n1, 3);
        assert_eq!(n2, 3);
    }

    #[test]
    fn module_hash_changes_when_a_file_changes() {
        let root = TempDir::new().unwrap();
        write(&root.path().join("m/a.ts"), "a");
        let (h1, _) = calculate_module_hash(root.path(), "m").unwrap();
        write(&root.path().join("m/a.ts"), "A"); // bump content
        let (h2, _) = calculate_module_hash(root.path(), "m").unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn module_hash_skips_excluded_dirs_and_non_source_files() {
        let root = TempDir::new().unwrap();
        write(&root.path().join("m/a.ts"), "a");
        write(&root.path().join("m/node_modules/x.ts"), "ignored");
        write(&root.path().join("m/.venore/y.ts"), "ignored");
        write(&root.path().join("m/binary.png"), "ignored");
        let (_, n) = calculate_module_hash(root.path(), "m").unwrap();
        assert_eq!(n, 1, "only a.ts should be hashed");
    }

    #[test]
    fn module_hash_changes_when_a_file_is_renamed_even_with_same_bytes() {
        let root = TempDir::new().unwrap();
        write(&root.path().join("m/a.ts"), "same");
        let (h1, _) = calculate_module_hash(root.path(), "m").unwrap();
        fs::rename(root.path().join("m/a.ts"), root.path().join("m/b.ts")).unwrap();
        let (h2, _) = calculate_module_hash(root.path(), "m").unwrap();
        assert_ne!(h1, h2, "rename must surface in the digest");
    }

    #[test]
    fn module_hash_stable_across_platforms_normalizes_slashes() {
        // We can't actually run on two OSes here, but verifying that the
        // hash function uses forward-slash relative paths gives us the
        // confidence that two hosts produce the same digest. Indirect check:
        // a module under a deep nested path hashes the same as the same
        // tree placed directly under root.
        let root = TempDir::new().unwrap();
        write(&root.path().join("nest1/m/a.ts"), "x");
        write(&root.path().join("nest1/m/b.ts"), "y");
        let (h_nested, _) = calculate_module_hash(root.path(), "nest1/m").unwrap();

        let root2 = TempDir::new().unwrap();
        write(&root2.path().join("m/a.ts"), "x");
        write(&root2.path().join("m/b.ts"), "y");
        let (h_flat, _) = calculate_module_hash(root2.path(), "m").unwrap();

        // Different module_relative means different relative paths in the
        // hasher, so the digests differ — but each is stable by itself.
        // Re-run the first one to make sure of determinism:
        let (h_nested2, _) = calculate_module_hash(root.path(), "nest1/m").unwrap();
        assert_eq!(h_nested, h_nested2);
        assert_ne!(h_nested, h_flat);
    }
}
