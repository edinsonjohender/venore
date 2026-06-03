//! PHP-specific AST extraction
//!
//! Handles: namespace_use_declaration (`use Foo\Bar;`,
//! `use Foo\{Bar, Baz};`, `use function Foo\bar;`).

use tree_sitter::Node;

use super::extractors::get_node_text;
use super::Import;

/// Extract a PHP `use ...;` declaration.
///
/// Examples:
/// - `use App\Models\User;` → module `App\Models`, items `[User]`
/// - `use App\Models\{User, Post};` → module `App\Models`, items `[User, Post]`
/// - `use function App\Helpers\generate_id;` → module `App\Helpers`,
///   items `[generate_id]`
///
/// Returns `None` if the node shape doesn't include a parseable
/// namespace clause.
pub fn extract_import(node: Node, source: &str) -> Option<Import> {
    let line = node.start_position().row + 1;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            // `use Foo\Bar\Baz;` — a single namespace path.
            "namespace_use_clause" => {
                if let Some(import) = parse_use_clause(child, source, line) {
                    return Some(import);
                }
            }
            // `use Foo\Bar\{Baz, Qux};` — group form.
            "namespace_use_group" => {
                if let Some(import) = parse_use_group(child, source, line) {
                    return Some(import);
                }
            }
            _ => {}
        }
    }

    None
}

/// Parse a single `use Foo\Bar\Baz [as Alias];` clause.
fn parse_use_clause(node: Node, source: &str, line: usize) -> Option<Import> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "qualified_name" || kind == "name" {
            let text = get_node_text(child, source)?;
            return Some(split_qualified_name(&text, line));
        }
    }
    None
}

/// Parse `use Prefix\{Foo, Bar};` — emit one import row whose `module`
/// is the prefix and whose `items` are the brace-grouped names.
fn parse_use_group(node: Node, source: &str, line: usize) -> Option<Import> {
    let mut cursor = node.walk();

    let mut prefix: Option<String> = None;
    let mut items: Vec<String> = Vec::new();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "qualified_name" | "name" if prefix.is_none() => {
                prefix = get_node_text(child, source);
            }
            "namespace_use_group_clause" => {
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    if inner.kind() == "qualified_name" || inner.kind() == "name" {
                        if let Some(name) = get_node_text(inner, source) {
                            // Group clauses can also be prefixed (rare);
                            // collect the trailing identifier as the
                            // imported item.
                            let item = name
                                .rsplit('\\')
                                .next()
                                .unwrap_or(&name)
                                .to_string();
                            items.push(item);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let module = prefix?.trim_end_matches('\\').to_string();
    Some(Import { module, items, line })
}

/// Split `Foo\Bar\Baz` into module `Foo\Bar` and items `[Baz]`.
fn split_qualified_name(text: &str, line: usize) -> Import {
    // Trim leading backslash that marks an absolute namespace.
    let cleaned = text.trim_start_matches('\\');

    if let Some(idx) = cleaned.rfind('\\') {
        let module = cleaned[..idx].to_string();
        let last = cleaned[idx + 1..].to_string();
        let items = if last.is_empty() { vec![] } else { vec![last] };
        Import { module, items, line }
    } else {
        Import {
            module: cleaned.to_string(),
            items: vec![],
            line,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_php_classes_and_methods() {
        let config = ParseConfig {
            file_path: PathBuf::from("UserService.php"),
            language: Language::Php,
            content: r#"<?php

namespace App\Services;

class UserService {
    public function greet(string $name): string {
        return "Hello " . $name;
    }

    private function calculate(int $a, int $b): int {
        return $a + $b;
    }
}

interface Repository {
    public function save(object $entity): void;
}

trait Loggable {
    public function log(string $msg): void {}
}

enum Status {
    case Active;
    case Inactive;
}
"#
            .to_string(),
        };

        let result = parse_file(config).unwrap();

        assert!(result
            .symbols
            .iter()
            .any(|s| s.name == "UserService" && s.kind == SymbolKind::Class));
        assert!(result
            .symbols
            .iter()
            .any(|s| s.name == "Repository" && s.kind == SymbolKind::Interface));
        assert!(result
            .symbols
            .iter()
            .any(|s| s.name == "Loggable" && s.kind == SymbolKind::Trait));
        assert!(result
            .symbols
            .iter()
            .any(|s| s.name == "Status" && s.kind == SymbolKind::Enum));
        assert!(result
            .symbols
            .iter()
            .any(|s| s.name == "greet" && s.kind == SymbolKind::Method));
    }

    #[test]
    fn test_parse_php_imports() {
        let config = ParseConfig {
            file_path: PathBuf::from("App.php"),
            language: Language::Php,
            content: r#"<?php

namespace App\Http\Controllers;

use App\Models\User;
use App\Models\Post;
use Illuminate\Http\Request;

class HomeController {}
"#
            .to_string(),
        };

        let result = parse_file(config).unwrap();

        assert!(result.imports.iter().any(|i| {
            i.module == "App\\Models" && i.items.contains(&"User".to_string())
        }));
        assert!(result.imports.iter().any(|i| {
            i.module == "App\\Models" && i.items.contains(&"Post".to_string())
        }));
        assert!(result
            .imports
            .iter()
            .any(|i| i.module.contains("Illuminate")));
    }
}
