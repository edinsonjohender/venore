//! Chat Context Builder
//!
//! Reads `.context.md` files from disk and composes an enriched system prompt
//! with project and module context.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::chat::orchestrator::SYSTEM_PROMPT;
use crate::knowledge::KnowledgeRepository;
use crate::layers::ModuleLayerAnalysis;
use crate::llm::types::LlmTool;
use crate::memory::{MemoryRepository, ProjectMemory, format_project_memory};
use crate::mesh::ProjectProfile;
use crate::prompts::{
    render_template, ChatFragmentId, ChatFragmentMap, PromptRepository,
};
use crate::context::ContextRepository;
use crate::rag::{RagRepository, SearchResult};
use crate::session::SessionRepository;
use crate::traits::LlmProviderType;
use crate::Result;

// ============================================================================
// TYPES
// ============================================================================

/// Info about an available module that has a .context.md file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableModule {
    pub name: String,
    pub path: String,
    pub has_context: bool,
}

/// Context about the active dev session (injected into AI system prompt)
#[derive(Debug, Clone)]
pub struct SessionContext {
    pub name: String,
    pub objective: String,
    pub branch: String,
    pub base_branch: String,
    pub changed_files: Vec<String>,
    pub worktree_path: Option<String>,
}

/// Context about an active knowledge research session
#[derive(Debug, Clone)]
pub struct KnowledgeResearchContext {
    pub feature_name: String,
    pub feature_description: String,
    pub objective: String,
    pub intensity: String,
    pub hexagons: Vec<KnowledgeHexagonSummary>,
    pub evidence_count: usize,
    pub connected_projects: Vec<String>,
}

/// Summary of a hexagon for system prompt injection
#[derive(Debug, Clone)]
pub struct KnowledgeHexagonSummary {
    pub id: String,
    pub title: String,
    pub phase: String,
    pub percentage: i32,
    pub confidence: String,
    pub risk: String,
    pub is_dead_end: bool,
    pub agent_status: String,
    pub evidence_count: usize,
}

/// Mesh peer info passed to the context builder for system prompt injection
#[derive(Debug, Clone)]
pub struct MeshPeerContext {
    pub project_id: String,
    pub project_name: String,
    pub profile: Option<ProjectProfile>,
}

// ============================================================================
// PROVIDER-SPECIFIC FINAL REMINDERS
// ============================================================================

/// Final reminder appended at the very end of the assembled prompt for Gemini.
/// Gemini gives more weight to the last content it reads (recency bias),
/// so the most important behavioral rules go here as a hard anchor.
const GEMINI_FINAL_REMINDER: &str = r#"# FINAL REMINDER

You are an agent. Keep going until the task is fully resolved.
- When a tool fails or a build breaks, READ the error, diagnose it, fix it, and verify again.
- Never invent tool results. If you didn't call a tool, you don't know the result.
- Never declare success without running verification (build, tests, or check_health). One successful check is not enough if the user says it still fails — check again differently.
- Use tools for ALL actions. Never paste code as text — use edit_file or write_file.
- Call tools through the function-calling interface — make the call directly. Do not announce, describe, or write out a tool call as text; text that imitates a tool call is shown verbatim to the user and breaks the UI.
- After editing a file, use `read_file` to verify your edit was applied correctly and didn't introduce syntax errors.
- NEVER give up. NEVER say "this is beyond my capabilities" or "a human developer should investigate". If your approach failed 5 times, try a COMPLETELY DIFFERENT approach. You have unlimited tools — use them.
- Do NOT apologize repeatedly. Apologies waste tokens. Instead: fix the problem."#;

/// Final reminder for Gemini in a **Knowledge** project. The code reminder
/// above talks about files/builds/tests, which Knowledge mode doesn't have —
/// so knowledge sessions get this canvas-oriented anchor instead. Its key job
/// is to stop the duplicate-island behavior: Gemini, after a confusing turn,
/// re-ran a whole island it had already created (because it couldn't tell it
/// was already done). Recency slot, positive phrasing — what Gemini obeys.
const KNOWLEDGE_GEMINI_FINAL_REMINDER: &str = r#"# FINAL REMINDER

You organize an Ocean Canvas of islands (lighthouses) and knowledge_nodes. Keep going until the task is fully resolved.
- Map before you create. Before create_lighthouse or create_knowledge_node, check what already exists with list_islands / list_logbooks.
- NEVER recreate something you already created earlier in THIS conversation. If a tool result above shows you created an island or node, reuse that id — running the same creation again makes duplicate islands and nodes.
- To attach a node, pass the real lighthouse_id a previous create_lighthouse returned. Never a placeholder like ${...} or @{...}, and never an id you have not seen in a tool result this conversation.
- Never invent tool results. Don't say "created / added / done" without a success=true in this turn.
- Use tools for every action. Never paste tool-call syntax as text — call the function directly.
- Speak to the user with names, never UUIDs."#;

/// Returns the final reminder for a given provider, or None if not applicable.
/// `knowledge_mode` selects the canvas-oriented anchor over the code one for
/// Gemini Knowledge sessions.
fn provider_final_reminder(
    provider: &LlmProviderType,
    knowledge_mode: bool,
) -> Option<&'static str> {
    match provider {
        LlmProviderType::Gemini if knowledge_mode => Some(KNOWLEDGE_GEMINI_FINAL_REMINDER),
        LlmProviderType::Gemini => Some(GEMINI_FINAL_REMINDER),
        _ => None,
    }
}

// ============================================================================
// CONTEXT BUILDER
// ============================================================================

/// Builds an enriched system prompt from .context.md files on disk
pub struct ChatContextBuilder {
    base_prompt: Option<String>,
    project_path: Option<PathBuf>,
    project_memory: Option<ProjectMemory>,
    modules: Vec<(String, PathBuf)>,
    rag_results: Vec<SearchResult>,
    layer_summaries: Vec<ModuleLayerAnalysis>,
    terminal_output: Option<String>,
    tools: Vec<LlmTool>,
    session_context: Option<SessionContext>,
    knowledge_context: Option<KnowledgeResearchContext>,
    mesh_peers: Vec<MeshPeerContext>,
    provider: Option<LlmProviderType>,
    /// True when this is a Knowledge project. Only affects which provider
    /// final reminder is appended (canvas-oriented vs code-oriented).
    knowledge_mode: bool,
    chat_fragments: Option<ChatFragmentMap>,
    /// AI-connection attachments resolved by `connection_resolver`. Each
    /// entry is a self-contained markdown block: header (e.g. "Auth (faro)")
    /// + body (sections / .context.md / hex evidence). Rendered as a
    /// dedicated `## Adjuntos del chat` section so the AI sees them up
    /// front and treats them as conversation-scoped context.
    connection_blocks: Vec<crate::chat::connection_resolver::ConnectionBlock>,
}

impl ChatContextBuilder {
    pub fn new() -> Self {
        Self {
            base_prompt: None,
            project_path: None,
            project_memory: None,
            modules: Vec::new(),
            rag_results: Vec::new(),
            layer_summaries: Vec::new(),
            terminal_output: None,
            tools: Vec::new(),
            session_context: None,
            knowledge_context: None,
            mesh_peers: Vec::new(),
            provider: None,
            knowledge_mode: false,
            chat_fragments: None,
            connection_blocks: Vec::new(),
        }
    }

    /// Attach AI-connection blocks resolved by the registry. Replaces (not
    /// appends) any prior set so a re-build with no blocks correctly clears
    /// the section.
    pub fn with_connection_blocks(
        mut self,
        blocks: Vec<crate::chat::connection_resolver::ConnectionBlock>,
    ) -> Self {
        self.connection_blocks = blocks;
        self
    }

    /// Inject the chat-fragment map (loaded from `prompts` table) so blocks
    /// of the system prompt come from editable templates rather than inline
    /// string literals. If a fragment id is missing or the map is None, the
    /// builder falls back to its hardcoded legacy text. Disabled fragments
    /// cause the corresponding block to be skipped entirely.
    pub fn with_chat_fragments(mut self, map: ChatFragmentMap) -> Self {
        self.chat_fragments = Some(map);
        self
    }

    /// Render a chat-fragment block. Returns:
    /// - `Some(rendered)` when the fragment is found+enabled (or no map at all
    ///   → falls back to `legacy`).
    /// - `None` when the user explicitly disabled the fragment in the UI.
    fn render_block<F>(&self, id: &str, vars: HashMap<&str, String>, legacy: F) -> Option<String>
    where
        F: FnOnce() -> String,
    {
        match &self.chat_fragments {
            None => Some(legacy()),
            Some(map) => match map.get(id) {
                None => Some(legacy()),
                Some(entry) if !entry.is_enabled => None,
                Some(entry) => Some(render_template(&entry.content, &vars)),
            },
        }
    }

    /// Override the base system prompt (defaults to SYSTEM_PROMPT constant)
    pub fn with_base_prompt(mut self, prompt: String) -> Self {
        self.base_prompt = Some(prompt);
        self
    }

    /// Set the project path (reads root .context.md)
    pub fn with_project(mut self, path: &Path) -> Self {
        self.project_path = Some(path.to_path_buf());
        self
    }

    /// Set the project memory (compact knowledge block)
    pub fn with_project_memory(mut self, memory: ProjectMemory) -> Self {
        self.project_memory = Some(memory);
        self
    }

    /// Add a module whose .context.md should be included
    pub fn with_module(mut self, name: &str, path: &Path) -> Self {
        self.modules.push((name.to_string(), path.to_path_buf()));
        self
    }

    /// Add RAG search results to include in the system prompt
    pub fn with_rag_results(mut self, results: &[SearchResult]) -> Self {
        self.rag_results = results.to_vec();
        self
    }

    /// Add layer analysis summaries for modules (health overview for the AI)
    pub fn with_layer_summary(mut self, summaries: &[ModuleLayerAnalysis]) -> Self {
        self.layer_summaries = summaries.to_vec();
        self
    }

    /// Add recent terminal output for context (no ID exposed to the AI)
    pub fn with_terminal_output(mut self, output: &str) -> Self {
        self.terminal_output = Some(output.to_string());
        self
    }

    /// Provide the actual tool list (generates tool table dynamically in the prompt)
    pub fn with_tools(mut self, tools: &[LlmTool]) -> Self {
        self.tools = tools.to_vec();
        self
    }

    /// Add dev session context (branch, objective, changed files)
    pub fn with_session_context(mut self, ctx: SessionContext) -> Self {
        self.session_context = Some(ctx);
        self
    }

    /// Add knowledge research context (injects hexagon table and research instructions)
    pub fn with_knowledge_context(mut self, ctx: KnowledgeResearchContext) -> Self {
        self.knowledge_context = Some(ctx);
        self
    }

    /// Add connected mesh peers (enables ask_project awareness in system prompt)
    pub fn with_mesh_peers(mut self, peers: Vec<MeshPeerContext>) -> Self {
        self.mesh_peers = peers;
        self
    }

    /// Set the LLM provider (enables provider-specific tool rules and final reminders)
    pub fn with_provider(mut self, provider: LlmProviderType) -> Self {
        self.provider = Some(provider);
        self
    }

    /// Mark this as a Knowledge project so the canvas-oriented final reminder
    /// is appended instead of the code one (Gemini only).
    pub fn with_knowledge_mode(mut self, knowledge_mode: bool) -> Self {
        self.knowledge_mode = knowledge_mode;
        self
    }

    /// Build the enriched system prompt
    pub fn build_system_prompt(&self) -> Result<String> {
        let mut prompt = self.base_prompt.clone().unwrap_or_else(|| SYSTEM_PROMPT.to_string());

        // Inject project identity early so the AI knows the project name
        if let Some(ref project_path) = self.project_path {
            let project_name = project_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            let project_path_str = project_path.display().to_string();
            let mut vars = HashMap::new();
            vars.insert("project_name", project_name.clone());
            vars.insert("project_path", project_path_str.clone());
            if let Some(rendered) = self.render_block(ChatFragmentId::WORKING_PROJECT, vars, || {
                format!(
                    "\n\n## Working Project\nYou are operating on the project \"{}\" at `{}`.\nUse this project name for any generated artifacts (Docker images, packages, branches, etc.).\n",
                    project_name, project_path_str,
                )
            }) {
                prompt.push_str(&rendered);
            }
        }

        // Inject project memory (compact knowledge block) right after Working Project
        let has_memory_summary = if let Some(ref memory) = self.project_memory {
            let block = format_project_memory(memory);
            prompt.push('\n');
            prompt.push_str(&block);
            !memory.project_summary.is_empty()
        } else {
            false
        };

        // Check if we have any context to inject.
        //
        // DEPRECATED: the root `.context.md` injection below is a legacy
        // fallback, superseded by Project Memory (`.venore/project-memory.json`,
        // injected just above). It only fires when memory has no summary, i.e.
        // for legacy projects wizarded before Project Memory existed. New
        // projects never reach it. Slated for removal once no projects rely on
        // the old `.context.md` layout. (Module-level `.context.md` injection —
        // user-selected via the chat "context options" feature — is a separate,
        // still-live path and is NOT deprecated.)
        let has_project = if has_memory_summary {
            // Memory has a summary → skip injecting the full .context.md
            false
        } else {
            self.project_path.as_ref()
                .map(|p| p.join(".context.md").exists())
                .unwrap_or(false)
        };
        let has_modules = !self.modules.is_empty();
        let has_rag = !self.rag_results.is_empty();
        let has_layers = !self.layer_summaries.is_empty();

        if has_project || has_modules || has_rag || has_layers {
            if let Some(s) = self.render_block(
                ChatFragmentId::REFERENCE_HEADER,
                HashMap::new(),
                || "\n\n---\n\n# Reference Material (background knowledge — do NOT recite)\nUse the following to inform your answers. Extract only the specific facts needed to answer the user's question.\n".to_string(),
            ) {
                prompt.push_str(&s);
            }
        }

        // Project root context (skipped when memory has project_summary)
        if has_project {
            if let Some(ref project_path) = self.project_path {
                let context_file = project_path.join(".context.md");
                match std::fs::read_to_string(&context_file) {
                    Ok(content) => {
                        prompt.push_str("\n\n## Project Context\n\n");
                        prompt.push_str(&content);
                    }
                    Err(_) => {
                        tracing::warn!(
                            "Project .context.md not found at: {}",
                            context_file.display()
                        );
                    }
                }
            }
        }

        // Active dev session context
        if let Some(ref ctx) = self.session_context {
            let mut details = String::new();
            if !ctx.objective.is_empty() {
                details.push_str(&format!("- **Objective:** {}\n", ctx.objective));
            }
            details.push_str(&format!("- **Branch:** {} ← {}\n", ctx.branch, ctx.base_branch));
            if let Some(ref wt) = ctx.worktree_path {
                details.push_str(&format!("- **Working directory:** {} (isolated worktree)\n", wt));
            }
            if !ctx.changed_files.is_empty() {
                details.push_str(&format!(
                    "- **Changed files so far:** {}\n",
                    ctx.changed_files.join(", ")
                ));
            }
            let mut vars = HashMap::new();
            vars.insert("session_name", ctx.name.clone());
            vars.insert("details_lines", details.clone());
            if let Some(s) = self.render_block(ChatFragmentId::ACTIVE_DEV_SESSION, vars, || {
                let mut out = format!(
                    "\n\n## Active Dev Session\nYou are operating inside dev session \"{}\".\n",
                    ctx.name,
                );
                out.push_str(&details);
                out.push_str("\nAll file tools (`write_file`, `edit_file`, `read_file`) target this worktree automatically.\n");
                out.push_str("You MUST use tools to create and modify files. NEVER paste code as text. Focus on the session objective.\n");
                out
            }) {
                prompt.push_str(&s);
            }
        }

        // Logbooks hint — if the project's ocean layout has any
        // knowledge_node or lighthouse, surface that fact to the AI so it
        // doesn't fall back to file/grep tools when the user mentions
        // "logbook", "bitácora", "nodo", or asks about node-scoped notes.
        if let Some(ref project_path) = self.project_path {
            let project_str = project_path.to_string_lossy().to_string();
            if let Ok((node_count, names)) = count_knowledge_nodes(&project_str) {
                if node_count > 0 {
                    let names_line = if names.is_empty() {
                        String::new()
                    } else {
                        format!("Some node names: {}.\n", names.join(", "))
                    };
                    let mut vars = HashMap::new();
                    vars.insert("node_count", node_count.to_string());
                    vars.insert("names_line", names_line.clone());
                    if let Some(s) = self.render_block(ChatFragmentId::LOGBOOK_HINT, vars, || {
                        let mut out = String::new();
                        out.push_str("\n\n## Project logbooks\n");
                        out.push_str(&format!(
                            "This project has **{} knowledge node(s)** (also called \"logbooks\"). Each node stores markdown sections the user or the AI has written about a specific topic.\n",
                            node_count,
                        ));
                        out.push_str(&names_line);
                        out.push_str("\n### Related tools\n");
                        out.push_str("- `list_logbooks` — list ALL the project's logbooks (id + name + variant + section count). Use this for \"what logbooks do I have\", \"what nodes do we have\".\n");
                        out.push_str("- `search_logbook` — search for text in logbook content (does not return node names: returns hits with snippets). NEVER pass an empty query. USE THIS for \"find X in my logbooks\", NOT for listing nodes.\n");
                        out.push_str("- `read_logbook` — read one whole logbook by its node_id (UUID). Get the id from `list_logbooks` or `search_logbook` — never pass a name.\n");
                        out.push_str("\nNEVER use `search_text` or `list_files` for logbook questions — those are for code.\n");
                        out.push_str("\n**CRITICAL**: `list_logbooks`, `read_logbook` and `search_logbook` are **native tools** of the system. Call them as a function call directly. NEVER put them inside `run_terminal_command` — they are not shell commands.\n");
                        out
                    }) {
                        prompt.push_str(&s);
                    }

                    // Inline index — gives the AI all node_ids + section_ids
                    // up front so it can skip `list_logbooks` and skip the
                    // discovery half of `read_logbook` each turn. Section
                    // *contents* are NOT included; those still come from
                    // `read_logbook` on demand.
                    if let Some(index) = build_logbook_index(&project_str) {
                        prompt.push_str(&index);
                    }
                }
            }
        }

        // AI-connection attachments (Sparkles ↔ Sparkles). Rendered as a
        // dedicated section with each connected entity's full content
        // inline, so the AI doesn't need to call read_logbook /
        // read_hexagon / read_file for things the user explicitly pinned
        // to the chat. Refreshed every turn — if a connected node was
        // edited mid-conversation the next message sees the new content.
        if !self.connection_blocks.is_empty() {
            prompt.push_str(
                "\n\n## Chat attachments (while connected)\n\nThe following entities are connected to this conversation. Their content travels with you every turn — you do not need to re-read them, you already have them. If the user says \"this node\", \"that section\", \"compare these\", assume they refer to something here.\n",
            );
            for block in &self.connection_blocks {
                prompt.push_str(&format!("\n### {}\n\n", block.header));
                prompt.push_str(&block.body_markdown);
                if !block.body_markdown.ends_with('\n') {
                    prompt.push('\n');
                }
            }
        }

        // Knowledge research context
        if let Some(ref kctx) = self.knowledge_context {
            // Build the optional / dynamic parts so they can be passed as vars
            let description_line = if kctx.feature_description.is_empty() {
                String::new()
            } else {
                format!("Description: {}\n", kctx.feature_description)
            };
            let connected_line = if kctx.connected_projects.is_empty() {
                String::new()
            } else {
                format!(
                    "Connected projects: {}\n",
                    kctx.connected_projects.join(", "),
                )
            };
            let hexagons_section = if kctx.hexagons.is_empty() {
                String::new()
            } else {
                let mut section = String::new();
                section.push_str("\n### Current Hexagons\n");
                section.push_str("| ID | Title | Phase | Progress | Confidence | Risk | Status | Evidence |\n");
                section.push_str("|----|-------|-------|----------|------------|------|--------|----------|\n");
                for h in &kctx.hexagons {
                    let dead = if h.is_dead_end { " DEAD END" } else { "" };
                    section.push_str(&format!(
                        "| {} | {} | {} | {}% | {} | {} | {}{} | {} |\n",
                        &h.id[..8.min(h.id.len())],
                        h.title,
                        h.phase,
                        h.percentage,
                        h.confidence,
                        h.risk,
                        h.agent_status,
                        dead,
                        h.evidence_count,
                    ));
                }
                section
            };

            let mut header_vars = HashMap::new();
            header_vars.insert("feature_name", kctx.feature_name.clone());
            header_vars.insert("description_line", description_line.clone());
            header_vars.insert("objective", kctx.objective.clone());
            header_vars.insert("intensity", kctx.intensity.clone());
            header_vars.insert("connected_line", connected_line.clone());
            header_vars.insert("hexagons_section", hexagons_section.clone());
            if let Some(s) = self.render_block(ChatFragmentId::KNOWLEDGE_RESEARCH, header_vars, || {
                let mut out = String::new();
                out.push_str("\n\n## Knowledge Research Session\n");
                out.push_str(&format!(
                    "You are conducting structured research on: **{}**\n",
                    kctx.feature_name
                ));
                out.push_str(&description_line);
                out.push_str(&format!(
                    "Objective: {} | Intensity: {}\n",
                    kctx.objective, kctx.intensity
                ));
                out.push_str(&connected_line);
                out.push_str(&hexagons_section);
                out
            }) {
                prompt.push_str(&s);
            }

            let mut rule_vars = HashMap::new();
            rule_vars.insert("objective", kctx.objective.clone());
            if let Some(s) = self.render_block(
                "chat-fragment-knowledge-research-rules",
                rule_vars,
                || {
                    let mut out = String::new();
                    out.push_str("\n### Research Instructions\n");
                    out.push_str("- Use `plan_hexagons` to create research points from the seed topic\n");
                    out.push_str("- Use `web_search` and `web_fetch` to find information\n");
                    out.push_str("- Use `update_hexagon` to record progress on each point\n");
                    out.push_str("- Use `add_evidence` to store findings with sources\n");
                    out.push_str("- Use `mark_dead_end` when a research path leads nowhere\n");
                    out.push_str("- Use `generate_report` when the research is mature enough\n");
                    out.push_str(&format!("- Focus on the research objective: {}\n", kctx.objective));
                    out.push_str("- Do NOT use terminal or file editing tools unless specifically asked\n");
                    out
                },
            ) {
                prompt.push_str(&s);
            }
        }

        // Module contexts
        for (name, path) in &self.modules {
            let context_file = path.join(".context.md");
            match std::fs::read_to_string(&context_file) {
                Ok(content) => {
                    prompt.push_str(&format!("\n\n## Module: {}\n\n", name));
                    prompt.push_str(&content);
                }
                Err(_) => {
                    tracing::warn!(
                        "Module .context.md not found for '{}' at: {}",
                        name,
                        context_file.display()
                    );
                }
            }
        }

        // Module health summary from layer analysis
        if has_layers {
            if let Some(s) = self.render_block(
                ChatFragmentId::MODULE_HEALTH,
                HashMap::new(),
                || "\n\n## Module Health Summary\n\n| Module | Context | Tests | Docs | Connections | Issues |\n|--------|---------|-------|------|-------------|--------|\n".to_string(),
            ) {
                prompt.push_str(&s);
            }

            for analysis in &self.layer_summaries {
                let mut context_str = "-".to_string();
                let mut tests_str = "-".to_string();
                let mut docs_str = "-".to_string();
                let mut connections_str = "-".to_string();
                let mut issues_str = "-".to_string();

                for layer in &analysis.layers {
                    match layer.layer_type {
                        crate::layers::LayerType::Context => {
                            context_str = layer.details
                                .get("freshness")
                                .and_then(|v| v.as_str())
                                .unwrap_or(layer.status.as_str())
                                .to_string();
                        }
                        crate::layers::LayerType::Tests => {
                            let ratio = layer.details
                                .get("coverage_ratio")
                                .and_then(|v| v.as_f64())
                                .unwrap_or(0.0);
                            tests_str = format!("{}%", (ratio * 100.0).round());
                        }
                        crate::layers::LayerType::Documentation => {
                            let has_readme = layer.details
                                .get("has_readme")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            let ratio = layer.details
                                .get("doc_ratio")
                                .and_then(|v| v.as_f64())
                                .unwrap_or(0.0);
                            docs_str = if has_readme {
                                format!("README + {}%", (ratio * 100.0).round())
                            } else if ratio > 0.0 {
                                format!("{}%", (ratio * 100.0).round())
                            } else {
                                "none".to_string()
                            };
                        }
                        crate::layers::LayerType::Connections => {
                            let deps = layer.details
                                .get("dependency_count")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0);
                            let dependents = layer.details
                                .get("dependent_count")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0);
                            connections_str = format!("{} deps, {} dep.", deps, dependents);
                        }
                        crate::layers::LayerType::Status => {
                            let total = layer.details
                                .get("total_issues")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0);
                            if total == 0 {
                                issues_str = "clean".to_string();
                            } else {
                                let todo = layer.details
                                    .get("todo_count")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                let fixme = layer.details
                                    .get("fixme_count")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                let mut parts = Vec::new();
                                if todo > 0 { parts.push(format!("{} TODO", todo)); }
                                if fixme > 0 { parts.push(format!("{} FIXME", fixme)); }
                                issues_str = parts.join(", ");
                                if issues_str.is_empty() {
                                    issues_str = format!("{} issues", total);
                                }
                            }
                        }
                    }
                }

                prompt.push_str(&format!(
                    "| {} | {} | {} | {} | {} | {} |\n",
                    analysis.module_name, context_str, tests_str, docs_str, connections_str, issues_str
                ));
            }
        }

        // Recent terminal output
        if let Some(ref output) = self.terminal_output {
            if !output.is_empty() {
                let mut vars = HashMap::new();
                vars.insert("output", output.clone());
                if let Some(s) = self.render_block(
                    ChatFragmentId::TERMINAL_OUTPUT,
                    vars,
                    || format!("\n\n## Recent Terminal Output\n```\n{}\n```\n", output),
                ) {
                    prompt.push_str(&s);
                }
            }
        }

        // Tool instructions (generated from actual tool definitions)
        if !self.tools.is_empty() {
            // Build the dynamic markdown table of available tools
            let mut tools_table = String::new();
            tools_table.push_str("| Tool | Use for |\n");
            tools_table.push_str("|------|---------|\n");
            for tool in &self.tools {
                let short_desc = tool
                    .description
                    .split('\n')
                    .next()
                    .unwrap_or(&tool.description)
                    .trim_end_matches('.');
                tools_table.push_str(&format!("| `{}` | {} |\n", tool.name, short_desc));
            }
            tools_table.push('\n');

            let mut tools_vars = HashMap::new();
            tools_vars.insert("tools_table", tools_table.clone());
            if let Some(s) = self.render_block(ChatFragmentId::TOOLS_TABLE, tools_vars, || {
                let mut out = String::new();
                out.push_str("\n\n## Tools\n\n");
                out.push_str("You MUST use these tools to perform actions. NEVER output code as text — use the tools.\n\n");
                out.push_str(&tools_table);
                out.push_str("**Tool usage rules:**\n");
                out.push_str("- Use `edit_file` over `write_file` for modifications (preserves unchanged content)\n");
                out.push_str("- Use file tools over terminal (`read_file` not `cat`, `edit_file` not `sed`)\n");
                out.push_str("- Use `search_text` to find usages, TODOs, imports, strings across the project\n");
                out.push_str("- Use `search_code` to find definitions (functions, classes, types) by name or meaning\n");
                out.push_str("- A terminal is created automatically when needed\n");
                out.push_str("- After running a command, verify the result before continuing\n");
                out.push_str("- After editing a file, use `read_file` to verify the edit was applied correctly (catch duplicate lines, broken syntax)\n");
                out.push_str("- For complex multi-step tasks, consider using `enter_plan_mode` first to design your approach\n");
                out.push_str("- Use `ask_user` ONLY for technical decisions with multiple valid approaches (e.g. architecture choices, library selection). NEVER use it for greetings, simple questions, or when the user's intent is clear — just respond with text\n");
                out.push_str("- Use `task_create`/`task_update` for multi-step work to show progress\n");
                out.push_str("- **App startup:** To run/start/launch an app, use `spawn_agent` with type `executor`. It analyzes the project, installs dependencies, starts the app, and verifies health automatically\n");
                out.push_str("- Be proactive: run builds, tests, installs as needed\n");
                out.push_str("- Use `spawn_agent` to parallelize independent sub-tasks\n");
                out.push_str("- When fixing a bug, first use `search_text` to find ALL occurrences before fixing any.\n");
                out.push('\n');
                out.push_str("REMINDER: When the user asks you to write, create, or modify code — call the tool. Do NOT paste code in chat.\n");
                out
            }) {
                prompt.push_str(&s);
            }

            // PROMPT_STOP_RULES is system-controlled (safety) — always appended verbatim
            prompt.push_str(crate::chat::guardrails::PROMPT_STOP_RULES);

            // Inject connected mesh peers for ask_project awareness
            if !self.mesh_peers.is_empty() {
                if let Some(s) = self.render_block(
                    ChatFragmentId::MESH_PEERS,
                    HashMap::new(),
                    || "\n\n## Connected Projects (Mesh)\n\nOther Venore instances are connected to you. Each one runs an agent that is an expert on its own project and can read that project's full codebase to answer you — these are reasoning agents, not a search index. Consult them with `ask_project` when you need facts about another project's code, architecture, APIs, or conventions instead of guessing.\n\nAvailable projects:\n\n".to_string(),
                ) {
                    prompt.push_str(&s);
                }

                for peer in &self.mesh_peers {
                    prompt.push_str(&format!("- **{}**", peer.project_name));
                    if let Some(ref profile) = peer.profile {
                        let mut meta = Vec::new();
                        if let Some(ref lang) = profile.language {
                            meta.push(lang.clone());
                        }
                        if !profile.technologies.is_empty() {
                            meta.push(profile.technologies.join(", "));
                        }
                        if !meta.is_empty() {
                            prompt.push_str(&format!(" — {}", meta.join(" | ")));
                        }
                        prompt.push('\n');
                        if !profile.module_names.is_empty() {
                            let names = if profile.module_names.len() <= 8 {
                                profile.module_names.join(", ")
                            } else {
                                let shown: Vec<_> = profile.module_names[..8].to_vec();
                                format!(
                                    "{}, +{} more",
                                    shown.join(", "),
                                    profile.module_names.len() - 8
                                )
                            };
                            prompt.push_str(&format!("  Modules: {}\n", names));
                        }
                        if let Some(ref desc) = profile.description {
                            prompt.push_str(&format!("  {}\n", desc));
                        }
                    } else {
                        prompt.push('\n');
                    }
                }

                if let Some(s) = self.render_block(
                    "chat-fragment-mesh-peers-footer",
                    HashMap::new(),
                    || "\nTo consult a project, call `ask_project` with its name exactly as listed above, your question, and an optional `context_hint` to focus the search. Example: `ask_project` with `{\"project\": \"<name>\", \"question\": \"...\", \"context_hint\": \"auth\"}`.\nThe remote agent searches its own code and returns a synthesized answer (it may take a few seconds). Consult a project when its knowledge makes your answer more accurate; if you already know the answer, respond directly.\n".to_string(),
                ) {
                    prompt.push_str(&s);
                }
            }
        }

        // RAG results (code from project index)
        if !self.rag_results.is_empty() {
            if let Some(s) = self.render_block(
                ChatFragmentId::RAG_SNIPPETS,
                HashMap::new(),
                || "\n\n## Relevant Code from Project\n".to_string(),
            ) {
                prompt.push_str(&s);
            }

            for result in &self.rag_results {
                let chunk = &result.chunk;
                let lang = chunk.relative_path
                    .rsplit('.')
                    .next()
                    .unwrap_or("");

                prompt.push_str(&format!(
                    "\n### {} `{}` ({}:{}-{})\n```{}\n{}\n```\n",
                    chunk.chunk_type,
                    chunk.name,
                    chunk.relative_path,
                    chunk.line_start,
                    chunk.line_end,
                    lang,
                    chunk.content,
                ));
            }
        }

        // Provider-specific final reminder (must be the LAST section of the prompt)
        if let Some(ref provider) = self.provider {
            if let Some(reminder) = provider_final_reminder(provider, self.knowledge_mode) {
                prompt.push_str("\n\n---\n\n");
                prompt.push_str(reminder);
            }
        }

        Ok(prompt)
    }
}

impl Default for ChatContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Scan a project directory for modules that have .context.md files
pub fn scan_available_modules(project_path: &Path) -> Result<Vec<AvailableModule>> {
    let mut modules = Vec::new();

    // Check root
    let root_context = project_path.join(".context.md");
    if root_context.exists() {
        let name = project_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("root")
            .to_string();
        modules.push(AvailableModule {
            name: format!("{} (root)", name),
            path: project_path.to_string_lossy().to_string(),
            has_context: true,
        });
    }

    // Scan immediate subdirectories and common depth-2 patterns
    if let Ok(entries) = std::fs::read_dir(project_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            // Skip hidden directories and common non-module dirs
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if dir_name.starts_with('.')
                || dir_name == "node_modules"
                || dir_name == "target"
                || dir_name == "dist"
                || dir_name == "build"
                || dir_name == "__pycache__"
            {
                continue;
            }

            let context_file = path.join(".context.md");
            if context_file.exists() {
                modules.push(AvailableModule {
                    name: dir_name.to_string(),
                    path: path.to_string_lossy().to_string(),
                    has_context: true,
                });
            }

            // Check depth-2 (e.g. src/components, crates/venore-core)
            if let Ok(sub_entries) = std::fs::read_dir(&path) {
                for sub_entry in sub_entries.flatten() {
                    let sub_path = sub_entry.path();
                    if !sub_path.is_dir() {
                        continue;
                    }

                    let sub_name = sub_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if sub_name.starts_with('.') {
                        continue;
                    }

                    let sub_context = sub_path.join(".context.md");
                    if sub_context.exists() {
                        modules.push(AvailableModule {
                            name: format!("{}/{}", dir_name, sub_name),
                            path: sub_path.to_string_lossy().to_string(),
                            has_context: true,
                        });
                    }
                }
            }
        }
    }

    Ok(modules)
}

/// Count knowledge_node + lighthouse entries in the project's ocean layout
/// and return up to 10 of their names. Returns `(0, [])` when the layout
/// has none, when the file is missing, or when reading fails — the caller
/// treats those cases identically (skip injecting the logbook hint).
fn count_knowledge_nodes(project_path: &str) -> Result<(u32, Vec<String>)> {
    use crate::ocean::NodeVariant;
    let layout = crate::ocean::service::with_service(project_path, |service| service.get_layout())?;
    let mut count: u32 = 0;
    let mut names: Vec<String> = Vec::new();
    for entry in layout.positions.values() {
        match entry.node_variant {
            NodeVariant::KnowledgeNode | NodeVariant::Lighthouse => {
                count += 1;
                if names.len() < 10 {
                    names.push(entry.module_name.clone());
                }
            }
            // Module / Buoy / Cylinder are code-representational, not part of
            // the knowledge graph the chat reports on.
            NodeVariant::Module | NodeVariant::Buoy | NodeVariant::Cylinder => {}
        }
    }
    Ok((count, names))
}

/// Build a structural index of all knowledge nodes + sections for the
/// system prompt. Lets the AI skip `list_logbooks` and skip `read_logbook`
/// when it only needs section ids (e.g. to call `propose_logbook_write`
/// with `edit_section_id=…`). Section *contents* are NOT included — those
/// are heavy and still come from `read_logbook` on demand.
///
/// Cost guard: above ~80 nodes the index gets unwieldy in tokens, so we
/// fall back to "name only" for the overflow tail.
fn build_logbook_index(project_path: &str) -> Option<String> {
    use crate::ocean::NodeVariant;
    const FULL_INDEX_NODE_CAP: usize = 80;

    let layout = crate::ocean::service::with_service(project_path, |service| service.get_layout())
        .ok()?;

    // Collect knowledge nodes with their section list
    type Row = (
        String,        // node_id
        String,        // name
        &'static str,  // variant
        Option<String>, // lighthouse_id
        Vec<(String, String)>, // (section_id, section_name)
    );
    let mut rows: Vec<Row> = Vec::new();
    for (node_id, entry) in &layout.positions {
        let variant = match entry.node_variant {
            NodeVariant::KnowledgeNode => "knowledge_node",
            NodeVariant::Lighthouse => "lighthouse",
            NodeVariant::Module | NodeVariant::Buoy | NodeVariant::Cylinder => continue,
        };
        let sections: Vec<(String, String)> = layout
            .knowledge_data
            .get(node_id)
            .map(|d| d.sections.iter().map(|s| (s.id.clone(), s.name.clone())).collect())
            .unwrap_or_default();
        rows.push((
            node_id.clone(),
            entry.module_name.clone(),
            variant,
            entry.lighthouse_id.clone(),
            sections,
        ));
    }
    if rows.is_empty() {
        return None;
    }

    // Stable order: lighthouses first, then alphabetical
    rows.sort_by(|a, b| {
        let a_lh = a.2 == "lighthouse";
        let b_lh = b.2 == "lighthouse";
        b_lh.cmp(&a_lh).then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase()))
    });

    let mut out = String::new();
    out.push_str("\n### Logbook index (snapshot — you already have the IDs, do not call `list_logbooks`)\n\n");

    // Compact table for everyone
    out.push_str("| node_id | name | variant | sections |\n");
    out.push_str("|---------|------|---------|----------|\n");
    for (id, name, variant, _lh, sections) in &rows {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            id,
            name.replace('|', "/"),
            variant,
            sections.len(),
        ));
    }

    // Per-node section breakdown — only for nodes that have sections, and
    // capped to keep the prompt bounded.
    let nodes_with_sections: Vec<&Row> = rows
        .iter()
        .filter(|r| !r.4.is_empty())
        .take(FULL_INDEX_NODE_CAP)
        .collect();

    if !nodes_with_sections.is_empty() {
        out.push_str("\n#### Sections per node (id · name)\n");
        for (node_id, name, _variant, _lh, sections) in &nodes_with_sections {
            out.push_str(&format!("- **{}** ({}):\n", name, node_id));
            for (sec_id, sec_name) in sections.iter() {
                out.push_str(&format!("  - `{}` — {}\n", sec_id, sec_name));
            }
        }
        let truncated = rows.iter().filter(|r| !r.4.is_empty()).count() - nodes_with_sections.len();
        if truncated > 0 {
            out.push_str(&format!(
                "\n_(+{} additional nodes with sections omitted from the index — use `read_logbook` to see them.)_\n",
                truncated,
            ));
        }
    }

    out.push_str("\n**Rules:**\n");
    out.push_str("- You already have node_ids and section_ids above: do NOT call `list_logbooks` to get them. Call `read_logbook` ONLY when you need the **content** of a section (e.g. to extend or rewrite it).\n");
    out.push_str("- For writing/editing: if the user says \"same section\", \"add a paragraph\", \"rewrite\", use `propose_logbook_write` with `edit_section_id=<section_id>` and pass the FULL body in `content_markdown` (include the previous text + your addition).\n");
    out.push_str("- For creating a new section: omit `edit_section_id`. Reusing the same `name` replaces a previous pending.\n");

    Some(out)
}

// ============================================================================
// FULL CONTEXT ORCHESTRATION
// ============================================================================

/// Module input for context building (name + path).
pub struct ContextModule {
    pub name: String,
    pub path: String,
}

/// Dependencies needed to build the full chat context.
pub struct ChatContextDeps {
    pub prompt_repo: Option<Arc<PromptRepository>>,
    pub rag_repo: Option<Arc<RagRepository>>,
    pub session_repo: Option<Arc<SessionRepository>>,
    pub memory_repo: Option<Arc<MemoryRepository>>,
    pub knowledge_repo: Option<Arc<KnowledgeRepository>>,
    pub context_repo: Option<Arc<ContextRepository>>,
}

/// Build a `SessionContext` from a dev session ID by looking up the session
/// and computing changed files.
pub async fn build_session_context(
    session_repo: &SessionRepository,
    dev_session_id: &str,
    project_path: Option<&str>,
) -> Option<SessionContext> {
    use crate::session::git_ops;

    let session = session_repo.get(dev_session_id).await.ok()??;

    let changed_files = if !session.worktree_path.is_empty()
        && Path::new(&session.worktree_path).exists()
    {
        git_ops::get_worktree_diff_files(
            Path::new(&session.worktree_path),
            &session.base_branch,
        )
        .ok()
        .map(|files| files.into_iter().map(|f| f.filename).collect())
        .unwrap_or_default()
    } else if let Some(pp) = project_path {
        git_ops::get_diff_files(
            Path::new(pp),
            &session.base_branch,
            &session.session_branch,
        )
        .ok()
        .map(|files| files.into_iter().map(|f| f.filename).collect())
        .unwrap_or_default()
    } else {
        Vec::new()
    };

    let worktree_path = if session.worktree_path.is_empty() {
        None
    } else {
        Some(session.worktree_path.clone())
    };

    Some(SessionContext {
        name: session.name,
        objective: session.objective,
        branch: session.session_branch,
        base_branch: session.base_branch,
        changed_files,
        worktree_path,
    })
}

/// Build a `KnowledgeResearchContext` from a feature ID.
pub async fn build_knowledge_context(
    knowledge_repo: &KnowledgeRepository,
    feature_id: &str,
) -> Option<KnowledgeResearchContext> {
    let feature = knowledge_repo.get_feature(feature_id).await.ok()??;
    let hexagons = knowledge_repo.list_hexagons_by_feature(feature_id).await.ok().unwrap_or_default();

    let mut total_evidence = 0usize;
    let mut hex_summaries = Vec::new();

    for hex in &hexagons {
        let ev_count = knowledge_repo.count_evidence_by_hexagon(&hex.id).await.ok().unwrap_or(0);
        total_evidence += ev_count;
        hex_summaries.push(KnowledgeHexagonSummary {
            id: hex.id.clone(),
            title: hex.title.clone(),
            phase: hex.phase.clone(),
            percentage: hex.percentage,
            confidence: hex.confidence.clone(),
            risk: hex.risk.clone(),
            is_dead_end: hex.is_dead_end,
            agent_status: hex.agent_status.clone(),
            evidence_count: ev_count,
        });
    }

    let project_links = knowledge_repo.list_project_links_by_feature(feature_id).await.ok().unwrap_or_default();
    let connected_projects: Vec<String> = project_links.iter().map(|l| l.project_path.clone()).collect();

    Some(KnowledgeResearchContext {
        feature_name: feature.name,
        feature_description: feature.description,
        objective: feature.objective,
        intensity: feature.intensity,
        hexagons: hex_summaries,
        evidence_count: total_evidence,
        connected_projects,
    })
}

/// Build the full system prompt for a chat session.
///
/// Orchestrates all context sources: prompt registry, project context,
/// module contexts, RAG search, layer analysis, terminal output, dev session,
/// tool instructions, and mesh peers.
///
/// Returns `(system_prompt, dev_session_name)`.
pub async fn build_full_chat_context(
    deps: &ChatContextDeps,
    project_path: Option<&str>,
    modules: Option<&[ContextModule]>,
    messages: &[crate::chat::ChatMessageInput],
    provider: LlmProviderType,
    project_id: Option<&str>,
    dev_session_id: Option<&str>,
    llm_tools: Option<&[LlmTool]>,
    knowledge_feature_id: Option<&str>,
    // Project kind ("code" | "knowledge") — picks the prompt category.
    // `None` is treated as "code" for backwards compat.
    project_kind: Option<&str>,
    // Pre-resolved AI-connection attachments. The caller runs
    // `connection_resolver::resolve_connections` first (so it can evict
    // stale entries from its registry) and passes the resulting blocks
    // here. None / empty = no attachments section in the system prompt.
    connection_blocks: Option<Vec<crate::chat::connection_resolver::ConnectionBlock>>,
) -> (String, Option<String>) {
    let mut builder = ChatContextBuilder::new()
        .with_provider(provider)
        .with_knowledge_mode(project_kind == Some("knowledge"));

    if let Some(blocks) = connection_blocks {
        if !blocks.is_empty() {
            builder = builder.with_connection_blocks(blocks);
        }
    }
    let mut dev_session_name: Option<String> = None;

    // Resolve prompt from registry. Knowledge-kind projects use a dedicated
    // category (`chat-knowledge`) so the role description doesn't tell the
    // model it can write files / run commands when the toolset doesn't
    // include those capabilities.
    if let Some(ref prompt_repo) = deps.prompt_repo {
        let provider_str = provider.as_str();
        let category = match project_kind {
            Some("knowledge") => "chat-knowledge",
            _ => "chat",
        };
        // Try the kind-specific category first, fall back to plain "chat"
        // if the user hasn't seeded the knowledge variants yet.
        let resolved = match prompt_repo.resolve_prompt(category, provider_str).await {
            Ok(p) => Some(p),
            Err(_) if category != "chat" => prompt_repo.resolve_prompt("chat", provider_str).await.ok(),
            Err(_) => None,
        };
        if let Some(p) = resolved {
            builder = builder.with_base_prompt(p.content);
        }
        // Load editable system-prompt fragments (Phase 5)
        if let Ok(fragments) = prompt_repo.list_chat_fragments().await {
            let map = crate::prompts::build_fragment_map(&fragments);
            builder = builder.with_chat_fragments(map);
        }
    }

    if let Some(project_path) = project_path {
        builder = builder.with_project(Path::new(project_path));
    }

    // Load project memory if available.
    //
    // Source of truth is `<project>/.venore/project-memory.json` (Phase 5
    // dropped the DB dual-write). Try the file first; fall back to the
    // SQLite repo only for legacy projects that still have a row but no
    // file on disk (silent-migration window). Without this fallback the
    // chat would silently start without the memory block — which is
    // exactly the bug observed when the AI re-investigates a wizarded
    // project from scratch.
    if let Some(pp) = project_path {
        let path = Path::new(pp);
        match crate::memory::file_storage::load(path) {
            Ok(Some(memory)) => {
                tracing::debug!(
                    project_path = %pp,
                    "Injecting project memory from .venore/project-memory.json"
                );
                builder = builder.with_project_memory(memory);
            }
            Ok(None) => {
                // File missing — try the legacy DB path before giving up.
                if let (Some(ref memory_repo), Some(pid)) = (&deps.memory_repo, project_id) {
                    match memory_repo.get_by_project(pid).await {
                        Ok(Some(memory)) => {
                            tracing::debug!(
                                project_id = %pid,
                                "Injecting project memory from DB (legacy, no file present)"
                            );
                            builder = builder.with_project_memory(memory);
                        }
                        Ok(None) => {}
                        Err(e) => {
                            tracing::warn!(
                                "Failed to load project memory from DB (continuing without): {}",
                                e
                            );
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    project_path = %pp,
                    error = %e,
                    "Failed to load project memory from file (continuing without)"
                );
            }
        }
    }

    if let Some(modules) = modules {
        for m in modules {
            builder = builder.with_module(&m.name, Path::new(&m.path));
        }
    }

    // RAG search
    if let (Some(ref rag_repo), Some(project_id)) = (&deps.rag_repo, project_id) {
        let query = messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .map(|m| m.content.as_str())
            .unwrap_or("");

        if !query.is_empty() {
            match crate::rag::search_code(rag_repo, project_id, query, 10, 8000).await {
                Ok(results) => {
                    if !results.is_empty() {
                        builder = builder.with_rag_results(&results);
                    }
                }
                Err(e) => {
                    tracing::warn!("RAG search failed (continuing without): {}", e);
                }
            }
        }
    }

    // Layer analysis — file-first (`.venore/module-layers.json`) with DB fallback
    // and silent migration. On-the-fly recompute remains the last-resort path
    // when neither file nor DB has any layers for the project (legacy chat
    // sessions, fresh projects without wizard run).
    if let (Some(ref ctx_repo), Some(pid)) = (&deps.context_repo, project_id) {
        let layers_result = match project_path {
            Some(pp) => {
                crate::context::file_storage::load_layers_file_first(Path::new(pp), pid, ctx_repo).await
            }
            None => ctx_repo.get_all_layers(pid).await,
        };
        match layers_result {
            Ok(db_layers) if !db_layers.is_empty() => {
                // Convert records to ModuleLayerAnalysis for the builder
                let summaries = db_layers_to_analysis(&db_layers);
                if !summaries.is_empty() {
                    builder = builder.with_layer_summary(&summaries);
                }
            }
            _ => {
                // Fallback: compute on-the-fly (legacy path)
                if let Some(project_path) = project_path {
                    if let Some(modules) = modules {
                        if !modules.is_empty() {
                            let project_dir = Path::new(project_path);
                            let all_layers = vec![
                                "context".to_string(), "tests".to_string(),
                                "documentation".to_string(), "connections".to_string(),
                                "status".to_string(),
                            ];

                            let layer_summaries: Vec<ModuleLayerAnalysis> = modules
                                .iter()
                                .map(|m| {
                                    let module_path = Path::new(&m.path);
                                    let relative = module_path
                                        .strip_prefix(project_dir)
                                        .map(|p| p.to_string_lossy().to_string())
                                        .unwrap_or_else(|_| m.path.clone());

                                    crate::layers::analyze_module_layers(
                                        project_dir, &relative, None, &all_layers,
                                    )
                                })
                                .collect();

                            if !layer_summaries.is_empty() {
                                builder = builder.with_layer_summary(&layer_summaries);
                            }
                        }
                    }
                }
            }
        }
    } else if let Some(project_path) = project_path {
        // No context_repo — always compute on-the-fly
        if let Some(modules) = modules {
            if !modules.is_empty() {
                let project_dir = Path::new(project_path);
                let all_layers = vec![
                    "context".to_string(), "tests".to_string(),
                    "documentation".to_string(), "connections".to_string(),
                    "status".to_string(),
                ];

                let layer_summaries: Vec<ModuleLayerAnalysis> = modules
                    .iter()
                    .map(|m| {
                        let module_path = Path::new(&m.path);
                        let relative = module_path
                            .strip_prefix(project_dir)
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|_| m.path.clone());

                        crate::layers::analyze_module_layers(
                            project_dir, &relative, None, &all_layers,
                        )
                    })
                    .collect();

                if !layer_summaries.is_empty() {
                    builder = builder.with_layer_summary(&layer_summaries);
                }
            }
        }
    }

    // Terminal output context
    {
        let mgr = crate::terminal::TerminalSessionManager::global();
        let guard = mgr.lock();
        if let Ok(m) = guard {
            let target_id = if let Some(sid) = dev_session_id {
                m.get_session_terminal(sid).map(|s| s.to_string())
            } else {
                m.list_unbound().into_iter().next()
            };
            if let Some(tid) = target_id {
                if let Ok(output) = m.get_recent_output(&tid, 50) {
                    if !output.is_empty() {
                        builder = builder.with_terminal_output(&output);
                    }
                }
            }
        }
    }

    // Dev session context
    if let (Some(dev_session_id), Some(ref session_repo)) = (dev_session_id, &deps.session_repo) {
        let ctx = build_session_context(session_repo, dev_session_id, project_path).await;
        if let Some(ctx) = ctx {
            dev_session_name = Some(ctx.name.clone());
            builder = builder.with_session_context(ctx);
        }
    }

    // Knowledge research context
    if let (Some(feature_id), Some(ref knowledge_repo)) = (knowledge_feature_id, &deps.knowledge_repo) {
        if let Some(kctx) = build_knowledge_context(knowledge_repo, feature_id).await {
            tracing::debug!(feature_id, "Injecting knowledge research context into system prompt");
            builder = builder.with_knowledge_context(kctx);
        }
    }

    // Tool instructions
    if let Some(tools) = llm_tools {
        builder = builder.with_tools(tools);
    }

    // Mesh peers
    {
        let transport = crate::mesh::MeshTransport::global();
        let t = transport.lock().await;
        let peer_ids = if t.is_running() {
            t.connected_peers()
        } else {
            Vec::new()
        };
        drop(t);

        if !peer_ids.is_empty() {
            let mesh = crate::mesh::MeshDiscovery::global();
            let peers: Vec<MeshPeerContext> = {
                let guard = mesh.lock();
                match guard {
                    Ok(g) => peer_ids
                        .iter()
                        .filter_map(|pid| {
                            g.get_peer_registration(pid).ok().map(|r| MeshPeerContext {
                                project_id: r.project_id.clone(),
                                project_name: r.project_name.clone(),
                                profile: r.profile.clone(),
                            })
                        })
                        .collect(),
                    Err(_) => Vec::new(),
                }
            };

            if !peers.is_empty() {
                builder = builder.with_mesh_peers(peers);
            }
        }
    }

    let prompt = builder.build_system_prompt().unwrap_or_else(|e| {
        tracing::warn!("build_system_prompt failed, using fallback: {}", e);
        SYSTEM_PROMPT.to_string()
    });

    (prompt, dev_session_name)
}

// ============================================================================
// DB layers → ModuleLayerAnalysis conversion
// ============================================================================

/// Convert DB layer records (flat list) into grouped ModuleLayerAnalysis structs
fn db_layers_to_analysis(records: &[crate::context::ModuleLayerRecord]) -> Vec<ModuleLayerAnalysis> {
    use std::collections::HashMap;
    use crate::layers::{LayerType, LayerStatus, LayerAnalysis};

    let mut by_module: HashMap<String, Vec<&crate::context::ModuleLayerRecord>> = HashMap::new();
    for r in records {
        by_module.entry(r.module_name.clone()).or_default().push(r);
    }

    by_module.into_iter().map(|(module_name, recs)| {
        let module_path = recs.first().map(|r| r.module_path.clone()).unwrap_or_default();
        let layers = recs.into_iter().filter_map(|r| {
            let layer_type = LayerType::from_config_name(&r.layer_type)?;
            let status = match r.status.as_str() {
                "complete" => LayerStatus::Complete,
                "partial" => LayerStatus::Partial,
                _ => LayerStatus::Missing,
            };
            let details: HashMap<String, serde_json::Value> =
                serde_json::from_str(&r.details_json).unwrap_or_default();
            Some(LayerAnalysis { layer_type, status, details })
        }).collect();

        ModuleLayerAnalysis { module_name, module_path, layers }
    }).collect()
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_build_system_prompt_no_context() {
        let builder = ChatContextBuilder::new();
        let prompt = builder.build_system_prompt().unwrap();
        assert!(prompt.contains("Venore AI"));
        assert!(!prompt.contains("## Project Context"));
    }

    #[test]
    fn test_build_system_prompt_with_project() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".context.md"), "# Test Project\nThis is a test.").unwrap();

        let builder = ChatContextBuilder::new().with_project(dir.path());
        let prompt = builder.build_system_prompt().unwrap();

        // Project identity section appears early
        assert!(prompt.contains("## Working Project"));
        assert!(prompt.contains("Use this project name"));
        // Project context from .context.md also present
        assert!(prompt.contains("## Project Context"));
        assert!(prompt.contains("# Test Project"));
    }

    #[test]
    fn test_build_system_prompt_project_identity_before_reference_material() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".context.md"), "# My App").unwrap();

        let builder = ChatContextBuilder::new().with_project(dir.path());
        let prompt = builder.build_system_prompt().unwrap();

        let identity_pos = prompt.find("## Working Project").unwrap();
        let reference_pos = prompt.find("# Reference Material").unwrap();
        assert!(identity_pos < reference_pos, "Project identity must appear before reference material");
    }

    #[test]
    fn test_build_system_prompt_with_modules() {
        let dir = tempfile::tempdir().unwrap();

        let mod_dir = dir.path().join("auth");
        fs::create_dir(&mod_dir).unwrap();
        fs::write(mod_dir.join(".context.md"), "Auth module context").unwrap();

        let builder = ChatContextBuilder::new()
            .with_module("auth", &mod_dir);
        let prompt = builder.build_system_prompt().unwrap();

        assert!(prompt.contains("## Module: auth"));
        assert!(prompt.contains("Auth module context"));
    }

    #[test]
    fn test_build_system_prompt_missing_files_graceful() {
        let dir = tempfile::tempdir().unwrap();
        let nonexistent = dir.path().join("nonexistent");

        let builder = ChatContextBuilder::new()
            .with_project(&nonexistent)
            .with_module("missing", &nonexistent);

        // Should not error, just skip missing .context.md
        let prompt = builder.build_system_prompt().unwrap();
        assert!(prompt.contains("Venore AI"));
        // Project identity still appears (derived from path, not .context.md)
        assert!(prompt.contains("## Working Project"));
        assert!(prompt.contains("nonexistent"));
        // But no .context.md content sections
        assert!(!prompt.contains("## Project Context"));
        assert!(!prompt.contains("## Module"));
    }

    #[test]
    fn test_scan_available_modules() {
        let dir = tempfile::tempdir().unwrap();

        // Create root context
        fs::write(dir.path().join(".context.md"), "root").unwrap();

        // Create module with context
        let mod_dir = dir.path().join("api");
        fs::create_dir(&mod_dir).unwrap();
        fs::write(mod_dir.join(".context.md"), "api context").unwrap();

        // Create module without context
        let no_ctx = dir.path().join("utils");
        fs::create_dir(&no_ctx).unwrap();

        let modules = scan_available_modules(dir.path()).unwrap();

        // Should find root + api (not utils)
        assert!(modules.len() >= 2);
        assert!(modules.iter().any(|m| m.name.contains("root")));
        assert!(modules.iter().any(|m| m.name == "api"));
    }

    /// Helper: create a minimal tool list for testing tool rules injection
    fn dummy_tools() -> Vec<LlmTool> {
        vec![LlmTool {
            name: "read_file".into(),
            description: "Read a file".into(),
            parameters: serde_json::json!({}),
        }]
    }

    #[test]
    fn test_gemini_final_reminder_appended() {
        let builder = ChatContextBuilder::new()
            .with_provider(LlmProviderType::Gemini);
        let prompt = builder.build_system_prompt().unwrap();
        assert!(
            prompt.contains("# FINAL REMINDER"),
            "Gemini prompt must include the final reminder"
        );
        assert!(
            prompt.contains("Keep going until the task is fully resolved"),
            "Final reminder must contain agentic persistence rule"
        );
    }

    #[test]
    fn test_non_gemini_no_final_reminder() {
        let builder = ChatContextBuilder::new()
            .with_provider(LlmProviderType::Anthropic);
        let prompt = builder.build_system_prompt().unwrap();
        assert!(
            !prompt.contains("# FINAL REMINDER"),
            "Anthropic prompt must NOT include the Gemini final reminder"
        );
    }

    #[test]
    fn test_knowledge_gemini_gets_canvas_reminder() {
        let builder = ChatContextBuilder::new()
            .with_provider(LlmProviderType::Gemini)
            .with_knowledge_mode(true);
        let prompt = builder.build_system_prompt().unwrap();
        // Canvas-oriented anchor with the duplicate-island guard.
        assert!(
            prompt.contains("NEVER recreate something you already created"),
            "knowledge Gemini must get the don't-recreate rule"
        );
        // The code reminder must not leak into Knowledge mode.
        assert!(
            !prompt.contains("After editing a file"),
            "knowledge reminder must not carry code-oriented rules"
        );
    }

    #[test]
    fn test_code_gemini_keeps_code_reminder() {
        let builder = ChatContextBuilder::new()
            .with_provider(LlmProviderType::Gemini)
            .with_knowledge_mode(false);
        let prompt = builder.build_system_prompt().unwrap();
        assert!(
            prompt.contains("After editing a file"),
            "code Gemini keeps the code-oriented reminder"
        );
        assert!(
            !prompt.contains("NEVER recreate something you already created"),
            "code reminder must not carry the knowledge dedup rule"
        );
    }

    #[test]
    fn test_all_providers_get_autonomy_rules() {
        // All providers (including Gemini) receive the same autonomy rules
        for provider in &[LlmProviderType::Gemini, LlmProviderType::Anthropic, LlmProviderType::OpenAI] {
            let builder = ChatContextBuilder::new()
                .with_provider(*provider)
                .with_tools(&dummy_tools());
            let prompt = builder.build_system_prompt().unwrap();

            assert!(
                prompt.contains("spawn_agent` to parallelize"),
                "{:?} must receive 'spawn_agent to parallelize' rule", provider
            );
            assert!(
                prompt.contains("Be proactive"),
                "{:?} must receive 'Be proactive' rule", provider
            );
            assert!(
                prompt.contains("search_text` to find ALL occurrences"),
                "{:?} must receive search_text bug-fix rule", provider
            );
            // Sequential-only rules should NOT appear for any provider
            assert!(
                !prompt.contains("Execute one tool at a time"),
                "{:?} must NOT receive sequential execution rule", provider
            );
        }
    }

    #[test]
    fn test_gemini_final_reminder_after_rag() {
        let rag_result = SearchResult {
            chunk: crate::rag::RagChunk {
                id: "1".into(),
                file_id: "f1".into(),
                project_id: "p".into(),
                relative_path: "src/main.rs".into(),
                name: "main".into(),
                chunk_type: "function".into(),
                content: "fn main() {}".into(),
                line_start: 1,
                line_end: 1,
                metadata: None,
            },
            score: 1.0,
            search_method: "fts".into(),
        };

        let builder = ChatContextBuilder::new()
            .with_provider(LlmProviderType::Gemini)
            .with_rag_results(&[rag_result]);
        let prompt = builder.build_system_prompt().unwrap();

        let rag_pos = prompt.find("## Relevant Code from Project").unwrap();
        let reminder_pos = prompt.find("# FINAL REMINDER").unwrap();
        assert!(
            reminder_pos > rag_pos,
            "Final reminder must appear AFTER RAG results"
        );
    }
}
