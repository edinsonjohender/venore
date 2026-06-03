//! AST Parser for extracting code structure using Tree-sitter
//!
//! Parses source code files and extracts:
//! - Symbols (functions, classes, interfaces, enums)
//! - Imports
//! - Exports
//!
//! Supports: TypeScript, JavaScript, TSX, Python, Rust, Java, Go, C#, PHP, Ruby, Kotlin, C, C++, GDScript

mod extractors;
mod lang_config;
mod ts_js;
mod python;
mod rust_lang;
mod java;
mod go_lang;
mod csharp;
mod php;
mod ruby;
mod kotlin;
mod c_lang;
mod cpp_lang;
mod gdscript;

use crate::error::{VenoreError, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tree_sitter::{Parser, Node};

pub use lang_config::NodeMapping;

/// Supported programming languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    TypeScript,
    JavaScript,
    TSX,
    Python,
    Rust,
    Java,
    Go,
    CSharp,
    Php,
    Ruby,
    Kotlin,
    C,
    Cpp,
    GDScript,
}

impl Language {
    /// Get the tree-sitter language for this language
    pub fn tree_sitter_language(&self) -> tree_sitter::Language {
        match self {
            Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Language::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Language::TSX => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Language::Python => tree_sitter_python::LANGUAGE.into(),
            Language::Rust => tree_sitter_rust::LANGUAGE.into(),
            Language::Java => tree_sitter_java::LANGUAGE.into(),
            Language::Go => tree_sitter_go::LANGUAGE.into(),
            Language::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
            Language::Php => tree_sitter_php::LANGUAGE_PHP.into(),
            Language::Ruby => tree_sitter_ruby::LANGUAGE.into(),
            Language::Kotlin => tree_sitter_kotlin_ng::LANGUAGE.into(),
            Language::C => tree_sitter_c::LANGUAGE.into(),
            Language::Cpp => tree_sitter_cpp::LANGUAGE.into(),
            Language::GDScript => tree_sitter_gdscript::LANGUAGE.into(),
        }
    }

    /// Detect language from file extension
    ///
    /// C/C++ header files (`.h`) are ambiguous: they can be either C or
    /// C++. We default to C++ because C++ is a near-superset of C and
    /// the C++ grammar parses C headers correctly. Pure-C projects
    /// that need stricter parsing can be addressed later by inspecting
    /// the project type.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "ts" => Some(Language::TypeScript),
            "tsx" => Some(Language::TSX),
            "js" | "jsx" => Some(Language::JavaScript),
            "py" => Some(Language::Python),
            "rs" => Some(Language::Rust),
            "java" => Some(Language::Java),
            "go" => Some(Language::Go),
            "cs" => Some(Language::CSharp),
            "php" => Some(Language::Php),
            "rb" => Some(Language::Ruby),
            "kt" | "kts" => Some(Language::Kotlin),
            "c" => Some(Language::C),
            "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" | "h" => Some(Language::Cpp),
            "gd" => Some(Language::GDScript),
            _ => None,
        }
    }

    /// Every file extension Venore considers source code by default.
    ///
    /// Drives the default `ScanConfig::target_extensions` for the
    /// analysis pipeline and the wizard. The exhaustiveness witness
    /// below makes the compiler fail when a new `Language` variant is
    /// added without listing the corresponding extension in the slice.
    pub fn all_extensions() -> &'static [&'static str] {
        #[allow(dead_code)]
        fn exhaustiveness_witness(lang: Language) {
            match lang {
                Language::TypeScript => (),
                Language::JavaScript => (),
                Language::TSX => (),
                Language::Python => (),
                Language::Rust => (),
                Language::Java => (),
                Language::Go => (),
                Language::CSharp => (),
                Language::Php => (),
                Language::Ruby => (),
                Language::Kotlin => (),
                Language::C => (),
                Language::Cpp => (),
                Language::GDScript => (),
            }
        }
        &[
            "ts", "tsx", "js", "jsx", "py", "rs", "java", "go", "cs", "php", "rb", "kt", "kts",
            "c", "cpp", "cc", "cxx", "hpp", "hh", "hxx", "h", "gd",
        ]
    }

    /// Human-readable name for the language (used in UI and
    /// `detect_technologies`).
    pub fn display_name(&self) -> &'static str {
        match self {
            Language::TypeScript => "TypeScript",
            Language::JavaScript => "JavaScript",
            Language::TSX => "TypeScript",
            Language::Python => "Python",
            Language::Rust => "Rust",
            Language::Java => "Java",
            Language::Go => "Go",
            Language::CSharp => "C#",
            Language::Php => "PHP",
            Language::Ruby => "Ruby",
            Language::Kotlin => "Kotlin",
            Language::C => "C",
            Language::Cpp => "C++",
            Language::GDScript => "GDScript",
        }
    }

    /// Map to LSP language identifier string.
    pub fn to_lsp_language_id(&self) -> &'static str {
        match self {
            Language::TypeScript => "typescript",
            Language::TSX => "typescriptreact",
            Language::JavaScript => "javascript",
            Language::Python => "python",
            Language::Rust => "rust",
            Language::Java => "java",
            Language::Go => "go",
            Language::CSharp => "csharp",
            Language::Php => "php",
            Language::Ruby => "ruby",
            Language::Kotlin => "kotlin",
            Language::C => "c",
            Language::Cpp => "cpp",
            Language::GDScript => "gdscript",
        }
    }

    /// Whether this language has a tree-sitter grammar available
    pub fn is_parseable(&self) -> bool {
        true // All variants have grammars
    }

    /// Get the node mapping configuration for this language
    fn node_mapping(&self) -> &'static NodeMapping {
        match self {
            Language::TypeScript | Language::JavaScript | Language::TSX => &lang_config::TS_JS_MAPPING,
            Language::Python => &lang_config::PYTHON_MAPPING,
            Language::Rust => &lang_config::RUST_MAPPING,
            Language::Java => &lang_config::JAVA_MAPPING,
            Language::Php => &lang_config::PHP_MAPPING,
            Language::Ruby => &lang_config::RUBY_MAPPING,
            Language::Kotlin => &lang_config::KOTLIN_MAPPING,
            Language::C => &lang_config::C_MAPPING,
            Language::Cpp => &lang_config::CPP_MAPPING,
            Language::GDScript => &lang_config::GDSCRIPT_MAPPING,
            Language::Go => &lang_config::GO_MAPPING,
            Language::CSharp => &lang_config::CSHARP_MAPPING,
        }
    }
}

/// Configuration for parsing a file
#[derive(Debug, Clone)]
pub struct ParseConfig {
    /// Path to the file being parsed
    pub file_path: PathBuf,

    /// Programming language
    pub language: Language,

    /// Source code content
    pub content: String,
}

/// Kind of symbol found in code
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Class,
    Interface,
    Enum,
    Type,
    Constant,
    Variable,
    Method,
    Struct,
    Trait,
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SymbolKind::Function => write!(f, "function"),
            SymbolKind::Class => write!(f, "class"),
            SymbolKind::Interface => write!(f, "interface"),
            SymbolKind::Enum => write!(f, "enum"),
            SymbolKind::Type => write!(f, "type"),
            SymbolKind::Constant => write!(f, "const"),
            SymbolKind::Variable => write!(f, "variable"),
            SymbolKind::Method => write!(f, "method"),
            SymbolKind::Struct => write!(f, "struct"),
            SymbolKind::Trait => write!(f, "trait"),
        }
    }
}

/// A symbol found in the code (function, class, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    /// Symbol name
    pub name: String,

    /// Kind of symbol
    pub kind: SymbolKind,

    /// Starting line number (1-indexed)
    pub line_start: usize,

    /// Ending line number (1-indexed)
    pub line_end: usize,

    /// Optional signature (for functions)
    pub signature: Option<String>,
}

/// An import statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Import {
    /// Module being imported from
    pub module: String,

    /// Items being imported (empty for default imports)
    pub items: Vec<String>,

    /// Line number (1-indexed)
    pub line: usize,
}

/// An export statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Export {
    /// Name of exported item
    pub name: String,

    /// Kind of export
    pub kind: SymbolKind,

    /// Line number (1-indexed)
    pub line: usize,
}

/// Result of parsing a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseResult {
    /// Path to the parsed file
    pub file_path: PathBuf,

    /// Language that was parsed
    pub language: Language,

    /// Symbols found in the file
    pub symbols: Vec<Symbol>,

    /// Imports found in the file
    pub imports: Vec<Import>,

    /// Exports found in the file
    pub exports: Vec<Export>,

    /// Time taken to parse in milliseconds
    pub parse_duration_ms: u64,
}

/// Parse a source code file and extract structure
pub fn parse_file(config: ParseConfig) -> Result<ParseResult> {
    let start = std::time::Instant::now();

    let mut parser = Parser::new();
    let language = config.language.tree_sitter_language();

    parser
        .set_language(&language)
        .map_err(|e| VenoreError::ParseError(format!("Failed to set language: {}", e)))?;

    let tree = parser
        .parse(&config.content, None)
        .ok_or_else(|| VenoreError::ParseError("Failed to parse file".to_string()))?;

    let root_node = tree.root_node();
    let mut symbols = Vec::new();
    let mut imports = Vec::new();
    let mut exports = Vec::new();

    extract_nodes(
        root_node,
        &config.content,
        &config.language,
        &mut symbols,
        &mut imports,
        &mut exports,
    );

    let parse_duration_ms = start.elapsed().as_millis() as u64;

    Ok(ParseResult {
        file_path: config.file_path,
        language: config.language,
        symbols,
        imports,
        exports,
        parse_duration_ms,
    })
}

/// Recursively extract nodes from the AST using data-driven dispatch
fn extract_nodes(
    node: Node,
    source: &str,
    language: &Language,
    symbols: &mut Vec<Symbol>,
    imports: &mut Vec<Import>,
    exports: &mut Vec<Export>,
) {
    let kind = node.kind();
    let mapping = language.node_mapping();

    // Check if this node maps to a symbol
    if let Some(symbol_kind) = mapping.symbol_kind_for(kind) {
        if let Some(symbol) = extractors::extract_symbol_generic(node, source, symbol_kind) {
            symbols.push(symbol);
        }
    }

    // Check variable/arrow function patterns (TS/JS specific)
    if mapping.variable_nodes.contains(&kind) {
        if let Some(symbol) = ts_js::extract_variable_function(node, source) {
            symbols.push(symbol);
        }
    }

    // Check container nodes for nested symbols (e.g., Rust impl blocks)
    if mapping.container_nodes.contains(&kind)
        && *language == Language::Rust { rust_lang::extract_impl_methods(node, source, symbols) }

    // Extract imports
    if mapping.import_nodes.contains(&kind) {
        match *language {
            Language::TypeScript | Language::JavaScript | Language::TSX => {
                if let Some(import) = ts_js::extract_import(node, source) {
                    imports.push(import);
                }
            }
            Language::Python => {
                python::extract_imports(node, source, imports);
            }
            Language::Rust => {
                if let Some(import) = rust_lang::extract_use_declaration(node, source) {
                    imports.push(import);
                }
            }
            Language::Java => {
                if let Some(import) = java::extract_import(node, source) {
                    imports.push(import);
                }
            }
            Language::Go => {
                go_lang::extract_imports(node, source, imports);
            }
            Language::CSharp => {
                if let Some(import) = csharp::extract_using(node, source) {
                    imports.push(import);
                }
            }
            Language::Php => {
                if let Some(import) = php::extract_import(node, source) {
                    imports.push(import);
                }
            }
            Language::Ruby => {
                if let Some(import) = ruby::extract_import(node, source) {
                    imports.push(import);
                }
            }
            Language::Kotlin => {
                if let Some(import) = kotlin::extract_import(node, source) {
                    imports.push(import);
                }
            }
            Language::C => {
                if let Some(import) = c_lang::extract_include(node, source) {
                    imports.push(import);
                }
            }
            Language::Cpp => {
                if let Some(import) = cpp_lang::extract_include(node, source) {
                    imports.push(import);
                }
            }
            Language::GDScript => {
                if let Some(import) = gdscript::extract_import(node, source) {
                    imports.push(import);
                }
            }
        }
    }

    // Extract exports
    if mapping.export_nodes.contains(&kind) {
        match *language {
            Language::TypeScript | Language::JavaScript | Language::TSX => {
                ts_js::extract_export(node, source, exports, symbols);
            }
            _ => {}
        }
    }

    // Rust: detect pub visibility as exports
    if *language == Language::Rust {
        rust_lang::extract_pub_export(node, source, exports);
    }

    // Go: uppercase names are exported
    if *language == Language::Go {
        go_lang::extract_exported_symbol(node, source, mapping, exports);
    }

    // Python: decorated definitions (wrapping function/class)
    if *language == Language::Python && kind == "decorated_definition" {
        python::extract_decorated(node, source, mapping, symbols);
        // Don't recurse into decorated_definition children — we already handled them
        return;
    }

    // Go: type_spec needs disambiguation (struct vs interface)
    if *language == Language::Go && kind == "type_spec" {
        if let Some(symbol) = go_lang::extract_type_spec(node, source) {
            symbols.push(symbol);
        }
        // Don't recurse further into type_spec
        return;
    }

    // Recursively process children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        extract_nodes(child, source, language, symbols, imports, exports);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_from_extension() {
        assert_eq!(Language::from_extension("ts"), Some(Language::TypeScript));
        assert_eq!(Language::from_extension("tsx"), Some(Language::TSX));
        assert_eq!(Language::from_extension("js"), Some(Language::JavaScript));
        assert_eq!(Language::from_extension("jsx"), Some(Language::JavaScript));
        assert_eq!(Language::from_extension("rs"), Some(Language::Rust));
        assert_eq!(Language::from_extension("py"), Some(Language::Python));
        assert_eq!(Language::from_extension("java"), Some(Language::Java));
        assert_eq!(Language::from_extension("go"), Some(Language::Go));
        assert_eq!(Language::from_extension("cs"), Some(Language::CSharp));
        assert_eq!(Language::from_extension("php"), Some(Language::Php));
        assert_eq!(Language::from_extension("rb"), Some(Language::Ruby));
        assert_eq!(Language::from_extension("kt"), Some(Language::Kotlin));
        assert_eq!(Language::from_extension("kts"), Some(Language::Kotlin));
        assert_eq!(Language::from_extension("c"), Some(Language::C));
        assert_eq!(Language::from_extension("cpp"), Some(Language::Cpp));
        assert_eq!(Language::from_extension("cc"), Some(Language::Cpp));
        assert_eq!(Language::from_extension("cxx"), Some(Language::Cpp));
        assert_eq!(Language::from_extension("hpp"), Some(Language::Cpp));
        assert_eq!(Language::from_extension("h"), Some(Language::Cpp));
        assert_eq!(Language::from_extension("gd"), Some(Language::GDScript));
        assert_eq!(Language::from_extension("unknown"), None);
    }

    #[test]
    fn test_parse_typescript_functions() {
        let config = ParseConfig {
            file_path: PathBuf::from("test.ts"),
            language: Language::TypeScript,
            content: r#"
function greet(name: string) {
    return `Hello ${name}`;
}

const add = (a: number, b: number) => a + b;
            "#.to_string(),
        };

        let result = parse_file(config).unwrap();

        assert_eq!(result.symbols.len(), 2);
        assert!(result.symbols.iter().any(|s| s.name == "greet" && s.kind == SymbolKind::Function));
        assert!(result.symbols.iter().any(|s| s.name == "add" && s.kind == SymbolKind::Function));
    }

    #[test]
    fn test_parse_typescript_classes() {
        let config = ParseConfig {
            file_path: PathBuf::from("test.ts"),
            language: Language::TypeScript,
            content: r#"
class User {
    constructor(public name: string) {}

    greet() {
        return `Hello ${this.name}`;
    }
}

class Product {
    id: number;
}
            "#.to_string(),
        };

        let result = parse_file(config).unwrap();

        assert!(result.symbols.iter().any(|s| s.name == "User" && s.kind == SymbolKind::Class));
        assert!(result.symbols.iter().any(|s| s.name == "Product" && s.kind == SymbolKind::Class));
    }

    #[test]
    fn test_parse_typescript_interfaces() {
        let config = ParseConfig {
            file_path: PathBuf::from("test.ts"),
            language: Language::TypeScript,
            content: r#"
interface User {
    name: string;
    age: number;
}

interface Product {
    id: number;
    title: string;
}
            "#.to_string(),
        };

        let result = parse_file(config).unwrap();

        assert!(result.symbols.iter().any(|s| s.name == "User" && s.kind == SymbolKind::Interface));
        assert!(result.symbols.iter().any(|s| s.name == "Product" && s.kind == SymbolKind::Interface));
    }

    #[test]
    fn test_parse_imports() {
        let config = ParseConfig {
            file_path: PathBuf::from("test.ts"),
            language: Language::TypeScript,
            content: r#"
import { useState } from 'react';
import type { User } from './types';
import fs from 'fs';
            "#.to_string(),
        };

        let result = parse_file(config).unwrap();

        assert_eq!(result.imports.len(), 3);
        assert!(result.imports.iter().any(|i| i.module == "react"));
        assert!(result.imports.iter().any(|i| i.module == "./types"));
        assert!(result.imports.iter().any(|i| i.module == "fs"));

        let react_import = result.imports.iter().find(|i| i.module == "react").unwrap();
        assert!(react_import.items.contains(&"useState".to_string()));
    }

    #[test]
    fn test_parse_exports() {
        let config = ParseConfig {
            file_path: PathBuf::from("test.ts"),
            language: Language::TypeScript,
            content: r#"
export function greet(name: string) {
    return `Hello ${name}`;
}

export class User {
    name: string;
}

export const VERSION = "1.0.0";
            "#.to_string(),
        };

        let result = parse_file(config).unwrap();

        assert!(result.exports.iter().any(|e| e.name == "greet" && e.kind == SymbolKind::Function));
        assert!(result.exports.iter().any(|e| e.name == "User" && e.kind == SymbolKind::Class));
        assert!(result.exports.iter().any(|e| e.name == "VERSION" && e.kind == SymbolKind::Constant));

        assert!(result.symbols.iter().any(|s| s.name == "greet"));
        assert!(result.symbols.iter().any(|s| s.name == "User"));
    }

    #[test]
    fn test_handles_syntax_errors() {
        let config = ParseConfig {
            file_path: PathBuf::from("test.ts"),
            language: Language::TypeScript,
            content: r#"
function valid() {
    return "ok";
}

// Incomplete function (syntax error)
function broken(
            "#.to_string(),
        };

        let result = parse_file(config);
        assert!(result.is_ok());

        let result = result.unwrap();
        assert!(result.symbols.iter().any(|s| s.name == "valid"));
    }

    #[test]
    fn test_parse_duration_recorded() {
        let config = ParseConfig {
            file_path: PathBuf::from("test.ts"),
            language: Language::TypeScript,
            content: "function test() { return 42; }".to_string(),
        };

        let result = parse_file(config).unwrap();
        assert!(result.parse_duration_ms < 1000);
    }

    #[test]
    fn test_line_numbers() {
        let config = ParseConfig {
            file_path: PathBuf::from("test.ts"),
            language: Language::TypeScript,
            content: r#"
function first() {
    return 1;
}

function second() {
    return 2;
}
            "#.to_string(),
        };

        let result = parse_file(config).unwrap();

        let first = result.symbols.iter().find(|s| s.name == "first").unwrap();
        let second = result.symbols.iter().find(|s| s.name == "second").unwrap();

        assert!(first.line_start < second.line_start);
    }
}
