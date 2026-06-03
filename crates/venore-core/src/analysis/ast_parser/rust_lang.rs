//! Rust-specific AST extraction
//!
//! Handles: use_declaration, impl_item methods, pub visibility exports

use tree_sitter::Node;
use super::{Import, Export, Symbol, SymbolKind};
use super::extractors::{get_node_text, extract_symbol_generic};

/// Extract `use` declaration as an import
pub fn extract_use_declaration(node: Node, source: &str) -> Option<Import> {
    // `use std::collections::HashMap;`
    // The argument child contains the use path
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "use_as_clause" | "scoped_identifier" | "identifier" | "use_wildcard" | "scoped_use_list" | "use_list" => {
                let text = get_node_text(child, source)?;
                let (module, items) = parse_use_path(&text);
                return Some(Import {
                    module,
                    items,
                    line: node.start_position().row + 1,
                });
            }
            _ => {}
        }
    }
    None
}

/// Parse a Rust use path into module + items
fn parse_use_path(text: &str) -> (String, Vec<String>) {
    let text = text.trim().trim_end_matches(';');

    // `std::collections::{HashMap, HashSet}` → module="std::collections", items=["HashMap", "HashSet"]
    if let Some(brace_start) = text.find('{') {
        let module = text[..brace_start].trim_end_matches(':').trim_end_matches(':').to_string();
        let items_str = &text[brace_start + 1..text.len() - 1]; // strip { }
        let items: Vec<String> = items_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        return (module, items);
    }

    // `std::collections::HashMap` → module="std::collections", items=["HashMap"]
    if let Some(pos) = text.rfind("::") {
        let module = text[..pos].to_string();
        let item = text[pos + 2..].to_string();
        return (module, vec![item]);
    }

    // `serde` → module="serde", items=[]
    (text.to_string(), vec![])
}

/// Extract methods from an `impl` block
pub fn extract_impl_methods(node: Node, source: &str, symbols: &mut Vec<Symbol>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "declaration_list" {
            let mut inner_cursor = child.walk();
            for item in child.children(&mut inner_cursor) {
                if item.kind() == "function_item" {
                    if let Some(symbol) = extract_symbol_generic(item, source, SymbolKind::Method) {
                        symbols.push(symbol);
                    }
                }
            }
        }
    }
}

/// Detect `pub` items and add them as exports
pub fn extract_pub_export(node: Node, source: &str, exports: &mut Vec<Export>) {
    let kind = node.kind();

    // Only check exportable top-level items
    let symbol_kind = match kind {
        "function_item" => SymbolKind::Function,
        "struct_item" => SymbolKind::Struct,
        "enum_item" => SymbolKind::Enum,
        "trait_item" => SymbolKind::Trait,
        "type_item" => SymbolKind::Type,
        _ => return,
    };

    // Check for visibility_modifier ("pub") as first child
    if let Some(first_child) = node.child(0) {
        if first_child.kind() == "visibility_modifier" {
            if let Some(name_node) = node.child_by_field_name("name") {
                if let Some(name) = get_node_text(name_node, source) {
                    exports.push(Export {
                        name,
                        kind: symbol_kind,
                        line: node.start_position().row + 1,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn test_parse_rust_functions_and_structs() {
        let config = ParseConfig {
            file_path: PathBuf::from("test.rs"),
            language: Language::Rust,
            content: r#"
fn greet(name: &str) -> String {
    format!("Hello {}", name)
}

struct User {
    name: String,
    age: u32,
}

enum Color {
    Red,
    Green,
    Blue,
}

trait Greetable {
    fn greet(&self) -> String;
}
"#.to_string(),
        };

        let result = parse_file(config).unwrap();

        assert!(result.symbols.iter().any(|s| s.name == "greet" && s.kind == SymbolKind::Function));
        assert!(result.symbols.iter().any(|s| s.name == "User" && s.kind == SymbolKind::Struct));
        assert!(result.symbols.iter().any(|s| s.name == "Color" && s.kind == SymbolKind::Enum));
        assert!(result.symbols.iter().any(|s| s.name == "Greetable" && s.kind == SymbolKind::Trait));
    }

    #[test]
    fn test_parse_rust_impl_methods() {
        let config = ParseConfig {
            file_path: PathBuf::from("test.rs"),
            language: Language::Rust,
            content: r#"
struct Calculator;

impl Calculator {
    fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    fn multiply(&self, a: i32, b: i32) -> i32 {
        a * b
    }
}
"#.to_string(),
        };

        let result = parse_file(config).unwrap();

        assert!(result.symbols.iter().any(|s| s.name == "Calculator" && s.kind == SymbolKind::Struct));
        assert!(result.symbols.iter().any(|s| s.name == "add" && s.kind == SymbolKind::Method));
        assert!(result.symbols.iter().any(|s| s.name == "multiply" && s.kind == SymbolKind::Method));
    }

    #[test]
    fn test_parse_rust_use_declarations() {
        let config = ParseConfig {
            file_path: PathBuf::from("test.rs"),
            language: Language::Rust,
            content: r#"
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use crate::error::Result;
"#.to_string(),
        };

        let result = parse_file(config).unwrap();

        assert!(result.imports.iter().any(|i| i.module == "std::collections" && i.items.contains(&"HashMap".to_string())));
        assert!(result.imports.iter().any(|i| i.module == "serde" && i.items.contains(&"Serialize".to_string())));
        assert!(result.imports.iter().any(|i| i.module == "crate::error" && i.items.contains(&"Result".to_string())));
    }

    #[test]
    fn test_parse_rust_pub_exports() {
        let config = ParseConfig {
            file_path: PathBuf::from("test.rs"),
            language: Language::Rust,
            content: r#"
pub fn public_fn() -> bool { true }

fn private_fn() -> bool { false }

pub struct PublicStruct {
    pub field: String,
}

struct PrivateStruct;
"#.to_string(),
        };

        let result = parse_file(config).unwrap();

        assert!(result.exports.iter().any(|e| e.name == "public_fn" && e.kind == SymbolKind::Function));
        assert!(result.exports.iter().any(|e| e.name == "PublicStruct" && e.kind == SymbolKind::Struct));
        assert!(!result.exports.iter().any(|e| e.name == "private_fn"));
        assert!(!result.exports.iter().any(|e| e.name == "PrivateStruct"));
    }
}
