//! Hash-based cache for staleness detection
//!
//! Tracks code hashes to determine when .context.md files need regeneration.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use anyhow::{Context, Result};

/// Cache entry for a single file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// Path to the source code file
    pub file_path: PathBuf,
    /// SHA-256 hash of the code content
    pub code_hash: String,
    /// Path to the generated .context.md file
    pub context_path: PathBuf,
    /// ISO 8601 timestamp of last check
    pub last_checked: String,
}

/// Hash cache for tracking code changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashCache {
    /// Map of file path (normalized) to cache entry
    entries: HashMap<String, CacheEntry>,
    /// Cache version for migration compatibility
    version: String,
}

impl Default for HashCache {
    fn default() -> Self {
        Self::new()
    }
}

impl HashCache {
    /// Create a new empty cache
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            version: "1.0".to_string(),
        }
    }

    /// Load cache from disk (`.venore/hash-cache.json`)
    ///
    /// Creates a new empty cache if file doesn't exist.
    pub fn load(project_root: &Path) -> Result<Self> {
        let cache_path = Self::cache_path(project_root);

        if !cache_path.exists() {
            return Ok(Self::new());
        }

        let contents = fs::read_to_string(&cache_path)
            .context("Failed to read cache file")?;

        let cache: HashCache = serde_json::from_str(&contents)
            .context("Failed to parse cache JSON")?;

        Ok(cache)
    }

    /// Save cache to disk
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let cache_path = Self::cache_path(project_root);

        // Create .venore directory if it doesn't exist
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent)
                .context("Failed to create .venore directory")?;
        }

        let json = serde_json::to_string_pretty(&self)
            .context("Failed to serialize cache")?;

        fs::write(&cache_path, json)
            .context("Failed to write cache file")?;

        Ok(())
    }

    /// Get the cache file path
    fn cache_path(project_root: &Path) -> PathBuf {
        project_root.join(".venore").join("hash-cache.json")
    }

    /// Normalize path for cache key (convert to string, handle case sensitivity)
    fn normalize_path(path: &Path) -> String {
        path.to_string_lossy().to_lowercase().replace('\\', "/")
    }

    /// Update cache entry for a file
    pub fn update(&mut self, file_path: &Path, code_hash: String, context_path: &Path) {
        let key = Self::normalize_path(file_path);
        let entry = CacheEntry {
            file_path: file_path.to_path_buf(),
            code_hash,
            context_path: context_path.to_path_buf(),
            last_checked: chrono::Utc::now().to_rfc3339(),
        };
        self.entries.insert(key, entry);
    }

    /// Get cache entry for a file
    pub fn get(&self, file_path: &Path) -> Option<&CacheEntry> {
        let key = Self::normalize_path(file_path);
        self.entries.get(&key)
    }

    /// Remove cache entry for a file
    pub fn remove(&mut self, file_path: &Path) -> Option<CacheEntry> {
        let key = Self::normalize_path(file_path);
        self.entries.remove(&key)
    }

    /// Check if a .context.md file is stale (code changed since generation)
    ///
    /// Returns `true` if:
    /// - No cache entry exists for the file
    /// - Current code hash differs from cached hash
    /// - .context.md file doesn't exist
    ///
    /// Returns `false` if hashes match and .context.md exists.
    pub fn is_stale(&self, file_path: &Path, current_hash: &str) -> bool {
        // Check if we have a cache entry
        let entry = match self.get(file_path) {
            Some(e) => e,
            None => return true, // No cache entry = stale
        };

        // Check if .context.md still exists
        if !entry.context_path.exists() {
            return true; // Context file deleted = stale
        }

        // Compare hashes
        entry.code_hash != current_hash
    }

    /// Get all cached entries
    pub fn entries(&self) -> impl Iterator<Item = &CacheEntry> {
        self.entries.values()
    }

    /// Get number of cached entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all cache entries
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_new_cache() {
        let cache = HashCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_update_and_get() {
        let mut cache = HashCache::new();
        let file_path = PathBuf::from("src/main.rs");
        let context_path = PathBuf::from("src/.context/main.context.md");

        cache.update(&file_path, "sha256-abc123".to_string(), &context_path);

        let entry = cache.get(&file_path).unwrap();
        assert_eq!(entry.code_hash, "sha256-abc123");
        assert_eq!(entry.file_path, file_path);
    }

    #[test]
    fn test_is_stale_no_entry() {
        let cache = HashCache::new();
        let file_path = PathBuf::from("src/main.rs");

        assert!(cache.is_stale(&file_path, "sha256-abc123"));
    }

    #[test]
    fn test_is_stale_hash_mismatch() {
        let mut cache = HashCache::new();
        let file_path = PathBuf::from("src/main.rs");
        let context_path = PathBuf::from("src/.context/main.context.md");

        cache.update(&file_path, "sha256-abc123".to_string(), &context_path);

        // Different hash = stale
        assert!(cache.is_stale(&file_path, "sha256-xyz789"));
    }

    #[test]
    fn test_is_not_stale_hash_match() {
        let mut cache = HashCache::new();
        let temp_dir = TempDir::new().unwrap();
        let context_path = temp_dir.path().join("test.context.md");

        // Create the context file
        fs::write(&context_path, "test content").unwrap();

        let file_path = PathBuf::from("src/main.rs");
        cache.update(&file_path, "sha256-abc123".to_string(), &context_path);

        // Same hash + file exists = not stale
        assert!(!cache.is_stale(&file_path, "sha256-abc123"));
    }

    #[test]
    fn test_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        // Create and populate cache
        let mut cache = HashCache::new();
        let file_path = PathBuf::from("src/main.rs");
        let context_path = PathBuf::from("src/.context/main.context.md");
        cache.update(&file_path, "sha256-abc123".to_string(), &context_path);

        // Save
        cache.save(project_root).unwrap();

        // Load
        let loaded = HashCache::load(project_root).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.get(&file_path).unwrap().code_hash, "sha256-abc123");
    }

    #[test]
    fn test_remove_entry() {
        let mut cache = HashCache::new();
        let file_path = PathBuf::from("src/main.rs");
        let context_path = PathBuf::from("src/.context/main.context.md");

        cache.update(&file_path, "sha256-abc123".to_string(), &context_path);
        assert_eq!(cache.len(), 1);

        cache.remove(&file_path);
        assert_eq!(cache.len(), 0);
        assert!(cache.get(&file_path).is_none());
    }

    #[test]
    fn test_clear_cache() {
        let mut cache = HashCache::new();
        cache.update(&PathBuf::from("file1.rs"), "hash1".to_string(), &PathBuf::from("file1.md"));
        cache.update(&PathBuf::from("file2.rs"), "hash2".to_string(), &PathBuf::from("file2.md"));

        assert_eq!(cache.len(), 2);

        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_path_normalization() {
        let mut cache = HashCache::new();

        // Windows path
        let path1 = PathBuf::from("src\\Main.rs");
        cache.update(&path1, "hash1".to_string(), &PathBuf::from("context.md"));

        // Unix path (should normalize to same key)
        let path2 = PathBuf::from("src/main.rs");
        let entry = cache.get(&path2);

        // Should find the entry (normalized keys match)
        assert!(entry.is_some());
    }
}
