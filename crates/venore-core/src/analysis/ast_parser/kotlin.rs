//! Kotlin-specific AST extraction
//!
//! Handles: `import com.example.Foo`, `import com.example.bar.*`,
//! `import com.example.Baz as Quux`.

use tree_sitter::Node;

use super::extractors::get_node_text;
use super::Import;

/// Extract a Kotlin `import` declaration.
///
/// In `tree-sitter-kotlin-ng` the import statement is shaped as:
///   import qualified_identifier ('.' '*' | 'as' identifier)?
///
/// The qualified identifier (e.g. `com.example.Foo`) lives as a
/// `qualified_identifier` child. Wildcard imports add a trailing `.*`
/// and aliases add `as Name` — both surface as direct children of
/// the import node.
pub fn extract_import(node: Node, source: &str) -> Option<Import> {
    let line = node.start_position().row + 1;

    let mut cursor = node.walk();
    let mut qualified: Option<String> = None;
    let mut wildcard = false;

    for child in node.children(&mut cursor) {
        let kind = child.kind();
        match kind {
            // Most imports go through `qualified_identifier`; a bare
            // `import foo` (single-segment) falls through `identifier`.
            "qualified_identifier" | "identifier" | "simple_identifier" => {
                if qualified.is_none() {
                    qualified = get_node_text(child, source);
                }
            }
            "*" => {
                wildcard = true;
            }
            _ => {}
        }
    }

    let raw = qualified?;
    let cleaned = raw.trim().trim_end_matches('.').to_string();

    if wildcard {
        return Some(Import {
            module: cleaned,
            items: vec![],
            line,
        });
    }

    if let Some(idx) = cleaned.rfind('.') {
        let module = cleaned[..idx].to_string();
        let last = cleaned[idx + 1..].to_string();
        let items = if last.is_empty() { vec![] } else { vec![last] };
        Some(Import { module, items, line })
    } else {
        Some(Import {
            module: cleaned,
            items: vec![],
            line,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_kotlin_classes_and_functions() {
        let config = ParseConfig {
            file_path: PathBuf::from("UserService.kt"),
            language: Language::Kotlin,
            content: r#"
package com.acme.app

class UserService(private val repo: UserRepository) {
    fun greet(name: String): String = "Hello $name"

    fun calculate(a: Int, b: Int): Int = a + b
}

interface Repository<T> {
    fun save(entity: T)
}

object Logger {
    fun log(msg: String) {}
}

fun topLevelHelper(x: Int): Int = x * 2
"#
            .to_string(),
        };

        let result = parse_file(config).unwrap();

        // Class detection (UserService).
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.name == "UserService"),
            "expected UserService class symbol; got {:?}",
            result.symbols
        );

        // Top-level functions.
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.name == "topLevelHelper" && s.kind == SymbolKind::Function),
            "expected topLevelHelper function symbol"
        );

        // Object declarations register as Class on the canvas.
        assert!(
            result.symbols.iter().any(|s| s.name == "Logger"),
            "expected Logger object symbol"
        );
    }

    #[test]
    #[ignore]
    fn debug_dump_import_header() {
        use tree_sitter::Parser;
        let source = "import com.acme.models.User\n";
        let mut parser = Parser::new();
        parser
            .set_language(&Language::Kotlin.tree_sitter_language())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        fn walk(n: tree_sitter::Node, src: &str, depth: usize) {
            let indent = "  ".repeat(depth);
            let txt = src[n.byte_range()].chars().take(40).collect::<String>();
            println!("{indent}{} [{}..{}]: {txt:?}", n.kind(), n.start_byte(), n.end_byte());
            let mut c = n.walk();
            for ch in n.children(&mut c) {
                walk(ch, src, depth + 1);
            }
        }
        walk(tree.root_node(), source, 0);
    }

    #[test]
    fn test_parse_kotlin_imports() {
        let config = ParseConfig {
            file_path: PathBuf::from("App.kt"),
            language: Language::Kotlin,
            content: r#"
package com.acme.app

import com.acme.models.User
import com.acme.models.Post
import kotlinx.coroutines.*
import com.acme.util.Helper as H

class App
"#
            .to_string(),
        };

        let result = parse_file(config).unwrap();

        assert!(
            result.imports.iter().any(|i| i.module == "com.acme.models"
                && i.items.contains(&"User".to_string())),
            "expected User import; got {:?}",
            result.imports
        );
        assert!(result
            .imports
            .iter()
            .any(|i| i.module == "com.acme.models" && i.items.contains(&"Post".to_string())));
        // Wildcard imports surface as bare module.
        assert!(result
            .imports
            .iter()
            .any(|i| i.module == "kotlinx.coroutines" && i.items.is_empty()));
    }
}
