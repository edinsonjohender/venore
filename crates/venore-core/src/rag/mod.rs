//! RAG Module — Code Indexing & Search
//!
//! Indexes project source code into SQLite FTS5 for full-text search.
//! Chunks files using tree-sitter AST (when available) or as whole files.
//! Provides search capabilities for chat context enrichment.

pub mod types;
pub mod chunker;
pub mod repository;
pub mod indexer;
pub mod searcher;
pub mod embeddings;
pub mod graph_query;
pub mod logbook_repository;
pub mod logbook;

pub use types::*;
pub use repository::RagRepository;
pub use indexer::{index_project, index_project_with_embeddings, index_project_with_graph, detect_changed_files, IndexConfig};
pub use searcher::{search_code, search_code_hybrid, search_logbook_hybrid, search_hybrid, HybridSearchable};
pub use embeddings::{cosine_similarity, embedding_to_blob, blob_to_embedding, merge_results, create_embedding_provider};
pub use graph_query::{GraphQuery, GraphQueryResult, execute_graph_query, classify_query};
pub use logbook_repository::{LogbookRepository, LogbookChunk};
pub use logbook::{index_logbook_node, embed_logbook_chunks, remove_logbook_node};
