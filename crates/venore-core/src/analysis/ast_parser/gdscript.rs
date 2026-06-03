//! GDScript-specific AST extraction.
//!
//! Imports in GDScript are not statements but call expressions:
//! `const Foo = preload("res://foo.gd")` or
//! `var foo = load("res://foo.gd")`. We dispatch on `call` nodes
//! (filtered by called identifier), mirroring the Ruby approach.

use tree_sitter::Node;

use super::extractors::get_node_text;
use super::Import;

const IMPORT_FUNCTIONS: &[&str] = &["preload", "load"];

/// Extract a `preload(...)` / `load(...)` call as an import.
///
/// Returns `None` for any other call so the dispatch loop can keep
/// scanning sibling nodes.
pub fn extract_import(node: Node, source: &str) -> Option<Import> {
    let line = node.start_position().row + 1;

    let function_name = find_function_name(node, source)?;
    if !IMPORT_FUNCTIONS.contains(&function_name.as_str()) {
        return None;
    }

    let path = find_first_string_argument(node, source)?;

    Some(Import {
        module: path,
        items: vec![],
        line,
    })
}

fn find_function_name(node: Node, source: &str) -> Option<String> {
    // GDScript `call` nodes expose the function expression as either
    // a `function` field or the first child. Walk both.
    if let Some(func) = node.child_by_field_name("function") {
        return get_node_text(func, source);
    }
    if let Some(func) = node.child_by_field_name("name") {
        return get_node_text(func, source);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            return get_node_text(child, source);
        }
    }
    None
}

fn find_first_string_argument(node: Node, source: &str) -> Option<String> {
    let arguments = node
        .child_by_field_name("arguments")
        .or_else(|| {
            // Some grammar versions name it `argument_list` and surface
            // it as an unnamed child.
            let mut cursor = node.walk();
            let mut found = None;
            for child in node.children(&mut cursor) {
                if matches!(child.kind(), "arguments" | "argument_list") {
                    found = Some(child);
                    break;
                }
            }
            found
        })?;

    let mut cursor = arguments.walk();
    for child in arguments.children(&mut cursor) {
        if matches!(child.kind(), "string" | "string_literal") {
            let text = get_node_text(child, source)?;
            return Some(
                text.trim_matches(|c: char| matches!(c, '"' | '\''))
                    .to_string(),
            );
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use std::path::PathBuf;

    #[test]
    #[ignore]
    fn debug_dump_gdscript_ast() {
        use tree_sitter::Parser;
        let source = r#"
class_name Player
extends Node2D

const Helper = preload("res://lib/helpers.gd")

signal damaged(amount: int)

@export var speed: float = 200.0

class Inner:
    var x = 0

    func compute(a: int) -> int:
        return a + 1

func _ready() -> void:
    speed = 250.0

func damage(amount: int) -> void:
    emit_signal("damaged", amount)
"#;
        let mut parser = Parser::new();
        parser
            .set_language(&Language::GDScript.tree_sitter_language())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        fn walk(n: tree_sitter::Node, src: &str, depth: usize) {
            let indent = "  ".repeat(depth);
            let txt = src[n.byte_range()].chars().take(40).collect::<String>();
            println!(
                "{indent}{} [{}..{}]: {txt:?}",
                n.kind(),
                n.start_byte(),
                n.end_byte()
            );
            let mut c = n.walk();
            for ch in n.children(&mut c) {
                walk(ch, src, depth + 1);
            }
        }
        walk(tree.root_node(), source, 0);
    }

    #[test]
    fn test_parse_gdscript_classes_and_functions() {
        let config = ParseConfig {
            file_path: PathBuf::from("player.gd"),
            language: Language::GDScript,
            content: r#"
class_name Player
extends Node2D

class Inner:
    func inner_method() -> int:
        return 1

func _ready() -> void:
    pass

func damage(amount: int) -> void:
    pass
"#
            .to_string(),
        };

        let result = parse_file(config).unwrap();

        // At minimum, the top-level functions should be extracted.
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.name == "_ready" && s.kind == SymbolKind::Function),
            "expected _ready function; got {:?}",
            result.symbols
        );
        assert!(result
            .symbols
            .iter()
            .any(|s| s.name == "damage" && s.kind == SymbolKind::Function));
    }

    #[test]
    fn test_parse_gdscript_preload_imports() {
        let config = ParseConfig {
            file_path: PathBuf::from("player.gd"),
            language: Language::GDScript,
            content: r#"
const Helper = preload("res://lib/helpers.gd")
const Config = preload("res://config.gd")
var loaded = load("res://runtime.gd")

# Should NOT be treated as an import — preload is a method call on an
# object, not the global preload function.
var x = obj.preload_thing("hi")
"#
            .to_string(),
        };

        let result = parse_file(config).unwrap();

        assert!(
            result
                .imports
                .iter()
                .any(|i| i.module == "res://lib/helpers.gd"),
            "expected helpers.gd import; got {:?}",
            result.imports
        );
        assert!(result
            .imports
            .iter()
            .any(|i| i.module == "res://config.gd"));
        assert!(result
            .imports
            .iter()
            .any(|i| i.module == "res://runtime.gd"));
    }
}
