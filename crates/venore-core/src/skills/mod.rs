//! Skills module — slash command registry for AI prompt expansion.
//!
//! Skills are predefined prompt templates that users can invoke via `/command`
//! in the chat input. They are expanded to full prompts before sending.

pub mod registry;

pub use registry::{Skill, list_skills};
