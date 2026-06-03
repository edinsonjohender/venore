//! C++-specific AST extraction.
//!
//! Tree-sitter-cpp produces the same `preproc_include` shape as
//! tree-sitter-c, so we delegate to the C extractor. Class / namespace
//! / template symbol extraction goes through the generic symbol path
//! driven by `lang_config::CPP_MAPPING`.

use tree_sitter::Node;

use super::c_lang;
use super::Import;

pub fn extract_include(node: Node, source: &str) -> Option<Import> {
    c_lang::extract_include(node, source)
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_cpp_classes_and_namespaces() {
        let config = ParseConfig {
            file_path: PathBuf::from("widget.cpp"),
            language: Language::Cpp,
            content: r#"
#include <string>
#include "widget.h"

namespace acme {

class Widget {
public:
    Widget(int id);
    std::string greet(const std::string& name);

private:
    int id_;
};

struct Point {
    int x;
    int y;
};

enum class Status {
    Ok,
    Err
};

int free_function(int a, int b) {
    return a + b;
}

}  // namespace acme
"#
            .to_string(),
        };

        let result = parse_file(config).unwrap();

        assert!(result
            .symbols
            .iter()
            .any(|s| s.name == "acme" && s.kind == SymbolKind::Class));
        assert!(result
            .symbols
            .iter()
            .any(|s| s.name == "Widget" && s.kind == SymbolKind::Class));
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
            .any(|s| s.name == "free_function" && s.kind == SymbolKind::Function));
    }

    #[test]
    fn test_parse_cpp_includes() {
        let config = ParseConfig {
            file_path: PathBuf::from("main.cpp"),
            language: Language::Cpp,
            content: r#"
#include <iostream>
#include <vector>
#include "app/config.h"

int main() { return 0; }
"#
            .to_string(),
        };

        let result = parse_file(config).unwrap();

        assert!(result.imports.iter().any(|i| i.module == "iostream"));
        assert!(result.imports.iter().any(|i| i.module == "vector"));
        assert!(result.imports.iter().any(|i| i.module == "app/config.h"));
    }
}
