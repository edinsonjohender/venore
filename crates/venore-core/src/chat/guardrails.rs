//! Guardrails for LLM responses.
//!
//! Detects "narrated actions" — text that claims to have executed actions (file edits,
//! commands, git operations) without actually calling tools. This is a known pattern
//! with Gemini models.

/// Action verbs in past tense (Spanish + English) that indicate the model claims
/// to have performed an action.
const ACTION_VERBS: &[&str] = &[
    // Spanish — preterite (1st person)
    "revertí",
    "creé",
    "modifiqué",
    "eliminé",
    "instalé",
    "ejecuté",
    "restauré",
    "escribí",
    "actualicé",
    "borré",
    "moví",
    "renombré",
    "agregué",
    "añadí",
    "corregí",
    "cambié",
    "conecté",
    "asigné",
    // Spanish — present perfect ("he creado", "ha quedado", etc.) and
    // bare participles. We match the participle so "he creado", "ha
    // creado", "han creado", "se ha creado" all trigger.
    "creado",
    "agregado",
    "añadido",
    "modificado",
    "actualizado",
    "renombrado",
    "movido",
    "borrado",
    "eliminado",
    "ejecutado",
    "instalado",
    "conectado",
    "asignado",
    // English
    "reverted",
    "created",
    "modified",
    "deleted",
    "installed",
    "ran ",
    "executed",
    "fixed",
    "updated",
    "wrote",
    "restored",
    "removed",
    "moved",
    "renamed",
    "added",
    "changed",
    "committed",
    "pushed",
    "pulled",
];

/// Indicators that the text references files, paths, or shell commands.
const FILE_COMMAND_INDICATORS: &[&str] = &[
    // File extensions
    ".tsx",
    ".ts",
    ".rs",
    ".js",
    ".jsx",
    ".json",
    ".toml",
    ".css",
    ".html",
    ".py",
    ".md",
    // Path patterns
    "src/",
    "./",
    "../",
    "crates/",
    "node_modules/",
    "package.json",
    "Cargo.toml",
    // Shell commands
    "git ",
    "npm ",
    "cargo ",
    "pnpm ",
    "yarn ",
    "bun ",
    "npx ",
    "mkdir ",
    "rm ",
    "cp ",
    "mv ",
    // Knowledge mode entities — extends the guard to Knowledge projects,
    // where the AI narrates actions on faros / nodos / secciones rather
    // than on code files. Leading space matches "el nodo X", "los dos
    // nodos que pediste", "creé un faro", "para la sección Y", etc.
    " nodo ",
    " nodos ",
    " nodos.",
    " nodos,",
    " faro ",
    " faros ",
    " faro.",
    " faros.",
    " isla ",
    " islas ",
    " sección",
    " secciones",
    " bitácora",
    " bitácoras",
    " conexión",
    " conexiones",
    " the node ",
    " the nodes ",
    " the lighthouse",
    " the section",
    " the connection",
    " the logbook",
    " knowledge node",
    " knowledge_node",
];

/// Prefixes that indicate future intent or questions — NOT narrated actions.
const EXCLUSION_PREFIXES: &[&str] = &[
    // Spanish future intent
    "voy a ",
    "puedo ",
    "debería ",
    "podría ",
    "quieres que ",
    "¿quieres que ",
    "te sugiero ",
    "propongo ",
    "necesito ",
    "haré ",
    "debo ",
    // English future intent
    "i'll ",
    "i will ",
    "i can ",
    "i should ",
    "i could ",
    "i would ",
    "shall i ",
    "should i ",
    "would you like ",
    "do you want ",
    "let me ",
    "i need to ",
    "i'm going to ",
    // Questions
    "?",
];

/// Message injected 2 iterations before the step limit.
pub const STEP_LIMIT_WARNING: &str = "\
IMPORTANT: You are approaching the maximum number of steps allowed. \
You have 2 more tool iterations remaining. \
Wrap up your current work: if you have pending changes, verify them now. \
If the task is not complete, summarize what you've done and what remains.";

// =============================================================================
// REPETITION DETECTION
// =============================================================================

/// Number of times the same (tool, args) pair must appear in the window to trigger.
const REPETITION_THRESHOLD: usize = 3;
/// Size of the sliding window for recent tool calls.
const REPETITION_WINDOW: usize = 15;

/// Tracks recent tool calls to detect repetitive loops.
pub struct RepetitionTracker {
    recent_calls: Vec<(String, u64)>, // (tool_name, args_hash)
}

impl Default for RepetitionTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl RepetitionTracker {
    pub fn new() -> Self {
        Self {
            recent_calls: Vec::new(),
        }
    }

    /// Record a tool call. Returns `true` if the same (tool, args) has been seen
    /// `REPETITION_THRESHOLD` or more times within the sliding window.
    pub fn record_and_check(&mut self, tool_name: &str, arguments: &serde_json::Value) -> bool {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        let args_str = arguments.to_string();
        args_str.hash(&mut hasher);
        let args_hash = hasher.finish();

        self.recent_calls.push((tool_name.to_string(), args_hash));

        // Trim to sliding window
        if self.recent_calls.len() > REPETITION_WINDOW {
            let excess = self.recent_calls.len() - REPETITION_WINDOW;
            self.recent_calls.drain(..excess);
        }

        // Count occurrences of this exact (tool, args) in the window
        let count = self
            .recent_calls
            .iter()
            .filter(|(name, hash)| name == tool_name && *hash == args_hash)
            .count();

        count >= REPETITION_THRESHOLD
    }
}

/// Message injected when tool call repetition is detected.
pub const REPETITION_CORRECTION: &str = "\
STOP. You are repeating the same tool call multiple times without progress. \
Try a DIFFERENT approach: \
- Re-read the error messages carefully. \
- Use a different tool or different arguments. \
- If you're stuck in a loop, step back and reconsider your strategy. \
- If you've tried 3+ approaches and none work, explain what you've tried and ask the user for guidance.";

// =============================================================================
// FOCUS CHAIN
// =============================================================================

/// Inject a focus reminder every N iterations.
pub const FOCUS_CHAIN_INTERVAL: usize = 5;

/// Build a focus reminder message from the user's original request.
pub fn build_focus_reminder(original_message: &str) -> String {
    let truncated = if original_message.len() > 500 {
        let boundary = original_message.floor_char_boundary(497);
        format!("{}...", &original_message[..boundary])
    } else {
        original_message.to_string()
    };

    format!(
        "TASK REMINDER: The user's original request was:\n\"{}\"\n\
         Stay focused on this task. If you are done, verify your work. \
         If you are stuck, try a different approach.",
        truncated
    )
}

// =============================================================================
// PROMPT STOP RULES
// =============================================================================

/// Rules injected into the system prompt to prevent loops.
pub const PROMPT_STOP_RULES: &str = "\n## Retry Rules\n\
- If the same approach fails 3 times, try a COMPLETELY DIFFERENT approach.\n\
- If 3 different approaches all fail, STOP and explain what you tried.\n\
- Never call check_health more than 3 times in a row.\n\
- Never run the same terminal command more than 2 times if you get the same error.\n\
- If you are going in circles, re-read the error messages carefully before trying again.\n";

// =============================================================================
// CHECKPOINT
// =============================================================================

/// After this many tool calls, inject a checkpoint message.
pub const CHECKPOINT_TOOL_CALLS: u32 = 50;

/// Message injected at the checkpoint to force self-evaluation.
pub const CHECKPOINT_MESSAGE: &str = "\
CHECKPOINT: You have used 50 tool calls. Before continuing, evaluate your progress:\n\
1. What was the original task?\n\
2. What have you accomplished so far?\n\
3. What remains to be done?\n\
4. Are you making progress, or going in circles?\n\
If you are not making progress, STOP and explain the situation to the user. \
If you are making progress, continue — but be efficient with your remaining tool calls.";

// =============================================================================
// SURRENDER DETECTION
// =============================================================================

/// Phrases that indicate the model is giving up or trying to hand the task
/// back to the user instead of continuing to work.
const SURRENDER_PHRASES: &[&str] = &[
    // Spanish — present tense
    "más allá de mis capacidades",
    "requiere intervención humana",
    "no puedo resolver",
    "no puedo solucion",
    "recomiendo que un desarrollador",
    "necesitarás investigar",
    "necesitas investigar",
    "escapa de mi alcance",
    "fuera de mi alcance",
    "no tengo la capacidad",
    "está fuera de mis posibilidades",
    "te sugiero que",
    "deberías investigar manualmente",
    "no me es posible",
    // Spanish — past tense / giving up
    "me rindo",
    "he agotado",
    "no he podido resolver",
    "no he podido solucionar",
    "he fallado",
    "límite de mi capacidad",
    "imposible de depurar",
    "imposible de resolver",
    "desarrollador humano",
    "intervención de un desarrollador",
    "lamento no haber podido",
    "mis disculpas por no poder",
    // English
    "beyond my capabilities",
    "beyond my ability",
    "requires human intervention",
    "cannot resolve this",
    "cannot solve this",
    "recommend a developer",
    "you should investigate",
    "you'll need to investigate",
    "outside my scope",
    "beyond the scope of what i can",
    "i'm unable to",
    "i am unable to",
    "not possible for me to",
    "i cannot help further",
    "i give up",
    "i've exhausted",
    "i have exhausted",
    "reached the limit",
    "a human developer",
    "human intervention",
    "impossible to debug",
    "impossible to resolve",
];

/// Message injected when the model tries to surrender/give up on the task.
pub const SURRENDER_CORRECTION: &str = "\
STOP. You are NOT allowed to give up or hand the task back to the user. \
You are an agent — your job is to keep working until the task is resolved. \
If your current approach failed, try a COMPLETELY DIFFERENT approach: \
- Re-read the error messages carefully — what do they actually say? \
- Search the codebase for clues you may have missed. \
- Undo any changes you made that didn't help. \
- Try the simplest possible fix first. \
Do NOT apologize or explain why it's hard. Just keep working.";

/// Returns `true` if the text contains phrases indicating the model is
/// giving up or trying to hand the task back to the user.
pub fn detect_surrender(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.len() < 30 {
        return false;
    }

    let lower = trimmed.to_lowercase();
    SURRENDER_PHRASES.iter().any(|phrase| lower.contains(phrase))
}

// =============================================================================
// NARRATED ACTIONS
// =============================================================================

/// Message injected as role=user when narrated actions are detected.
pub const CORRECTION_MESSAGE: &str = "\
IMPORTANT: You did NOT actually execute any of those actions. \
You only described what you would do in text — no tool was called, \
no file was modified, no command was run. \
You MUST use the available tools (read_file, write_file, execute_command, etc.) \
to perform real actions. Do it now.";

/// Returns `true` if the text appears to narrate completed actions (past tense verbs
/// combined with file/command indicators) without having called any tools.
///
/// Requires BOTH an action verb AND a file/command indicator to trigger.
/// Excludes text that expresses future intent or asks questions.
/// Strip pasted tool-call syntax from assistant text.
///
/// Some models (notably Gemini) call tools correctly through the
/// structured channel AND echo the call as code-formatted text in the
/// reply: `` `tool(arg='value')` ``, `venore.create_node(name='X')`,
/// or bare lines like `propose_logbook_write(node_id='abc', ...)`.
/// The pasted text is noise — it duplicates the action, often spans many
/// lines (when args contain long content_markdown), and it gets re-fed to
/// the LLM next turn, encouraging more of the same.
///
/// This function removes those literal duplications without touching real
/// prose. It targets three shapes:
///   1. Backticked tool calls — single or multi-line: `` `name(...)` ``
///      and `` `prefix.name(...)` ``.
///   2. Bare lines whose entire content is a tool-call expression
///      (often what Gemini does when not even bothering with backticks).
///   3. Long runs of blank lines created by the previous removals.
///
/// Conservative on purpose: it WON'T touch a sentence like "I called
/// `read_file` to check" — only removes lines/expressions that ARE the
/// tool-call literal. No-op on text that doesn't contain matches, so
/// it's safe to run unconditionally regardless of provider.
pub fn strip_tool_call_syntax(text: &str) -> String {
    use once_cell::sync::Lazy;
    use regex::Regex;

    // Backticked single-call (DOTALL so multi-line arg blobs match).
    // Non-greedy to avoid eating across multiple calls.
    static RE_BACKTICK_CALL: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?s)`(?:[a-zA-Z_][\w]*\.)?[a-zA-Z_][\w]*\([^`]*\)`").unwrap()
    });
    // Bare lines whose ENTIRE content is `name(...)` or `prefix.name(...)`.
    // Must contain at least one `arg=` or `arg :` token to avoid eating
    // legitimate prose like "see foo(bar)" — function-call literature
    // without keyword args is too generic to risk stripping.
    static RE_BARE_LINE_CALL: Lazy<Regex> = Lazy::new(|| {
        Regex::new(
            r"(?m)^[ \t]*(?:[a-zA-Z_][\w]*\.)?[a-zA-Z_][\w]*\([^()\n]*\b\w+\s*=[^()\n]*\)[ \t]*$"
        )
        .unwrap()
    });
    // Collapse 3+ consecutive newlines to 2 after removals so the result
    // doesn't look gappy.
    static RE_BLANK_RUN: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"\n{3,}").unwrap());

    let stripped = RE_BACKTICK_CALL.replace_all(text, "");
    let stripped = RE_BARE_LINE_CALL.replace_all(&stripped, "");
    let stripped = RE_BLANK_RUN.replace_all(&stripped, "\n\n");
    stripped.trim().to_string()
}

pub fn detect_narrated_actions(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.len() < 15 {
        return false;
    }

    let lower = trimmed.to_lowercase();

    // Exclude future intent and questions
    for prefix in EXCLUSION_PREFIXES {
        if *prefix == "?" {
            if lower.contains('?') {
                // Only exclude if the question mark is dominant (short text asking a question)
                // For long texts with actions AND a question, don't exclude
                if lower.len() < 120 {
                    return false;
                }
            }
        } else if lower.starts_with(prefix) {
            return false;
        }
    }

    let has_action_verb = ACTION_VERBS.iter().any(|v| lower.contains(v));
    let has_indicator = FILE_COMMAND_INDICATORS.iter().any(|ind| lower.contains(ind));

    has_action_verb && has_indicator
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_limit_warning_not_empty() {
        assert!(!STEP_LIMIT_WARNING.is_empty());
        assert!(STEP_LIMIT_WARNING.contains("approaching"));
    }

    // ─── Tool-call syntax stripper ──────────────────────────────

    #[test]
    fn strip_removes_backticked_call() {
        let input = "Hecho. `propose_logbook_write(node_id='abc', name='X')` Listo.";
        let out = strip_tool_call_syntax(input);
        assert!(!out.contains("propose_logbook_write"));
        assert!(out.contains("Hecho."));
        assert!(out.contains("Listo."));
    }

    #[test]
    fn strip_removes_venore_prefixed_call() {
        let input = "He hecho lo siguiente:\n`venore.create_knowledge_node(name='Auth')`\nListo.";
        let out = strip_tool_call_syntax(input);
        assert!(!out.contains("venore.create_knowledge_node"));
        assert!(out.contains("He hecho"));
        assert!(out.contains("Listo."));
    }

    #[test]
    fn strip_removes_multiline_backticked_call() {
        let input = "OK.\n`propose_logbook_write(node_id='a',\n  name='Long',\n  content_markdown='line1\nline2')`\nFin.";
        let out = strip_tool_call_syntax(input);
        assert!(!out.contains("propose_logbook_write"));
        assert!(!out.contains("content_markdown"));
        assert!(out.contains("OK."));
        assert!(out.contains("Fin."));
    }

    #[test]
    fn strip_removes_bare_line_call_with_kwargs() {
        let input = "Resultado:\npropose_logbook_write(node_id='abc', name='X')\n\nFin.";
        let out = strip_tool_call_syntax(input);
        assert!(!out.contains("propose_logbook_write"));
        assert!(out.contains("Resultado:"));
        assert!(out.contains("Fin."));
    }

    #[test]
    fn strip_keeps_inline_function_mention() {
        // Real prose mentioning a function — must NOT be stripped.
        let input = "If you call `read_file` you'll see the result.";
        let out = strip_tool_call_syntax(input);
        assert!(out.contains("`read_file`"));
        assert!(out.contains("you'll see"));
    }

    #[test]
    fn strip_keeps_normal_prose_with_parentheses() {
        // No `arg=` token → bare-line stripper must skip it.
        let input = "El cálculo (3 + 5) da 8.\nEsto es una nota normal.";
        let out = strip_tool_call_syntax(input);
        assert_eq!(out, input.trim());
    }

    #[test]
    fn strip_collapses_blank_runs_after_removal() {
        let input = "Antes\n\n`tool(arg=1)`\n\n\n`other(arg=2)`\n\nDespués";
        let out = strip_tool_call_syntax(input);
        assert!(!out.contains("tool("));
        assert!(!out.contains("\n\n\n"), "should not have triple newlines");
        assert!(out.starts_with("Antes"));
        assert!(out.ends_with("Después"));
    }

    #[test]
    fn strip_is_noop_on_clean_text() {
        let input = "Listo, agregué la sección JWT.";
        assert_eq!(strip_tool_call_syntax(input), input);
    }

    // ─── Surrender detection ────────────────────────────────────

    #[test]
    fn detects_surrender_spanish() {
        assert!(detect_surrender(
            "El problema requiere una depuración más profunda que está más allá de mis capacidades actuales."
        ));
    }

    #[test]
    fn detects_surrender_recommend_developer() {
        assert!(detect_surrender(
            "Recomiendo que un desarrollador revise la configuración del proyecto manualmente."
        ));
    }

    #[test]
    fn detects_surrender_english() {
        assert!(detect_surrender(
            "This issue is beyond my capabilities. You should investigate the logs manually."
        ));
    }

    #[test]
    fn detects_surrender_unable() {
        assert!(detect_surrender(
            "I'm unable to resolve this error. The problem requires deeper debugging."
        ));
    }

    #[test]
    fn detects_surrender_exhausted_spanish() {
        assert!(detect_surrender(
            "He agotado todas las soluciones que se me ocurren, desde reinstalar dependencias y corregir el código."
        ));
    }

    #[test]
    fn detects_surrender_limit_spanish() {
        assert!(detect_surrender(
            "He llegado al límite de mi capacidad para resolver este problema."
        ));
    }

    #[test]
    fn detects_surrender_me_rindo() {
        assert!(detect_surrender(
            "Me rindo. He intentado absolutamente todo y el problema persiste."
        ));
    }

    #[test]
    fn detects_surrender_human_developer_spanish() {
        assert!(detect_surrender(
            "Este problema requiere la intervención de un desarrollador humano con experiencia."
        ));
    }

    #[test]
    fn detects_surrender_lamento() {
        assert!(detect_surrender(
            "Lamento no haber podido resolver el problema. He documentado todos mis intentos."
        ));
    }

    #[test]
    fn detects_surrender_he_fallado() {
        assert!(detect_surrender(
            "He fallado. A pesar de mi persistencia, no he podido solucionar el problema."
        ));
    }

    #[test]
    fn ignores_normal_text_surrender() {
        assert!(!detect_surrender(
            "Voy a intentar otra estrategia para resolver el problema."
        ));
    }

    #[test]
    fn ignores_short_text_surrender() {
        assert!(!detect_surrender("no puedo"));
    }

    // ─── Positive cases: should detect narrated actions ──────────

    #[test]
    fn detects_spanish_revert() {
        assert!(detect_narrated_actions(
            "Revertí los cambios en Navbar.tsx y restauré la versión anterior."
        ));
    }

    #[test]
    fn detects_english_create() {
        assert!(detect_narrated_actions(
            "Created src/utils/auth.ts with the authentication helper functions."
        ));
    }

    #[test]
    fn detects_english_git_command() {
        assert!(detect_narrated_actions(
            "I ran git restore . to revert all unstaged changes."
        ));
    }

    #[test]
    fn detects_spanish_modify() {
        assert!(detect_narrated_actions(
            "Modifiqué el archivo src/components/Header.tsx para agregar el botón."
        ));
    }

    #[test]
    fn detects_english_delete() {
        assert!(detect_narrated_actions(
            "Deleted the old config from package.json and updated dependencies."
        ));
    }

    #[test]
    fn detects_cargo_command() {
        assert!(detect_narrated_actions(
            "Executed cargo build to verify the changes compile correctly."
        ));
    }

    #[test]
    fn detects_npm_install() {
        assert!(detect_narrated_actions(
            "Installed the package using npm install react-router-dom."
        ));
    }

    #[test]
    fn detects_multiple_actions() {
        assert!(detect_narrated_actions(
            "Updated the Cargo.toml and modified src/main.rs to add the new dependency."
        ));
    }

    // ─── Negative cases: should NOT detect ───────────────────────

    #[test]
    fn ignores_descriptive_text() {
        assert!(!detect_narrated_actions(
            "El módulo de auth usa JWT para autenticación."
        ));
    }

    #[test]
    fn ignores_short_text() {
        assert!(!detect_narrated_actions("3"));
    }

    #[test]
    fn ignores_empty_text() {
        assert!(!detect_narrated_actions(""));
    }

    // ─── Knowledge-mode narration detection ─────────────────────

    #[test]
    fn detects_narrated_node_creation() {
        assert!(detect_narrated_actions(
            "He creado los dos nodos que solicitaste."
        ));
    }

    #[test]
    fn detects_narrated_section_creation() {
        assert!(detect_narrated_actions(
            "Creé las secciones recomendadas en cada nodo."
        ));
    }

    #[test]
    fn detects_narrated_lighthouse_creation() {
        assert!(detect_narrated_actions(
            "Agregué un nuevo faro para tu proyecto."
        ));
    }

    #[test]
    fn detects_narrated_isla_creation() {
        assert!(detect_narrated_actions(
            "He creado las dos islas que necesitas."
        ));
    }

    #[test]
    fn ignores_future_intent_node() {
        assert!(!detect_narrated_actions(
            "Voy a crear el nodo de Frontend en la isla principal."
        ));
    }

    #[test]
    fn ignores_question_about_node() {
        assert!(!detect_narrated_actions(
            "¿Quieres que cree un nodo nuevo en esta isla?"
        ));
    }

    #[test]
    fn ignores_future_intent_spanish() {
        assert!(!detect_narrated_actions(
            "Voy a leer el archivo src/main.rs para entender la estructura."
        ));
    }

    #[test]
    fn ignores_question_spanish() {
        assert!(!detect_narrated_actions(
            "¿Quieres que revierta los cambios en main.rs?"
        ));
    }

    #[test]
    fn ignores_future_intent_english() {
        assert!(!detect_narrated_actions(
            "I'll read the file src/main.rs to understand the structure."
        ));
    }

    #[test]
    fn ignores_let_me_intent() {
        assert!(!detect_narrated_actions(
            "Let me check the src/utils.ts file for errors."
        ));
    }

    #[test]
    fn ignores_error_reference() {
        // Has a file indicator (.rs) but no action verb
        assert!(!detect_narrated_actions(
            "El error está en main.rs línea 42."
        ));
    }

    #[test]
    fn ignores_action_verb_without_indicator() {
        // Has action verb but no file/command indicator
        assert!(!detect_narrated_actions(
            "I deleted the old approach and wrote a new one from scratch."
        ));
    }

    #[test]
    fn ignores_should_i() {
        assert!(!detect_narrated_actions(
            "Should I update the package.json with the new version?"
        ));
    }

    // ─── Repetition detection ──────────────────────────────────

    #[test]
    fn repetition_triggers_at_threshold() {
        let mut tracker = RepetitionTracker::new();
        let args = serde_json::json!({"file_path": "/src/main.rs"});

        assert!(!tracker.record_and_check("read_file", &args));
        assert!(!tracker.record_and_check("read_file", &args));
        assert!(tracker.record_and_check("read_file", &args)); // 3rd = threshold
    }

    #[test]
    fn repetition_different_args_no_trigger() {
        let mut tracker = RepetitionTracker::new();

        assert!(!tracker.record_and_check("read_file", &serde_json::json!({"path": "a.rs"})));
        assert!(!tracker.record_and_check("read_file", &serde_json::json!({"path": "b.rs"})));
        assert!(!tracker.record_and_check("read_file", &serde_json::json!({"path": "c.rs"})));
    }

    #[test]
    fn repetition_different_tools_no_trigger() {
        let mut tracker = RepetitionTracker::new();
        let args = serde_json::json!({"query": "foo"});

        assert!(!tracker.record_and_check("search_text", &args));
        assert!(!tracker.record_and_check("search_code", &args));
        assert!(!tracker.record_and_check("read_file", &args));
    }

    #[test]
    fn repetition_window_slides() {
        let mut tracker = RepetitionTracker::new();
        let target_args = serde_json::json!({"cmd": "check_health"});

        // Record 2 of the target call
        tracker.record_and_check("run_terminal_command", &target_args);
        tracker.record_and_check("run_terminal_command", &target_args);

        // Fill with 15 different calls to push the first 2 out of the window
        for i in 0..REPETITION_WINDOW {
            let different_args = serde_json::json!({"file": format!("file_{}.rs", i)});
            tracker.record_and_check("read_file", &different_args);
        }

        // Now the original 2 are out of window, this should NOT trigger (only 1 in window)
        assert!(!tracker.record_and_check("run_terminal_command", &target_args));
    }

    // ─── Focus chain ───────────────────────────────────────────

    #[test]
    fn focus_reminder_truncates_long_messages() {
        let long_msg = "a".repeat(600);
        let reminder = build_focus_reminder(&long_msg);
        assert!(reminder.contains("TASK REMINDER"));
        assert!(reminder.len() < 700); // truncated + surrounding text
    }

    #[test]
    fn focus_reminder_short_message_intact() {
        let reminder = build_focus_reminder("Fix the login bug");
        assert!(reminder.contains("Fix the login bug"));
        assert!(reminder.contains("TASK REMINDER"));
    }
}
