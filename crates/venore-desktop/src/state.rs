//! App state para Tauri
//!
//! Mantiene instancias de servicios y repositories.

use std::sync::{Arc, Mutex};
use venore_core::agents::AgentRepository;
use venore_core::memory::MemoryRepository;
use venore_core::prompts::PromptRepository;
use venore_core::chat::ChatRepository;
use venore_core::llm::LlmGateway;
use venore_core::infrastructure::config::DefaultConfigStore;
use venore_core::knowledge::KnowledgeRepository;
use venore_core::research::ResearchRepository;
use venore_core::project::ProjectRepository;
use venore_core::rag::{RagRepository, LogbookRepository};
use venore_core::context::ContextRepository;
use venore_core::session::SessionRepository;
use venore_core::traits::ConfigStore;

/// Global state for the Tauri application
pub struct AppState {
    /// LLM Gateway - Main entry point for LLM operations
    pub llm_gateway: Arc<LlmGateway>,

    /// Configuration store - API keys + task settings
    pub config_store: Arc<DefaultConfigStore>,

    /// Chat repository - session and message persistence
    pub chat_repository: Arc<ChatRepository>,

    /// Project repository - registered project persistence
    pub project_repository: Arc<ProjectRepository>,

    /// Knowledge repository - knowledge features, hexagons, evidence
    pub knowledge_repository: Arc<KnowledgeRepository>,

    /// Research repository - research engine runs
    pub research_repository: Arc<ResearchRepository>,

    /// RAG repository - code indexing and FTS5 search
    pub rag_repository: Arc<RagRepository>,

    /// Logbook repository - knowledge logbook indexing and hybrid search
    pub logbook_repository: Arc<LogbookRepository>,

    /// Agent repository - agent profiles and teams
    pub agent_repository: Arc<AgentRepository>,

    /// Memory repository - project memory for system prompt
    pub memory_repository: Arc<MemoryRepository>,

    /// Session repository - branch-per-session workflow
    pub session_repository: Arc<SessionRepository>,

    /// Prompt repository - centralized LLM prompt registry
    pub prompt_repository: Arc<PromptRepository>,

    /// Context repository - module contexts and layer analysis
    pub context_repository: Arc<ContextRepository>,

    /// Config directory path (for worktree storage, etc.)
    pub config_dir: std::path::PathBuf,
}

/// Extract an `Arc<T>` field from `LazyAppState`, returning
/// `Err(NotFound)` if the backend is not yet initialized.
macro_rules! get_state_field {
    ($lazy:expr, $field:ident) => {{
        let guard = $lazy.get();
        match guard.as_ref() {
            Some(state) => Ok(std::sync::Arc::clone(&state.$field)),
            None => Err(venore_core::error::VenoreError::NotFound(
                "Backend not initialized".into(),
            )),
        }
    }};
}
pub(crate) use get_state_field;

/// Lazy-initialized app state wrapper
pub struct LazyAppState {
    state: Mutex<Option<AppState>>,
    /// Always-ready, transient cross-window UI registry — see `ai_connections.rs`.
    /// Initialized eagerly because it has no dependencies and is queried before
    /// the heavy backend (DB / repos) finishes booting.
    pub ai_connections: std::sync::Arc<crate::ai_connections::AiConnectionRegistry>,
}

impl LazyAppState {
    /// Create a new uninitialized LazyAppState
    pub fn new() -> Self {
        Self {
            state: Mutex::new(None),
            ai_connections: std::sync::Arc::new(
                crate::ai_connections::AiConnectionRegistry::new(),
            ),
        }
    }

    /// Initialize the app state (called from BootScreen)
    pub async fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        let new_state = AppState::new().await?;
        let mut state = self.state.lock().unwrap();
        *state = Some(new_state);
        Ok(())
    }

    /// Get the app state (panics if not initialized)
    pub fn get(&self) -> std::sync::MutexGuard<'_, Option<AppState>> {
        self.state.lock().unwrap()
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.state.lock().unwrap().is_some()
    }
}

impl AppState {
    /// Create a new AppState
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Cannot create config directory (~/.venore)
    /// - Cannot connect to SQLite database
    /// - Cannot initialize database
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // 1. Determine config directory
        // In development, use temp directory
        // In production, use user's home directory
        let config_dir = if cfg!(debug_assertions) {
            // Development: Use temp directory to avoid permission issues
            std::env::temp_dir()
                .join("venore-dev")
        } else {
            // Production: Store in user's home directory
            dirs::home_dir()
                .ok_or("Cannot determine home directory")?
                .join(".venore")
        };

        // Create config directory if it doesn't exist
        if !config_dir.exists() {
            std::fs::create_dir_all(&config_dir)
                .map_err(|e| format!("Failed to create config directory at {}: {}", config_dir.display(), e))?;
        }

        // 2. Setup database
        let db_path = config_dir.join("config.db");

        // Create empty database file if it doesn't exist (SQLite requirement)
        if !db_path.exists() {
            std::fs::File::create(&db_path)
                .map_err(|e| format!("Failed to create database file at {}: {}", db_path.display(), e))?;
            tracing::info!("Created new database file at: {}", db_path.display());
        }

        let db_url = format!("sqlite:{}", db_path.display());

        tracing::info!("Initializing config store at: {}", db_url);

        // 3. Create config store
        let config_store = DefaultConfigStore::new(&db_url).await?;
        config_store.initialize().await?;
        let config_store = Arc::new(config_store);

        tracing::info!("Config store initialized successfully");

        // 4. Create LLM gateway (with DB-backed task config resolution)
        use venore_core::infrastructure::config::KeyringApiKeyStore;
        let gateway_key_store = KeyringApiKeyStore::new();
        let llm_gateway = LlmGateway::with_config_store(
            Box::new(gateway_key_store),
            config_store.clone() as Arc<dyn venore_core::traits::TaskConfigStore>,
        );

        tracing::info!("LLM gateway created successfully");

        // 5. Create project repository (shares SQLite pool)
        // Must initialize BEFORE chat repository (chat migration depends on projects table)
        let project_repository = ProjectRepository::new(config_store.pool().clone());
        project_repository.initialize().await
            .map_err(|e| format!("Failed to initialize project repository: {}", e))?;
        let project_repository = Arc::new(project_repository);

        tracing::info!("Project repository initialized successfully");

        // 5b. Create knowledge repository (shares SQLite pool, depends on projects table)
        let knowledge_repository = KnowledgeRepository::new(config_store.pool().clone());
        knowledge_repository.initialize().await
            .map_err(|e| format!("Failed to initialize knowledge repository: {}", e))?;
        let knowledge_repository = Arc::new(knowledge_repository);

        tracing::info!("Knowledge repository initialized successfully");

        // 5c. Create research repository (shares SQLite pool, depends on knowledge_features table)
        let research_repository = ResearchRepository::new(config_store.pool().clone());
        research_repository.initialize().await
            .map_err(|e| format!("Failed to initialize research repository: {}", e))?;
        // Mark any stale "running" runs as paused (app restart recovery)
        let paused = research_repository.pause_stale_runs().await.unwrap_or(0);
        if paused > 0 {
            tracing::info!(paused, "Recovered stale research runs on startup");
        }
        let research_repository = Arc::new(research_repository);

        tracing::info!("Research repository initialized successfully");

        // 6. Create chat repository (shares SQLite pool)
        let chat_repository = ChatRepository::new(config_store.pool().clone());
        chat_repository.initialize().await
            .map_err(|e| format!("Failed to initialize chat repository: {}", e))?;
        let chat_repository = Arc::new(chat_repository);

        tracing::info!("Chat repository initialized successfully");

        // 7. Create RAG repository (shares SQLite pool)
        let rag_repository = RagRepository::new(config_store.pool().clone());
        rag_repository.initialize().await
            .map_err(|e| format!("Failed to initialize RAG repository: {}", e))?;
        let rag_repository = Arc::new(rag_repository);

        tracing::info!("RAG repository initialized successfully");

        // 7b. Create logbook repository (shares SQLite pool) — knowledge index
        let logbook_repository = LogbookRepository::new(config_store.pool().clone());
        logbook_repository.initialize().await
            .map_err(|e| format!("Failed to initialize logbook repository: {}", e))?;
        let logbook_repository = Arc::new(logbook_repository);

        tracing::info!("Logbook repository initialized successfully");

        // 8. Create agent repository (shares SQLite pool)
        let agent_repository = AgentRepository::new(config_store.pool().clone());
        agent_repository.initialize().await
            .map_err(|e| format!("Failed to initialize agent repository: {}", e))?;
        agent_repository.seed_defaults().await
            .map_err(|e| format!("Failed to seed agent defaults: {}", e))?;
        let agent_repository = Arc::new(agent_repository);

        tracing::info!("Agent repository initialized successfully");

        // 8b. Create memory repository (shares SQLite pool)
        let memory_repository = MemoryRepository::new(config_store.pool().clone());
        memory_repository.initialize().await
            .map_err(|e| format!("Failed to initialize memory repository: {}", e))?;
        let memory_repository = Arc::new(memory_repository);

        tracing::info!("Memory repository initialized successfully");

        // 9. Create session repository (shares SQLite pool)
        let session_repository = SessionRepository::new(config_store.pool().clone());
        session_repository.initialize().await
            .map_err(|e| format!("Failed to initialize session repository: {}", e))?;
        let session_repository = Arc::new(session_repository);

        tracing::info!("Session repository initialized successfully");

        // 10. Create prompt repository (shares SQLite pool)
        let prompt_repository = PromptRepository::new(config_store.pool().clone());
        prompt_repository.initialize().await
            .map_err(|e| format!("Failed to initialize prompt repository: {}", e))?;
        prompt_repository.seed_defaults().await
            .map_err(|e| format!("Failed to seed prompt defaults: {}", e))?;
        prompt_repository.seed_provider_prompts().await
            .map_err(|e| format!("Failed to seed provider prompts: {}", e))?;
        prompt_repository.seed_gemini_v4().await
            .map_err(|e| format!("Failed to upgrade Gemini prompt: {}", e))?;
        prompt_repository.seed_gemini_v5().await
            .map_err(|e| format!("Failed to upgrade Gemini prompt to v5: {}", e))?;
        prompt_repository.seed_chat_fragments().await
            .map_err(|e| format!("Failed to seed chat fragments: {}", e))?;
        prompt_repository.seed_mesh_fragments_v2().await
            .map_err(|e| format!("Failed to upgrade mesh fragments to v2: {}", e))?;
        prompt_repository.seed_knowledge_prompts().await
            .map_err(|e| format!("Failed to seed knowledge prompts: {}", e))?;
        let prompt_repository = Arc::new(prompt_repository);

        tracing::info!("Prompt repository initialized successfully");

        // 9. Context repository (module contexts + layer analysis)
        let context_repository = ContextRepository::new(config_store.pool().clone());
        context_repository.initialize().await
            .map_err(|e| format!("Failed to initialize context repository: {}", e))?;
        let context_repository = Arc::new(context_repository);

        tracing::info!("Context repository initialized successfully");

        Ok(Self {
            llm_gateway: Arc::new(llm_gateway),
            config_store,
            chat_repository,
            project_repository,
            knowledge_repository,
            research_repository,
            rag_repository,
            logbook_repository,
            agent_repository,
            memory_repository,
            session_repository,
            prompt_repository,
            context_repository,
            config_dir,
        })
    }
}
