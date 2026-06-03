//! Go-specific AST extraction
//!
//! Handles: import_declaration (grouped), type_spec disambiguation, uppercase exports

use tree_sitter::Node;
use super::{Import, Export, Symbol, SymbolKind};
use super::extractors::get_node_text;
use super::lang_config::NodeMapping;

/// Extract Go imports (supports grouped imports)
pub fn extract_imports(node: Node, source: &str, imports: &mut Vec<Import>) {
    // Go imports can be:
    //   import "fmt"
    //   import (
    //       "fmt"
    //       "os"
    //   )
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "import_spec" => {
                if let Some(import) = extract_import_spec(child, source) {
                    imports.push(import);
                }
            }
            "import_spec_list" => {
                let mut inner_cursor = child.walk();
                for spec in child.children(&mut inner_cursor) {
                    if spec.kind() == "import_spec" {
                        if let Some(import) = extract_import_spec(spec, source) {
                            imports.push(import);
                        }
                    }
                }
            }
            "interpreted_string_literal" => {
                // Single import: `import "fmt"`
                if let Some(text) = get_node_text(child, source) {
                    let module = text.trim_matches('"').to_string();
                    if !module.is_empty() {
                        imports.push(Import {
                            module,
                            items: vec![],
                            line: child.start_position().row + 1,
                        });
                    }
                }
            }
            _ => {}
        }
    }
}

/// Extract a single import spec
fn extract_import_spec(node: Node, source: &str) -> Option<Import> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "interpreted_string_literal" {
            let text = get_node_text(child, source)?;
            let module = text.trim_matches('"').to_string();
            if !module.is_empty() {
                return Some(Import {
                    module,
                    items: vec![],
                    line: node.start_position().row + 1,
                });
            }
        }
    }
    None
}

/// Disambiguate Go `type_spec` into struct, interface, or type alias
pub fn extract_type_spec(node: Node, source: &str) -> Option<Symbol> {
    let name_node = node.child_by_field_name("name")?;
    let name = get_node_text(name_node, source)?;

    // The type child tells us what kind it is
    let type_node = node.child_by_field_name("type")?;
    let kind = match type_node.kind() {
        "struct_type" => SymbolKind::Struct,
        "interface_type" => SymbolKind::Interface,
        _ => SymbolKind::Type,
    };

    Some(Symbol {
        name,
        kind,
        line_start: node.start_position().row + 1,
        line_end: node.end_position().row + 1,
        signature: None,
    })
}

/// In Go, names starting with an uppercase letter are exported
pub fn extract_exported_symbol(node: Node, source: &str, mapping: &NodeMapping, exports: &mut Vec<Export>) {
    let kind = node.kind();

    // Check symbols from the mapping
    let symbol_kind = if let Some(sk) = mapping.symbol_kind_for(kind) {
        sk
    } else if kind == "type_spec" {
        // type_spec — peek at the type child
        if let Some(type_node) = node.child_by_field_name("type") {
            match type_node.kind() {
                "struct_type" => SymbolKind::Struct,
                "interface_type" => SymbolKind::Interface,
                _ => SymbolKind::Type,
            }
        } else {
            return;
        }
    } else {
        return;
    };

    if let Some(name_node) = node.child_by_field_name("name") {
        if let Some(name) = get_node_text(name_node, source) {
            if name.starts_with(|c: char| c.is_uppercase()) {
                exports.push(Export {
                    name,
                    kind: symbol_kind,
                    line: node.start_position().row + 1,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn test_parse_go_functions_and_types() {
        let config = ParseConfig {
            file_path: PathBuf::from("main.go"),
            language: Language::Go,
            content: r#"package main

func greet(name string) string {
    return "Hello " + name
}

type User struct {
    Name string
    Age  int
}

type Reader interface {
    Read(p []byte) (n int, err error)
}

func (u *User) String() string {
    return u.Name
}
"#.to_string(),
        };

        let result = parse_file(config).unwrap();

        assert!(result.symbols.iter().any(|s| s.name == "greet" && s.kind == SymbolKind::Function));
        assert!(result.symbols.iter().any(|s| s.name == "User" && s.kind == SymbolKind::Struct));
        assert!(result.symbols.iter().any(|s| s.name == "Reader" && s.kind == SymbolKind::Interface));
        assert!(result.symbols.iter().any(|s| s.name == "String" && s.kind == SymbolKind::Method));
    }

    #[test]
    fn test_parse_go_imports() {
        let config = ParseConfig {
            file_path: PathBuf::from("main.go"),
            language: Language::Go,
            content: r#"package main

import (
    "fmt"
    "os"
    "net/http"
)

func main() {
    fmt.Println("hello")
}
"#.to_string(),
        };

        let result = parse_file(config).unwrap();

        assert!(result.imports.iter().any(|i| i.module == "fmt"));
        assert!(result.imports.iter().any(|i| i.module == "os"));
        assert!(result.imports.iter().any(|i| i.module == "net/http"));
    }

    #[test]
    fn test_parse_go_exports() {
        let config = ParseConfig {
            file_path: PathBuf::from("main.go"),
            language: Language::Go,
            content: r#"package main

func PublicFunc() {}

func privateFunc() {}

type PublicStruct struct {}

type privateStruct struct {}
"#.to_string(),
        };

        let result = parse_file(config).unwrap();

        assert!(result.exports.iter().any(|e| e.name == "PublicFunc"));
        assert!(result.exports.iter().any(|e| e.name == "PublicStruct"));
        assert!(!result.exports.iter().any(|e| e.name == "privateFunc"));
        assert!(!result.exports.iter().any(|e| e.name == "privateStruct"));
    }
}
