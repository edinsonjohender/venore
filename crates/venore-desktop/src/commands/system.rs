//! System health check commands
//!
//! Commands to verify backend initialization status.

use serde::Serialize;
use tauri::Manager;
use crate::state::LazyAppState;
use crate::utils::{CommandResult, StateCommandResult};
use venore_core::error::VenoreError;
use venore_core::traits::ApiKeyStore;

// ── File attachment reading ─────────────────────────────────────────

/// Maximum file size for attachments (20 MB)
const MAX_ATTACHMENT_SIZE: u64 = 20 * 1024 * 1024;

#[derive(Serialize)]
pub struct FileAttachmentData {
    pub name: String,
    pub mime_type: String,
    pub size: u64,
    pub data_base64: String,
    pub is_image: bool,
}

/// Read a file from disk and return its metadata + base64 content.
/// Used by the chat input to prepare attachments before sending.
#[tauri::command]
pub async fn read_file_for_attachment(path: String) -> CommandResult<FileAttachmentData> {
    use base64::Engine;
    use std::path::Path;

    let result: Result<FileAttachmentData, VenoreError> = (|| {
        let file_path = Path::new(&path);

        // Validate file exists
        if !file_path.exists() {
            return Err(VenoreError::FileNotFound(path.clone()));
        }

        let metadata = std::fs::metadata(file_path)
            .map_err(|e| VenoreError::FileReadError(format!("Failed to read metadata: {}", e)))?;

        if metadata.len() > MAX_ATTACHMENT_SIZE {
            return Err(VenoreError::Unknown(format!(
                "File too large: {} bytes (max {})",
                metadata.len(),
                MAX_ATTACHMENT_SIZE
            )));
        }

        let name = file_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let ext = file_path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        let mime_type = match ext.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "svg" => "image/svg+xml",
            "bmp" => "image/bmp",
            "pdf" => "application/pdf",
            "txt" => "text/plain",
            "md" => "text/markdown",
            "json" => "application/json",
            "csv" => "text/csv",
            "xml" => "application/xml",
            "html" | "htm" => "text/html",
            "css" => "text/css",
            "js" | "mjs" => "text/javascript",
            "ts" | "tsx" => "text/typescript",
            "rs" => "text/x-rust",
            "py" => "text/x-python",
            "go" => "text/x-go",
            "java" => "text/x-java",
            "yaml" | "yml" => "text/yaml",
            "toml" => "text/toml",
            "log" => "text/plain",
            _ => "application/octet-stream",
        }
        .to_string();

        let is_image = mime_type.starts_with("image/");

        let data = std::fs::read(file_path)
            .map_err(|e| VenoreError::FileReadError(format!("Failed to read file: {}", e)))?;

        let data_base64 = base64::engine::general_purpose::STANDARD.encode(&data);

        Ok(FileAttachmentData {
            name,
            mime_type,
            size: metadata.len(),
            data_base64,
            is_image,
        })
    })();

    result.into()
}

#[derive(Serialize)]
pub struct SystemCheckResponse {
    pub success: bool,
    pub message: String,
}

/// Initialize the backend (called from BootScreen)
/// This is where AppState::new() actually runs
#[tauri::command]
pub async fn initialize_backend(
    state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<SystemCheckResponse> {
    tracing::info!("initialize_backend called - starting AppState initialization");

    match state.initialize().await {
        Ok(_) => {
            tracing::info!("AppState initialized successfully");
            Ok(CommandResult::ok(SystemCheckResponse {
                success: true,
                message: "Backend initialized".to_string(),
            }))
        }
        Err(e) => {
            tracing::error!("Failed to initialize AppState: {}", e);
            Ok(CommandResult::ok(SystemCheckResponse {
                success: false,
                message: format!("Initialization failed: {}", e),
            }))
        }
    }
}

/// Check if backend is initialized and ready
#[tauri::command]
pub async fn check_backend(
    state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<SystemCheckResponse> {
    if state.is_initialized() {
        Ok(CommandResult::ok(SystemCheckResponse {
            success: true,
            message: "Backend initialized".to_string(),
        }))
    } else {
        Ok(CommandResult::ok(SystemCheckResponse {
            success: false,
            message: "Backend not initialized".to_string(),
        }))
    }
}

/// Check if database connection is working
#[tauri::command]
pub async fn check_database(
    state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<SystemCheckResponse> {
    let config_store = {
        let state_guard = state.get();
        match state_guard.as_ref() {
            Some(app_state) => std::sync::Arc::clone(&app_state.config_store),
            None => {
                return Ok(CommandResult::ok(SystemCheckResponse {
                    success: false,
                    message: "AppState not initialized".to_string(),
                }))
            }
        }
    };

    match config_store.list_configured_providers().await {
        Ok(_) => Ok(CommandResult::ok(SystemCheckResponse {
            success: true,
            message: "Database connected".to_string(),
        })),
        Err(e) => Ok(CommandResult::ok(SystemCheckResponse {
            success: false,
            message: format!("Database error: {}", e),
        })),
    }
}

/// Check if LLM gateway is available
#[tauri::command]
pub async fn check_llm_gateway(
    state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<SystemCheckResponse> {
    let state_guard = state.get();

    if state_guard.is_some() {
        Ok(CommandResult::ok(SystemCheckResponse {
            success: true,
            message: "LLM gateway ready".to_string(),
        }))
    } else {
        Ok(CommandResult::ok(SystemCheckResponse {
            success: false,
            message: "AppState not initialized".to_string(),
        }))
    }
}

/// Resize window to project view dimensions
#[tauri::command]
pub async fn resize_window(
    window: tauri::Window,
    width: f64,
    height: f64,
) -> CommandResult<()> {
    let result: Result<(), VenoreError> = (|| {
        window
            .set_size(tauri::Size::Logical(tauri::LogicalSize { width, height }))
            .map_err(|e| VenoreError::Unknown(format!("Failed to resize window: {}", e)))?;

        window
            .center()
            .map_err(|e| VenoreError::Unknown(format!("Failed to center window: {}", e)))?;

        Ok(())
    })();

    result.into()
}

/// Open a pop-out detail window for an ocean node
#[tauri::command]
pub async fn open_node_window(
    app: tauri::AppHandle,
    project_path: String,
    module_id: String,
    module_name: String,
    node_variant: String,
) -> CommandResult<()> {
    use tauri::webview::WebviewWindowBuilder;
    use tauri::WebviewUrl;

    let result: Result<(), VenoreError> = (|| {
        let label = format!("node-{}", &module_id[..12.min(module_id.len())]);

        if let Some(existing) = app.get_webview_window(&label) {
            existing
                .set_focus()
                .map_err(|e| VenoreError::Unknown(format!("Failed to focus window: {}", e)))?;
            return Ok(());
        }

        let encoded_path = url::form_urlencoded::byte_serialize(project_path.as_bytes())
            .collect::<String>();
        let encoded_name = url::form_urlencoded::byte_serialize(module_name.as_bytes())
            .collect::<String>();
        let encoded_variant = url::form_urlencoded::byte_serialize(node_variant.as_bytes())
            .collect::<String>();
        let encoded_id = url::form_urlencoded::byte_serialize(module_id.as_bytes())
            .collect::<String>();
        let url = format!(
            "index.html?window=node&projectPath={}&moduleId={}&moduleName={}&nodeVariant={}",
            encoded_path, encoded_id, encoded_name, encoded_variant,
        );

        WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(url.into()))
            .title(format!("Venore - {}", module_name))
            .inner_size(640.0, 720.0)
            .min_inner_size(400.0, 480.0)
            .decorations(false)
            .resizable(true)
            .build()
            .map_err(|e| VenoreError::Unknown(format!("Failed to create node window: {}", e)))?;

        Ok(())
    })();

    result.into()
}

/// Open a pop-out chat window for a specific session
#[tauri::command]
pub async fn open_chat_window(
    app: tauri::AppHandle,
    session_id: String,
    project_path: String,
    session_name: String,
    project_id: Option<String>,
) -> CommandResult<()> {
    use tauri::webview::WebviewWindowBuilder;
    use tauri::WebviewUrl;

    let result: Result<(), VenoreError> = (|| {
        let label = format!("chat-{}", &session_id[..8.min(session_id.len())]);

        // Focus existing window instead of duplicating
        if let Some(existing) = app.get_webview_window(&label) {
            existing
                .set_focus()
                .map_err(|e| VenoreError::Unknown(format!("Failed to focus window: {}", e)))?;
            return Ok(());
        }

        // Build URL with query params
        let encoded_path = url::form_urlencoded::byte_serialize(project_path.as_bytes())
            .collect::<String>();
        let encoded_name = url::form_urlencoded::byte_serialize(session_name.as_bytes())
            .collect::<String>();
        let url = format!(
            "index.html?window=chat&sessionId={}&projectPath={}&sessionName={}{}",
            session_id,
            encoded_path,
            encoded_name,
            project_id
                .as_ref()
                .map(|pid| format!("&projectId={}", pid))
                .unwrap_or_default(),
        );

        WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(url.into()))
            .title(format!("Venore - {}", session_name))
            .inner_size(500.0, 700.0)
            .min_inner_size(360.0, 400.0)
            .decorations(false)
            .resizable(true)
            .build()
            .map_err(|e| VenoreError::Unknown(format!("Failed to create chat window: {}", e)))?;

        Ok(())
    })();

    result.into()
}

// ── Open a second main window (in-process) ─────────────────────────
//
// Creates an additional Tauri webview window in the same process, booting
// into the launcher screen. Mesh registration is keyed per opened project,
// so multiple windows (each with its own project) coexist as separate
// peers within a single process — no detached child, no console flicker,
// no duplicated SQLite/LLM/RAG infrastructure.

/// Open a fresh Venore main window in the current process. URL has no
/// `?window=` param so `main.tsx` falls through to `<App />` and the user
/// lands on the launcher.
#[tauri::command]
pub async fn open_main_window(app: tauri::AppHandle) -> CommandResult<()> {
    use tauri::webview::WebviewWindowBuilder;
    use tauri::WebviewUrl;

    let result: Result<(), VenoreError> = (|| {
        let label = format!("main-{}", &uuid::Uuid::new_v4().to_string()[..8]);

        WebviewWindowBuilder::new(&app, &label, WebviewUrl::App("index.html".into()))
            .title("Venore")
            .inner_size(800.0, 730.0)
            .min_inner_size(800.0, 730.0)
            .decorations(false)
            .resizable(true)
            .build()
            .map_err(|e| VenoreError::Unknown(format!("Failed to create main window: {}", e)))?;

        tracing::info!(label = %label, "Opened additional main window");
        Ok(())
    })();

    result.into()
}

