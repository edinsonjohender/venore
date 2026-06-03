//! C#-specific AST extraction
//!
//! Handles: using_directive

use tree_sitter::Node;
use super::Import;
use super::extractors::get_node_text;

/// Extract C# `using` directive as an import
pub fn extract_using(node: Node, source: &str) -> Option<Import> {
    // `using System.Collections.Generic;`
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        // The namespace is usually a qualified_name or identifier
        if child.kind() == "qualified_name" || child.kind() == "identifier" {
            let module = get_node_text(child, source)?;
            return Some(Import {
                module,
                items: vec![],
                line: node.start_position().row + 1,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn test_parse_csharp_classes_and_methods() {
        let config = ParseConfig {
            file_path: PathBuf::from("Test.cs"),
            language: Language::CSharp,
            content: r#"
public class UserService
{
    public string Greet(string name)
    {
        return "Hello " + name;
    }

    private int Calculate(int a, int b)
    {
        return a + b;
    }
}

public interface IRepository
{
    void Save(object entity);
}

public enum Status
{
    Active,
    Inactive
}

public struct Point
{
    public int X;
    public int Y;
}
"#.to_string(),
        };

        let result = parse_file(config).unwrap();

        assert!(result.symbols.iter().any(|s| s.name == "UserService" && s.kind == SymbolKind::Class));
        assert!(result.symbols.iter().any(|s| s.name == "Greet" && s.kind == SymbolKind::Method));
        assert!(result.symbols.iter().any(|s| s.name == "Calculate" && s.kind == SymbolKind::Method));
        assert!(result.symbols.iter().any(|s| s.name == "IRepository" && s.kind == SymbolKind::Interface));
        assert!(result.symbols.iter().any(|s| s.name == "Status" && s.kind == SymbolKind::Enum));
        assert!(result.symbols.iter().any(|s| s.name == "Point" && s.kind == SymbolKind::Struct));
    }

    #[test]
    fn test_parse_csharp_usings() {
        let config = ParseConfig {
            file_path: PathBuf::from("Test.cs"),
            language: Language::CSharp,
            content: r#"
using System;
using System.Collections.Generic;
using Microsoft.AspNetCore.Mvc;

public class App {}
"#.to_string(),
        };

        let result = parse_file(config).unwrap();

        assert!(result.imports.iter().any(|i| i.module == "System"));
        assert!(result.imports.iter().any(|i| i.module == "System.Collections.Generic"));
        assert!(result.imports.iter().any(|i| i.module == "Microsoft.AspNetCore.Mvc"));
    }
}
