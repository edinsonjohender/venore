//! Java-specific AST extraction
//!
//! Handles: import_declaration

use tree_sitter::Node;
use super::Import;
use super::extractors::get_node_text;

/// Extract Java import declaration
pub fn extract_import(node: Node, source: &str) -> Option<Import> {
    // `import java.util.HashMap;` or `import static org.junit.Assert.*;`
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "scoped_identifier" {
            let text = get_node_text(child, source)?;
            // Split at last dot: module = "java.util", item = "HashMap"
            if let Some(dot_pos) = text.rfind('.') {
                let module = text[..dot_pos].to_string();
                let item = text[dot_pos + 1..].to_string();
                let items = if item == "*" { vec![] } else { vec![item] };
                return Some(Import {
                    module,
                    items,
                    line: node.start_position().row + 1,
                });
            } else {
                return Some(Import {
                    module: text,
                    items: vec![],
                    line: node.start_position().row + 1,
                });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn test_parse_java_classes_and_methods() {
        let config = ParseConfig {
            file_path: PathBuf::from("Test.java"),
            language: Language::Java,
            content: r#"
public class UserService {
    public String greet(String name) {
        return "Hello " + name;
    }

    private int calculate(int a, int b) {
        return a + b;
    }
}

interface Repository {
    void save(Object entity);
}

enum Status {
    ACTIVE,
    INACTIVE
}
"#.to_string(),
        };

        let result = parse_file(config).unwrap();

        assert!(result.symbols.iter().any(|s| s.name == "UserService" && s.kind == SymbolKind::Class));
        assert!(result.symbols.iter().any(|s| s.name == "greet" && s.kind == SymbolKind::Method));
        assert!(result.symbols.iter().any(|s| s.name == "calculate" && s.kind == SymbolKind::Method));
        assert!(result.symbols.iter().any(|s| s.name == "Repository" && s.kind == SymbolKind::Interface));
        assert!(result.symbols.iter().any(|s| s.name == "Status" && s.kind == SymbolKind::Enum));
    }

    #[test]
    fn test_parse_java_imports() {
        let config = ParseConfig {
            file_path: PathBuf::from("Test.java"),
            language: Language::Java,
            content: r#"
import java.util.HashMap;
import java.util.List;
import org.springframework.beans.factory.annotation.Autowired;

public class App {}
"#.to_string(),
        };

        let result = parse_file(config).unwrap();

        assert!(result.imports.iter().any(|i| i.module == "java.util" && i.items.contains(&"HashMap".to_string())));
        assert!(result.imports.iter().any(|i| i.module == "java.util" && i.items.contains(&"List".to_string())));
        assert!(result.imports.iter().any(|i| i.module.contains("annotation")));
    }
}
