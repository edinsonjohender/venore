//! TypeScript / JavaScript / TSX specific extraction logic

use tree_sitter::Node;
use super::{Import, Export, Symbol, SymbolKind};
use super::extractors::{get_node_text, extract_symbol_generic};

/// Extract arrow function from variable declaration (TS/JS pattern)
pub fn extract_variable_function(node: Node, source: &str) -> Option<Symbol> {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if child.kind() == "variable_declarator" {
            let name_node = child.child_by_field_name("name")?;
            let name = get_node_text(name_node, source)?;

            if let Some(value_node) = child.child_by_field_name("value") {
                if value_node.kind() == "arrow_function" {
                    return Some(Symbol {
                        name,
                        kind: SymbolKind::Function,
                        line_start: child.start_position().row + 1,
                        line_end: child.end_position().row + 1,
                        signature: Some(get_node_text(value_node, source).unwrap_or_default()),
                    });
                }
            }
        }
    }

    None
}

/// Extract import statement (TS/JS)
pub fn extract_import(node: Node, source: &str) -> Option<Import> {
    let mut module = String::new();
    let mut items = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "string" => {
                let text = get_node_text(child, source)?;
                module = text.trim_matches(|c| c == '"' || c == '\'' || c == '`').to_string();
            }
            "import_clause" => {
                extract_import_items(child, source, &mut items);
            }
            _ => {}
        }
    }

    if module.is_empty() {
        return None;
    }

    Some(Import {
        module,
        items,
        line: node.start_position().row + 1,
    })
}

/// Extract items from import clause
fn extract_import_items(node: Node, source: &str, items: &mut Vec<String>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                if let Some(name) = get_node_text(child, source) {
                    items.push(name);
                }
            }
            "named_imports" => {
                extract_import_items(child, source, items);
            }
            "import_specifier" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if let Some(name) = get_node_text(name_node, source) {
                        items.push(name);
                    }
                }
            }
            _ => {
                extract_import_items(child, source, items);
            }
        }
    }
}

/// Extract export statement (TS/JS)
pub fn extract_export(node: Node, source: &str, exports: &mut Vec<Export>, symbols: &mut Vec<Symbol>) {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_declaration" | "class_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if let Some(name) = get_node_text(name_node, source) {
                        let kind = if child.kind() == "function_declaration" {
                            SymbolKind::Function
                        } else {
                            SymbolKind::Class
                        };

                        exports.push(Export {
                            name: name.clone(),
                            kind: kind.clone(),
                            line: node.start_position().row + 1,
                        });

                        if let Some(symbol) = extract_symbol_generic(child, source, kind) {
                            symbols.push(symbol);
                        }
                    }
                }
            }
            "lexical_declaration" => {
                if let Some(declarator) = child.child(1) {
                    if declarator.kind() == "variable_declarator" {
                        if let Some(name_node) = declarator.child_by_field_name("name") {
                            if let Some(name) = get_node_text(name_node, source) {
                                exports.push(Export {
                                    name,
                                    kind: SymbolKind::Constant,
                                    line: node.start_position().row + 1,
                                });
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
