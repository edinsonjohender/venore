//! Skills Tauri commands — slash command registry

use crate::utils::CommandResult;
use venore_core::skills::Skill;

/// List all available skills (slash commands)
#[tauri::command]
pub async fn list_skills() -> CommandResult<Vec<Skill>> {
    CommandResult::ok(venore_core::skills::list_skills())
}
