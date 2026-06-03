//! Test básico del módulo LLM
//!
//! Ejecutar con:
//! ```bash
//! cargo run --example test_llm_basic
//! ```
//!
//! IMPORTANTE: Configurar ANTHROPIC_API_KEY antes de ejecutar:
//! ```bash
//! export ANTHROPIC_API_KEY=sk-ant-api03-...
//! cargo run --example test_llm_basic
//! ```

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

    println!("🚀 Venore LLM Module - Test Básico\n");

    // 1. Crear store (usando mock para testing)
    let store = MockConfigStore::new();

    // 2. Obtener API key desde environment variable
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY not set. Set it with: export ANTHROPIC_API_KEY=sk-ant-...");

    println!("✅ API Key encontrada (length: {})", api_key.len());

    // 3. Guardar API key en el store
    store
        .store_api_key(LlmProviderType::Anthropic, api_key)
        .await?;

    println!("✅ API Key guardada en store");

    // 4. Crear gateway
    let gateway = LlmGateway::new(Box::new(store));

    println!("✅ LLM Gateway creado\n");

    // 5. Test de conexión
    println!("🔍 Testing conexión con Anthropic...");
    let test_result = gateway
        .test_connection(LlmProviderType::Anthropic, None)
        .await?;

    if test_result.success {
        println!("✅ Conexión exitosa!");
        println!("   - Modelo: {}", test_result.model);
        println!("   - Latencia: {}ms", test_result.latency_ms);
    } else {
        println!("❌ Conexión falló: {:?}", test_result.error);
        return Ok(());
    }

    // 6. Crear request simple
    println!("\n💬 Generando texto con Claude...");

    let request = LlmRequest {
        model: "claude-haiku-4-5".into(),
        messages: vec![LlmMessage {
            role: MessageRole::User,
            content: "Di 'Hola desde Venore!' en una línea".into(),
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

    let options = GatewayOptions::for_task(LlmTask::Chat);

    // 7. Generar
    let response = gateway.complete(request, options).await?;

    println!("\n📝 Respuesta de Claude:");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("{}", response.content);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    if let Some(usage) = response.usage {
        println!("\n📊 Token Usage:");
        println!("   - Prompt tokens: {}", usage.prompt_tokens);
        println!("   - Completion tokens: {}", usage.completion_tokens);
        println!("   - Total tokens: {}", usage.total_tokens);
    }

    println!("\n✅ Test completado exitosamente!\n");

    Ok(())
}
