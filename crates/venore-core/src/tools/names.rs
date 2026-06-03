//! Centralized tool name constants.
//!
//! Single source of truth for all tool identifiers used across definitions,
//! executor dispatch, permissions, and chat command handling.

// ── Terminal tools ───────────────────────────────────────────────────
pub const RUN_TERMINAL_COMMAND: &str = "run_terminal_command";
pub const READ_TERMINAL_OUTPUT: &str = "read_terminal_output";
pub const RUN_APP: &str = "run_app";
pub const CHECK_HEALTH: &str = "check_health";

// ── File tools ───────────────────────────────────────────────────────
pub const READ_FILE: &str = "read_file";
pub const WRITE_FILE: &str = "write_file";
pub const EDIT_FILE: &str = "edit_file";
pub const MULTI_EDIT_FILE: &str = "multi_edit_file";
pub const LIST_FILES: &str = "list_files";

// ── Search tools ─────────────────────────────────────────────────────
pub const SEARCH_CODE: &str = "search_code";
pub const SEARCH_TEXT: &str = "search_text";

// ── Web tools ────────────────────────────────────────────────────────
pub const WEB_FETCH: &str = "web_fetch";
pub const WEB_SEARCH: &str = "web_search";

// ── Interaction tools ────────────────────────────────────────────────
pub const ASK_USER: &str = "ask_user";

// ── Mesh tools ───────────────────────────────────────────────────────
pub const ASK_PROJECT: &str = "ask_project";
pub const ASK_CALLER: &str = "ask_caller";

// ── Task tools ───────────────────────────────────────────────────────
pub const TASK_CREATE: &str = "task_create";
pub const TASK_UPDATE: &str = "task_update";
pub const TASK_LIST: &str = "task_list";

// ── Knowledge tools ─────────────────────────────────────────────────
pub const PLAN_HEXAGONS: &str = "plan_hexagons";
pub const UPDATE_HEXAGON: &str = "update_hexagon";
pub const ADD_EVIDENCE: &str = "add_evidence";
pub const MARK_DEAD_END: &str = "mark_dead_end";
pub const GENERATE_REPORT: &str = "generate_report";

// ── Plan tools ───────────────────────────────────────────────────────
pub const ENTER_PLAN_MODE: &str = "enter_plan_mode";
pub const SUBMIT_PLAN: &str = "submit_plan";

// ── Sub-agent tools ──────────────────────────────────────────────────
pub const SPAWN_AGENT: &str = "spawn_agent";

// ── Logbook tools (per-node logbook) ─────────────────────────────────
pub const LIST_LOGBOOKS: &str = "list_logbooks";
pub const READ_LOGBOOK: &str = "read_logbook";
pub const SEARCH_LOGBOOK: &str = "search_logbook";
pub const PROPOSE_LOGBOOK_WRITE: &str = "propose_logbook_write";

// ── Structure tools (faros, nodos, conexiones del Ocean Canvas) ──────
pub const CREATE_LIGHTHOUSE: &str = "create_lighthouse";
pub const CREATE_KNOWLEDGE_NODE: &str = "create_knowledge_node";
pub const CREATE_CONNECTION: &str = "create_connection";
pub const PROMOTE_TO_LIGHTHOUSE: &str = "promote_to_lighthouse";
pub const SET_NODE_LIGHTHOUSE: &str = "set_node_lighthouse";
pub const RENAME_NODE: &str = "rename_node";
pub const LIST_CONNECTIONS: &str = "list_connections";
pub const LIST_ISLANDS: &str = "list_islands";
pub const QUERY_NEIGHBORHOOD: &str = "query_neighborhood";

// ── Convenience sets ─────────────────────────────────────────────────

/// Tools that modify files (used for post-processing: snapshots, LSP diagnostics).
pub const FILE_EDIT_TOOLS: &[&str] = &[WRITE_FILE, EDIT_FILE, MULTI_EDIT_FILE];

/// Tools safe to execute in parallel (read-only, no interaction).
pub const PARALLELIZABLE_TOOLS: &[&str] = &[
    READ_FILE, LIST_FILES, SEARCH_CODE, SEARCH_TEXT, WEB_FETCH, WEB_SEARCH,
    LIST_LOGBOOKS, READ_LOGBOOK, SEARCH_LOGBOOK,
];

