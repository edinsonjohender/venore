//! Example: Depth Levels Comparison
//!
//! Demonstrates the 4 depth levels (Minimal, Normal, Detailed, Expert)
//! and their impact on token usage and content detail.
//!
//! Usage:
//! ```bash
//! $env:GEMINI_API_KEY="your-key"
//! cargo run --example test_depth_levels
//! ```

use venore_core::llm::prelude::*;
use venore_core::context::{
    ContextWriter, ContextMetadata, ModuleIdentity, ModuleType, ModuleStatus,
    Layer, LayerType, LayerStatus, Dependencies, GenerationMetadata,
    calculate_code_hash, DepthLevel,
};
use venore_core::context::prompts::ContextPromptBuilder;
use venore_core::infrastructure::config::KeyringApiKeyStore;
use venore_core::traits::ApiKeyStore;
use std::path::Path;
use std::env;
use std::fs;
use chrono::Utc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    venore_core::utils::init_logger(tracing::Level::INFO);

    println!("\n================================================================================");
    println!("DEPTH LEVELS COMPARISON");
    println!("================================================================================\n");

    // Sample code to analyze
    let code = r#"
import React, { useState, useCallback } from 'react';

interface ButtonProps {
    label: string;
    onClick?: () => void;
    variant?: 'primary' | 'secondary';
    disabled?: boolean;
}

/**
 * Reusable button component with multiple variants
 */
export const Button: React.FC<ButtonProps> = ({
    label,
    onClick,
    variant = 'primary',
    disabled = false
}) => {
    const [isPressed, setIsPressed] = useState(false);

    const handleClick = useCallback(() => {
        if (!disabled && onClick) {
            setIsPressed(true);
            onClick();
            setTimeout(() => setIsPressed(false), 200);
        }
    }, [disabled, onClick]);

    const className = `btn btn-${variant} ${isPressed ? 'pressed' : ''} ${disabled ? 'disabled' : ''}`;

    return (
        <button
            className={className}
            onClick={handleClick}
            disabled={disabled}
            aria-label={label}
        >
            {label}
        </button>
    );
};

export default Button;
"#;

    let file_path = Path::new("components/Button.tsx");

    // Get API key
    let api_key = env::var("GEMINI_API_KEY")
        .or_else(|_| env::var("ANTHROPIC_API_KEY"))
        .map_err(|_| "No API key found. Set GEMINI_API_KEY or ANTHROPIC_API_KEY")?;

    let provider = if env::var("GEMINI_API_KEY").is_ok() {
        LlmProviderType::Gemini
    } else {
        LlmProviderType::Anthropic
    };

    println!("Provider: {}", provider.as_str());
    println!("Code size: {} bytes\n", code.len());

    // Setup LLM
    let keyring = KeyringApiKeyStore::new();
    keyring.store_api_key(provider, api_key).await?;
    let gateway = LlmGateway::new(Box::new(keyring));

    let model = match provider {
        LlmProviderType::Gemini => "gemini-2.5-flash".to_string(),
        LlmProviderType::Anthropic => "claude-sonnet-4-5".to_string(),
        _ => "gpt-4.1".to_string(),
    };

    // Test all 4 depth levels
    let depths = [
        DepthLevel::Minimal,
        DepthLevel::Normal,
        DepthLevel::Detailed,
        DepthLevel::Expert,
    ];

    for (i, depth) in depths.iter().enumerate() {
        println!("================================================================================");
        println!("DEPTH LEVEL {}/{}: {:?}", i + 1, depths.len(), depth);
        println!("================================================================================\n");

        // Build prompt for this depth level
        let prompt = ContextPromptBuilder::build_prompt(file_path, code, *depth);
        println!("Prompt size: {} chars", prompt.len());

        // Call LLM
        let request = LlmRequest {
            model: model.clone(),
            messages: vec![
                LlmMessage {
                    role: MessageRole::System,
                    content: "You are a code documentation expert.".to_string(),
                    tool_call_id: None,
                    tool_calls: None,
                    content_parts: None,
                },
                LlmMessage {
                    role: MessageRole::User,
                    content: prompt,
                    tool_call_id: None,
                    tool_calls: None,
                    content_parts: None,
                },
            ],
            temperature: Some(0.3),
            max_tokens: Some(match depth {
                DepthLevel::Minimal => 1000,
                DepthLevel::Normal => 3000,
                DepthLevel::Detailed => 5000,
                DepthLevel::Expert => 8000,
            }),
            tools: None,
            json_schema: None,
            timeout_secs: Some(90),
            web_search: false,
        };

        let options = GatewayOptions::for_task(LlmTask::Analysis)
            .with_provider(provider);

        println!("Calling LLM API...");
        let start = std::time::Instant::now();
        let response = gateway.complete(request, options).await?;
        let elapsed = start.elapsed();

        // Display results
        let tokens = response.usage.as_ref().map(|u| u.total_tokens).unwrap_or(0);
        let output_size = response.content.len();

        println!("\n✅ Response received:");
        println!("  Tokens used: {}", tokens);
        println!("  Output size: {} chars", output_size);
        println!("  Time taken: {:.2}s", elapsed.as_secs_f32());

        // Cost estimation (GPT-4o pricing)
        let cost = (tokens as f64) * 0.000015; // Simplified pricing
        println!("  Est. cost: ${:.4}", cost);

        // Show first 200 chars of output
        println!("\n📄 Output preview (first 200 chars):");
        let preview = if output_size > 200 {
            format!("{}...", &response.content[..200])
        } else {
            response.content.clone()
        };
        println!("{}", preview);

        // Write to file for inspection
        let metadata = ContextMetadata {
            identity: ModuleIdentity {
                name: "Button".to_string(),
                module_type: ModuleType::Component,
                status: ModuleStatus::Stable,
                owner: None,
                tags: vec!["ui".to_string(), "react".to_string()],
            },
            layers: vec![Layer {
                layer_type: LayerType::Context,
                status: LayerStatus::Complete,
                coverage: Some(100),
                notes: Some(format!("Generated with depth: {:?}", depth)),
            }],
            connections: vec![],
            dependencies: Dependencies {
                internal: vec![],
                external: vec!["react@^18.0.0".to_string()],
                optional: vec![],
            },
            agent_context: None,
            generation: GenerationMetadata {
                analyzed_at: Utc::now().to_rfc3339(),
                agent: "venore-context-agent-v3".to_string(),
                model: model.clone(),
                provider: provider.as_str().to_string(),
                code_hash: Some(calculate_code_hash(code)),
                stale: false,
                tokens_used: Some(tokens),
                generation_time_ms: Some(elapsed.as_millis() as u64),
            },
        };

        let writer = ContextWriter::new();
        let output_path = writer.write(file_path, &response, &metadata)?;

        // Rename to include depth level
        let new_path = output_path.with_file_name(
            format!("Button.{:?}.context.md", depth).to_lowercase()
        );
        fs::rename(&output_path, &new_path)?;

        println!("\n💾 Saved to: {}", new_path.display());
        println!();
    }

    println!("\n================================================================================");
    println!("COMPARISON SUMMARY");
    println!("================================================================================\n");

    println!("All 4 depth levels generated successfully!");
    println!("\nDepth Level Characteristics:");
    println!("  Minimal  → ~500-1K tokens  | Quick overview, no code examples");
    println!("  Normal   → ~1.5-2.5K tokens | Standard depth, 1 code example (DEFAULT)");
    println!("  Detailed → ~3-4K tokens    | Comprehensive, 3 code examples");
    println!("  Expert   → ~5-8K tokens    | Maximum depth, 5 examples, line-by-line");

    println!("\n💡 Tip: Review the generated files to see the differences:");
    println!("  - Button.minimal.context.md");
    println!("  - Button.normal.context.md");
    println!("  - Button.detailed.context.md");
    println!("  - Button.expert.context.md");

    Ok(())
}
