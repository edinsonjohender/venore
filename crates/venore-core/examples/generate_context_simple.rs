//! Simple E2E Example: Generate Context
//!
//! Usage:
//! ```bash
//! $env:GEMINI_API_KEY="your-key"
//! cargo run --example generate_context_simple -- "C:\path\to\file.tsx"
//! ```

use venore_core::llm::prelude::*;
use venore_core::context::{
    ContextWriter, ContextMetadata, ModuleIdentity, ModuleType, ModuleStatus,
    Layer, LayerType, LayerStatus, Dependencies, GenerationMetadata,
    calculate_code_hash,
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
    println!("E2E CONTEXT GENERATION");
    println!("================================================================================\n");

    // Get file path
    let args: Vec<String> = env::args().collect();
    let file_path = if args.len() > 1 {
        args[1].clone()
    } else {
        "C:\\Users\\Edinson\\Downloads\\excalidraw-master\\ex-1\\examples\\with-script-in-browser\\components\\CustomFooter.tsx".to_string()
    };

    let module_path = Path::new(&file_path);

    if !module_path.exists() {
        eprintln!("File not found: {}", module_path.display());
        return Ok(());
    }

    println!("Target file: {}", module_path.display());

    // Read file content
    let code = fs::read_to_string(module_path)?;
    println!("File size: {} bytes\n", code.len());

    // Get API key
    let api_key = env::var("GEMINI_API_KEY")
        .or_else(|_| env::var("ANTHROPIC_API_KEY"))
        .map_err(|_| "No API key found")?;

    let provider = if env::var("GEMINI_API_KEY").is_ok() {
        LlmProviderType::Gemini
    } else {
        LlmProviderType::Anthropic
    };

    println!("Provider: {}", provider.as_str());

    // Setup LLM
    let keyring = KeyringApiKeyStore::new();
    keyring.store_api_key(provider, api_key).await?;
    let gateway = LlmGateway::new(Box::new(keyring));

    // Build V2 prompt using ContextPromptBuilder
    println!("Building V2 prompt...");
    let prompt = ContextPromptBuilder::build_v2_prompt(module_path, &code);
    println!("Prompt built ({} chars)\n", prompt.len());

    println!("Calling LLM API...");

    // Call LLM
    let model = match provider {
        LlmProviderType::Gemini => "gemini-2.5-flash".to_string(),
        LlmProviderType::Anthropic => "claude-sonnet-4-5".to_string(),
        _ => "gpt-4.1".to_string(),
    };

    let request = LlmRequest {
        model: model.clone(),
        messages: vec![
            LlmMessage {
                role: MessageRole::System,
                content: "You are a code documentation expert. Generate comprehensive context documentation following the structure provided.".to_string(),
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
        max_tokens: Some(4000),  // Increased for V2 comprehensive output
        tools: None,
        json_schema: None,
        timeout_secs: Some(90),  // Increased timeout
        web_search: false,
    };

    let options = GatewayOptions::for_task(LlmTask::Analysis)
        .with_provider(provider);

    let response = gateway.complete(request, options).await?;

    println!("LLM response received:");
    println!("  Model: {}", response.model);
    if let Some(usage) = &response.usage {
        println!("  Tokens: {}", usage.total_tokens);
    }
    println!();

    // Write context
    println!("Writing .context.md file...");

    // Extract module name from file path
    let module_name = module_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string();

    // Build V2 metadata
    let metadata = ContextMetadata {
        identity: ModuleIdentity {
            name: module_name.clone(),
            module_type: ModuleType::Component,
            status: ModuleStatus::Stable,
            owner: None,
            tags: vec!["component".to_string(), "ui".to_string()],
        },
        layers: vec![Layer {
            layer_type: LayerType::Context,
            status: LayerStatus::Complete,
            coverage: Some(100),
            notes: None,
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
            code_hash: Some(calculate_code_hash(&code)),
            stale: false,
            tokens_used: response.usage.as_ref().map(|u| u.total_tokens),
            generation_time_ms: None,
        },
    };

    let writer = ContextWriter::new();
    let output_path = writer.write(module_path, &response, &metadata)?;

    println!("\n================================================================================");
    println!("SUCCESS");
    println!("================================================================================");
    println!("\nGenerated: {}", output_path.display());
    println!("\nTo view:");
    println!("  Get-Content '{}'", output_path.display());

    Ok(())
}
