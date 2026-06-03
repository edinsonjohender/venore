//! Test avanzado del módulo LLM
//!
//! Ejecutar con:
//! ```bash
//! export ANTHROPIC_API_KEY=sk-ant-api03-...
//! cargo run --example test_llm_advanced
//! ```
//!
//! Demuestra:
//! - Diferentes tareas (onboarding, chat, analysis)
//! - Configuración personalizada
//! - Múltiples modelos
//! - Token tracking
//! - Error handling

use venore_core::llm::prelude::*;
use venore_core::infrastructure::config::MockConfigStore;
use venore_core::core::config::TaskSettings;
use venore_core::Result;


#[tokio::main]
async fn main() -> Result<()> {
    // Inicializar logging
    tracing_subscriber::fmt()
        .with_env_filter("venore_core=debug")
        .with_target(false)
        .compact()
        .init();

    println!("🚀 Venore LLM Module - Test Avanzado\n");

    // Setup
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY not set");

    let store = MockConfigStore::new();
    store
        .store_api_key(LlmProviderType::Anthropic, api_key)
        .await?;

    // Configurar settings personalizados para Chat
    let custom_chat_settings = TaskSettings {
        provider: LlmProviderType::Anthropic,
        model: "claude-haiku-4-5".into(),
        temperature: Some(0.8),
        max_tokens: Some(100),
        timeout_ms: Some(30000),
        streaming: Some(false),
    };

    store
        .set_task_settings(LlmTask::Chat, custom_chat_settings)
        .await?;

    let gateway = LlmGateway::new(Box::new(store));

    println!("✅ Setup completado\n");

    // ========================================================================
    // Test 1: Chat Task
    // ========================================================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📝 Test 1: Chat Task (temperatura alta)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let chat_request = LlmRequest {
        model: "claude-haiku-4-5".into(),
        messages: vec![LlmMessage {
            role: MessageRole::User,
            content: "Dame 3 nombres creativos para un proyecto de software".into(),
            tool_call_id: None,
            tool_calls: None,
            content_parts: None,
        }],
        temperature: Some(0.9),
        max_tokens: Some(100),
        tools: None,
        json_schema: None,
        timeout_secs: Some(30),
        web_search: false,
    };

    let chat_options = GatewayOptions::for_task(LlmTask::Chat);

    match gateway.complete(chat_request, chat_options).await {
        Ok(response) => {
            println!("✅ Respuesta recibida:");
            println!("{}", response.content);
            print_usage(&response.usage);
        }
        Err(e) => println!("❌ Error: {}", e),
    }

    println!();

    // ========================================================================
    // Test 2: Analysis Task
    // ========================================================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🔍 Test 2: Analysis Task (temperatura baja)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let code_to_analyze = r#"
function fibonacci(n) {
    if (n <= 1) return n;
    return fibonacci(n - 1) + fibonacci(n - 2);
}
"#;

    let analysis_request = LlmRequest {
        model: "claude-sonnet-4-5".into(),
        messages: vec![LlmMessage {
            role: MessageRole::User,
            content: format!(
                "Analiza este código y explica su complejidad en una línea:\n{}",
                code_to_analyze
            ),
            tool_call_id: None,
            tool_calls: None,
            content_parts: None,
        }],
        temperature: Some(0.2),
        max_tokens: Some(100),
        tools: None,
        json_schema: None,
        timeout_secs: Some(30),
        web_search: false,
    };

    let analysis_options = GatewayOptions::for_task(LlmTask::Analysis)
        .with_temperature(0.2); // Override para precisión

    match gateway.complete(analysis_request, analysis_options).await {
        Ok(response) => {
            println!("✅ Análisis:");
            println!("{}", response.content);
            print_usage(&response.usage);
        }
        Err(e) => println!("❌ Error: {}", e),
    }

    println!();

    // ========================================================================
    // Test 3: Onboarding Task (simular creación de .context.md)
    // ========================================================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📋 Test 3: Onboarding Task (generar contexto)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let project_info = r#"
Project: venore_v2
Language: Rust
Structure:
- crates/venore-core (business logic)
- crates/venore-desktop (Tauri app)
Purpose: Visual explorer for code organization with LLM integration
"#;

    let onboarding_request = LlmRequest {
        model: "claude-sonnet-4-5".into(),
        messages: vec![
            LlmMessage {
                role: MessageRole::System,
                content: "You are a helpful assistant that creates concise project context files."
                    .into(),
                tool_call_id: None,
                tool_calls: None,
                content_parts: None,
            },
            LlmMessage {
                role: MessageRole::User,
                content: format!(
                    "Create a brief 2-line description for this project:\n{}",
                    project_info
                ),
                tool_call_id: None,
                tool_calls: None,
                content_parts: None,
            },
        ],
        temperature: Some(0.3),
        max_tokens: Some(150),
        tools: None,
        json_schema: None,
        timeout_secs: Some(30),
        web_search: false,
    };

    let onboarding_options = GatewayOptions::for_task(LlmTask::Onboarding);

    match gateway.complete(onboarding_request, onboarding_options).await {
        Ok(response) => {
            println!("✅ Contexto generado:");
            println!("╔══════════════════════════════════════════╗");
            for line in response.content.lines() {
                println!("║ {:<40} ║", line);
            }
            println!("╚══════════════════════════════════════════╝");
            print_usage(&response.usage);
        }
        Err(e) => println!("❌ Error: {}", e),
    }

    println!();

    // ========================================================================
    // Test 4: Comparar modelos
    // ========================================================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("⚡ Test 4: Comparación de modelos");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let models = vec![
        ("claude-sonnet-4-5", "Sonnet"),
        ("claude-haiku-4-5", "Haiku"),
    ];

    let simple_prompt = "Di 'Hola' en francés";

    for (model_id, model_name) in models {
        println!("🔹 Modelo: {}", model_name);

        let request = LlmRequest {
            model: model_id.into(),
            messages: vec![LlmMessage {
                role: MessageRole::User,
                content: simple_prompt.into(),
                tool_call_id: None,
                tool_calls: None,
                content_parts: None,
            }],
            temperature: Some(0.7),
            max_tokens: Some(20),
            tools: None,
            json_schema: None,
            timeout_secs: Some(30),
            web_search: false,
        };

        let options = GatewayOptions::for_task(LlmTask::Chat).with_model(model_id);

        match gateway.complete(request, options).await {
            Ok(response) => {
                println!("   Respuesta: {}", response.content.trim());
                if let Some(usage) = response.usage {
                    println!("   Tokens: {} total", usage.total_tokens);
                }
            }
            Err(e) => println!("   ❌ Error: {}", e),
        }
        println!();
    }

    // ========================================================================
    // Resumen final
    // ========================================================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("✅ Todos los tests completados!");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    println!("\n📊 Resumen:");
    println!("   - ✅ Chat task (temperatura alta)");
    println!("   - ✅ Analysis task (temperatura baja)");
    println!("   - ✅ Onboarding task (generación de contexto)");
    println!("   - ✅ Comparación de modelos");
    println!();

    Ok(())
}

fn print_usage(usage: &Option<TokenUsage>) {
    if let Some(u) = usage {
        println!(
            "   📊 Tokens: {} prompt + {} completion = {} total",
            u.prompt_tokens, u.completion_tokens, u.total_tokens
        );
    }
}
