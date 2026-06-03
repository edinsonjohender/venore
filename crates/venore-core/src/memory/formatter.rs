//! Project Memory formatter — produces a compact markdown block for the system prompt.

use super::models::ProjectMemory;

/// Format a `ProjectMemory` into a compact markdown block (~50-80 lines)
/// ready for injection into the LLM system prompt.
pub fn format_project_memory(memory: &ProjectMemory) -> String {
    let mut out = String::with_capacity(1024);

    out.push_str("## Project Memory\n");

    // Identity line
    out.push_str(&format!("- **Project:** {}\n", memory.name));

    if !memory.description.is_empty() {
        out.push_str(&format!("- **Description:** {}\n", memory.description));
    }

    // Compact state / team / goals line
    let mut meta_parts = Vec::new();
    if !memory.state.is_empty() {
        meta_parts.push(format!("**State:** {}", memory.state));
    }
    if !memory.team_size.is_empty() {
        meta_parts.push(format!("**Team:** {}", memory.team_size));
    }
    if !memory.goals.is_empty() {
        meta_parts.push(format!("**Goals:** {}", memory.goals.join(", ")));
    }
    if !meta_parts.is_empty() {
        out.push_str(&format!("- {}\n", meta_parts.join(" | ")));
    }

    // Response language — strong instruction
    if !memory.response_language.is_empty() {
        let lang_name = language_display_name(&memory.response_language);
        out.push_str(&format!(
            "- **Response Language:** {} ({}) — ALWAYS respond in this language\n",
            lang_name, memory.response_language,
        ));
    }

    // Architecture
    if !memory.architecture.is_empty() {
        out.push_str(&format!("- **Architecture:** {}\n", memory.architecture));
    }

    // Tech debt
    if !memory.tech_debt.is_empty() {
        out.push_str(&format!("- **Tech Debt:** {}\n", memory.tech_debt));
    }

    // Conventions
    if !memory.conventions.is_empty() {
        out.push_str("\n### Conventions\n");
        for conv in &memory.conventions {
            out.push_str(&format!("- {}\n", conv));
        }
    }

    // Project Summary
    if !memory.project_summary.is_empty() {
        out.push_str("\n### Project Summary\n");
        out.push_str(&memory.project_summary);
        if !memory.project_summary.ends_with('\n') {
            out.push('\n');
        }
    }

    out
}

/// Map a language code to a human-readable name.
fn language_display_name(code: &str) -> &str {
    match code {
        "en" => "English",
        "es" => "Spanish",
        "zh" => "Chinese",
        "pt" => "Portuguese",
        "ja" => "Japanese",
        _ => code,
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::models::ProjectMemory;

    fn sample_memory() -> ProjectMemory {
        ProjectMemory {
            id: "mem-1".into(),
            project_id: "proj-1".into(),
            name: "Venore v2".into(),
            description: "Desktop app for code analysis".into(),
            state: "active".into(),
            team_size: "solo".into(),
            goals: vec!["document".into(), "maintain".into()],
            architecture: "Rust backend + Tauri + React frontend".into(),
            tech_debt: "".into(),
            response_language: "es".into(),
            conventions: vec![
                "Use VenoreError + Result<T> for all error handling".into(),
                "Use tracing macros, NOT println".into(),
            ],
            project_summary: "Venore generates .context.md files for codebases.".into(),
            created_at: "2025-01-01T00:00:00Z".into(),
            updated_at: "2025-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn test_format_contains_key_sections() {
        let mem = sample_memory();
        let output = format_project_memory(&mem);

        assert!(output.contains("## Project Memory"));
        assert!(output.contains("**Project:** Venore v2"));
        assert!(output.contains("**Description:** Desktop app"));
        assert!(output.contains("**State:** active"));
        assert!(output.contains("**Team:** solo"));
        assert!(output.contains("**Goals:** document, maintain"));
        assert!(output.contains("Spanish (es) — ALWAYS respond in this language"));
        assert!(output.contains("**Architecture:** Rust backend"));
        assert!(output.contains("### Conventions"));
        assert!(output.contains("Use VenoreError"));
        assert!(output.contains("### Project Summary"));
        assert!(output.contains("Venore generates"));
    }

    #[test]
    fn test_format_empty_optional_fields() {
        let mem = ProjectMemory {
            id: "mem-2".into(),
            project_id: "proj-2".into(),
            name: "Minimal".into(),
            description: String::new(),
            state: String::new(),
            team_size: String::new(),
            goals: Vec::new(),
            architecture: String::new(),
            tech_debt: String::new(),
            response_language: String::new(),
            conventions: Vec::new(),
            project_summary: String::new(),
            created_at: "2025-01-01T00:00:00Z".into(),
            updated_at: "2025-01-01T00:00:00Z".into(),
        };
        let output = format_project_memory(&mem);

        assert!(output.contains("**Project:** Minimal"));
        assert!(!output.contains("**Description:**"));
        assert!(!output.contains("### Conventions"));
        assert!(!output.contains("### Project Summary"));
    }
}
