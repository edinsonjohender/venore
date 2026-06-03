//! Context Writer - Writes .context.md files to filesystem
//!
//! Takes LLM-generated content and writes it to `.context.md` files
//! with proper formatting, metadata, and cross-platform path handling.

use crate::error::{VenoreError, Result};
use crate::llm::prelude::LlmResponse;
use crate::context::types::ContextMetadata;
use crate::context::frontmatter::FrontmatterBuilder;
use crate::utils::path;
use std::path::{Path, PathBuf};
use tracing::{info, debug};

/// Options for writing context files
#[derive(Debug, Clone)]
pub struct WriteOptions {
    /// Create parent directories if they don't exist
    pub create_dirs: bool,

    /// Overwrite existing files
    pub overwrite: bool,

    /// Directory name for context files (default: ".context")
    pub context_dir_name: String,
}

impl Default for WriteOptions {
    fn default() -> Self {
        Self {
            create_dirs: true,
            overwrite: true,
            context_dir_name: ".context".to_string(),
        }
    }
}

/// Context Writer - Writes .context.md files
pub struct ContextWriter {
    options: WriteOptions,
}

impl ContextWriter {
    /// Create a new ContextWriter with default options
    pub fn new() -> Self {
        Self {
            options: WriteOptions::default(),
        }
    }

    /// Create a new ContextWriter with custom options
    pub fn with_options(options: WriteOptions) -> Self {
        Self { options }
    }

    /// Write a context file for a module
    ///
    /// # Arguments
    /// - `module_path`: Path to the source file (e.g., `src/components/Button/index.tsx`)
    /// - `llm_response`: Response from LLM with generated content
    /// - `metadata`: Additional metadata for the context file
    ///
    /// # Returns
    /// Path to the written context file
    ///
    /// # Errors
    /// - `FileWriteError`: Cannot create directory or write file
    /// - `PermissionDenied`: No write permissions
    /// - `InvalidPath`: Path is invalid or unsafe
    ///
    /// # Example
    /// ```ignore
    /// use venore_core::context::writer::{ContextWriter, ContextMetadata};
    /// use venore_core::llm::types::{LlmResponse, LlmProviderType, TokenUsage};
    /// use std::path::Path;
    ///
    /// let writer = ContextWriter::new();
    /// let module_path = Path::new("src/components/Button/index.tsx");
    ///
    /// let response = LlmResponse {
    ///     content: "# Button Component\n\nA reusable button...".to_string(),
    ///     provider: LlmProviderType::Anthropic,
    ///     model: "claude-sonnet-4-5".to_string(),
    ///     usage: Some(TokenUsage {
    ///         prompt_tokens: 500,
    ///         completion_tokens: 300,
    ///         total_tokens: 800,
    ///     }),
    /// };
    ///
    /// let metadata = ContextMetadata::default();
    /// // let output_path = writer.write(module_path, &response, &metadata)?;
    /// ```
    pub fn write(
        &self,
        module_path: &Path,
        llm_response: &LlmResponse,
        metadata: &ContextMetadata,
    ) -> Result<PathBuf> {
        // Validate input path
        if !module_path.exists() && !module_path.is_absolute() {
            debug!("Module path does not exist or is not absolute: {:?}", module_path);
        }

        // Determine output path
        let output_path = self.determine_output_path(module_path)?;

        info!(
            "Writing context file: {} (provider: {}, model: {})",
            output_path.display(),
            llm_response.provider.as_str(),
            llm_response.model
        );

        // Create parent directory if needed
        if self.options.create_dirs {
            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    VenoreError::FileWriteError(format!(
                        "Cannot create directory {}: {}",
                        parent.display(),
                        e
                    ))
                })?;
            }
        }

        // Check if file exists and overwrite option
        if output_path.exists() && !self.options.overwrite {
            return Err(VenoreError::FileWriteError(format!(
                "File already exists and overwrite is disabled: {}",
                output_path.display()
            )));
        }

        // Format content
        let formatted_content = self.format_content(llm_response, metadata)?;

        // Write file
        std::fs::write(&output_path, formatted_content).map_err(|e| {
            VenoreError::FileWriteError(format!(
                "Cannot write file {}: {}",
                output_path.display(),
                e
            ))
        })?;

        info!("✅ Context file written successfully: {}", output_path.display());

        Ok(output_path)
    }

    /// Determine output path for context file
    ///
    /// Example:
    /// - Input: `src/components/Button/index.tsx`
    /// - Output: `src/components/Button/.context/Button.context.md`
    fn determine_output_path(&self, module_path: &Path) -> Result<PathBuf> {
        // Get parent directory
        let parent_dir = module_path.parent().ok_or_else(|| {
            VenoreError::InvalidPath(format!(
                "Cannot determine parent directory for: {}",
                module_path.display()
            ))
        })?;

        // Get file stem (name without extension)
        let file_stem = path::get_file_name(module_path, false).ok_or_else(|| {
            VenoreError::InvalidPath(format!(
                "Cannot determine file name for: {}",
                module_path.display()
            ))
        })?;

        // Build output path: parent/.context/filename.context.md
        let context_dir = parent_dir.join(&self.options.context_dir_name);
        let context_filename = format!("{}.context.md", file_stem);
        let output_path = context_dir.join(context_filename);

        Ok(output_path)
    }

    /// Format content with metadata header and footer
    fn format_content(
        &self,
        llm_response: &LlmResponse,
        metadata: &ContextMetadata,
    ) -> Result<String> {
        let mut content = String::new();

        // Build frontmatter using FrontmatterBuilder
        let frontmatter = FrontmatterBuilder::build(metadata)?;
        content.push_str(&frontmatter);
        content.push('\n');

        // Main content from LLM
        content.push_str(&llm_response.content);

        // Ensure newline at end
        if !content.ends_with('\n') {
            content.push('\n');
        }

        // Footer
        content.push_str("\n---\n");
        content.push_str(&format!(
            "<!-- Generated by Venore v{} using {} -->\n",
            metadata.generation.agent,
            metadata.generation.model
        ));

        Ok(content)
    }
}

impl Default for ContextWriter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::types::*;
    use crate::llm::prelude::{LlmProviderType, TokenUsage};
    use tempfile::TempDir;
    use chrono::Utc;

    fn create_test_response() -> LlmResponse {
        LlmResponse {
            content: "# Test Module\n\nThis is a test context.".to_string(),
            tool_calls: None,
            provider: LlmProviderType::Anthropic,
            model: "claude-sonnet-4-5".to_string(),
            usage: Some(TokenUsage {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
            }),
            sources: Vec::new(),
        }
    }

    fn create_test_metadata() -> ContextMetadata {
        ContextMetadata {
            identity: ModuleIdentity {
                name: "TestModule".to_string(),
                module_type: ModuleType::Component,
                status: ModuleStatus::Stable,
                owner: None,
                tags: vec!["test".to_string()],
            },
            layers: vec![],
            connections: vec![],
            dependencies: Dependencies::default(),
            agent_context: None,
            generation: GenerationMetadata {
                analyzed_at: Utc::now().to_rfc3339(),
                agent: "venore-context-agent-v3".to_string(),
                model: "claude-sonnet-4-5".to_string(),
                provider: "anthropic".to_string(),
                code_hash: None,
                stale: false,
                tokens_used: Some(150),
                generation_time_ms: None,
            },
        }
    }

    #[test]
    fn test_determine_output_path() {
        let writer = ContextWriter::new();
        let input = Path::new("src/components/Button/index.tsx");

        let output = writer.determine_output_path(input).unwrap();

        assert_eq!(
            output,
            PathBuf::from("src/components/Button/.context/index.context.md")
        );
    }

    #[test]
    fn test_format_content() {
        let writer = ContextWriter::new();
        let response = create_test_response();
        let metadata = create_test_metadata();

        let formatted = writer.format_content(&response, &metadata).unwrap();

        // Check header (V2 frontmatter)
        assert!(formatted.contains("---"));
        assert!(formatted.contains("name: \"TestModule\""));
        assert!(formatted.contains("type: component"));
        assert!(formatted.contains("status: stable"));
        assert!(formatted.contains("provider: \"anthropic\""));
        assert!(formatted.contains("model: \"claude-sonnet-4-5\""));

        // Check content
        assert!(formatted.contains("# Test Module"));
        assert!(formatted.contains("This is a test context."));

        // Check footer
        assert!(formatted.contains("Generated by Venore"));
    }

    #[test]
    fn test_write_creates_directory() {
        let temp_dir = TempDir::new().unwrap();
        let module_path = temp_dir.path().join("src/components/Button/index.tsx");

        // Create the source file
        std::fs::create_dir_all(module_path.parent().unwrap()).unwrap();
        std::fs::write(&module_path, "export const Button = () => {};").unwrap();

        let writer = ContextWriter::new();
        let response = create_test_response();
        let metadata = create_test_metadata();

        let output_path = writer.write(&module_path, &response, &metadata).unwrap();

        // Check that file was created
        assert!(output_path.exists());

        // Check that it's in the right place
        assert_eq!(
            output_path.parent().unwrap().file_name().unwrap(),
            ".context"
        );

        // Check content (V2 format)
        let content = std::fs::read_to_string(&output_path).unwrap();
        assert!(content.contains("# Test Module"));
        assert!(content.contains("name: \"TestModule\""));
        assert!(content.contains("type: component"));
    }

    #[test]
    fn test_write_overwrites_existing() {
        let temp_dir = TempDir::new().unwrap();
        let module_path = temp_dir.path().join("src/test.rs");

        // Create the source file
        std::fs::create_dir_all(module_path.parent().unwrap()).unwrap();
        std::fs::write(&module_path, "fn test() {}").unwrap();

        let writer = ContextWriter::new();
        let response = create_test_response();
        let metadata = create_test_metadata();

        // Write first time
        let output_path = writer.write(&module_path, &response, &metadata).unwrap();
        let first_content = std::fs::read_to_string(&output_path).unwrap();

        // Write second time with different content
        let mut response2 = response;
        response2.content = "# Updated Content".to_string();

        writer.write(&module_path, &response2, &metadata).unwrap();
        let second_content = std::fs::read_to_string(&output_path).unwrap();

        // Content should be different
        assert_ne!(first_content, second_content);
        assert!(second_content.contains("# Updated Content"));
    }

    #[test]
    fn test_write_respects_no_overwrite_option() {
        let temp_dir = TempDir::new().unwrap();
        let module_path = temp_dir.path().join("src/test.rs");

        // Create the source file
        std::fs::create_dir_all(module_path.parent().unwrap()).unwrap();
        std::fs::write(&module_path, "fn test() {}").unwrap();

        let options = WriteOptions {
            overwrite: false,
            ..Default::default()
        };
        let writer = ContextWriter::with_options(options);
        let response = create_test_response();
        let metadata = create_test_metadata();

        // Write first time - should succeed
        writer.write(&module_path, &response, &metadata).unwrap();

        // Write second time - should fail
        let result = writer.write(&module_path, &response, &metadata);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), VenoreError::FileWriteError(_)));
    }
}
