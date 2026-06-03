//! LSP server configuration and language mapping
//!
//! Hardcoded configs for known LSP servers (typescript-language-server, rust-analyzer)
//! and extension-to-language-id mapping.

/// Configuration for an LSP server binary.
#[derive(Debug, Clone)]
pub struct LspServerConfig {
    /// Human-readable name (e.g. "typescript-language-server")
    pub name: String,
    /// Binary command to spawn
    pub command: String,
    /// Arguments to pass to the binary
    pub args: Vec<String>,
    /// LSP language IDs this server handles
    pub language_ids: Vec<String>,
}

/// Return a default LSP server config for a given language ID, if known.
pub fn default_config_for_language(language_id: &str) -> Option<LspServerConfig> {
    match language_id {
        "typescript" | "javascript" | "typescriptreact" | "javascriptreact" => {
            Some(LspServerConfig {
                name: "typescript-language-server".to_string(),
                command: "typescript-language-server".to_string(),
                args: vec!["--stdio".to_string()],
                language_ids: vec![
                    "typescript".to_string(),
                    "javascript".to_string(),
                    "typescriptreact".to_string(),
                    "javascriptreact".to_string(),
                ],
            })
        }
        "rust" => Some(LspServerConfig {
            name: "rust-analyzer".to_string(),
            command: "rust-analyzer".to_string(),
            args: vec![],
            language_ids: vec!["rust".to_string()],
        }),
        _ => None,
    }
}

/// Map a file extension to an LSP language ID.
pub fn extension_to_language_id(ext: &str) -> Option<&'static str> {
    match ext.to_lowercase().as_str() {
        "ts" => Some("typescript"),
        "tsx" => Some("typescriptreact"),
        "js" | "jsx" => Some("javascript"),
        "py" => Some("python"),
        "rs" => Some("rust"),
        "java" => Some("java"),
        "go" => Some("go"),
        "cs" => Some("csharp"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extension_to_language_id() {
        assert_eq!(extension_to_language_id("ts"), Some("typescript"));
        assert_eq!(extension_to_language_id("tsx"), Some("typescriptreact"));
        assert_eq!(extension_to_language_id("js"), Some("javascript"));
        assert_eq!(extension_to_language_id("jsx"), Some("javascript"));
        assert_eq!(extension_to_language_id("rs"), Some("rust"));
        assert_eq!(extension_to_language_id("py"), Some("python"));
        assert_eq!(extension_to_language_id("go"), Some("go"));
        assert_eq!(extension_to_language_id("cs"), Some("csharp"));
        assert_eq!(extension_to_language_id("unknown"), None);
    }

    #[test]
    fn test_default_config_for_typescript() {
        let config = default_config_for_language("typescript").unwrap();
        assert_eq!(config.command, "typescript-language-server");
        assert!(config.args.contains(&"--stdio".to_string()));
        assert!(config.language_ids.contains(&"typescript".to_string()));
    }

    #[test]
    fn test_default_config_for_rust() {
        let config = default_config_for_language("rust").unwrap();
        assert_eq!(config.command, "rust-analyzer");
    }

    #[test]
    fn test_default_config_for_unknown() {
        assert!(default_config_for_language("haskell").is_none());
    }

    #[test]
    fn test_tsx_maps_to_ts_server() {
        let config = default_config_for_language("typescriptreact").unwrap();
        assert_eq!(config.command, "typescript-language-server");
    }
}
