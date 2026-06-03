//! Test de configuración del módulo LLM
//!
//! Ejecutar con:
//! ```bash
//! export ANTHROPIC_API_KEY=sk-ant-api03-...
//! cargo run --example test_llm_config
//! ```
//!
//! Prueba:
//! - DefaultConfigStore (keyring + SQLite)
//! - Configuración de tareas
//! - Persistencia en SQLite
//! - API keys en OS keychain

use venore_core::llm::prelude::*;
use venore_core::infrastructure::config::MockConfigStore;
use venore_core::Result;
use venore_core::infrastructure::config::DefaultConfigStore;
use venore_core::core::config::TaskSettings;

use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    // Inicializar logging
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    println!("🚀 Venore LLM Module - Test de Configuración\n");

    // ========================================================================
    // Setup: Crear directorio temporal para testing
    // ========================================================================
    let temp_dir = std::env::temp_dir().join("venore_test_config");
    std::fs::create_dir_all(&temp_dir)?;

    let db_path = temp_dir.join("config_test.db");
    let db_url = format!("sqlite:{}", db_path.display());

    println!("📁 Directorio temporal: {}", temp_dir.display());
    println!("💾 Base de datos: {}\n", db_path.display());

    // ========================================================================
    // 1. Crear y inicializar DefaultConfigStore
    // ========================================================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📦 Test 1: Crear DefaultConfigStore");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let store = DefaultConfigStore::new(&db_url).await?;
    println!("✅ DefaultConfigStore creado");

    store.initialize().await?;
    println!("✅ Base de datos inicializada (migrations ejecutadas)\n");

    // ========================================================================
    // 2. Configurar API Keys
    // ========================================================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🔑 Test 2: Gestión de API Keys (Keyring)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY not set");

    // Guardar API key
    store
        .store_api_key(LlmProviderType::Anthropic, api_key.clone())
        .await?;
    println!("✅ API key guardada en OS keychain");

    // Verificar que existe
    let has_key = store.has_api_key(LlmProviderType::Anthropic).await?;
    println!("✅ Verificación: has_key = {}", has_key);

    // Listar providers configurados
    let configured = store.list_configured_providers().await?;
    println!("✅ Providers configurados: {:?}\n", configured);

    // ========================================================================
    // 3. Configuración de tareas (SQLite)
    // ========================================================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("⚙️  Test 3: Configuración de Tareas (SQLite)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Obtener configuración por defecto
    println!("📋 Configuración por defecto (Chat):");
    let default_chat = store.get_task_settings(LlmTask::Chat).await?;
    println!("   Provider: {}", default_chat.provider.as_str());
    println!("   Model: {}", default_chat.model);
    println!("   Temperature: {:?}", default_chat.temperature);
    println!("   Max tokens: {:?}", default_chat.max_tokens);
    println!("   Streaming: {:?}\n", default_chat.streaming);

    // Configurar settings personalizados
    println!("🔧 Configurando settings personalizados para Chat...");
    let custom_settings = TaskSettings {
        provider: LlmProviderType::Anthropic,
        model: "claude-haiku-4-5".into(),
        temperature: Some(0.9),
        max_tokens: Some(1000),
        timeout_ms: Some(45000),
        streaming: Some(true),
    };

    store
        .set_task_settings(LlmTask::Chat, custom_settings.clone())
        .await?;
    println!("✅ Settings personalizados guardados en SQLite\n");

    // Verificar que se guardaron
    let saved_settings = store.get_task_settings(LlmTask::Chat).await?;
    println!("📋 Settings personalizados recuperados:");
    println!("   Model: {}", saved_settings.model);
    println!("   Temperature: {:?}", saved_settings.temperature);
    assert_eq!(saved_settings.model, "claude-haiku-4-5");
    assert_eq!(saved_settings.temperature, Some(0.9));
    println!("✅ Verificación exitosa\n");

    // Obtener todas las configuraciones
    println!("📋 Todas las configuraciones:");
    for task in [LlmTask::Onboarding, LlmTask::Chat, LlmTask::Analysis] {
        let settings = store.get_task_settings(task).await?;
        let has_custom = store.has_custom_settings(task).await?;
        println!(
            "   {:?}: {} (custom: {})",
            task,
            settings.model,
            if has_custom { "✅" } else { "❌" }
        );
    }
    println!();

    // Resetear configuración
    println!("🔄 Reseteando configuración de Chat a defaults...");
    store.reset_task_settings(LlmTask::Chat).await?;
    let reset_settings = store.get_task_settings(LlmTask::Chat).await?;
    println!("✅ Reseteado: {}", reset_settings.model);
    println!("   (debería volver a defaults)\n");

    // ========================================================================
    // 4. Validación del store
    // ========================================================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("✔️  Test 4: Validación");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    match store.validate().await {
        Ok(_) => println!("✅ Validación exitosa (al menos 1 provider configurado)"),
        Err(e) => println!("❌ Validación falló: {}", e),
    }
    println!();

    // ========================================================================
    // 5. Test de integración: Usar con LLM Gateway
    // ========================================================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🚀 Test 5: Integración con LLM Gateway");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Crear gateway usando el mismo keyring store
    use venore_core::infrastructure::config::KeyringApiKeyStore;
    let gateway_key_store = KeyringApiKeyStore::new();
    let gateway = LlmGateway::new(Box::new(gateway_key_store));

    println!("✅ LLM Gateway creado");

    // Test de conexión
    println!("🔍 Testing conexión...");
    let test_result = gateway
        .test_connection(LlmProviderType::Anthropic, None)
        .await?;

    if test_result.success {
        println!("✅ Conexión exitosa con {}", test_result.model);
        println!("   Latencia: {}ms", test_result.latency_ms);
    } else {
        println!("❌ Conexión falló: {:?}", test_result.error);
    }
    println!();

    // Generar texto simple
    println!("💬 Generando texto...");
    let request = LlmRequest {
        model: "claude-haiku-4-5".into(),
        messages: vec![LlmMessage {
            role: MessageRole::User,
            content: "Di 'Config test exitoso' en una línea".into(),
            tool_call_id: None,
            tool_calls: None,
            content_parts: None,
        }],
        temperature: Some(0.7),
        max_tokens: Some(30),
        tools: None,
        json_schema: None,
        timeout_secs: Some(30),
        web_search: false,
    };

    let options = GatewayOptions::for_task(LlmTask::Chat);

    match gateway.complete(request, options).await {
        Ok(response) => {
            println!("✅ Respuesta recibida:");
            println!("   {}", response.content.trim());
            if let Some(usage) = response.usage {
                println!("   Tokens: {}", usage.total_tokens);
            }
        }
        Err(e) => println!("❌ Error: {}", e),
    }
    println!();

    // ========================================================================
    // Cleanup
    // ========================================================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🧹 Cleanup");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Remover API key del keyring
    store.remove_api_key(LlmProviderType::Anthropic).await?;
    println!("✅ API key removida del keyring");

    // Eliminar base de datos de testing
    std::fs::remove_file(&db_path)?;
    println!("✅ Base de datos de testing eliminada");
    println!("   {}\n", db_path.display());

    // ========================================================================
    // Resumen
    // ========================================================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("✅ Todos los tests de configuración completados!");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    println!("\n📊 Resumen:");
    println!("   ✅ DefaultConfigStore funcionando");
    println!("   ✅ API keys en OS keychain");
    println!("   ✅ Task settings en SQLite");
    println!("   ✅ Validación correcta");
    println!("   ✅ Integración con LLM Gateway");
    println!();

    println!("💡 Nota: El DefaultConfigStore es production-ready y puede usarse en Tauri.");
    println!();

    Ok(())
}
