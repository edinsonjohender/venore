//! Generic AST extractors shared across all languages

use tree_sitter::Node;
use super::{Symbol, SymbolKind};

/// Get text content of a node
pub fn get_node_text(node: Node, source: &str) -> Option<String> {
    let start = node.start_byte();
    let end = node.end_byte();

    if start < end && end <= source.len() {
        Some(source[start..end].to_string())
    } else {
        None
    }
}

/// Extract a symbol from a node using the "name" field (works for most languages)
///
/// For C-style function declarations the `name` field isn't on the
/// top-level node — it lives inside a chain of `declarator` /
/// `function_declarator` children. When `name` is missing we walk
/// down the `declarator` chain looking for an `identifier` /
/// `field_identifier`. This keeps the dispatch loop language-agnostic
/// without requiring a per-language override hook.
pub fn extract_symbol_generic(node: Node, source: &str, kind: SymbolKind) -> Option<Symbol> {
    let name_node = node
        .child_by_field_name("name")
        .or_else(|| resolve_declarator_name(node))?;
    let name = get_node_text(name_node, source)?;

    let signature = if kind == SymbolKind::Function || kind == SymbolKind::Method {
        Some(get_node_text(node, source).unwrap_or_default())
    } else {
        None
    };

    Some(Symbol {
        name,
        kind,
        line_start: node.start_position().row + 1,
        line_end: node.end_position().row + 1,
        signature,
    })
}

/// Walk through nested `declarator` fields to find the leaf identifier
/// node. Mirrors the C / C++ grammar shape:
///
///   function_definition
///     declarator: function_declarator
///       declarator: identifier  ← this is the name
///
/// Handles arbitrary nesting (e.g. pointer declarators) by recursing
/// until a node without a `declarator` field is reached.
fn resolve_declarator_name(node: Node) -> Option<Node> {
    let mut current = node;
    loop {
        match current.child_by_field_name("declarator") {
            Some(next) => {
                // If the next node is itself an identifier-like leaf,
                // return it directly.
                if matches!(next.kind(), "identifier" | "field_identifier" | "type_identifier") {
                    return Some(next);
                }
                current = next;
            }
            None => {
                // No more declarator chain; pick up an identifier-like
                // child if present (struct/enum names use this shape).
                let mut cursor = current.walk();
                for child in current.children(&mut cursor) {
                    if matches!(
                        child.kind(),
                        "identifier" | "field_identifier" | "type_identifier"
                    ) {
                        return Some(child);
                    }
                }
                return None;
            }
        }
    }
}
