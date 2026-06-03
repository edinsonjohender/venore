//! Python-specific AST extraction
//!
//! Handles: import_statement, import_from_statement, decorated_definition

use tree_sitter::Node;
use super::{Import, Symbol};
use super::extractors::{get_node_text, extract_symbol_generic};
use super::lang_config::NodeMapping;

/// Extract Python imports (both `import x` and `from x import y`)
pub fn extract_imports(node: Node, source: &str, imports: &mut Vec<Import>) {
    match node.kind() {
        "import_statement" => {
            // `import os` or `import os, sys`
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "dotted_name" {
                    if let Some(module) = get_node_text(child, source) {
                        imports.push(Import {
                            module,
                            items: vec![],
                            line: node.start_position().row + 1,
                        });
                    }
                }
                // `import os as operating_system`
                if child.kind() == "aliased_import" {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        if let Some(module) = get_node_text(name_node, source) {
                            imports.push(Import {
                                module,
                                items: vec![],
                                line: node.start_position().row + 1,
                            });
                        }
                    }
                }
            }
        }
        "import_from_statement" => {
            // `from os.path import join, exists`
            let mut module = String::new();
            let mut items = Vec::new();

            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                match child.kind() {
                    "dotted_name" | "relative_import" => {
                        if module.is_empty() {
                            if let Some(text) = get_node_text(child, source) {
                                module = text;
                            }
                        } else {
                            // This is an imported name
                            if let Some(name) = get_node_text(child, source) {
                                items.push(name);
                            }
                        }
                    }
                    "aliased_import" => {
                        if let Some(name_node) = child.child_by_field_name("name") {
                            if let Some(name) = get_node_text(name_node, source) {
                                items.push(name);
                            }
                        }
                    }
                    _ => {}
                }
            }

            if !module.is_empty() {
                imports.push(Import {
                    module,
                    items,
                    line: node.start_position().row + 1,
                });
            }
        }
        _ => {}
    }
}

/// Extract decorated definitions (functions/classes with @decorators)
pub fn extract_decorated(node: Node, source: &str, mapping: &NodeMapping, symbols: &mut Vec<Symbol>) {
    // A decorated_definition wraps a function_definition or class_definition
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(symbol_kind) = mapping.symbol_kind_for(child.kind()) {
            if let Some(mut symbol) = extract_symbol_generic(child, source, symbol_kind) {
                // Use the decorated_definition's span (includes decorators)
                symbol.line_start = node.start_position().row + 1;
                symbols.push(symbol);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn test_parse_python_functions_and_classes() {
        let config = ParseConfig {
            file_path: PathBuf::from("test.py"),
            language: Language::Python,
            content: r#"
def greet(name: str) -> str:
    return f"Hello {name}"

class User:
    def __init__(self, name: str):
        self.name = name

    def say_hello(self):
        return f"Hi, I'm {self.name}"
"#.to_string(),
        };

        let result = parse_file(config).unwrap();

        assert!(result.symbols.iter().any(|s| s.name == "greet" && s.kind == SymbolKind::Function));
        assert!(result.symbols.iter().any(|s| s.name == "User" && s.kind == SymbolKind::Class));
        // Methods inside class are also detected as functions
        assert!(result.symbols.iter().any(|s| s.name == "__init__" && s.kind == SymbolKind::Function));
        assert!(result.symbols.iter().any(|s| s.name == "say_hello" && s.kind == SymbolKind::Function));
    }

    #[test]
    fn test_parse_python_imports() {
        let config = ParseConfig {
            file_path: PathBuf::from("test.py"),
            language: Language::Python,
            content: r#"
import os
from pathlib import Path
from typing import List, Optional
"#.to_string(),
        };

        let result = parse_file(config).unwrap();

        assert!(result.imports.iter().any(|i| i.module == "os"));
        assert!(result.imports.iter().any(|i| i.module == "pathlib"));
        let typing_import = result.imports.iter().find(|i| i.module == "typing").unwrap();
        assert!(typing_import.items.contains(&"List".to_string()));
        assert!(typing_import.items.contains(&"Optional".to_string()));
    }

    #[test]
    fn test_parse_python_decorated() {
        let config = ParseConfig {
            file_path: PathBuf::from("test.py"),
            language: Language::Python,
            content: r#"
@app.route("/hello")
def hello():
    return "world"

@dataclass
class Config:
    host: str
    port: int
"#.to_string(),
        };

        let result = parse_file(config).unwrap();

        assert!(result.symbols.iter().any(|s| s.name == "hello" && s.kind == SymbolKind::Function));
        assert!(result.symbols.iter().any(|s| s.name == "Config" && s.kind == SymbolKind::Class));
    }
}
