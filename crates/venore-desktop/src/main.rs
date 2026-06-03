//! # Venore Desktop App
//!
//! Desktop application built with Tauri.
//! The Rust backend is a thin wrapper around venore-core — no business logic lives here.

// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use once_cell::sync::OnceCell;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{filter::EnvFilter, fmt, prelude::*};

use venore_desktop_lib::{LazyAppState, commands};

// Holds the appender worker so it isn't dropped (which would flush + close
// the file). Lives for the duration of the process.
static LOG_GUARD: OnceCell<WorkerGuard> = OnceCell::new();

/// Where the rolling log files live.
///
/// - debug:   `%TEMP%/venore-dev/logs`  (matches `state.rs` config dir)
/// - release: `~/.venore/logs`
fn resolve_log_dir() -> PathBuf {
    let base = if cfg!(debug_assertions) {
        std::env::temp_dir().join("venore-dev")
    } else {
        dirs::home_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join(".venore")
    };
    base.join("logs")
}

/// Delete log files older than `days` days. Best-effort: any IO error is
/// silently ignored — we'd rather start the app than block boot on log cleanup.
fn cleanup_old_logs(log_dir: &Path, days: u64) {
    let Ok(entries) = std::fs::read_dir(log_dir) else { return };
    let Some(cutoff) = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(days * 86_400))
    else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let Ok(modified) = meta.modified() else { continue };
        if modified < cutoff {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Initialize tracing with a rolling daily file appender (+ stdout in debug).
/// Returns the directory holding `venore.log.YYYY-MM-DD`.
fn init_logging() -> PathBuf {
    let log_dir = resolve_log_dir();
    let _ = std::fs::create_dir_all(&log_dir);
    cleanup_old_logs(&log_dir, 7);

    let file_appender = tracing_appender::rolling::daily(&log_dir, "venore.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    let _ = LOG_GUARD.set(guard);

    // `RUST_LOG` overrides at runtime; default level is info.
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let file_layer = fmt::layer()
        .with_target(false)
        .with_ansi(false)
        .compact()
        .with_writer(non_blocking);

    let stdout_layer = fmt::layer()
        .with_target(false)
        .compact();

    tracing_subscriber::registry()
        .with(env_filter)
        .with(file_layer)
        .with(stdout_layer)
        .init();

    log_dir
}

#[tokio::main]
async fn main() {
    let log_dir = init_logging();
    tracing::info!(log_dir = %log_dir.display(), "Logging initialized");

    tracing::info!("Starting Venore Desktop...");

    // Create the lazy state (not yet initialized)
    let state = LazyAppState::new();

    tracing::info!("LazyAppState created (not initialized yet)");

    // Run the Tauri app
    tauri::Builder::default()
        .manage(state)
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_oauth::init())
        .plugin(tauri_plugin_os::init())
        .invoke_handler(tauri::generate_handler![
            // System initialization command
            commands::system::initialize_backend,

            // System health commands
            commands::system::check_backend,
            commands::system::check_database,
            commands::system::check_llm_gateway,
            commands::system::resize_window,
            commands::system::open_chat_window,
            commands::system::open_node_window,
            commands::system::open_main_window,
            commands::system::read_file_for_attachment,

            // AI connection registry (cross-window Sparkles ↔ Sparkles state)
            commands::ai_connections::list_ai_connections,
            commands::ai_connections::register_ai_connection,
            commands::ai_connections::unregister_ai_connection,
            commands::ai_connections::toggle_ai_connection,
            commands::ai_connections::disconnect_all_ai_connections,

            // Project commands
            commands::projects::register_project,
            commands::projects::open_existing_project,
            commands::projects::get_project,
            commands::projects::list_projects,
            commands::projects::create_knowledge_project,

            // Knowledge commands
            commands::knowledge::create_knowledge_feature,
            commands::knowledge::get_knowledge_feature,
            commands::knowledge::list_knowledge_features,
            commands::knowledge::update_knowledge_feature,
            commands::knowledge::delete_knowledge_feature,
            commands::knowledge::create_knowledge_hexagon,
            commands::knowledge::get_knowledge_hexagon,
            commands::knowledge::list_knowledge_hexagons,
            commands::knowledge::update_knowledge_hexagon,
            commands::knowledge::delete_knowledge_hexagon,
            commands::knowledge::create_knowledge_evidence,
            commands::knowledge::list_knowledge_evidence,
            commands::knowledge::delete_knowledge_evidence,

            // Research engine commands
            commands::research::start_research,
            commands::research::pause_research,
            commands::research::stop_research,
            commands::research::send_research_instruction,
            commands::research::get_research_status,

            // Context commands
            commands::context::generate_context,
            commands::context::get_stale_modules,
            commands::snapshot::resnapshot_project,

            // LLM API Key commands
            commands::llm::set_api_key,
            commands::llm::get_api_key,
            commands::llm::has_api_key,
            commands::llm::remove_api_key,
            commands::llm::get_configured_providers,

            // LLM Task Configuration commands
            commands::llm::get_task_settings,
            commands::llm::set_task_settings,
            commands::llm::get_all_task_settings,
            commands::llm::reset_task_settings,

            // LLM Provider Information commands
            commands::llm::list_providers,
            commands::llm::get_available_models,
            commands::llm::test_connection,
            commands::llm::get_ollama_models,

            // LLM Generation command
            commands::llm::generate_text,

            // LLM Boot data (preload all AI config)
            commands::llm::get_ai_boot_data,

            // Wizard commands (Onboarding flow)
            commands::wizard::scan_project_files,
            commands::wizard::detect_project_modules,
            commands::wizard::get_module_groups,
            commands::wizard::detect_project_type,
            commands::wizard::cancel_wizard_session,
            commands::wizard::check_wizard_checkpoint,
            commands::wizard::load_full_checkpoint,
            commands::wizard::delete_wizard_checkpoint,
            commands::wizard::restore_wizard_session,
            commands::wizard::get_recommended_modules,
            commands::wizard::validate_wizard_step,
            commands::wizard::wizard_index_project,

            // Dashboard commands
            commands::dashboard::get_project_dashboard,

            // Ocean Canvas commands
            commands::ocean::initialize_ocean_layout,
            commands::ocean::compute_ocean_layers,
            commands::ocean::move_ocean_node,
            commands::ocean::move_ocean_nodes,
            commands::ocean::create_knowledge_node,
            commands::ocean::create_lighthouse,
            commands::ocean::delete_ocean_node,
            commands::ocean::rename_ocean_node,
            commands::ocean::set_node_lighthouse,
            commands::ocean::dissolve_lighthouse,
            commands::ocean::delete_lighthouse_cluster,
            commands::ocean::get_knowledge_node,
            commands::ocean::update_node_subtype,
            commands::ocean::add_node_section,
            commands::ocean::update_node_section,
            commands::ocean::delete_node_section,
            commands::ocean::reorder_node_sections,
            commands::ocean::extract_section_to_node,
            commands::ocean::promote_to_lighthouse,
            commands::ocean::create_ocean_connection,
            commands::ocean::delete_ocean_connection,
            commands::ocean::set_lighthouse_color,
            commands::ocean::save_ocean_camera,
            commands::ocean::get_module_details,
            commands::ocean::get_node_states,
            commands::ocean::dismiss_node_state,
            commands::ocean::get_rover_status,

            // Pending logbook writes (AI write preview)
            commands::pending_writes::list_pending_writes,
            commands::pending_writes::list_session_pending_writes,
            commands::pending_writes::accept_pending_write,
            commands::pending_writes::discard_pending_write,
            commands::pending_writes::regenerate_pending_write,

            // RAG commands
            commands::rag::index_project_code,
            commands::rag::search_project_code,
            commands::rag::get_rag_index_status,
            commands::rag::query_code_graph,
            commands::rag::get_project_modules,
            commands::rag::get_module_detail,
            commands::rag::analyze_and_index_project,

            // Chat commands
            commands::chat::send_chat_message,
            commands::chat::stop_chat_stream,
            commands::chat::approve_tool_call,
            commands::chat::clear_session_approvals,
            commands::chat::respond_to_agent,
            commands::chat::approve_plan,
            commands::chat::create_chat_session,
            commands::chat::list_chat_sessions,
            commands::chat::delete_chat_session,
            commands::chat::rename_chat_session,
            commands::chat::get_chat_messages,
            commands::chat::get_chat_snapshots,
            commands::chat::get_chat_context_options,
            commands::chat::get_or_create_dev_session_chat,
            commands::chat::get_session_activity,
            commands::chat::generate_chat_title,
            commands::chat::get_session_stream_status,

            // GitHub commands
            commands::github::github_auth_status,
            commands::github::github_validate_session,
            commands::github::github_start_device_flow,
            commands::github::github_cancel_device_flow,
            commands::github::github_store_pat,
            commands::github::github_disconnect,
            commands::github::github_accept_gcm,
            commands::github::github_detect_repo,
            commands::github::github_list_pulls,
            commands::github::github_list_issues,
            commands::github::github_get_pr_detail,
            commands::github::github_get_pr_files,
            commands::github::github_get_comments,
            commands::github::github_list_user_repos,
            commands::github::github_clone_repo,
            commands::github::github_inspect_clone_destination,
            commands::github::github_analyze_pr,
            commands::github::github_stop_pr_analysis,

            // Cloud auth commands
            commands::cloud::cloud_auth_status,
            commands::cloud::cloud_start_sign_in,
            commands::cloud::cloud_start_oauth,
            commands::cloud::cloud_sign_in_with_email,
            commands::cloud::cloud_sign_up_with_email,
            commands::cloud::cloud_sign_out,
            commands::cloud::cloud_get_user,

            // Terminal commands
            commands::terminal::spawn_terminal,
            commands::terminal::write_terminal,
            commands::terminal::resize_terminal,
            commands::terminal::kill_terminal,
            commands::terminal::list_terminals,

            // Agent commands
            commands::agents::list_agent_profiles,
            commands::agents::get_agent_profile,
            commands::agents::create_agent_profile,
            commands::agents::update_agent_profile,
            commands::agents::delete_agent_profile,
            commands::agents::list_agent_teams,
            commands::agents::get_agent_team,
            commands::agents::create_agent_team,
            commands::agents::update_agent_team,
            commands::agents::delete_agent_team,
            commands::agents::list_agent_rules,
            commands::agents::get_agent_rule,
            commands::agents::create_agent_rule,
            commands::agents::update_agent_rule,
            commands::agents::delete_agent_rule,

            // Tool Category & Definition commands
            commands::agents::list_tool_categories,
            commands::agents::get_tool_category,
            commands::agents::create_tool_category,
            commands::agents::update_tool_category,
            commands::agents::delete_tool_category,
            commands::agents::list_tool_definitions,
            commands::agents::get_tool_definition,
            commands::agents::create_tool_definition,
            commands::agents::update_tool_definition,
            commands::agents::delete_tool_definition,
            commands::agents::list_chat_modes,
            commands::agents::get_chat_mode,
            commands::agents::create_chat_mode,
            commands::agents::update_chat_mode,
            commands::agents::delete_chat_mode,

            // Prompt registry commands
            commands::prompts::list_prompts,
            commands::prompts::get_prompt,
            commands::prompts::update_prompt,
            commands::prompts::reset_prompt,
            commands::prompts::list_prompt_versions,
            commands::prompts::list_prompt_tasks,
            commands::prompts::get_task_prompts,
            commands::prompts::save_task_prompt,
            commands::prompts::list_chat_fragments,
            commands::prompts::set_prompt_enabled,

            // Memory commands
            commands::memory::get_project_memory,
            commands::memory::read_project_memory_by_path,
            commands::memory::save_project_memory,
            commands::memory::delete_project_memory,
            commands::memory::regenerate_memory_summary,
            commands::memory::generate_project_memory,

            // Pipeline commands
            commands::agents::start_pipeline,
            commands::agents::list_pipeline_runs,
            commands::agents::get_pipeline_steps,
            commands::agents::get_run_analysis_context,
            commands::agents::get_analysis_depth,
            commands::agents::set_analysis_depth,

            // Context Updater commands
            commands::context_updater::check_for_updates,
            commands::context_updater::run_context_update,
            commands::context_updater::complete_context_update,
            commands::context_updater::get_updater_state,
            commands::context_updater::update_updater_state,

            // Session commands
            commands::session::create_session,
            commands::session::list_sessions,
            commands::session::get_session,
            commands::session::complete_session,
            commands::session::abandon_session,
            commands::session::session_diff_files,
            commands::session::session_commits,
            commands::session::list_git_branches,
            commands::session::revert_to_snapshot,

            // Skills commands
            commands::skills::list_skills,

            // Editor commands
            commands::editor::read_file,
            commands::editor::write_file,

            // Mesh commands (lifecycle-based)
            commands::mesh::mesh_init,
            commands::mesh::mesh_get_peers,
            commands::mesh::mesh_transport_status,
            commands::mesh::mesh_connect_peer,
            commands::mesh::mesh_disconnect_peer,
            commands::mesh::mesh_unregister_project,

            // Health check
            commands::health,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            if let tauri::RunEvent::Exit = event {
                tracing::info!("Application exiting — cleaning up terminal sessions");
                let manager = venore_core::terminal::TerminalSessionManager::global();
                if let Ok(mut mgr) = manager.lock() {
                    mgr.clear();
                };

                // Shutdown mesh (atomic: stop loop → transport → unregister).
                // `tauri::async_runtime::block_on` works inside the Exit
                // callback even though no Tokio Handle is current —
                // `tokio::runtime::Handle::try_current()` returns Err here
                // and silently skipped the cleanup, leaking peer files.
                tracing::info!("Application exiting — stopping mesh");
                tauri::async_runtime::block_on(async {
                    venore_core::mesh::lifecycle::mesh_stop().await;
                });

                // Clean up LSP servers
                tracing::info!("Application exiting — cleaning up LSP servers");
                tauri::async_runtime::block_on(async {
                    let lsp = venore_core::lsp::LspManager::global();
                    let mut mgr = lsp.lock().await;
                    mgr.stop_all().await;
                });
            }
        });
}
