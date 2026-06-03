//! Skill Registry — hardcoded slash commands for AI prompt expansion.

use serde::Serialize;

/// A skill (slash command) that expands to a full prompt.
#[derive(Debug, Clone, Serialize)]
pub struct Skill {
    /// Command name (without `/` prefix), e.g. "commit"
    pub name: String,
    /// Short description shown in the palette
    pub description: String,
    /// Full prompt text sent to the AI when the skill is invoked
    pub prompt: String,
}

/// Returns all available skills.
pub fn list_skills() -> Vec<Skill> {
    vec![
        Skill {
            name: "commit".into(),
            description: "Review staged changes, generate commit message, commit".into(),
            prompt: "Review the currently staged git changes (use `run_terminal_command` with `git diff --cached`). \
                     Generate an appropriate commit message following conventional commits format. \
                     Then create the commit. If nothing is staged, check `git status` first and suggest what to stage.".into(),
        },
        Skill {
            name: "fix".into(),
            description: "Find and fix the error in recent terminal output".into(),
            prompt: "Read the recent terminal output to identify any errors or failures. \
                     Analyze the error, find the root cause in the code, and fix it. \
                     After fixing, re-run the failing command to verify the fix works.".into(),
        },
        Skill {
            name: "test".into(),
            description: "Write tests for recently modified files".into(),
            prompt: "Identify the files that were recently modified (check `git diff --name-only` or recent terminal output). \
                     Write comprehensive tests for those files following the existing test patterns in the project. \
                     Run the tests to verify they pass.".into(),
        },
        Skill {
            name: "review".into(),
            description: "Review code changes in current session".into(),
            prompt: "Review all code changes made in the current session. Use `git diff` to see the changes. \
                     Provide a thorough code review covering: correctness, edge cases, error handling, \
                     naming, and potential improvements. Be specific and actionable.".into(),
        },
        Skill {
            name: "explain".into(),
            description: "Explain how the code in the active modules works".into(),
            prompt: "Explain how the code in the currently active context modules works. \
                     Cover the architecture, key components, data flow, and how the pieces fit together. \
                     If no modules are selected, explain the overall project structure.".into(),
        },
    ]
}
