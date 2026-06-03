//! Query router — classifies a chat turn so the orchestrator can pick
//! the right context strategy.
//!
//! Venore's value proposition is instant, precise answers about a
//! project from its precomputed Project Memory. The system prompt
//! already instructs the model to answer informational questions from
//! memory, but that's a soft signal. This router adds a hard one: for
//! questions that are *clearly* about the project as a whole, we strip
//! the codebase-investigation tools entirely so the model physically
//! cannot waste a turn running `list_files` / `read_file` to
//! re-discover what the memory already states.
//!
//! Design: **high precision for `ProjectQuestion`**. A false positive
//! (stripping tools from a real code task) is costly — the model can't
//! act. A false negative just falls back to the prompt-driven behavior,
//! which already works. So the classifier only fires on tightly-matched
//! project-level phrasing and defaults to `CodeTask` otherwise.
//!
//! Bilingual: the user base writes in English and Spanish (tú forms),
//! so both are matched.

/// Classification of a chat turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryClass {
    /// An informational question about the project as a whole, answerable
    /// from Project Memory (purpose, architecture, modules, stack,
    /// opinion, risks). Route to memory-only: investigation tools are
    /// suppressed.
    ProjectQuestion,
    /// Anything actionable or code-specific. Full agent with all tools.
    CodeTask,
}

/// Tool names suppressed when a turn is classified as `ProjectQuestion`.
///
/// These are the codebase-exploration / mutation / execution tools. We
/// keep non-investigation tools (`ask_user`, `web_search`, knowledge
/// tools) available even on project questions. Hardcoded by name because
/// this is a behavior policy, not a tool inventory — the inventory still
/// lives in the DB.
pub const INVESTIGATION_TOOLS: &[&str] = &[
    "read_file",
    "list_files",
    "search_text",
    "search_code",
    "web_fetch",
    "write_file",
    "edit_file",
    "multi_edit_file",
    "run_terminal_command",
    "read_terminal_output",
    "run_app",
    "check_health",
];

/// Action verbs that force `CodeTask` regardless of other signals.
/// Presence of any of these means the user wants something *done*, not
/// just explained.
const ACTION_VERBS: &[&str] = &[
    // English
    "fix", "add", "create", "implement", "refactor", "rename", "run",
    "build", "debug", "edit", "write", "delete", "remove", "install",
    "update", "change", "move", "generate", "test", "deploy", "migrate",
    "optimize", "replace", "configure", "setup", "set up",
    // Spanish (tú forms + infinitives)
    "arregla", "arreglar", "agrega", "agregar", "añade", "añadir",
    "crea", "crear", "implementa", "implementar", "refactoriza",
    "refactorizar", "renombra", "corre", "correr", "ejecuta", "ejecutar",
    "construye", "depura", "edita", "editar", "escribe", "escribir",
    "borra", "borrar", "elimina", "eliminar", "instala", "instalar",
    "actualiza", "actualizar", "cambia", "cambiar", "mueve", "genera",
    "generar", "prueba", "despliega", "migra", "optimiza", "reemplaza",
    "configura", "configurar", "haz", "hazme", "modifica", "modificar",
];

/// Project-level interrogative phrases. Presence of any of these (in the
/// absence of action verbs / code references) marks a `ProjectQuestion`.
const PROJECT_PHRASES: &[&str] = &[
    // English
    "what is this", "what's this", "what is the project", "what does this project",
    "what does the project", "what does this app", "what does the app",
    "what does it do", "what is it about", "tell me about this",
    "tell me about the project", "overview", "high level", "high-level",
    "architecture", "tech stack", "what stack", "what technolog",
    "what modules", "which modules", "project structure", "how is it organized",
    "how is this organized", "what's the purpose", "what is the purpose",
    "summarize the project", "summary of the project", "what are the risks",
    "tech debt", "technical debt", "your opinion", "what do you think",
    // Spanish
    "de qué va", "de que va", "qué es esto", "que es esto",
    "qué es el proyecto", "que es el proyecto", "qué hace", "que hace",
    "qué hace el proyecto", "que hace el proyecto", "qué hace la app",
    "que hace la app", "de qué trata", "de que trata", "resumen",
    "resúmeme", "resumeme", "arquitectura", "qué stack", "que stack",
    "qué tecnolog", "que tecnolog", "qué módulos", "que modulos",
    "estructura del proyecto", "cómo está organizado", "como esta organizado",
    "cuál es el propósito", "cual es el proposito", "cuál es el objetivo",
    "cual es el objetivo", "cuáles son los riesgos", "cuales son los riesgos",
    "deuda técnica", "deuda tecnica", "qué opinas", "que opinas",
    "qué piensas", "que piensas", "para qué sirve", "para que sirve",
];

/// Classify a chat turn from the user's latest message.
pub fn classify(message: &str) -> QueryClass {
    let lower = message.to_lowercase();
    let trimmed = lower.trim();

    if trimmed.is_empty() {
        return QueryClass::CodeTask;
    }

    // Hard negative signals → CodeTask. Code references and action verbs
    // mean the user wants something specific or actionable.
    if mentions_code_reference(trimmed) {
        return QueryClass::CodeTask;
    }
    if contains_word_from(trimmed, ACTION_VERBS) {
        return QueryClass::CodeTask;
    }

    // Positive signal → ProjectQuestion.
    if PROJECT_PHRASES.iter().any(|p| trimmed.contains(p)) {
        return QueryClass::ProjectQuestion;
    }

    // Default: keep tools. The prompt still biases toward memory.
    QueryClass::CodeTask
}

/// Heuristic: does the message reference a specific code artifact?
/// File paths, extensions, function-call syntax, or backtick-quoted
/// identifiers all indicate a code-specific question that may need the
/// actual source, not just the memory overview.
fn mentions_code_reference(text: &str) -> bool {
    if text.contains('/') || text.contains('\\') || text.contains('`') {
        return true;
    }
    // Function-call-ish tokens: `foo(`, `bar()`.
    if text.contains("()") || text.contains("(") && text.contains(")") && text.contains("fn ") {
        return true;
    }
    // Common source-file extensions appearing as `.ext`.
    const EXT_HINTS: &[&str] = &[
        ".rs", ".ts", ".tsx", ".js", ".jsx", ".py", ".go", ".java",
        ".cs", ".php", ".rb", ".kt", ".kts", ".c", ".cpp", ".h", ".hpp",
        ".gd", ".json", ".toml", ".yaml", ".yml", ".md",
    ];
    EXT_HINTS.iter().any(|e| text.contains(e))
}

/// Whitespace/punctuation-aware substring check: matches `needle` only
/// when it appears as a whole word, so "add" doesn't match "address"
/// and "haz" doesn't match "hazard".
fn contains_word_from(text: &str, words: &[&str]) -> bool {
    for w in words {
        if w.contains(' ') {
            // Multi-word phrase: plain substring is fine.
            if text.contains(w) {
                return true;
            }
            continue;
        }
        let mut start = 0;
        while let Some(idx) = text[start..].find(w) {
            let abs = start + idx;
            let before_ok = abs == 0
                || !text[..abs]
                    .chars()
                    .next_back()
                    .map(|c| c.is_alphanumeric())
                    .unwrap_or(false);
            let after = abs + w.len();
            let after_ok = after >= text.len()
                || !text[after..]
                    .chars()
                    .next()
                    .map(|c| c.is_alphanumeric())
                    .unwrap_or(false);
            if before_ok && after_ok {
                return true;
            }
            start = abs + w.len();
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_project(msg: &str) -> bool {
        classify(msg) == QueryClass::ProjectQuestion
    }

    #[test]
    fn project_questions_english() {
        assert!(is_project("what is this project?"));
        assert!(is_project("what does it do"));
        assert!(is_project("give me an overview"));
        assert!(is_project("what's the architecture"));
        assert!(is_project("what is the tech stack here"));
        assert!(is_project("what do you think of it?"));
        assert!(is_project("what are the risks"));
    }

    #[test]
    fn project_questions_spanish() {
        assert!(is_project("de qué va la app"));
        assert!(is_project("de que va el proyecto"));
        assert!(is_project("qué hace este proyecto"));
        assert!(is_project("cuál es el objetivo"));
        assert!(is_project("qué opinas de ella?"));
        assert!(is_project("cuáles son los riesgos"));
        assert!(is_project("para qué sirve"));
        assert!(is_project("cómo está organizado"));
    }

    #[test]
    fn code_tasks_action_verbs() {
        assert!(!is_project("fix the broken import"));
        assert!(!is_project("add a /users endpoint"));
        assert!(!is_project("arregla el bug del login"));
        assert!(!is_project("implementa el sistema de inventario"));
        assert!(!is_project("refactoriza la arquitectura del módulo"));
        assert!(!is_project("crea un nuevo componente"));
    }

    #[test]
    fn code_tasks_code_references() {
        // Mentions a file → needs the actual source, not the overview.
        assert!(!is_project("what does customer_behavior.gd do"));
        assert!(!is_project("explain src/core/state.rs"));
        assert!(!is_project("qué hace la función `compute`"));
        assert!(!is_project("what's in package.json"));
    }

    #[test]
    fn action_verb_with_project_word_stays_code_task() {
        // "refactor the architecture" contains "architecture" (project
        // phrase) but the action verb wins — it's a task.
        assert!(!is_project("refactor the architecture"));
        assert!(!is_project("actualiza la arquitectura"));
    }

    #[test]
    fn word_boundary_avoids_false_action_match() {
        // "address" contains "add" but isn't an action verb — the
        // project phrase "what is this" must still win, not get
        // overridden by a spurious "add" match inside "address".
        assert!(is_project("what is this project, and what is its address"));
    }

    #[test]
    fn ambiguous_defaults_to_code_task() {
        // No clear project phrase, no action verb → keep tools (safe).
        assert!(!is_project("the inventory system"));
        assert!(!is_project("customers"));
        assert!(!is_project("hello"));
    }
}
