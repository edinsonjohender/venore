//! Code Chunker
//!
//! Extracts code chunks from files using tree-sitter AST parsing.
//! Falls back to whole-file chunking for unsupported languages.

use std::path::Path;

use crate::analysis::ast_parser::{parse_file, Language, ParseConfig, SymbolKind};
use crate::error::Result;
use crate::rag::types::RagChunk;

/// Maximum size for a whole-file chunk (10KB)
const MAX_FILE_CHUNK_BYTES: usize = 10 * 1024;

/// Extract code chunks from a file using tree-sitter when available
pub fn chunk_file(
    file_id: &str,
    project_id: &str,
    file_path: &Path,
    relative_path: &str,
    content: &str,
) -> Result<Vec<RagChunk>> {
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    let language = Language::from_extension(ext);

    let can_parse = language.is_some_and(|l: Language| l.is_parseable());

    if can_parse {
        match chunk_with_ast(file_id, project_id, file_path, relative_path, content, language.unwrap()) {
            Ok(chunks) if !chunks.is_empty() => return Ok(chunks),
            Ok(_) => {
                tracing::debug!("AST produced no chunks for {}, falling back to file chunk", relative_path);
            }
            Err(e) => {
                tracing::debug!("AST parsing failed for {}: {}, falling back to file chunk", relative_path, e);
            }
        }
    }

    // Fallback: whole-file chunk
    Ok(chunk_as_file(file_id, project_id, relative_path, content))
}

/// Extract chunks using tree-sitter AST
fn chunk_with_ast(
    file_id: &str,
    project_id: &str,
    file_path: &Path,
    relative_path: &str,
    content: &str,
    language: Language,
) -> Result<Vec<RagChunk>> {
    let config = ParseConfig {
        file_path: file_path.to_path_buf(),
        language,
        content: content.to_string(),
    };

    let parse_result = parse_file(config)?;
    let lines: Vec<&str> = content.lines().collect();
    let mut chunks = Vec::new();

    // Extract import header as a chunk
    if !parse_result.imports.is_empty() {
        let first_import_line = parse_result.imports.iter().map(|i| i.line).min().unwrap_or(1);
        let last_import_line = parse_result.imports.iter().map(|i| i.line).max().unwrap_or(1);

        let import_content = extract_lines(&lines, first_import_line, last_import_line);
        if !import_content.trim().is_empty() {
            chunks.push(RagChunk {
                id: uuid::Uuid::new_v4().to_string(),
                file_id: file_id.to_string(),
                project_id: project_id.to_string(),
                chunk_type: "imports".to_string(),
                name: format!("imports:{}", relative_path),
                content: import_content,
                line_start: first_import_line as u32,
                line_end: last_import_line as u32,
                relative_path: relative_path.to_string(),
                metadata: None,
            });
        }
    }

    // Extract each symbol as a chunk
    for symbol in &parse_result.symbols {
        let chunk_type = match symbol.kind {
            SymbolKind::Function => "function",
            SymbolKind::Class => "class",
            SymbolKind::Interface => "interface",
            SymbolKind::Enum => "enum",
            SymbolKind::Type => "type",
            SymbolKind::Constant => "constant",
            SymbolKind::Variable => "variable",
            SymbolKind::Method => "method",
            SymbolKind::Struct => "struct",
            SymbolKind::Trait => "trait",
        };

        let symbol_content = extract_lines(&lines, symbol.line_start, symbol.line_end);
        if symbol_content.trim().is_empty() {
            continue;
        }

        let metadata = symbol.signature.as_ref().map(|sig| {
            serde_json::json!({ "signature": sig }).to_string()
        });

        chunks.push(RagChunk {
            id: uuid::Uuid::new_v4().to_string(),
            file_id: file_id.to_string(),
            project_id: project_id.to_string(),
            chunk_type: chunk_type.to_string(),
            name: symbol.name.clone(),
            content: symbol_content,
            line_start: symbol.line_start as u32,
            line_end: symbol.line_end as u32,
            relative_path: relative_path.to_string(),
            metadata,
        });
    }

    Ok(chunks)
}

/// Create a single whole-file chunk (capped at MAX_FILE_CHUNK_BYTES)
fn chunk_as_file(
    file_id: &str,
    project_id: &str,
    relative_path: &str,
    content: &str,
) -> Vec<RagChunk> {
    let truncated = if content.len() > MAX_FILE_CHUNK_BYTES {
        // Find the last valid char boundary
        let mut end = MAX_FILE_CHUNK_BYTES;
        while !content.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        &content[..end]
    } else {
        content
    };

    let line_count = truncated.lines().count().max(1) as u32;

    vec![RagChunk {
        id: uuid::Uuid::new_v4().to_string(),
        file_id: file_id.to_string(),
        project_id: project_id.to_string(),
        chunk_type: "file".to_string(),
        name: relative_path.to_string(),
        content: truncated.to_string(),
        line_start: 1,
        line_end: line_count,
        relative_path: relative_path.to_string(),
        metadata: None,
    }]
}

/// Extract lines from content (1-indexed, inclusive)
fn extract_lines(lines: &[&str], start: usize, end: usize) -> String {
    let start_idx = start.saturating_sub(1);
    let end_idx = end.min(lines.len());

    if start_idx >= lines.len() {
        return String::new();
    }

    lines[start_idx..end_idx].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_chunk_typescript_file() {
        let content = r#"import { useState } from 'react';

function greet(name: string): string {
    return `Hello ${name}`;
}

const add = (a: number, b: number) => a + b;

interface User {
    name: string;
    age: number;
}
"#;

        let chunks = chunk_file(
            "file-1",
            "proj-1",
            &PathBuf::from("src/utils.ts"),
            "src/utils.ts",
            content,
        ).unwrap();

        // Should have: imports + greet + add + User
        assert!(chunks.len() >= 3, "Expected at least 3 chunks, got {}", chunks.len());

        // Check we have a function chunk
        assert!(chunks.iter().any(|c| c.name == "greet" && c.chunk_type == "function"));
    }

    #[test]
    fn test_chunk_unsupported_language_falls_back() {
        let content = "print('hello world')";

        let chunks = chunk_file(
            "file-1",
            "proj-1",
            &PathBuf::from("script.rb"),
            "script.rb",
            content,
        ).unwrap();

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].chunk_type, "file");
        assert_eq!(chunks[0].name, "script.rb");
    }

    #[test]
    fn test_chunk_go_file_with_ast() {
        let content = "package main\n\nfunc main() {\n\tfmt.Println(\"hello\")\n}\n";

        let chunks = chunk_file(
            "file-1",
            "proj-1",
            &PathBuf::from("main.go"),
            "main.go",
            content,
        ).unwrap();

        // Go is now parseable — should get at least a function chunk
        assert!(chunks.iter().any(|c| c.name == "main" && c.chunk_type == "function"));
    }

    #[test]
    fn test_chunk_large_file_truncated() {
        let content = "x".repeat(20_000);

        let chunks = chunk_file(
            "file-1",
            "proj-1",
            &PathBuf::from("big.txt"),
            "big.txt",
            &content,
        ).unwrap();

        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].content.len() <= MAX_FILE_CHUNK_BYTES);
    }

    #[test]
    fn test_extract_lines() {
        let lines = vec!["line1", "line2", "line3", "line4"];
        assert_eq!(extract_lines(&lines, 2, 3), "line2\nline3");
        assert_eq!(extract_lines(&lines, 1, 1), "line1");
    }
}
