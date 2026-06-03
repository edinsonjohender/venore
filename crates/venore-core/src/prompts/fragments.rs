//! Chat-fragment prompts — editable blocks of the chat system prompt.
//!
//! Each block in `chat::context::ChatContextBuilder::build_system_prompt`
//! that previously lived as inline `format!`/`push_str` strings is now seeded
//! as a row in the `prompts` table under category `chat-fragment`.
//!
//! - The Rust code still decides WHEN a fragment is appended (conditional
//!   logic, dynamic table generation, etc).
//! - The Rust code still generates the DYNAMIC parts (rows of a table, list
//!   of items).
//! - But the static / human-readable text comes from the DB and can be edited
//!   in the Prompts UI tab.
//! - `is_enabled = false` lets the user disable a fragment without deleting it.

use std::collections::HashMap;

use crate::Result;
use super::models::Prompt;
use super::repository::PromptRepository;

pub const CATEGORY_CHAT_FRAGMENT: &str = "chat-fragment";

// -----------------------------------------------------------------------------
// Fragment IDs (stable keys used by chat::context to look fragments up)
// -----------------------------------------------------------------------------

/// All chat-fragment IDs in one place. Use the constants instead of string
/// literals when looking up fragments at runtime.
pub struct ChatFragmentId;

impl ChatFragmentId {
    pub const WORKING_PROJECT: &'static str = "chat-fragment-working-project";
    pub const REFERENCE_HEADER: &'static str = "chat-fragment-reference-header";
    pub const ACTIVE_DEV_SESSION: &'static str = "chat-fragment-active-dev-session";
    pub const LOGBOOK_HINT: &'static str = "chat-fragment-logbook-hint";
    pub const KNOWLEDGE_RESEARCH: &'static str = "chat-fragment-knowledge-research";
    pub const MODULE_HEALTH: &'static str = "chat-fragment-module-health";
    pub const TERMINAL_OUTPUT: &'static str = "chat-fragment-terminal-output";
    pub const TOOLS_TABLE: &'static str = "chat-fragment-tools-table";
    pub const MESH_PEERS: &'static str = "chat-fragment-mesh-peers";
    pub const RAG_SNIPPETS: &'static str = "chat-fragment-rag-snippets";
}

// -----------------------------------------------------------------------------
// Runtime map (used by chat::context to look fragments up synchronously)
// -----------------------------------------------------------------------------

/// In-memory copy of a chat fragment so the (sync) prompt builder can look it
/// up without touching the DB.
#[derive(Debug, Clone)]
pub struct ChatFragmentEntry {
    pub content: String,
    pub is_enabled: bool,
}

pub type ChatFragmentMap = HashMap<String, ChatFragmentEntry>;

/// Build a `ChatFragmentMap` from the prompts loaded from the DB.
pub fn build_fragment_map(prompts: &[Prompt]) -> ChatFragmentMap {
    prompts
        .iter()
        .filter(|p| p.category == CATEGORY_CHAT_FRAGMENT)
        .map(|p| {
            (
                p.id.clone(),
                ChatFragmentEntry {
                    content: p.content.clone(),
                    is_enabled: p.is_enabled,
                },
            )
        })
        .collect()
}

// -----------------------------------------------------------------------------
// Renderer — minimal {{var}} substitution
// -----------------------------------------------------------------------------

/// Replace every `{{name}}` occurrence in `template` with the matching value
/// from `vars`. Unknown placeholders are left as-is so a typo is visible
/// instead of silently dropped. Whitespace inside the braces is tolerated.
pub fn render_template(template: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            // find closing "}}"
            if let Some(rel) = template[i + 2..].find("}}") {
                let raw = &template[i + 2..i + 2 + rel];
                let key = raw.trim();
                if let Some(val) = vars.get(key) {
                    out.push_str(val);
                } else {
                    // leave the placeholder visible
                    out.push_str(&template[i..i + 2 + rel + 2]);
                }
                i = i + 2 + rel + 2;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

// -----------------------------------------------------------------------------
// Seed
// -----------------------------------------------------------------------------

impl PromptRepository {
    /// Seed default chat-fragment prompts. Idempotent: only inserts fragments
    /// that don't already exist by id, so editing a fragment in the UI won't
    /// be overwritten on next boot.
    pub async fn seed_chat_fragments(&self) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let fragments = default_chat_fragments(&now);

        let mut inserted = 0;
        for fragment in &fragments {
            if self.get_prompt(&fragment.id).await.is_ok() {
                continue;
            }
            self.create_prompt(fragment).await?;

            let version_id = uuid::Uuid::new_v4().to_string();
            sqlx::query::<sqlx::Sqlite>(
                "INSERT INTO prompt_versions (id, prompt_id, version, content, created_at)
                 VALUES (?, ?, 1, ?, ?)",
            )
            .bind(&version_id)
            .bind(&fragment.id)
            .bind(&fragment.content)
            .bind(&now)
            .execute(self.pool())
            .await
            .map_err(|e| {
                crate::VenoreError::DatabaseError(format!(
                    "Failed to save chat-fragment seed version for '{}': {}",
                    fragment.id, e
                ))
            })?;
            inserted += 1;
        }

        if inserted > 0 {
            tracing::info!("Seeded {} chat-fragment prompts", inserted);
        } else {
            tracing::debug!("Chat fragments already seeded");
        }
        Ok(())
    }

    /// v2 of the mesh-peer fragments. The v1 header just said "you can query
    /// these projects"; v2 explains what a connected project actually IS (an
    /// expert agent that reads its own codebase, not a search index) and the
    /// footer now documents `context_hint` and when to consult vs answer
    /// directly. Existing DBs seeded the v1 text via `seed_chat_fragments`
    /// (insert-only), so they need this version-gated update to converge.
    /// Idempotent — skips fragments whose content already matches.
    pub async fn seed_mesh_fragments_v2(&self) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let canonical = default_chat_fragments(&now);
        let ids = [ChatFragmentId::MESH_PEERS, "chat-fragment-mesh-peers-footer"];
        for id in ids {
            let existing = match self.get_prompt(id).await {
                Ok(p) => p,
                // Not seeded yet — seed_chat_fragments will insert the new text.
                Err(_) => continue,
            };
            let Some(frag) = canonical.iter().find(|f| f.id == id) else {
                continue;
            };
            if existing.content == frag.content {
                continue;
            }
            self.update_prompt(id, &frag.content).await?;
            tracing::info!(fragment = id, "Upgraded mesh fragment to v2 text");
        }
        Ok(())
    }
}

fn fragment(id: &str, name: &str, content: &str, variables: &str, now: &str) -> Prompt {
    Prompt {
        id: id.into(),
        name: name.into(),
        category: CATEGORY_CHAT_FRAGMENT.into(),
        provider: "base".into(),
        content: content.into(),
        variables: variables.into(),
        is_template: true,
        is_enabled: true,
        version: 1,
        created_at: now.into(),
        updated_at: now.into(),
    }
}

fn default_chat_fragments(now: &str) -> Vec<Prompt> {
    vec![
        // ---------------------------------------------------------------------
        // working-project — always rendered when project_path is set
        // ---------------------------------------------------------------------
        fragment(
            ChatFragmentId::WORKING_PROJECT,
            "Working Project header",
            "\n\n## Working Project\nYou are operating on the project \"{{project_name}}\" at `{{project_path}}`.\nUse this project name for any generated artifacts (Docker images, packages, branches, etc.).\n",
            r#"["project_name","project_path"]"#,
            now,
        ),
        // ---------------------------------------------------------------------
        // reference-header — printed once if any of project/modules/rag/layers exist
        // ---------------------------------------------------------------------
        fragment(
            ChatFragmentId::REFERENCE_HEADER,
            "Reference Material header",
            "\n\n---\n\n# Reference Material (background knowledge — do NOT recite)\nUse the following to inform your answers. Extract only the specific facts needed to answer the user's question.\n",
            "[]",
            now,
        ),
        // ---------------------------------------------------------------------
        // active-dev-session — wraps the dynamic session details
        // ---------------------------------------------------------------------
        fragment(
            ChatFragmentId::ACTIVE_DEV_SESSION,
            "Active dev session block",
            "\n\n## Active Dev Session\nYou are operating inside dev session \"{{session_name}}\".\n{{details_lines}}\nAll file tools (`write_file`, `edit_file`, `read_file`) target this worktree automatically.\nYou MUST use tools to create and modify files. NEVER paste code as text. Focus on the session objective.\n",
            r#"["session_name","details_lines"]"#,
            now,
        ),
        // ---------------------------------------------------------------------
        // logbook-hint — full block (the {{names_line}} is computed in Rust)
        // ---------------------------------------------------------------------
        fragment(
            ChatFragmentId::LOGBOOK_HINT,
            "Logbooks hint",
            "\n\n## Project logbooks\n\nThis project contains **{{node_count}} node(s)** in total. A \"node\" can be a **lighthouse** (anchor of an island) or a **knowledge_node** (sub-topic within an island). Each one has its own list of **sections** (markdown blocks).\n{{names_line}}\n### Structure vs content — don't mix them up\n\n- **Lighthouse / Knowledge_node / Connection** = structure (graph entities).\n- **Section** = content inside a node. It is a markdown paragraph, NOT a sub-topic.\n\nWhen the user describes a project with several sub-topics, what they want is structure: a lighthouse with multiple knowledge_node children. When they say \"jot this down\", they want a section. Telling them apart is your job.\n\n### Logbook and structure tools\n\nReading:\n- `list_logbooks` — list ALL the nodes (id + name + variant + section count). For \"what logbooks do I have\".\n- `read_logbook` — read one whole node by its `node_id` (UUID).\n- `search_logbook` — substring search across the content. Returns snippets. NEVER pass an empty query.\n\nContent (section):\n- `propose_logbook_write` — add or replace a **section** inside an existing node. Does NOT create nodes.\n\nStructure (graph):\n- `create_lighthouse(name, near_node_id?)` — create a new lighthouse. For \"a project X with sub-topics\".\n- `create_knowledge_node(name, lighthouse_id?, near_node_id?)` — create a knowledge_node. Pass `lighthouse_id` to attach it to an existing lighthouse.\n- `create_connection(from_node_id, to_node_id)` — directed edge between two nodes.\n- `promote_to_lighthouse(node_id)` — turn a knowledge_node into a lighthouse.\n- `set_node_lighthouse(node_id, lighthouse_id?)` — reassign a node to another lighthouse or detach it.\n\nNEVER use `search_text` or `list_files` for logbook questions — those are for code.\n\nThe logbook and structure tools are **native tools**. Call them as a function call. NEVER put them inside `run_terminal_command`.\n",
            r#"["node_count","names_line"]"#,
            now,
        ),
        // ---------------------------------------------------------------------
        // knowledge-research — header + dynamic content (hexagons table built in Rust)
        // ---------------------------------------------------------------------
        fragment(
            ChatFragmentId::KNOWLEDGE_RESEARCH,
            "Knowledge research intro",
            "\n\n## Knowledge Research Session\nYou are conducting structured research on: **{{feature_name}}**\n{{description_line}}Objective: {{objective}} | Intensity: {{intensity}}\n{{connected_line}}{{hexagons_section}}",
            r#"["feature_name","description_line","objective","intensity","connected_line","hexagons_section"]"#,
            now,
        ),
        // ---------------------------------------------------------------------
        // knowledge-research-rules — appended after the hexagons table
        // ---------------------------------------------------------------------
        fragment(
            "chat-fragment-knowledge-research-rules",
            "Knowledge research rules",
            "\n### Research Instructions\n- Use `plan_hexagons` to create research points from the seed topic\n- Use `web_search` and `web_fetch` to find information\n- Use `update_hexagon` to record progress on each point\n- Use `add_evidence` to store findings with sources\n- Use `mark_dead_end` when a research path leads nowhere\n- Use `generate_report` when the research is mature enough\n- Focus on the research objective: {{objective}}\n- Do NOT use terminal or file editing tools unless specifically asked\n",
            r#"["objective"]"#,
            now,
        ),
        // ---------------------------------------------------------------------
        // module-health — table header (rows generated in Rust)
        // ---------------------------------------------------------------------
        fragment(
            ChatFragmentId::MODULE_HEALTH,
            "Module health header",
            "\n\n## Module Health Summary\n\n| Module | Context | Tests | Docs | Connections | Issues |\n|--------|---------|-------|------|-------------|--------|\n",
            "[]",
            now,
        ),
        // ---------------------------------------------------------------------
        // terminal-output — wraps the output in a fenced block
        // ---------------------------------------------------------------------
        fragment(
            ChatFragmentId::TERMINAL_OUTPUT,
            "Recent terminal output",
            "\n\n## Recent Terminal Output\n```\n{{output}}\n```\n",
            r#"["output"]"#,
            now,
        ),
        // ---------------------------------------------------------------------
        // tools-table — header + intro + rules (table generated in Rust)
        // ---------------------------------------------------------------------
        fragment(
            ChatFragmentId::TOOLS_TABLE,
            "Tools table & rules",
            "\n\n## Tools\n\nYou MUST use these tools to perform actions. NEVER output code as text — use the tools.\n\n{{tools_table}}\n**Tool usage rules:**\n- Use `edit_file` over `write_file` for modifications (preserves unchanged content)\n- Use file tools over terminal (`read_file` not `cat`, `edit_file` not `sed`)\n- Use `search_text` to find usages, TODOs, imports, strings across the project\n- Use `search_code` to find definitions (functions, classes, types) by name or meaning\n- A terminal is created automatically when needed\n- After running a command, verify the result before continuing\n- After editing a file, use `read_file` to verify the edit was applied correctly (catch duplicate lines, broken syntax)\n- For complex multi-step tasks, consider using `enter_plan_mode` first to design your approach\n- Use `ask_user` ONLY for technical decisions with multiple valid approaches (e.g. architecture choices, library selection). NEVER use it for greetings, simple questions, or when the user's intent is clear — just respond with text\n- Use `task_create`/`task_update` for multi-step work to show progress\n- **App startup:** To run/start/launch an app, use `spawn_agent` with type `executor`. It analyzes the project, installs dependencies, starts the app, and verifies health automatically\n- Be proactive: run builds, tests, installs as needed\n- Use `spawn_agent` to parallelize independent sub-tasks\n- When fixing a bug, first use `search_text` to find ALL occurrences before fixing any.\n\nREMINDER: When the user asks you to write, create, or modify code — call the tool. Do NOT paste code in chat.\n",
            r#"["tools_table"]"#,
            now,
        ),
        // ---------------------------------------------------------------------
        // mesh-peers — header + closing instructions (peer list built in Rust)
        // ---------------------------------------------------------------------
        fragment(
            ChatFragmentId::MESH_PEERS,
            "Mesh peers header",
            "\n\n## Connected Projects (Mesh)\n\nOther Venore instances are connected to you. Each one runs an agent that is an expert on its own project and can read that project's full codebase to answer you — these are reasoning agents, not a search index. Consult them with `ask_project` when you need facts about another project's code, architecture, APIs, or conventions instead of guessing.\n\nAvailable projects:\n\n",
            "[]",
            now,
        ),
        fragment(
            "chat-fragment-mesh-peers-footer",
            "Mesh peers footer",
            "\nTo consult a project, call `ask_project` with its name exactly as listed above, your question, and an optional `context_hint` to focus the search. Example: `ask_project` with `{\"project\": \"<name>\", \"question\": \"...\", \"context_hint\": \"auth\"}`.\nThe remote agent searches its own code and returns a synthesized answer (it may take a few seconds). Consult a project when its knowledge makes your answer more accurate; if you already know the answer, respond directly.\n",
            "[]",
            now,
        ),
        // ---------------------------------------------------------------------
        // rag-snippets — header (snippets generated in Rust)
        // ---------------------------------------------------------------------
        fragment(
            ChatFragmentId::RAG_SNIPPETS,
            "RAG snippets header",
            "\n\n## Relevant Code from Project\n",
            "[]",
            now,
        ),
    ]
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_replaces_placeholders() {
        let mut vars = HashMap::new();
        vars.insert("name", "Edinson".to_string());
        vars.insert("path", "/tmp/foo".to_string());
        let out = render_template("Hi {{name}}, see {{path}}.", &vars);
        assert_eq!(out, "Hi Edinson, see /tmp/foo.");
    }

    #[test]
    fn render_tolerates_whitespace() {
        let mut vars = HashMap::new();
        vars.insert("x", "ok".to_string());
        assert_eq!(render_template("[{{ x }}]", &vars), "[ok]");
    }

    #[test]
    fn render_keeps_unknown_placeholders() {
        let vars = HashMap::new();
        let out = render_template("a {{missing}} b", &vars);
        assert_eq!(out, "a {{missing}} b");
    }

    #[test]
    fn render_handles_multiple_occurrences() {
        let mut vars = HashMap::new();
        vars.insert("v", "X".to_string());
        assert_eq!(render_template("{{v}}-{{v}}-{{v}}", &vars), "X-X-X");
    }

    #[test]
    fn render_handles_no_placeholders() {
        let vars = HashMap::new();
        assert_eq!(render_template("plain text", &vars), "plain text");
    }
}
