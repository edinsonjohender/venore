//! C-specific AST extraction.
//!
//! Handles `#include "header.h"` (user header) and
//! `#include <header.h>` (system header). The C++ grammar emits the
//! same `preproc_include` shape, so `cpp_lang.rs` delegates here.

use tree_sitter::Node;

use super::extractors::get_node_text;
use super::Import;

/// Extract a `#include` directive.
///
/// The `path` field of `preproc_include` is either:
/// - a `string_literal` (`"foo.h"`) — quotes are stripped
/// - a `system_lib_string` (`<foo.h>`) — `<>` are stripped
pub fn extract_include(node: Node, source: &str) -> Option<Import> {
    let line = node.start_position().row + 1;

    let path_node = match node.child_by_field_name("path") {
        Some(n) => n,
        None => {
            // Some grammar versions surface the path as a direct child
            // without a named field. Fall through to a walk.
            let mut cursor = node.walk();
            let mut found = None;
            for child in node.children(&mut cursor) {
                if matches!(child.kind(), "string_literal" | "system_lib_string") {
                    found = Some(child);
                    break;
                }
            }
            found?
        }
    };

    let raw = get_node_text(path_node, source)?;
    let cleaned = raw
        .trim_matches(|c: char| matches!(c, '"' | '<' | '>'))
        .to_string();

    Some(Import {
        module: cleaned,
        items: vec![],
        line,
    })
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_c_functions_and_structs() {
        let config = ParseConfig {
            file_path: PathBuf::from("main.c"),
            language: Language::C,
            content: r#"
#include <stdio.h>
#include "config.h"

struct Point {
    int x;
    int y;
};

enum Status {
    OK,
    ERR
};

int add(int a, int b) {
    return a + b;
}

int main(void) {
    return 0;
}
"#
            .to_string(),
        };

        let result = parse_file(config).unwrap();

        assert!(result
            .symbols
            .iter()
            .any(|s| s.name == "Point" && s.kind == SymbolKind::Struct));
        assert!(result
            .symbols
            .iter()
            .any(|s| s.name == "Status" && s.kind == SymbolKind::Enum));
        assert!(result
            .symbols
            .iter()
            .any(|s| s.name == "add" && s.kind == SymbolKind::Function));
        assert!(result
            .symbols
            .iter()
            .any(|s| s.name == "main" && s.kind == SymbolKind::Function));
    }

    #[test]
    fn test_parse_c_includes() {
        let config = ParseConfig {
            file_path: PathBuf::from("main.c"),
            language: Language::C,
            content: r#"
#include <stdio.h>
#include <stdlib.h>
#include "myapp/config.h"
#include "lib/util.h"

int main(void) { return 0; }
"#
            .to_string(),
        };

        let result = parse_file(config).unwrap();

        assert!(result.imports.iter().any(|i| i.module == "stdio.h"));
        assert!(result.imports.iter().any(|i| i.module == "stdlib.h"));
        assert!(result.imports.iter().any(|i| i.module == "myapp/config.h"));
        assert!(result.imports.iter().any(|i| i.module == "lib/util.h"));
    }
}
