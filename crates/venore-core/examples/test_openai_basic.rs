//! Test básico del módulo LLM con OpenAI
//!
//! Ejecutar con:
//! ```bash
//! export OPENAI_API_KEY=sk-...
//! cargo run --example test_openai_basic
//! ```
//!
//! Prueba:
//! - Setup básico del módulo
//! - Conexión con OpenAI API
//! - Generación simple de texto
//! - Token tracking

use venore_core::llm::prelude::*;
use venore_core::infrastructure::config::MockConfigStore;
use venore_core::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Inicializar logging
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    println!("🚀 Venore LLM Module - Test Básico (OpenAI)\n");

    // ========================================================================
    // 1. Setup
    // ========================================================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📦 Setup");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Obtener API key del environment
    let api_key = std::env::var("OPENAI_API_KEY")
        .expect("OPENAI_API_KEY environment variable not set");

    println!("✅ API Key encontrada (length: {})", api_key.len());

    // Crear mock store y guardar API key
    let store = MockConfigStore::new();
    store
        .store_api_key(LlmProviderType::OpenAI, api_key)
        .await?;

    println!("✅ API Key guardada en store");

    // Crear gateway
    let gateway = LlmGateway::new(Box::new(store));
    println!("✅ LLM Gateway creado\n");

    // ========================================================================
    // 2. Test de conexión
    // ========================================================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🔍 Test de Conexión");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    println!("🔍 Testing conexión con OpenAI...");

    let test_result = gateway
        .test_connection(LlmProviderType::OpenAI, None)
        .await?;

    if test_result.success {
        println!("✅ Conexión exitosa!");
        println!("   Modelo: {}", test_result.model);
        println!("   Latencia: {}ms", test_result.latency_ms);
    } else {
        println!("❌ Conexión falló:");
        if let Some(error) = test_result.error {
            println!("   Error: {}", error);
        }
        return Ok(());
    }

    println!();

    // ========================================================================
    // 3. Generación de texto simple
    // ========================================================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("💬 Generación de Texto");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    println!("💬 Generando texto con GPT-4o...");

    let request = LlmRequest {
        model: "gpt-4.1".into(),
        messages: vec![LlmMessage {
            role: MessageRole::User,
            content: "Di 'Hola desde Venore' en una línea".into(),
            tool_call_id: None,
            tool_calls: None,
            content_parts: None,
        }],
        temperature: Some(0.7),
        max_tokens: Some(50),
        tools: None,
        json_schema: None,
        timeout_secs: Some(30),
        web_search: false,
    };

    let options = GatewayOptions::for_task(LlmTask::Chat)
        .with_provider(LlmProviderType::OpenAI);  // Explicitly use OpenAI

    let response = gateway.complete(request, options).await?;

    println!("📝 Respuesta de GPT-4o:");
    println!("   {}", response.content.trim());
    println!();

    if let Some(usage) = response.usage {
        println!("📊 Token Usage:");
        println!("   Prompt tokens: {}", usage.prompt_tokens);
        println!("   Completion tokens: {}", usage.completion_tokens);
        println!("   Total tokens: {}", usage.total_tokens);
    }

    println!();

    // ========================================================================
    // Resumen
    // ========================================================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("✅ Test completado exitosamente!");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    println!("\n📊 Resumen:");
    println!("   - ✅ Configuración de API key");
    println!("   - ✅ Test de conexión con OpenAI");
    println!("   - ✅ Generación de texto con GPT-4o");
    println!("   - ✅ Token tracking");
    println!();

    Ok(())
}
