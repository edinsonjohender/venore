//! Default agent profile and team seeds
//!
//! Inserts 10 template profiles + 3 sub-agent profiles + 1 template team on first initialization.

use crate::{MapDbErr, Result};
use crate::tools::definitions;
use crate::tools::names as N;
use super::models::*;
use super::repository::AgentRepository;

impl AgentRepository {
    /// Seed default templates if none exist yet
    pub async fn seed_defaults(&self) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();

        // Seed profiles + team
        let profile_count = self.count_template_profiles().await?;
        if profile_count > 0 {
            tracing::debug!("Agent templates already seeded ({} profiles), skipping", profile_count);
        } else {
            tracing::info!("Seeding default agent profiles and teams");

            let profiles = default_profiles(&now);
            for profile in &profiles {
                self.create_profile(profile).await?;
            }

            let team = default_team(&now);
            self.create_team(&team).await?;

            tracing::info!("Seeded {} agent profiles + 1 team", profiles.len());
        }

        // Migrate template prompts — update if still on old defaults
        self.migrate_template_prompts(&now).await?;

        // Migrate seed models — fix stale versioned model IDs
        self.migrate_seed_models().await?;

        // Seed rules
        let rule_count = self.count_template_rules().await?;
        if rule_count > 0 {
            tracing::debug!("Agent rules already seeded ({} rules), skipping", rule_count);
        } else {
            tracing::info!("Seeding default agent rules");

            let rules = default_rules(&now);
            for rule in &rules {
                self.create_rule(rule).await?;
            }

            tracing::info!("Seeded {} agent rules", rules.len());
        }

        // Seed tool categories
        let cat_count = self.count_template_tool_categories().await?;
        if cat_count > 0 {
            tracing::debug!("Tool categories already seeded ({} categories), skipping", cat_count);
        } else {
            tracing::info!("Seeding default tool categories");

            let categories = default_tool_categories(&now);
            for cat in &categories {
                self.create_tool_category(cat).await?;
            }

            tracing::info!("Seeded {} tool categories", categories.len());
        }

        // Seed tool definitions
        let tool_count = self.count_template_tool_definitions().await?;
        if tool_count > 0 {
            tracing::debug!("Tool definitions already seeded ({} tools), skipping", tool_count);
        } else {
            tracing::info!("Seeding default tool definitions");

            let tools = default_tool_definitions(&now);
            for tool in &tools {
                self.create_tool_definition(tool).await?;
            }

            tracing::info!("Seeded {} tool definitions", tools.len());
        }

        // Seed sub-agent profiles (migration — idempotent)
        self.seed_sub_agent_profiles(&now).await?;

        // Backfill new categories + tools added after the initial seed.
        // The base seed only runs when the tables are empty, so existing
        // installations need this idempotent migration to pick up additions.
        self.backfill_new_tool_categories(&now).await?;
        self.backfill_missing_tool_definitions(&now).await?;

        // Seed default chat modes (idempotent — only inserts if missing)
        self.backfill_default_chat_modes(&now).await?;

        // Migrate ask_project into the dedicated cat-mesh category and make it
        // available in both Code and Knowledge modes (existing DBs only).
        self.migrate_mesh_category(&now).await?;

        Ok(())
    }

    /// Move `ask_project` out of `cat-agent` into the dedicated `cat-mesh`
    /// category, refresh its description/parameters from the canonical Rust
    /// definition, and add `cat-mesh` to the Code and Knowledge modes.
    ///
    /// Why: `ask_project` used to share `cat-agent` with `spawn_agent`, so it
    /// was only reachable in Code mode — a Knowledge chat with a connected peer
    /// was told (in the system prompt) it could call `ask_project`, but the
    /// dispatcher rejected it. The base seed change fixes fresh DBs; this
    /// idempotent migration converges existing ones. Only touches template rows
    /// (`is_template`), so user customizations are left alone.
    async fn migrate_mesh_category(&self, now: &str) -> Result<()> {
        // 1. Re-point the ask_project tool definition to cat-mesh and refresh
        //    its schema (description + params) from the canonical definition.
        if let Ok(mut def) = self.get_tool_definition("tool-ask-project").await {
            if def.is_template {
                let canonical = definitions::mesh_tools()
                    .into_iter()
                    .find(|t| t.name == N::ASK_PROJECT);
                let needs_update = def.category_id != "cat-mesh"
                    || canonical.as_ref().is_some_and(|c| c.description != def.description);
                if needs_update {
                    def.category_id = "cat-mesh".to_string();
                    if let Some(c) = canonical {
                        def.description = c.description;
                        def.parameters_json = serde_json::to_string(&c.parameters)
                            .unwrap_or(def.parameters_json);
                    }
                    def.updated_at = now.to_string();
                    self.update_tool_definition(&def).await?;
                    tracing::info!("Migrated ask_project to cat-mesh");
                }
            }
        }

        // 2. Ensure both modes expose cat-mesh.
        for mode_id in ["mode-code", "mode-knowledge"] {
            if let Ok(mut mode) = self.get_chat_mode(mode_id).await {
                if !mode.category_ids.iter().any(|c| c == "cat-mesh") {
                    mode.category_ids.push("cat-mesh".to_string());
                    mode.updated_at = now.to_string();
                    self.update_chat_mode(&mode).await?;
                    tracing::info!(mode = mode_id, "Added cat-mesh to mode");
                }
            }
        }

        Ok(())
    }

    /// Insert default chat modes if they don't exist. Idempotent.
    async fn backfill_default_chat_modes(&self, now: &str) -> Result<()> {
        let wanted = default_chat_modes(now);
        let mut added = 0u32;
        for mode in &wanted {
            if self.get_chat_mode(&mode.id).await.is_err() {
                self.create_chat_mode(mode).await?;
                added += 1;
            }
        }
        if added > 0 {
            tracing::info!(added, "Seeded default chat modes");
        }
        Ok(())
    }

    /// Insert any tool category that doesn't exist yet. Idempotent.
    async fn backfill_new_tool_categories(&self, now: &str) -> Result<()> {
        let wanted = default_tool_categories(now);
        let mut added = 0u32;
        for cat in &wanted {
            if self.get_tool_category(&cat.id).await.is_err() {
                self.create_tool_category(cat).await?;
                added += 1;
            }
        }
        if added > 0 {
            tracing::info!(added, "Backfilled tool categories");
        }
        Ok(())
    }

    /// Insert any tool definition that doesn't exist yet. Idempotent.
    /// Also corrects category drift: if a template tool was previously
    /// inserted with the fallback category (`cat-terminal`) because the
    /// category_map didn't yet list it, this migration moves it to its
    /// canonical category. User-created (`is_template = false`) tools
    /// are never touched.
    async fn backfill_missing_tool_definitions(&self, now: &str) -> Result<()> {
        let wanted = default_tool_definitions(now);
        let mut added = 0u32;
        let mut recategorized = 0u32;
        for tool in &wanted {
            match self.get_tool_definition(&tool.id).await {
                Err(_) => {
                    self.create_tool_definition(tool).await?;
                    added += 1;
                }
                Ok(mut existing) => {
                    // Only auto-correct templates that landed in the
                    // fallback category. Anything else is presumed
                    // intentional (user moved it via the UI).
                    if existing.is_template
                        && existing.category_id == "cat-terminal"
                        && tool.category_id != "cat-terminal"
                    {
                        let from = existing.category_id.clone();
                        existing.category_id = tool.category_id.clone();
                        existing.updated_at = now.to_string();
                        if self.update_tool_definition(&existing).await.is_ok() {
                            tracing::info!(
                                tool = %tool.name,
                                from = %from,
                                to = %tool.category_id,
                                "Recategorized template tool to canonical category",
                            );
                            recategorized += 1;
                        }
                    }
                }
            }
        }
        if added > 0 {
            tracing::info!(added, "Backfilled tool definitions");
        }
        if recategorized > 0 {
            tracing::info!(recategorized, "Recategorized template tools");
        }
        Ok(())
    }

    /// Seed sub-agent profiles if not already present (idempotent migration).
    async fn seed_sub_agent_profiles(&self, now: &str) -> Result<()> {
        if self.get_profile("sub-agent-executor").await.is_ok() {
            return Ok(());
        }
        let profiles = sub_agent_profiles(now);
        for p in &profiles {
            self.create_profile(p).await?;
        }
        tracing::info!("Seeded {} sub-agent profiles", profiles.len());
        Ok(())
    }

    /// Update template profiles whose system_prompt still matches the old default.
    /// This ensures prompt improvements reach existing databases.
    async fn migrate_template_prompts(&self, now: &str) -> Result<()> {
        let defaults = default_profiles(now);

        for default in &defaults {
            if let Ok(existing) = self.get_profile(&default.id).await {
                if existing.is_template && existing.system_prompt != default.system_prompt {
                    // Migrate if the old prompt lacks ```json and the new one has it.
                    // This only applies to profiles whose updated default adds JSON blocks.
                    let is_old_default = !existing.system_prompt.contains("```json");
                    let new_has_json = default.system_prompt.contains("```json");

                    if is_old_default && new_has_json {
                        let mut updated = existing.clone();
                        updated.system_prompt = default.system_prompt.clone();
                        updated.temperature = default.temperature;
                        updated.updated_at = now.into();
                        self.update_profile(&updated).await?;
                        tracing::info!("Migrated template prompt for '{}'", default.id);
                    }
                }
            }
        }

        Ok(())
    }

    /// Migrate stale versioned model IDs → short aliases (idempotent).
    async fn migrate_seed_models(&self) -> Result<()> {
        let updated = sqlx::query::<sqlx::Sqlite>(
            "UPDATE agent_profiles SET model = ?2 WHERE model = ?1 AND is_template = 1"
        )
        .bind("claude-sonnet-4-5-20250929")
        .bind("claude-sonnet-4-5")
        .execute(self.pool())
        .await
        .db_err("migrate seed models")?;

        if updated.rows_affected() > 0 {
            tracing::info!(count = updated.rows_affected(), "Migrated agent seed models");
        }
        Ok(())
    }
}

fn default_profiles(now: &str) -> Vec<AgentProfile> {
    vec![
        AgentProfile {
            id: "triager-general".into(),
            name: "General Triager".into(),
            description: "Routes incoming tasks to the appropriate specialist based on task type and complexity.".into(),
            stage: AgentStage::Triager,
            system_prompt: "You are a task triager. Analyze the incoming task and determine which specialist agent should handle it. Consider the task type, complexity, and required expertise.".into(),
            provider: "anthropic".into(),
            model: "claude-sonnet-4-5".into(),
            temperature: 0.2,
            is_template: true,
            is_enabled: true,
            rules_json: "[]".into(),
            criteria_json: "[]".into(),
            tools_json: "[]".into(),
            max_tokens_per_run: 30000,
            created_at: now.into(),
            updated_at: now.into(),
        },
        AgentProfile {
            id: "triager-priority".into(),
            name: "Priority Triager".into(),
            description: "Evaluates task urgency and assigns priority levels before routing.".into(),
            stage: AgentStage::Triager,
            system_prompt: "You are a priority assessment agent. Evaluate the urgency and importance of each task, assign a priority level (critical, high, medium, low), and route accordingly.".into(),
            provider: "anthropic".into(),
            model: "claude-sonnet-4-5".into(),
            temperature: 0.1,
            is_template: true,
            is_enabled: true,
            rules_json: "[]".into(),
            criteria_json: "[]".into(),
            tools_json: "[]".into(),
            max_tokens_per_run: 30000,
            created_at: now.into(),
            updated_at: now.into(),
        },
        AgentProfile {
            id: "spec-architecture".into(),
            name: "Architecture Analyst".into(),
            description: "Analyzes code architecture, module boundaries, and dependency patterns.".into(),
            stage: AgentStage::Specialist,
            system_prompt: "You are an architecture specialist. Analyze the codebase structure, identify module boundaries, evaluate dependency patterns, and suggest architectural improvements.".into(),
            provider: "anthropic".into(),
            model: "claude-sonnet-4-5".into(),
            temperature: 0.3,
            is_template: true,
            is_enabled: true,
            rules_json: "[]".into(),
            criteria_json: "[]".into(),
            tools_json: "[]".into(),
            max_tokens_per_run: 30000,
            created_at: now.into(),
            updated_at: now.into(),
        },
        AgentProfile {
            id: "spec-security".into(),
            name: "Security Reviewer".into(),
            description: "Identifies security vulnerabilities, checks for OWASP top 10 issues.".into(),
            stage: AgentStage::Specialist,
            system_prompt: "You are a security specialist. Review code for vulnerabilities including OWASP top 10, injection flaws, authentication issues, and data exposure risks.".into(),
            provider: "openai".into(),
            model: "gpt-4.1".into(),
            temperature: 0.2,
            is_template: true,
            is_enabled: true,
            rules_json: "[]".into(),
            criteria_json: "[]".into(),
            tools_json: "[]".into(),
            max_tokens_per_run: 30000,
            created_at: now.into(),
            updated_at: now.into(),
        },
        AgentProfile {
            id: "spec-performance".into(),
            name: "Performance Analyst".into(),
            description: "Identifies performance bottlenecks, memory leaks, and optimization opportunities.".into(),
            stage: AgentStage::Specialist,
            system_prompt: "You are a performance specialist. Analyze code for performance bottlenecks, memory leaks, unnecessary allocations, and optimization opportunities.".into(),
            provider: "anthropic".into(),
            model: "claude-sonnet-4-5".into(),
            temperature: 0.3,
            is_template: true,
            is_enabled: true,
            rules_json: "[]".into(),
            criteria_json: "[]".into(),
            tools_json: "[]".into(),
            max_tokens_per_run: 30000,
            created_at: now.into(),
            updated_at: now.into(),
        },
        AgentProfile {
            id: "spec-testing".into(),
            name: "Test Coverage Analyst".into(),
            description: "Evaluates test coverage, identifies untested paths, and suggests test strategies.".into(),
            stage: AgentStage::Specialist,
            system_prompt: "You are a testing specialist. Analyze test coverage, identify untested code paths, evaluate test quality, and suggest testing strategies.".into(),
            provider: "openai".into(),
            model: "gpt-4.1".into(),
            temperature: 0.3,
            is_template: true,
            is_enabled: true,
            rules_json: "[]".into(),
            criteria_json: "[]".into(),
            tools_json: "[]".into(),
            max_tokens_per_run: 30000,
            created_at: now.into(),
            updated_at: now.into(),
        },
        AgentProfile {
            id: "spec-documentation".into(),
            name: "Documentation Reviewer".into(),
            description: "Reviews and generates documentation, checks for missing or outdated docs.".into(),
            stage: AgentStage::Specialist,
            system_prompt: "You are a documentation specialist. Review existing documentation for completeness and accuracy, identify missing docs, and generate clear technical documentation.".into(),
            provider: "anthropic".into(),
            model: "claude-sonnet-4-5".into(),
            temperature: 0.5,
            is_template: true,
            is_enabled: false,
            rules_json: "[]".into(),
            criteria_json: "[]".into(),
            tools_json: "[]".into(),
            max_tokens_per_run: 30000,
            created_at: now.into(),
            updated_at: now.into(),
        },
        AgentProfile {
            id: "spec-patterns".into(),
            name: "Pattern Detector".into(),
            description: "Identifies design patterns, anti-patterns, and code smells across the codebase.".into(),
            stage: AgentStage::Specialist,
            system_prompt: "You are a pattern detection specialist. Identify design patterns being used, detect anti-patterns and code smells, and suggest pattern improvements.".into(),
            provider: "anthropic".into(),
            model: "claude-sonnet-4-5".into(),
            temperature: 0.3,
            is_template: true,
            is_enabled: true,
            rules_json: "[]".into(),
            criteria_json: "[]".into(),
            tools_json: "[]".into(),
            max_tokens_per_run: 30000,
            created_at: now.into(),
            updated_at: now.into(),
        },
        AgentProfile {
            id: "reporter-summary".into(),
            name: "Summary Reporter".into(),
            description: "Synthesizes specialist findings into a coherent summary report.".into(),
            stage: AgentStage::Reporter,
            system_prompt: r#"You are a report synthesizer. Combine findings from multiple specialist agents into a structured JSON report.

You MUST output a ```json``` fenced code block with the following schema:

```json
{
  "overall_score": <0-100>,
  "summary": "<1-2 sentence overall assessment>",
  "categories": [
    {
      "name": "<category name matching the specialist that produced it>",
      "score": <0-100>,
      "status": "<good|warning|critical>",
      "findings_count": <number of findings in this category>
    }
  ],
  "findings": [
    {
      "title": "<short finding title>",
      "category": "<category name>",
      "severity": "<critical|warning|info|good>",
      "description": "<1-2 sentence description>"
    }
  ]
}
```

Rules:
- Categories are DYNAMIC — one per specialist agent that provided input
- Score thresholds: 80-100 = "good", 50-79 = "warning", 0-49 = "critical"
- Order findings by severity: critical → warning → info → good
- Keep descriptions concise and actionable
- After the JSON block, you may optionally add markdown with detailed recommendations"#.into(),
            provider: "anthropic".into(),
            model: "claude-sonnet-4-5".into(),
            temperature: 0.3,
            is_template: true,
            is_enabled: true,
            rules_json: "[]".into(),
            criteria_json: "[]".into(),
            tools_json: "[]".into(),
            max_tokens_per_run: 30000,
            created_at: now.into(),
            updated_at: now.into(),
        },
        AgentProfile {
            id: "reporter-context".into(),
            name: "Context Writer".into(),
            description: "Generates and updates .context.md files from analysis results.".into(),
            stage: AgentStage::Reporter,
            system_prompt: "You are a context file writer. Generate well-structured .context.md files that capture module purpose, architecture decisions, key patterns, and maintenance notes.".into(),
            provider: "anthropic".into(),
            model: "claude-sonnet-4-5".into(),
            temperature: 0.4,
            is_template: true,
            is_enabled: true,
            rules_json: "[]".into(),
            criteria_json: "[]".into(),
            tools_json: "[]".into(),
            max_tokens_per_run: 30000,
            created_at: now.into(),
            updated_at: now.into(),
        },
    ]
}

fn default_rules(now: &str) -> Vec<AgentRule> {
    vec![
        AgentRule {
            id: "rule-no-secrets".into(),
            name: "No Hardcoded Secrets".into(),
            description: "Detect hardcoded API keys, passwords, tokens, and other secrets in source code.".into(),
            scope: vec!["file".into()],
            severity: Severity::Critical,
            is_active: true,
            is_template: true,
            created_at: now.into(),
            updated_at: now.into(),
        },
        AgentRule {
            id: "rule-error-handling".into(),
            name: "Proper Error Handling".into(),
            description: "Ensure errors are handled explicitly — no silent catches, no unwrap in production paths.".into(),
            scope: vec!["module".into()],
            severity: Severity::Warning,
            is_active: true,
            is_template: true,
            created_at: now.into(),
            updated_at: now.into(),
        },
        AgentRule {
            id: "rule-naming-conventions".into(),
            name: "Follow Naming Conventions".into(),
            description: "Enforce consistent naming conventions: snake_case for Rust, camelCase for TypeScript, etc.".into(),
            scope: vec!["file".into()],
            severity: Severity::Info,
            is_active: true,
            is_template: true,
            created_at: now.into(),
            updated_at: now.into(),
        },
        AgentRule {
            id: "rule-resolve-todos".into(),
            name: "Resolve TODO Comments".into(),
            description: "Flag TODO, FIXME, HACK, and XXX comments that should be resolved before release.".into(),
            scope: vec!["file".into()],
            severity: Severity::Warning,
            is_active: true,
            is_template: true,
            created_at: now.into(),
            updated_at: now.into(),
        },
        AgentRule {
            id: "rule-document-apis".into(),
            name: "Document Public APIs".into(),
            description: "Ensure all public functions, types, and modules have documentation comments.".into(),
            scope: vec!["module".into()],
            severity: Severity::Warning,
            is_active: true,
            is_template: true,
            created_at: now.into(),
            updated_at: now.into(),
        },
        AgentRule {
            id: "rule-low-complexity".into(),
            name: "Keep Complexity Low".into(),
            description: "Flag functions with high cyclomatic complexity. Prefer small, focused functions.".into(),
            scope: vec!["file".into()],
            severity: Severity::Warning,
            is_active: true,
            is_template: true,
            created_at: now.into(),
            updated_at: now.into(),
        },
    ]
}

fn default_team(now: &str) -> AgentTeam {
    AgentTeam {
        id: "team-default".into(),
        name: "Default Analysis Team".into(),
        description: "Standard team for comprehensive code analysis. Includes triaging, architecture, security, performance review, and summary reporting.".into(),
        profile_ids: vec![
            "triager-general".into(),
            "triager-priority".into(),
            "spec-architecture".into(),
            "spec-security".into(),
            "spec-performance".into(),
            "reporter-summary".into(),
        ],
        is_template: true,
        created_at: now.into(),
        updated_at: now.into(),
    }
}

fn default_tool_categories(now: &str) -> Vec<ToolCategory> {
    vec![
        ToolCategory { id: "cat-terminal".into(), name: "Terminal".into(), description: "Shell execution and app startup tools".into(), icon: "terminal".into(), color: "#f59e0b".into(), display_order: 0, is_template: true, created_at: now.into(), updated_at: now.into() },
        ToolCategory { id: "cat-verification".into(), name: "Verification".into(), description: "Health checks and app validation".into(), icon: "shield-check".into(), color: "#10b981".into(), display_order: 1, is_template: true, created_at: now.into(), updated_at: now.into() },
        ToolCategory { id: "cat-file".into(), name: "File".into(), description: "Read, write, edit, and list files".into(), icon: "file".into(), color: "#3b82f6".into(), display_order: 2, is_template: true, created_at: now.into(), updated_at: now.into() },
        ToolCategory { id: "cat-search".into(), name: "Search".into(), description: "Code index search and text/regex search".into(), icon: "search".into(), color: "#8b5cf6".into(), display_order: 3, is_template: true, created_at: now.into(), updated_at: now.into() },
        ToolCategory { id: "cat-web".into(), name: "Web".into(), description: "Fetch URLs and search the web".into(), icon: "globe".into(), color: "#06b6d4".into(), display_order: 4, is_template: true, created_at: now.into(), updated_at: now.into() },
        ToolCategory { id: "cat-interaction".into(), name: "Interaction".into(), description: "User communication during agentic loop".into(), icon: "message-circle".into(), color: "#ec4899".into(), display_order: 5, is_template: true, created_at: now.into(), updated_at: now.into() },
        ToolCategory { id: "cat-task".into(), name: "Task".into(), description: "Task tracking and progress management".into(), icon: "list-checks".into(), color: "#f97316".into(), display_order: 6, is_template: true, created_at: now.into(), updated_at: now.into() },
        ToolCategory { id: "cat-plan".into(), name: "Plan".into(), description: "Plan mode for complex multi-step tasks".into(), icon: "map".into(), color: "#64748b".into(), display_order: 7, is_template: true, created_at: now.into(), updated_at: now.into() },
        ToolCategory { id: "cat-agent".into(), name: "Agent".into(), description: "Spawn specialized sub-agents".into(), icon: "bot".into(), color: "#a855f7".into(), display_order: 8, is_template: true, created_at: now.into(), updated_at: now.into() },
        ToolCategory { id: "cat-logbook".into(), name: "Logbook".into(), description: "Per-node logbooks: list, read, search".into(), icon: "book-open".into(), color: "#22c55e".into(), display_order: 9, is_template: true, created_at: now.into(), updated_at: now.into() },
        ToolCategory { id: "cat-knowledge".into(), name: "Knowledge".into(), description: "Hexagons, evidence and research workflow".into(), icon: "hexagon".into(), color: "#a855f7".into(), display_order: 10, is_template: true, created_at: now.into(), updated_at: now.into() },
        ToolCategory { id: "cat-mesh".into(), name: "Mesh".into(), description: "Consult agents in other connected Venore projects".into(), icon: "network".into(), color: "#0ea5e9".into(), display_order: 11, is_template: true, created_at: now.into(), updated_at: now.into() },
    ]
}

/// Read-only tools that are safe for plan mode / research agents
const READ_ONLY_TOOLS: &[&str] = &[
    N::READ_FILE, N::LIST_FILES, N::SEARCH_CODE, N::SEARCH_TEXT,
    N::WEB_FETCH, N::WEB_SEARCH, N::READ_TERMINAL_OUTPUT, N::CHECK_HEALTH,
    N::ASK_USER, N::TASK_LIST, N::ENTER_PLAN_MODE, N::SPAWN_AGENT,
    N::LIST_LOGBOOKS, N::READ_LOGBOOK, N::SEARCH_LOGBOOK, N::LIST_CONNECTIONS,
    N::LIST_ISLANDS, N::QUERY_NEIGHBORHOOD,
];

fn default_tool_definitions(now: &str) -> Vec<ToolDefinition> {
    // Category mapping: tool_name -> category_id
    let category_map: &[(&str, &str)] = &[
        (N::RUN_TERMINAL_COMMAND, "cat-terminal"),
        (N::READ_TERMINAL_OUTPUT, "cat-terminal"),
        (N::RUN_APP, "cat-terminal"),
        (N::CHECK_HEALTH, "cat-verification"),
        (N::READ_FILE, "cat-file"),
        (N::WRITE_FILE, "cat-file"),
        (N::EDIT_FILE, "cat-file"),
        (N::MULTI_EDIT_FILE, "cat-file"),
        (N::LIST_FILES, "cat-file"),
        (N::SEARCH_CODE, "cat-search"),
        (N::SEARCH_TEXT, "cat-search"),
        (N::WEB_FETCH, "cat-web"),
        (N::WEB_SEARCH, "cat-web"),
        (N::ASK_USER, "cat-interaction"),
        (N::TASK_CREATE, "cat-task"),
        (N::TASK_UPDATE, "cat-task"),
        (N::TASK_LIST, "cat-task"),
        (N::ENTER_PLAN_MODE, "cat-plan"),
        (N::SUBMIT_PLAN, "cat-plan"),
        (N::SPAWN_AGENT, "cat-agent"),
        (N::ASK_PROJECT, "cat-mesh"),
        (N::LIST_LOGBOOKS, "cat-logbook"),
        (N::READ_LOGBOOK, "cat-logbook"),
        (N::SEARCH_LOGBOOK, "cat-logbook"),
        (N::LIST_CONNECTIONS, "cat-logbook"),
        (N::LIST_ISLANDS, "cat-logbook"),
        (N::QUERY_NEIGHBORHOOD, "cat-logbook"),
        (N::PROPOSE_LOGBOOK_WRITE, "cat-logbook"),
        (N::CREATE_LIGHTHOUSE, "cat-logbook"),
        (N::CREATE_KNOWLEDGE_NODE, "cat-logbook"),
        (N::CREATE_CONNECTION, "cat-logbook"),
        (N::PROMOTE_TO_LIGHTHOUSE, "cat-logbook"),
        (N::SET_NODE_LIGHTHOUSE, "cat-logbook"),
        (N::RENAME_NODE, "cat-logbook"),
        (N::PLAN_HEXAGONS, "cat-knowledge"),
        (N::UPDATE_HEXAGON, "cat-knowledge"),
        (N::ADD_EVIDENCE, "cat-knowledge"),
        (N::MARK_DEAD_END, "cat-knowledge"),
        (N::GENERATE_REPORT, "cat-knowledge"),
    ];

    let all_tools = definitions::all_tools();
    let mut result = Vec::new();

    for llm_tool in &all_tools {
        let category_id = category_map
            .iter()
            .find(|(name, _)| *name == llm_tool.name.as_str())
            .map(|(_, cat)| *cat)
            .unwrap_or_else(|| {
                tracing::warn!(tool = %llm_tool.name, "Tool not in category_map, defaulting to cat-terminal");
                "cat-terminal"
            });

        let is_read_only = READ_ONLY_TOOLS.contains(&llm_tool.name.as_str());
        let params_json = serde_json::to_string(&llm_tool.parameters)
            .unwrap_or_else(|_| "{}".to_string());

        let tool_id = format!("tool-{}", llm_tool.name.replace('_', "-"));

        result.push(ToolDefinition {
            id: tool_id,
            name: llm_tool.name.clone(),
            description: llm_tool.description.clone(),
            category_id: category_id.to_string(),
            parameters_json: params_json,
            is_read_only,
            is_enabled: true,
            is_template: true,
            created_at: now.into(),
            updated_at: now.into(),
        });
    }

    result
}

fn sub_agent_profiles(now: &str) -> Vec<AgentProfile> {
    vec![
        AgentProfile {
            id: "sub-agent-executor".into(),
            name: "Executor Agent".into(),
            description: "Starts and verifies applications — analyzes project, installs deps, runs the app, and checks health.".into(),
            stage: AgentStage::SubAgent,
            system_prompt: r#"You are an executor agent. Your job is to start and verify an application.

## Task
{task}

## Mandatory Workflow — follow IN ORDER, do not skip steps

### Step 1: Analyze the project
- `list_files` to see project structure
- `read_file` on the config file:
  - Node.js: package.json (check "scripts")
  - Python: requirements.txt / pyproject.toml
  - Docker: Dockerfile / docker-compose.yml
  - Rust: Cargo.toml  |  Go: go.mod
- Identify: app type, start command, port, dependencies status

### Step 2: Install dependencies (if needed)
- Node.js: npm install (if no node_modules/)
- Python: pip install -r requirements.txt
- Docker: docker build (if Dockerfile present)
- If install fails, report the error

### Step 3: Start the app
- Use `run_app` with the correct command and port
- Do NOT use `run_terminal_command` for starting apps

### Step 4: Verify health
- After run_app returns RUNNING, IMMEDIATELY call `check_health`
- URL: http://localhost:PORT
- If UNHEALTHY: use `read_terminal_output` to check logs, report the error
- If HEALTHY: report success with the URL

## Rules
- NEVER skip the analysis step (Step 1)
- NEVER skip health check after run_app
- NEVER modify source code — read-only for files
- Report errors clearly so the main agent can fix them"#.into(),
            provider: "".into(),
            model: "".into(),
            temperature: 0.2,
            is_template: true,
            is_enabled: true,
            rules_json: "[]".into(),
            criteria_json: "[]".into(),
            tools_json: r#"["tool-read-file","tool-list-files","tool-search-code","tool-search-text","tool-run-terminal-command","tool-read-terminal-output","tool-run-app","tool-check-health"]"#.into(),
            max_tokens_per_run: 30000,
            created_at: now.into(),
            updated_at: now.into(),
        },
        AgentProfile {
            id: "sub-agent-research".into(),
            name: "Research Agent".into(),
            description: "Reads code, searches the codebase, and fetches web resources to gather information.".into(),
            stage: AgentStage::SubAgent,
            system_prompt: "You are a research sub-agent. Gather information to answer the task below.\nUse read/search/web tools. Return a concise summary of findings.\n\nTask: {task}".into(),
            provider: "".into(),
            model: "".into(),
            temperature: 0.3,
            is_template: true,
            is_enabled: true,
            rules_json: "[]".into(),
            criteria_json: "[]".into(),
            tools_json: r#"["tool-read-file","tool-list-files","tool-search-code","tool-search-text","tool-web-fetch","tool-web-search"]"#.into(),
            max_tokens_per_run: 30000,
            created_at: now.into(),
            updated_at: now.into(),
        },
        AgentProfile {
            id: "sub-agent-general".into(),
            name: "General Agent".into(),
            description: "General-purpose sub-agent with access to all enabled tools.".into(),
            stage: AgentStage::SubAgent,
            system_prompt: "You are a general-purpose sub-agent. Complete the task below using the tools available.\nBe concise and return actionable results.\n\nTask: {task}".into(),
            provider: "".into(),
            model: "".into(),
            temperature: 0.3,
            is_template: true,
            is_enabled: true,
            rules_json: "[]".into(),
            criteria_json: "[]".into(),
            tools_json: "[]".into(),
            max_tokens_per_run: 30000,
            created_at: now.into(),
            updated_at: now.into(),
        },
    ]
}

// =============================================================================
// Chat modes
// =============================================================================
//
// Each project kind ("code" | "knowledge") has a default mode that decides
// which categories of tools the chat sees. Plan mode is orthogonal — it
// overrides any active mode at runtime.

fn default_chat_modes(now: &str) -> Vec<ChatMode> {
    vec![
        ChatMode {
            id: "mode-code".into(),
            name: "Code".into(),
            description: "Full developer toolset: file editing, terminal, search, web. Default for Codebase projects.".into(),
            category_ids: vec![
                "cat-terminal".into(),
                "cat-verification".into(),
                "cat-file".into(),
                "cat-search".into(),
                "cat-web".into(),
                "cat-interaction".into(),
                "cat-task".into(),
                "cat-plan".into(),
                "cat-agent".into(),
                "cat-logbook".into(),
                "cat-mesh".into(),
            ],
            tool_ids: vec![],
            sub_agent_ids: vec![
                "sub-agent-executor".into(),
                "sub-agent-research".into(),
                "sub-agent-general".into(),
            ],
            rule_ids: vec![],
            prompt_id: None,
            is_template: true,
            is_default_for_kind: Some("code".into()),
            created_at: now.into(),
            updated_at: now.into(),
        },
        ChatMode {
            id: "mode-knowledge".into(),
            name: "Knowledge".into(),
            description: "Logbook-focused toolset: logbook, search read-only, web, research. Default for Knowledge projects.".into(),
            category_ids: vec![
                "cat-logbook".into(),
                "cat-knowledge".into(),
                "cat-search".into(),
                "cat-web".into(),
                "cat-interaction".into(),
                "cat-task".into(),
                "cat-mesh".into(),
            ],
            tool_ids: vec![],
            sub_agent_ids: vec!["sub-agent-research".into(), "sub-agent-general".into()],
            rule_ids: vec![],
            prompt_id: None,
            is_template: true,
            is_default_for_kind: Some("knowledge".into()),
            created_at: now.into(),
            updated_at: now.into(),
        },
    ]
}
