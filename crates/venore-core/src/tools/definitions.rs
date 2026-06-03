//! Tool Definitions
//!
//! Defines the available tools (schemas) that can be passed to LLM providers.

use crate::llm::types::LlmTool;
use super::names as N;

/// Terminal tools — always available. The AI just says what to run;
/// the execution layer resolves which terminal to use (or spawns one).
pub fn terminal_tools() -> Vec<LlmTool> {
    vec![
        LlmTool {
            name: N::RUN_TERMINAL_COMMAND.into(),
            description: "Execute a shell command in the project's terminal. The command runs in a real PTY visible to the user.\n\nUse for: builds, tests, git commands, package installs, running scripts.\nDo NOT use for: reading files (use read_file), editing files (use edit_file), listing files (use list_files).\nFor starting apps/servers that listen on ports, use `run_app` instead — it verifies the app is actually running.\n\nThe terminal persists between commands — state (cd, env vars) carries over.\nAfter execution you will receive the command output automatically.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute"
                    }
                },
                "required": ["command"]
            }),
        },
        LlmTool {
            name: N::READ_TERMINAL_OUTPUT.into(),
            description: "Read recent output lines from the terminal. Use when you need to re-check results from a previous command or review logs.\n\nDefault: last 50 lines. Set `lines` for more or fewer.\nYou usually don't need this — output is auto-returned after run_terminal_command. Only use when you need older output.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "lines": {
                        "type": "integer",
                        "description": "Number of recent lines to read (default: 50)"
                    }
                },
                "required": []
            }),
        },
        LlmTool {
            name: N::RUN_APP.into(),
            description: "Start a long-running app (server, Docker container, dev server) and wait for it to listen.\n\n\
                Use INSTEAD OF run_terminal_command when:\n\
                - Starting a web server (npm start/dev, vite, next dev, go run, cargo run)\n\
                - Running Docker containers (docker run, docker-compose up)\n\
                - Any command that starts a process listening on a port\n\n\
                This tool automatically:\n\
                1. Checks if the port is available (suggests alternatives if busy)\n\
                2. Executes the command\n\
                3. Waits for the port to start listening\n\
                4. Reports status: RUNNING, FAILED, or PORT_BUSY\n\n\
                After run_app returns RUNNING, ALWAYS use check_health to verify the app responds correctly.\n\
                Do NOT use for: builds, tests, installs, git commands — use run_terminal_command for those.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The command to start the app (e.g. 'npm run dev', 'docker run -d -p 8080:80 nginx')"
                    },
                    "port": {
                        "type": "integer",
                        "description": "Port the app listens on. Auto-detected from command flags if omitted."
                    },
                    "wait_timeout_secs": {
                        "type": "integer",
                        "description": "Seconds to wait for the app to start (default: 15, max: 60)"
                    }
                },
                "required": ["command"]
            }),
        },
    ]
}

/// Verification tools — health checks and app validation.
pub fn verification_tools() -> Vec<LlmTool> {
    vec![
        LlmTool {
            name: N::CHECK_HEALTH.into(),
            description: "Verify that a running app responds correctly via HTTP. Use after run_app returns RUNNING.\n\n\
                Checks:\n\
                1. HTTP connectivity — can we reach the URL?\n\
                2. Status code — does it match expected_status (default: any < 500)?\n\
                3. Content — does the response body contain expected_content?\n\n\
                Returns HEALTHY or UNHEALTHY with a response preview.\n\n\
                Use for:\n\
                - Verifying an app is actually working after run_app\n\
                - Checking specific pages or API endpoints\n\
                - Validating that the app shows the right content\n\n\
                If UNHEALTHY: read the response preview, diagnose the issue, fix it, and re-run. \
                NEVER tell the user the app is ready until check_health returns HEALTHY.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "URL to check (e.g. 'http://localhost:5173', 'http://localhost:3000/api/health')"
                    },
                    "expected_status": {
                        "type": "integer",
                        "description": "Expected HTTP status code (default: any status < 500 is OK)"
                    },
                    "expected_content": {
                        "type": "string",
                        "description": "Text that must appear in the response body (e.g. 'Login', 'Welcome', 'canvas')"
                    },
                    "retries": {
                        "type": "integer",
                        "description": "Number of retry attempts (default: 3)"
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "description": "Timeout per request in seconds (default: 5)"
                    }
                },
                "required": ["url"]
            }),
        },
    ]
}

/// File tools — read, write, edit, and list files directly.
pub fn file_tools() -> Vec<LlmTool> {
    vec![
        LlmTool {
            name: N::READ_FILE.into(),
            description: "Read a file's contents. Returns content with line numbers prefixed.\n\nIMPORTANT: Always read a file before editing it — never guess content.\n- Paths can be absolute or relative to the project root.\n- Default limit: 2000 lines. Use offset + limit for large files.\n- Max 100,000 characters returned.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to the file to read (absolute or relative to project root)"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Line number to start reading from (0-based, default: 0)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of lines to return (default: 2000)"
                    }
                },
                "required": ["file_path"]
            }),
        },
        LlmTool {
            name: N::WRITE_FILE.into(),
            description: "Create a new file or completely overwrite an existing file.\n\nPrefer edit_file for modifications — it preserves unchanged content and is less error-prone. Use write_file only when creating a brand new file or rewriting the entire content.\n\nParent directories are created automatically. Paths can be absolute or relative to the project root.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to the file to write (absolute or relative to project root)"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to write to the file"
                    }
                },
                "required": ["file_path", "content"]
            }),
        },
        LlmTool {
            name: N::EDIT_FILE.into(),
            description: "Replace a specific text string in a file. This is the preferred way to modify existing files.\n\nRULES:\n1. You MUST read_file first — never guess file contents.\n2. old_string must match text in the file exactly. Whitespace-tolerant fuzzy matching is applied as fallback.\n3. If old_string matches multiple locations, the edit FAILS — provide more surrounding context to make it unique, or set replace_all to true.\n4. Do NOT include line numbers in old_string or new_string.\n5. Include enough surrounding context lines to ensure uniqueness.\n\nPaths can be absolute or relative to the project root.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to the file to edit (absolute or relative to project root)"
                    },
                    "old_string": {
                        "type": "string",
                        "description": "The text to find and replace (must be unique in the file)"
                    },
                    "new_string": {
                        "type": "string",
                        "description": "The replacement text"
                    },
                    "replace_all": {
                        "type": "boolean",
                        "description": "Replace all occurrences instead of requiring uniqueness (default: false)"
                    }
                },
                "required": ["file_path", "old_string", "new_string"]
            }),
        },
        LlmTool {
            name: N::MULTI_EDIT_FILE.into(),
            description: "Apply multiple edits to the same file in one operation. \
                More efficient than calling edit_file multiple times. \
                Edits are applied sequentially in order — each edit sees the result of the previous one.\n\n\
                RULES:\n\
                1. You MUST read_file first — never guess file contents.\n\
                2. Each edit's old_string must match text in the file (fuzzy matching is applied).\n\
                3. If an edit fails, remaining edits still proceed on the last successful state.\n\
                4. The result reports which edits succeeded and which failed.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to the file to edit (absolute or relative to project root)"
                    },
                    "edits": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "old_string": { "type": "string", "description": "Text to find" },
                                "new_string": { "type": "string", "description": "Replacement text" }
                            },
                            "required": ["old_string", "new_string"]
                        },
                        "description": "Array of edits to apply sequentially"
                    }
                },
                "required": ["file_path", "edits"]
            }),
        },
        LlmTool {
            name: N::LIST_FILES.into(),
            description: "List files and directories at a path. Returns sorted paths.\n\n- Without pattern: lists ALL files up to depth 3 (best for exploring a directory).\n- With pattern: glob filter — '*.rs', '**/*.ts', 'src/**/*.py'.\n- For extensionless files (Dockerfile, Makefile, LICENSE), use the exact name: 'Dockerfile'.\n- Max depth: 3 levels. Max results: 500 entries.\n- Skips: .git, node_modules, target, dist, build, __pycache__, .next, .venv.\n\nPaths can be absolute or relative to the project root.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Directory path to list (absolute or relative to project root)"
                    },
                    "pattern": {
                        "type": "string",
                        "description": "Glob pattern to filter files (e.g. '*.rs', '**/*.ts')"
                    }
                },
                "required": ["path"]
            }),
        },
    ]
}

/// Search tools — RAG code search + text search.
pub fn search_tools() -> Vec<LlmTool> {
    vec![
        LlmTool {
            name: N::SEARCH_CODE.into(),
            description: "Search the project's code index for functions, classes, types, and other symbols by name or natural language description.\n\nUse for: finding where something is defined, locating relevant code before making changes, understanding how a feature is implemented.\nDo NOT use for: reading a file you already know the path to (use read_file), listing directory contents (use list_files).\n\nReturns matching code snippets with file paths, line numbers, and language. Results are ranked by relevance.\nThe index covers the entire project — you don't need to know file paths in advance.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query — function name, class name, natural language description (e.g. 'authenticateUser', 'error handling middleware', 'database connection')"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of results to return (default: 10)"
                    }
                },
                "required": ["query"]
            }),
        },
        LlmTool {
            name: N::SEARCH_TEXT.into(),
            description: "Search for text or regex patterns across project files (like grep).\n\nUse for: finding TODOs, locating usages of a variable/import/string, searching config values, finding error messages.\nDo NOT use for: finding symbol definitions (use search_code), reading a known file (use read_file).\n\nReturns matching lines with file paths and line numbers. Supports regex patterns and glob file filters.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Text or regex pattern to search for (e.g. 'TODO', 'API_KEY', 'import.*lodash')"
                    },
                    "path": {
                        "type": "string",
                        "description": "Subdirectory to scope the search (relative to project root, default: entire project)"
                    },
                    "file_pattern": {
                        "type": "string",
                        "description": "Glob pattern to filter files (e.g. '*.rs', '*.ts', '*.py')"
                    },
                    "case_sensitive": {
                        "type": "boolean",
                        "description": "Whether the search is case-sensitive (default: false)"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of matching lines to return (default: 50)"
                    }
                },
                "required": ["pattern"]
            }),
        },
    ]
}

/// Web tools — fetch URLs and search the web.
pub fn web_tools() -> Vec<LlmTool> {
    vec![
        LlmTool {
            name: N::WEB_FETCH.into(),
            description: "Fetch content from a URL and return it as readable text.\n\nUse for: reading documentation pages, API references, web articles, checking URLs.\nDo NOT use for: authenticated pages (GitHub, Jira, etc. — use dedicated tools instead).\n\nHTML pages are converted to readable text. Content is truncated to max_chars.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The URL to fetch (must be a valid http/https URL)"
                    },
                    "max_chars": {
                        "type": "integer",
                        "description": "Maximum characters to return (default: 50000)"
                    }
                },
                "required": ["url"]
            }),
        },
        LlmTool {
            name: N::WEB_SEARCH.into(),
            description: "Search the web and return up-to-date information with source URLs.\n\nUse for: documentation URLs, library versions, error solutions, current facts about external libraries / APIs / events.\nDo NOT use for: searching project code (use search_code or search_text).".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of results (default: 5)"
                    }
                },
                "required": ["query"]
            }),
        },
    ]
}

/// Interaction tools — bidirectional communication with the user during agentic loop.
pub fn interaction_tools() -> Vec<LlmTool> {
    vec![
        LlmTool {
            name: N::ASK_USER.into(),
            description: "Ask the user a question and wait for their response. The agentic loop pauses until the user replies.\n\nUse ONLY for: technical decisions with multiple valid approaches (architecture choices, library selection, destructive actions requiring confirmation).\nDo NOT use for: greetings, casual conversation, simple questions, or when the user's intent is clear. In those cases, respond directly with text.\n\nOptionally provide predefined options for the user to choose from. The user can always type a free-text response.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "question": {
                        "type": "string",
                        "description": "The question to ask the user"
                    },
                    "options": {
                        "type": "array",
                        "description": "Optional predefined answer options",
                        "items": {
                            "type": "object",
                            "properties": {
                                "label": { "type": "string", "description": "Short option label" },
                                "description": { "type": "string", "description": "Optional longer description" }
                            },
                            "required": ["label"]
                        }
                    }
                },
                "required": ["question"]
            }),
        },
    ]
}

/// Task management tools — create, update, and list tasks during a session.
pub fn task_tools() -> Vec<LlmTool> {
    vec![
        LlmTool {
            name: N::TASK_CREATE.into(),
            description: "Create a task to track progress on multi-step work. Tasks are visible to the user in the chat.\n\nUse for: breaking down complex requests into trackable steps, showing progress on multi-step operations.\nDo NOT use for: single simple actions that don't need tracking.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "subject": {
                        "type": "string",
                        "description": "Short task title (imperative form, e.g. 'Fix login bug')"
                    },
                    "description": {
                        "type": "string",
                        "description": "Optional detailed description of what needs to be done"
                    }
                },
                "required": ["subject"]
            }),
        },
        LlmTool {
            name: N::TASK_UPDATE.into(),
            description: "Update the status of an existing task.\n\nStatus transitions: pending → in_progress → completed\nMark tasks as in_progress when starting work, completed when done.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "task_id": {
                        "type": "string",
                        "description": "The ID of the task to update"
                    },
                    "status": {
                        "type": "string",
                        "enum": ["pending", "in_progress", "completed"],
                        "description": "New status for the task"
                    }
                },
                "required": ["task_id", "status"]
            }),
        },
        LlmTool {
            name: N::TASK_LIST.into(),
            description: "List all tasks in the current session with their statuses.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
    ]
}

/// Plan mode tools — enter planning mode and submit plans for approval.
pub fn plan_tools() -> Vec<LlmTool> {
    vec![
        LlmTool {
            name: N::ENTER_PLAN_MODE.into(),
            description: "Enter plan mode for complex multi-step tasks. In plan mode, only read-only tools are available (no file writes or terminal commands). Use this to explore the codebase and design an approach before making changes.\n\nUse for: complex tasks with multiple valid approaches, tasks touching many files, architectural decisions.\nDo NOT use for: simple, straightforward tasks.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        LlmTool {
            name: N::SUBMIT_PLAN.into(),
            description: "Submit a plan for user approval. Only available in plan mode. The user will see the plan and can approve or reject it. If approved, plan mode exits and all tools become available again.\n\nProvide a clear summary and numbered steps.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "summary": {
                        "type": "string",
                        "description": "Brief summary of the plan"
                    },
                    "steps": {
                        "type": "array",
                        "description": "Ordered list of implementation steps",
                        "items": {
                            "type": "string"
                        }
                    }
                },
                "required": ["summary", "steps"]
            }),
        },
    ]
}

/// Sub-agent tool — spawn specialized sub-agents for parallel work.
pub fn sub_agent_tools() -> Vec<LlmTool> {
    vec![
        LlmTool {
            name: N::SPAWN_AGENT.into(),
            description: "Spawn a specialized sub-agent to perform a focused task in parallel. The sub-agent runs independently with a limited tool set and returns its result.\n\nAgent types:\n- `research`: Can read files, search code/text, and fetch web pages. Use for investigating code, finding information.\n- `code`: Can read, write, and edit files plus search. Use for implementing focused code changes.\n- `test`: Can read/write files and run terminal commands. Use for writing and running tests.\n- `executor`: Can read files, search, run terminal commands, start apps, and verify health. Use when the user wants to start/run/launch an application.\n\nMax 3 concurrent sub-agents per session. Each has a 2-minute timeout.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "agent_type": {
                        "type": "string",
                        "enum": ["research", "code", "test", "executor"],
                        "description": "Type of sub-agent to spawn"
                    },
                    "task": {
                        "type": "string",
                        "description": "Clear description of what the sub-agent should do"
                    }
                },
                "required": ["agent_type", "task"]
            }),
        },
    ]
}

/// Mesh tools — cross-project communication via connected Venore instances.
pub fn mesh_tools() -> Vec<LlmTool> {
    vec![LlmTool {
        name: N::ASK_PROJECT.to_string(),
        description: "Consult another connected Venore project. Each connected project runs its own \
            agent that is an expert on that project and can read its full codebase to answer — these \
            are reasoning agents, not a keyword search. Use this when you need accurate information \
            about another project's code, architecture, APIs, or conventions instead of guessing. \
            The available project names come from the connected-projects list in your context.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "project": {
                    "type": "string",
                    "description": "The name of the target project, exactly as shown in the connected-projects list"
                },
                "question": {
                    "type": "string",
                    "description": "The question to ask the remote project's agent"
                },
                "context_hint": {
                    "type": "string",
                    "description": "Optional hint about which module or area to focus on (e.g. 'auth', 'api', 'payments')"
                }
            },
            "required": ["project", "question"]
        }),
    }]
}

/// Mesh agent tools — read-only investigation tools for the mesh sub-agent.
///
/// Only includes tools safe for a remote agent to use on behalf of another project:
/// file reading, directory listing, code search, and text search.
/// Excludes: write/edit (read-only), terminal (no PTY), ask_project (prevents recursion),
/// web tools (caller's responsibility), interaction tools (no user), task/plan/spawn.
pub fn mesh_agent_tools() -> Vec<LlmTool> {
    let mut tools = vec![];
    for tool in file_tools() {
        if tool.name == N::READ_FILE || tool.name == N::LIST_FILES {
            tools.push(tool);
        }
    }
    tools.extend(search_tools());
    // Phase 4b: allow the mesh sub-agent to ask the caller for clarification
    tools.push(LlmTool {
        name: N::ASK_CALLER.into(),
        description: "Ask the requesting agent a clarifying question. Use only when the question \
            is genuinely ambiguous and you need more information to provide an accurate answer. \
            The caller's agent will answer automatically. Do not overuse — prefer answering with \
            available information when possible.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "The clarifying question to ask the requesting agent"
                }
            },
            "required": ["question"]
        }),
    });
    tools
}

/// Knowledge research tools — available only when knowledge_feature_id is present.
pub fn knowledge_tools() -> Vec<LlmTool> {
    vec![
        LlmTool {
            name: N::PLAN_HEXAGONS.into(),
            description: "Decompose a research seed into structured hexagon research points. Each hexagon represents a specific angle or sub-question to investigate.\n\nCall this once at the start of a research session to plan the investigation. Creates hexagons in the database and returns their IDs and titles.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "seed": {
                        "type": "string",
                        "description": "The research topic or seed question to decompose"
                    },
                    "objective": {
                        "type": "string",
                        "description": "Research objective: validate, understand, compare, decide, or explore"
                    },
                    "count": {
                        "type": "integer",
                        "description": "Number of hexagons to create (default: 5, max: 12)"
                    }
                },
                "required": ["seed", "objective"]
            }),
        },
        LlmTool {
            name: N::UPDATE_HEXAGON.into(),
            description: "Update the progress of a research hexagon. Use to advance phase, update percentage, confidence, or risk level as research progresses.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "hexagon_id": {
                        "type": "string",
                        "description": "ID of the hexagon to update"
                    },
                    "phase": {
                        "type": "string",
                        "enum": ["discover", "define", "validate", "conclude"],
                        "description": "New research phase"
                    },
                    "percentage": {
                        "type": "integer",
                        "description": "Completion percentage (0-100)"
                    },
                    "confidence": {
                        "type": "string",
                        "enum": ["low", "medium", "high"],
                        "description": "Confidence level in findings"
                    },
                    "risk": {
                        "type": "string",
                        "enum": ["unknown", "low", "medium", "high"],
                        "description": "Risk assessment"
                    },
                    "notes": {
                        "type": "string",
                        "description": "Research notes or observations"
                    }
                },
                "required": ["hexagon_id"]
            }),
        },
        LlmTool {
            name: N::ADD_EVIDENCE.into(),
            description: "Record a research finding as evidence attached to a hexagon. Always cite sources when available.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "hexagon_id": {
                        "type": "string",
                        "description": "ID of the hexagon this evidence supports"
                    },
                    "content": {
                        "type": "string",
                        "description": "The evidence content — finding, quote, data point, or observation"
                    },
                    "source_url": {
                        "type": "string",
                        "description": "URL of the source (if from web)"
                    },
                    "source_type": {
                        "type": "string",
                        "enum": ["web", "code", "manual", "document", "api"],
                        "description": "Type of source (default: manual)"
                    },
                    "confidence": {
                        "type": "string",
                        "enum": ["low", "medium", "high"],
                        "description": "Confidence in this evidence (default: medium)"
                    }
                },
                "required": ["hexagon_id", "content"]
            }),
        },
        LlmTool {
            name: N::MARK_DEAD_END.into(),
            description: "Mark a research hexagon as a dead end — the investigation path leads nowhere useful. The hexagon is preserved but flagged.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "hexagon_id": {
                        "type": "string",
                        "description": "ID of the hexagon to mark as dead end"
                    },
                    "reason": {
                        "type": "string",
                        "description": "Why this research path is a dead end"
                    }
                },
                "required": ["hexagon_id", "reason"]
            }),
        },
        LlmTool {
            name: N::GENERATE_REPORT.into(),
            description: "Generate a comprehensive research report from all hexagons and evidence collected so far. Synthesizes findings into a structured summary.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
    ]
}

/// Knowledge research tool set — knowledge tools + web + search + interaction (no terminal, no file edit).
pub fn knowledge_research_tools() -> Vec<LlmTool> {
    let mut tools = knowledge_tools();
    tools.extend(web_tools());
    tools.extend(search_tools());
    tools.extend(interaction_tools());
    tools.extend(task_tools());
    // Add read-only file tools
    for tool in file_tools() {
        if tool.name == N::READ_FILE || tool.name == N::LIST_FILES {
            tools.push(tool);
        }
    }
    tools
}

/// Read-only tools — used during plan mode (no writes, no terminal commands).
pub fn read_only_tools() -> Vec<LlmTool> {
    let mut tools = vec![];
    // File reading only
    for tool in file_tools() {
        if tool.name == N::READ_FILE || tool.name == N::LIST_FILES {
            tools.push(tool);
        }
    }
    // All search tools
    tools.extend(search_tools());
    // Web tools
    tools.extend(web_tools());
    // Interaction + task tools
    tools.extend(interaction_tools());
    tools.extend(task_tools());
    // Plan tools (only submit_plan, not enter_plan_mode since already in plan mode)
    for tool in plan_tools() {
        if tool.name == N::SUBMIT_PLAN {
            tools.push(tool);
            break;
        }
    }
    // Logbook tools — read-only by definition, safe in plan mode.
    tools.extend(logbook_tools());
    tools
}

/// Logbook tools — read-only access to the logbooks (knowledge nodes) of
/// the current project. Lets the AI explore content the user has written
/// about nodes, even nodes the user hasn't explicitly mentioned in the chat.
pub fn logbook_tools() -> Vec<LlmTool> {
    vec![
        LlmTool {
            name: N::LIST_LOGBOOKS.into(),
            description: "List every logbook (knowledge node) in the current project. Returns one row per node with: node_id (UUID), name, variant (knowledge_node | lighthouse), and section_count.\n\nUSE THIS for any prompt asking 'what logbooks do I have', 'what nodes do we have', 'list the logbooks', 'show my knowledge nodes', or similar discovery questions. NEVER call search_logbook with an empty query for this purpose — that returns sections, not nodes.\n\nNo arguments. Returns the node_id values needed by read_logbook.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        LlmTool {
            name: N::READ_LOGBOOK.into(),
            description: "Read a single logbook by its node id. Returns subtype + every section's name, source (user vs ai), and full markdown content.\n\nUSE THIS when you already have a node_id from list_logbooks or search_logbook. The id is a UUID like '7f3a...' — never pass a name, snippet, or partial string here. If you only have a name, call list_logbooks first to get the id.\n\nWorks for knowledge_node and lighthouse variants. Returns an error for module nodes.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "node_id": {
                        "type": "string",
                        "description": "UUID of the knowledge node. Get it from list_logbooks or search_logbook results — do NOT pass a name."
                    },
                    "logbook_id": {
                        "type": "string",
                        "description": "Alias for node_id — accepted because some models prefer this naming. Pick either, not both."
                    }
                },
                "required": []
            }),
        },
        LlmTool {
            name: N::SEARCH_LOGBOOK.into(),
            description: "Search the CONTENT of all logbooks for a substring match. Returns hits with node_id, node_name, section_name, and a snippet around the match.\n\nUSE THIS when the user asks 'what have I written about X', 'find X in my logbooks'. The query MUST be a non-empty substring — never pass an empty string or wildcard.\n\nDO NOT use this to discover what logbooks exist (use list_logbooks for that — empty/wildcard queries on this tool return section snippets, not node names).\n\nCase-insensitive. Pair with read_logbook to fetch a full hit.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Non-empty substring to search for. Case-insensitive. NEVER pass empty or '*'."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max number of hits to return (default: 10, max: 50)."
                    }
                },
                "required": ["query"]
            }),
        },
        LlmTool {
            name: N::LIST_CONNECTIONS.into(),
            description: "List every directed manual connection (arrow) currently drawn between nodes in the project. Returns rows of `from_node_id → to_node_id` with both names resolved. Direction matters: `from` is the source, `to` is the target.\n\nUSE THIS when the user mentions \"the node connected to X\", \"what connects to Y\", \"how is island Z linked\", or to verify your graph view before drawing a new `create_connection`. It is the ONLY way to see arrows — `list_logbooks` only shows counts, not the actual edges.\n\nRead-only. No arguments.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        LlmTool {
            name: N::LIST_ISLANDS.into(),
            description: "Show every island (lighthouse + its child knowledge_nodes) grouped by theme. Each row gives the lighthouse's name, its child count, the names of its children, total sections across the island, and aggregate inbound/outbound manual connection counts. Floating nodes (no lighthouse) are listed at the bottom.\n\nUSE THIS BEFORE creating new structure (`create_lighthouse`, `create_knowledge_node`). It tells you which islands already exist *and what they contain by topic* — so you can decide whether a new concept belongs in an existing island or genuinely needs its own. `list_logbooks` only shows a flat list of nodes; this tool gives you the thematic view.\n\nExample: if the user wants to record something about OIDC and `list_islands` shows an \"Auth\" island containing OAuth, JWT, Sessions, RBAC, the right move is to add a node or section inside Auth — not to create a new island.\n\nRead-only. No arguments.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        LlmTool {
            name: N::QUERY_NEIGHBORHOOD.into(),
            description: "List the nodes within `radius` Manhattan-distance cells of a given node on the Ocean Canvas grid. Returns each neighbour's id, name, variant, island membership, and exact distance.\n\nUSE THIS when the user asks about spatial relations — \"what is near node X\", \"what nodes are around lighthouse Y\". Useful before drawing a new `create_connection` to reason about whether the link will be visually short or long, and to find candidate neighbours when the user describes a relation by proximity.\n\nDoes NOT show manual connections (use `list_connections` for those). The grid is integer Manhattan; distance 1 means an adjacent cell, distance 3 captures a tight cluster.\n\nRead-only.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "node_id": {
                        "type": "string",
                        "description": "UUID of the node whose neighbourhood you want to inspect. Get it from list_logbooks."
                    },
                    "radius": {
                        "type": "integer",
                        "description": "Manhattan distance (in grid cells). Default 3. Max 10."
                    }
                },
                "required": ["node_id"]
            }),
        },
        LlmTool {
            name: N::PROPOSE_LOGBOOK_WRITE.into(),
            description: "Propose a new section in a logbook (knowledge node), or propose an edit to an existing section. The proposal does NOT apply immediately — it enters a pending state and the user reviews/accepts/discards/regenerates it from the node panel (with a diff for edits). The chat is not the surface for the content; coordinate, don't paste.\n\nUSE THIS when the user asks you to add notes/findings/decisions/considerations to a specific node, or to update an existing section's contents. Always call `read_logbook` first to see the current sections and their `id` values.\n\nKey rules:\n- To ADD a brand new section: pass `node_id`, `name`, `content_markdown`, `prompt`. Omit `edit_section_id`.\n- To EXTEND OR REWRITE an existing section: pass `edit_section_id` (the `id:` shown by `read_logbook`) and the FULL new markdown body — not a patch, not just the addition. Re-include the existing text plus your additions/changes.\n- Don't create a new section when the user said \"in the same section\" / \"add a paragraph\" / \"continue this\". Use `edit_section_id` and pass the existing body + your new content.\n- If you previously proposed a Create for the same `name` and the user hasn't accepted yet, calling Create again with that same `name` replaces your previous pending — useful when iterating before approval.\n\nAnnounce intent in chat *before* calling — one short line saying what you're about to write and to which node. Don't dump the content into the chat.\n\nReturns a `write_id` and a confirmation that the proposal is awaiting approval. Do not retry on success; wait for the user.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "node_id": {
                        "type": "string",
                        "description": "UUID of the target knowledge node. Get from list_logbooks or search_logbook."
                    },
                    "name": {
                        "type": "string",
                        "description": "Section name. For an edit, this becomes the new name (pass the existing name to keep it)."
                    },
                    "content_markdown": {
                        "type": "string",
                        "description": "Full markdown body of the section. For edits, the COMPLETE new content — not a patch."
                    },
                    "prompt": {
                        "type": "string",
                        "description": "Short description (one sentence) of the user intent that triggered this write. Stored as ai_prompt so the user can regenerate later."
                    },
                    "edit_section_id": {
                        "type": "string",
                        "description": "Optional. Section UUID (the `id:` returned by read_logbook) to edit in place. Pass this — and the FULL rewritten body in `content_markdown` — whenever the user says 'in the same section', 'append a paragraph', 'rewrite this', or otherwise refers to existing content. Omit only when the user genuinely wants a brand new section."
                    }
                },
                "required": ["node_id", "name", "content_markdown", "prompt"]
            }),
        },
    ]
}

/// Structure tools — manipulate the Ocean Canvas graph itself: create
/// lighthouses, knowledge nodes, directed connections, promote
/// existing nodes, and re-assign a node to a lighthouse. These tools
/// affect the *structure* of the workspace; `propose_logbook_write` only
/// adds *content* to existing nodes.
pub fn structure_tools() -> Vec<LlmTool> {
    vec![
        LlmTool {
            name: N::CREATE_LIGHTHOUSE.into(),
            description: "Create a new **lighthouse** on the Ocean Canvas. A lighthouse is the anchor of an *island* — a thematic cluster of knowledge_nodes belonging to it. Use this when the user describes a new project, theme, or major topic that warrants its own island.\n\nThe lighthouse can host its own sections via `propose_logbook_write` and acts as the entry point of the cluster. Nodes attached to it (via `lighthouse_id`) form the rest of the island.\n\nPosition is auto-picked — the system finds the first free grid cell starting from (0, 0) and spiralling outward. Optionally pass `near_node_id` to place the new lighthouse in the neighbourhood of an existing node.\n\nReturns the new lighthouse's `node_id` (a UUID) — keep it to attach knowledge_nodes to the cluster afterwards.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Human-readable name of the lighthouse / island. Be concise and descriptive — this name shows up in the canvas."
                    },
                    "near_node_id": {
                        "type": "string",
                        "description": "Optional node UUID to place the new lighthouse nearby. Omit to place near (0,0)."
                    }
                },
                "required": ["name"]
            }),
        },
        LlmTool {
            name: N::CREATE_KNOWLEDGE_NODE.into(),
            description: "Create a new **knowledge_node** (sub-topic, child of an island) on the Ocean Canvas. A knowledge_node is a discrete topic; its sections hold the actual content (decisions, findings, notes).\n\nUse this when the user describes a sub-topic of an existing project. Pass `lighthouse_id` to attach the node to a lighthouse (so it becomes part of that island); omit it to leave the node floating free (no island membership).\n\nPosition is auto-picked — the system finds a free cell. If `lighthouse_id` is set, the node is placed near that lighthouse for a tidy cluster. Otherwise, pass `near_node_id` to place it near another node, or omit both for default placement.\n\nReturns the new node's `node_id` — use it to attach sections via `propose_logbook_write` or to draw connections via `create_connection`.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Human-readable name of the topic / sub-theme."
                    },
                    "lighthouse_id": {
                        "type": "string",
                        "description": "Optional UUID of the lighthouse this node belongs to. Get it from `list_logbooks` (variant=lighthouse) or from a previous `create_lighthouse` call."
                    },
                    "near_node_id": {
                        "type": "string",
                        "description": "Optional node UUID to place the new node nearby. If `lighthouse_id` is set, this defaults to that lighthouse."
                    }
                },
                "required": ["name"]
            }),
        },
        LlmTool {
            name: N::CREATE_CONNECTION.into(),
            description: "Create a directed **connection** between two nodes on the Ocean Canvas. Direction matters — `from_node_id → to_node_id`. To express bidirectional intent, create two arrows.\n\nUse this when the user describes a causal, dependency, or sequential relationship between two existing nodes (or a lighthouse and a node). Connections live separately from the parent/child lighthouse relation — they're free-form edges anyone can draw.\n\nFails if either node doesn't exist, if the two ids are the same, or if the same directed pair already has a connection. Returns the new connection's id.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "from_node_id": {
                        "type": "string",
                        "description": "UUID of the source node (origin of the arrow)."
                    },
                    "to_node_id": {
                        "type": "string",
                        "description": "UUID of the target node (tip of the arrow)."
                    }
                },
                "required": ["from_node_id", "to_node_id"]
            }),
        },
        LlmTool {
            name: N::PROMOTE_TO_LIGHTHOUSE.into(),
            description: "Convert an existing **knowledge_node** into a **lighthouse**, making it the anchor of a new island. After promotion, other nodes can be assigned to this lighthouse via `set_node_lighthouse`.\n\nUse this when the user realises a sub-topic has grown enough to deserve its own island, or when the structure needs reorganising.\n\nFails if the node doesn't exist or is already a lighthouse.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "node_id": {
                        "type": "string",
                        "description": "UUID of the knowledge_node to promote. Get it from `list_logbooks` or a previous `create_knowledge_node` call."
                    }
                },
                "required": ["node_id"]
            }),
        },
        LlmTool {
            name: N::SET_NODE_LIGHTHOUSE.into(),
            description: "Re-assign a **knowledge_node** to a different **lighthouse**, or detach it (make it floating). Nodes that belong to a lighthouse form its island; this tool moves a node from one island to another, or out of any island.\n\nUse this when the user wants to restructure an existing graph — e.g., \"node X actually belongs to project Y\".\n\nPass `lighthouse_id` to attach (must be the UUID of an existing lighthouse). Pass an empty string or omit to detach. Fails if the target node is itself a lighthouse, or if the new lighthouse_id doesn't point to a real lighthouse.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "node_id": {
                        "type": "string",
                        "description": "UUID of the knowledge_node to re-assign."
                    },
                    "lighthouse_id": {
                        "type": "string",
                        "description": "UUID of the new lighthouse. Empty string or omitted to detach the node from any island."
                    }
                },
                "required": ["node_id"]
            }),
        },
        LlmTool {
            name: N::RENAME_NODE.into(),
            description: "Rename an existing node (lighthouse or knowledge_node) in place. Use this when the user asks to change the name of a node — e.g. \"change 'Market' to 'Colombia Market'\".\n\n**ALWAYS prefer this over `create_knowledge_node` + delete.** Renaming preserves the node's id, sections, connections, and lighthouse_id; recreating loses everything. If the user says \"update the name / change its name / rename it\", this is the right tool.\n\nFails if the node doesn't exist or if `new_name` is empty.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "node_id": {
                        "type": "string",
                        "description": "UUID of the node to rename. Get it from `list_logbooks` — never make it up."
                    },
                    "new_name": {
                        "type": "string",
                        "description": "New human-readable name for the node. Non-empty."
                    }
                },
                "required": ["node_id", "new_name"]
            }),
        },
    ]
}

/// Hardcoded tool inventory for Knowledge-mode chats. Used as the fallback
/// when the DB-backed mode resolver cannot find a non-empty tool set for
/// `mode-knowledge` — this keeps file/terminal tools out of a Knowledge
/// agent even if the user wiped the chat_modes table.
///
/// Mirrors `mode-knowledge` from `agents::seed::default_chat_modes`:
/// logbook + knowledge research + read-only search + web + interaction +
/// task. Notably absent: terminal, file editing, plan, sub-agent spawn.
pub fn knowledge_mode_tools() -> Vec<LlmTool> {
    let mut tools = logbook_tools();
    tools.extend(structure_tools());
    tools.extend(knowledge_tools());
    // Read-only search tools only — search_text and search_code, no list_files
    // or grep over arbitrary disk roots.
    for tool in search_tools() {
        if tool.name == N::SEARCH_TEXT || tool.name == N::SEARCH_CODE {
            tools.push(tool);
        }
    }
    tools.extend(web_tools());
    tools.extend(interaction_tools());
    tools.extend(task_tools());
    // Cross-project consultation — a Knowledge workspace can interrogate a
    // connected codebase peer via ask_project (mirrors mode-knowledge).
    tools.extend(mesh_tools());
    tools
}

/// Tools available for a specific sub-agent type.
pub fn sub_agent_type_tools(agent_type: &str) -> Vec<LlmTool> {
    match agent_type {
        "research" => {
            let mut tools = vec![];
            for tool in file_tools() {
                if tool.name == N::READ_FILE || tool.name == N::LIST_FILES {
                    tools.push(tool);
                }
            }
            tools.extend(search_tools());
            tools.extend(web_tools());
            tools.extend(logbook_tools());
            tools
        }
        "code" => {
            let mut tools = file_tools();
            tools.extend(search_tools());
            tools
        }
        "test" => {
            let mut tools = vec![];
            for tool in file_tools() {
                if tool.name == N::READ_FILE || tool.name == N::WRITE_FILE || tool.name == N::LIST_FILES {
                    tools.push(tool);
                }
            }
            tools.extend(terminal_tools());
            tools.extend(search_tools());
            tools
        }
        "executor" => {
            let mut tools = vec![];
            for tool in file_tools() {
                if tool.name == N::READ_FILE || tool.name == N::LIST_FILES {
                    tools.push(tool);
                }
            }
            tools.extend(search_tools());
            tools.extend(terminal_tools());
            tools.extend(verification_tools());
            tools
        }
        _ => vec![],
    }
}

/// Main agent tools — everything except executor-only tools (run_app, check_health).
/// The main agent delegates app startup to the executor sub-agent, which has those tools.
pub fn main_agent_tools() -> Vec<LlmTool> {
    let mut tools = vec![];
    // Terminal tools minus run_app (executor-only)
    for tool in terminal_tools() {
        if tool.name != N::RUN_APP {
            tools.push(tool);
        }
    }
    // No verification_tools — check_health is executor-only
    tools.extend(file_tools());
    tools.extend(search_tools());
    tools.extend(web_tools());
    tools.extend(interaction_tools());
    tools.extend(task_tools());
    tools.extend(plan_tools());
    tools.extend(sub_agent_tools());
    tools.extend(mesh_tools());
    tools.extend(logbook_tools());
    tools
}

/// All tools — every category combined. Used to seed the user-facing tool
/// library. Mesh-internal tools (e.g. ask_caller) are deliberately excluded —
/// they're only routed to mesh sub-agents and shouldn't be user-editable.
pub fn all_tools() -> Vec<LlmTool> {
    let mut tools = terminal_tools();
    tools.extend(verification_tools());
    tools.extend(file_tools());
    tools.extend(search_tools());
    tools.extend(web_tools());
    tools.extend(interaction_tools());
    tools.extend(task_tools());
    tools.extend(plan_tools());
    tools.extend(sub_agent_tools());
    tools.extend(mesh_tools());
    tools.extend(knowledge_tools());
    tools.extend(logbook_tools());
    tools.extend(structure_tools());
    tools
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_agent_tools_read_only() {
        let tools = mesh_agent_tools();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        // Should include read-only investigation tools + ask_caller
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"list_files"));
        assert!(names.contains(&"search_code"));
        assert!(names.contains(&"search_text"));
        assert!(names.contains(&"ask_caller"), "mesh_agent_tools should include ask_caller");
        // Should NOT include write/interactive/recursive tools
        assert!(!names.contains(&"write_file"));
        assert!(!names.contains(&"edit_file"));
        assert!(!names.contains(&"run_terminal_command"));
        assert!(!names.contains(&"ask_project"));
        assert!(!names.contains(&"ask_user"));
        assert!(!names.contains(&"spawn_agent"));
        assert!(!names.contains(&"web_fetch"));
    }

    #[test]
    fn test_ask_caller_not_in_main_tools() {
        let tools = main_agent_tools();
        let has_ask_caller = tools.iter().any(|t| t.name == "ask_caller");
        assert!(!has_ask_caller, "main_agent_tools should NOT include ask_caller");
    }

    #[test]
    fn test_main_agent_tools_includes_ask_project() {
        let tools = main_agent_tools();
        let has_ask_project = tools.iter().any(|t| t.name == "ask_project");
        assert!(has_ask_project, "main_agent_tools() should include ask_project");
    }

    #[test]
    fn test_all_tools_includes_ask_project() {
        let tools = all_tools();
        let has_ask_project = tools.iter().any(|t| t.name == "ask_project");
        assert!(has_ask_project, "all_tools() should include ask_project");
    }

    #[test]
    fn test_mesh_tools_has_required_params() {
        let tools = mesh_tools();
        assert_eq!(tools.len(), 1);
        let ask = &tools[0];
        assert_eq!(ask.name, "ask_project");
        let required = ask.parameters["required"].as_array().unwrap();
        let names: Vec<&str> = required.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(names.contains(&"project"));
        assert!(names.contains(&"question"));
    }
}
