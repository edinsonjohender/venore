//! RAG types — data structures for code indexing and search

use serde::{Deserialize, Serialize};

/// A file that has been indexed in the RAG system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagFile {
    pub id: String,
    pub project_id: String,
    pub file_path: String,
    pub relative_path: String,
    pub content_hash: String,
    pub language: Option<String>,
    pub indexed_at: String,
}

/// A chunk of code extracted from a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagChunk {
    pub id: String,
    pub file_id: String,
    pub project_id: String,
    pub chunk_type: String,
    pub name: String,
    pub content: String,
    pub line_start: u32,
    pub line_end: u32,
    pub relative_path: String,
    pub metadata: Option<String>,
}

/// A search result with BM25 score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub chunk: RagChunk,
    pub score: f64,
    /// How this result was found: "fts", "embedding", or "hybrid"
    #[serde(default = "default_search_method")]
    pub search_method: String,
}

fn default_search_method() -> String {
    "fts".to_string()
}

/// Status of the RAG index for a project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStatus {
    pub project_id: String,
    pub is_indexed: bool,
    pub total_files: u32,
    pub total_chunks: u32,
    pub last_indexed_at: Option<String>,
}

/// Progress event emitted during indexing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexProgressEvent {
    pub project_id: String,
    pub current: u32,
    pub total: u32,
    pub current_file: String,
    pub status: String,
}

// =========================================================================
// GRAPH TYPES — Structural relationships between files, modules, and symbols
// =========================================================================

/// A module detected in the project (stored in rag_file_modules)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleInfo {
    pub project_id: String,
    pub module_name: String,
    pub module_path: String,
    pub file_count: u32,
}

/// A dependency edge between two modules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDep {
    pub from_module: String,
    pub to_module: String,
    pub dep_type: String,
}

/// A reference from one symbol to another (import, call, type_ref)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolRef {
    pub id: String,
    pub project_id: String,
    pub from_chunk_id: String,
    pub to_chunk_id: Option<String>,
    pub to_symbol_name: String,
    pub to_file_path: Option<String>,
    pub ref_type: String,
    pub line_number: Option<u32>,
}

/// Set of files that changed since last index
#[derive(Debug, Clone, Default)]
pub struct ChangeSet {
    /// Files not previously indexed
    pub new_files: Vec<std::path::PathBuf>,
    /// Files whose content hash changed
    pub modified_files: Vec<std::path::PathBuf>,
    /// Relative paths of files deleted from disk
    pub deleted_paths: Vec<String>,
}

impl ChangeSet {
    pub fn is_empty(&self) -> bool {
        self.new_files.is_empty() && self.modified_files.is_empty() && self.deleted_paths.is_empty()
    }

    pub fn total_changed(&self) -> usize {
        self.new_files.len() + self.modified_files.len()
    }
}

/// Result of indexing with graph population
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexGraphResult {
    pub indexed: u32,
    pub skipped: u32,
    pub removed: u32,
    pub modules_mapped: u32,
    pub deps_created: u32,
    pub refs_created: u32,
}
