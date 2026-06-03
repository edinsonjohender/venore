//! Ruby-specific AST extraction
//!
//! Handles: `require 'foo'`, `require_relative 'foo'`, `load 'foo'`.
//!
//! In tree-sitter-ruby these are plain `call` nodes — there is no
//! dedicated import-statement variant. We route every `call` through
//! this extractor and reject the ones whose method name isn't one of
//! the import-style identifiers, so the per-language dispatch in
//! `mod.rs` only has to know "Ruby imports live in call nodes".

use tree_sitter::Node;

use super::extractors::get_node_text;
use super::Import;

const IMPORT_METHODS: &[&str] = &["require", "require_relative", "load", "autoload"];

/// Extract a Ruby import-like `call` (require / require_relative / load).
///
/// Returns `None` if the node is a regular method call.
pub fn extract_import(node: Node, source: &str) -> Option<Import> {
    let line = node.start_position().row + 1;

    let method_name = find_method_name(node, source)?;
    if !IMPORT_METHODS.contains(&method_name.as_str()) {
        return None;
    }

    let module = find_first_string_argument(node, source)?;

    Some(Import {
        module,
        items: vec![],
        line,
    })
}

/// Locate the method identifier of a `call` node. Tree-sitter-ruby
/// stores it as the `method` field of the call.
fn find_method_name(node: Node, source: &str) -> Option<String> {
    if let Some(method) = node.child_by_field_name("method") {
        return get_node_text(method, source);
    }
    // Fallback: first identifier child.
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            return get_node_text(child, source);
        }
    }
    None
}

/// Pull out the string argument of `require 'foo'`. Handles both
/// quoted-string literals (`"foo"` / `'foo'`) and unquoted bare
/// identifiers (rare). Returns the inner string content, no quotes.
fn find_first_string_argument(node: Node, source: &str) -> Option<String> {
    let arguments = node.child_by_field_name("arguments")?;

    let mut cursor = arguments.walk();
    for child in arguments.children(&mut cursor) {
        match child.kind() {
            "string" | "bare_string" => {
                let text = get_node_text(child, source)?;
                return Some(unquote(&text));
            }
            "symbol" => {
                let text = get_node_text(child, source)?;
                return Some(text.trim_start_matches(':').to_string());
            }
            _ => {}
        }
    }

    None
}

fn unquote(s: &str) -> String {
    s.trim_matches(|c| c == '"' || c == '\'').to_string()
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_ruby_classes_and_methods() {
        let config = ParseConfig {
            file_path: PathBuf::from("user_service.rb"),
            language: Language::Ruby,
            content: r#"
module Acme
  class UserService
    def greet(name)
      "Hello #{name}"
    end

    def self.create_default
      new
    end

    private

    def calculate(a, b)
      a + b
    end
  end
end
"#
            .to_string(),
        };

        let result = parse_file(config).unwrap();

        assert!(result
            .symbols
            .iter()
            .any(|s| s.name == "Acme" && s.kind == SymbolKind::Class));
        assert!(result
            .symbols
            .iter()
            .any(|s| s.name == "UserService" && s.kind == SymbolKind::Class));
        assert!(result
            .symbols
            .iter()
            .any(|s| s.name == "greet" && s.kind == SymbolKind::Method));
        assert!(result
            .symbols
            .iter()
            .any(|s| s.name == "create_default" && s.kind == SymbolKind::Method));
    }

    #[test]
    fn test_parse_ruby_requires() {
        let config = ParseConfig {
            file_path: PathBuf::from("app.rb"),
            language: Language::Ruby,
            content: r#"
require 'json'
require "net/http"
require_relative 'lib/helpers'

# Should NOT be treated as an import:
my_object.require_something('foo')

class App
end
"#
            .to_string(),
        };

        let result = parse_file(config).unwrap();

        assert!(result.imports.iter().any(|i| i.module == "json"));
        assert!(result.imports.iter().any(|i| i.module == "net/http"));
        assert!(result.imports.iter().any(|i| i.module == "lib/helpers"));
        // The unrelated `.require_something(...)` call is a method call
        // on a receiver — our extractor rejects it because the method
        // name isn't in IMPORT_METHODS.
        assert!(!result
            .imports
            .iter()
            .any(|i| i.module.contains("require_something")));
    }
}
