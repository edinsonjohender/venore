//! Memory Tauri commands
//!
//! CRUD operations for project memory (compact knowledge block for system prompt).
//!
//! Persistence: `<project_path>/.venore/project-memory.json` is the source of
//! truth; the SQLite `project_memory` row is dual-written as a backup and
//! used as fallback when the file is missing (silent migration on first read).

use std::path::PathBuf;

use venore_core::error::VenoreError;
use venore_core::memory::{file_storage, ProjectMemory};
use venore_core::llm::prelude::*;
use venore_core::llm::JsonSchema;
use venore_core::project::ProjectRepository;

use crate::state::{get_state_field, LazyAppState};
use crate::utils::{IntoStateCommandResult, StateCommandResult};

use super::dto::memory::*;

/// Look up the on-disk path of a registered project so we can read/write the
/// portable `<project_path>/.venore/project-memory.json`.
async fn resolve_project_path(
    project_repository: &ProjectRepository,
    project_id: &str,
) -> Result<PathBuf, VenoreError> {
    let project = project_repository
        .find_by_id(project_id)
        .await?
        .ok_or_else(|| VenoreError::NotFound(format!("Project '{}' not registered", project_id)))?;
    Ok(PathBuf::from(project.path))
}

const CONTEXT_MD_MAX_BYTES: usize = 6000;
const FALLBACK_FILE_MAX_BYTES: usize = 2000;
const REGENERATE_MAX_BYTES: usize = 8000;

/// Truncate a UTF-8 string to at most `max_bytes` without splitting a multi-byte character.
fn truncate_utf8(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    &s[..s.floor_char_boundary(max_bytes)]
}

// =============================================================================
// Read memory directly from a path
// =============================================================================

/// Read `<project_path>/.venore/project-memory.json` directly, without
/// touching SQLite. Used by the wizard's Step 5 to detect whether the
/// project already has curated memory before kicking off an LLM call — the
/// wizard runs before `register_project`, so a project_id isn't available
/// yet for the path through `get_project_memory`.
#[tauri::command]
pub async fn read_project_memory_by_path(
    project_path: String,
) -> StateCommandResult<Option<ProjectMemoryDto>> {
    let result: Result<Option<ProjectMemoryDto>, VenoreError> = async {
        let path = std::path::Path::new(&project_path);
        Ok(file_storage::load(path)?.map(Into::into))
    }
    .await;
    result.into_state()
}

// =============================================================================
// Get project memory
// =============================================================================

#[tauri::command]
pub async fn get_project_memory(
    lazy_state: tauri::State<'_, LazyAppState>,
    project_id: String,
) -> StateCommandResult<Option<ProjectMemoryDto>> {
    let memory_repo = get_state_field!(&lazy_state, memory_repository);
    let project_repo = get_state_field!(&lazy_state, project_repository);
    let result: Result<Option<ProjectMemoryDto>, VenoreError> = async {
        let memory_repo = memory_repo?;
        let project_repo = project_repo?;

        let project_path = resolve_project_path(&project_repo, &project_id).await?;

        // 1) File is source of truth when present.
        if let Some(memory) = file_storage::load(&project_path)? {
            tracing::debug!(project_id = %project_id, "Loaded project memory from .venore/project-memory.json");
            return Ok(Some(memory.into()));
        }

        // 2) Fallback to SQLite (legacy projects without `.venore/project-memory.json`).
        let db_memory = memory_repo.get_by_project(&project_id).await?;
        if let Some(ref memory) = db_memory {
            // Silent migration: write the file so next read is file-first.
            if let Err(e) = file_storage::save(&project_path, memory) {
                tracing::warn!(
                    project_id = %project_id,
                    error = %e,
                    "Could not migrate project memory to .venore/project-memory.json (DB-only mode)"
                );
            } else {
                tracing::info!(project_id = %project_id, "Migrated project memory to .venore/project-memory.json");
            }
        }
        Ok(db_memory.map(|m| m.into()))
    }
    .await;
    result.into_state()
}

// =============================================================================
// Save project memory (upsert)
// =============================================================================

#[tauri::command]
pub async fn save_project_memory(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: SaveProjectMemoryRequest,
) -> StateCommandResult<ProjectMemoryDto> {
    let memory_repo = get_state_field!(&lazy_state, memory_repository);
    let project_repo = get_state_field!(&lazy_state, project_repository);
    let result: Result<ProjectMemoryDto, VenoreError> = async {
        let memory_repo = memory_repo?;
        let project_repo = project_repo?;

        let project_path = resolve_project_path(&project_repo, &request.project_id).await?;

        // Existing memory: read file first (source of truth), DB as fallback.
        // We need it for created_at preservation and id reuse.
        let existing = match file_storage::load(&project_path)? {
            Some(mem) => Some(mem),
            None => memory_repo.get_by_project(&request.project_id).await?,
        };
        let now = chrono::Utc::now().to_rfc3339();

        let memory = ProjectMemory {
            id: existing.as_ref().map(|e| e.id.clone())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            project_id: request.project_id.clone(),
            name: request.name,
            description: request.description,
            state: request.state,
            team_size: request.team_size,
            goals: request.goals,
            architecture: request.architecture,
            tech_debt: request.tech_debt,
            response_language: request.response_language,
            conventions: request.conventions,
            project_summary: request.project_summary,
            created_at: existing.map(|e| e.created_at).unwrap_or_else(|| now.clone()),
            updated_at: now,
        };

        // File is the sole write target. A failure here is hard — there's
        // no DB safety net anymore, so silently swallowing would let the
        // UI think the save succeeded when it didn't. The read path still
        // falls back to the legacy DB row when the file is missing, so
        // projects that pre-date the portable snapshot keep working.
        file_storage::save(&project_path, &memory)?;
        Ok(memory.into())
    }
    .await;
    result.into_state()
}

// =============================================================================
// Delete project memory
// =============================================================================

#[tauri::command]
pub async fn delete_project_memory(
    lazy_state: tauri::State<'_, LazyAppState>,
    id: String,
) -> StateCommandResult<()> {
    let memory_repo = get_state_field!(&lazy_state, memory_repository);
    let project_repo = get_state_field!(&lazy_state, project_repository);
    let result: Result<(), VenoreError> = async {
        let memory_repo = memory_repo?;
        let project_repo = project_repo?;

        // Look up the row first so we can locate the owning project_path and
        // delete the portable file too. If the row is gone we still allow the
        // DB delete to surface a NotFound from the repository.
        if let Some(memory) = memory_repo.get_by_id(&id).await? {
            if let Ok(project_path) = resolve_project_path(&project_repo, &memory.project_id).await {
                if let Err(e) = file_storage::delete(&project_path) {
                    tracing::warn!(
                        project_id = %memory.project_id,
                        error = %e,
                        "Failed to delete .venore/project-memory.json (continuing with DB delete)"
                    );
                }
            }
        }

        memory_repo.delete(&id).await?;
        Ok(())
    }
    .await;
    result.into_state()
}

// =============================================================================
// Generate full memory (reads .context.md, LLM fills all fields as JSON)
// =============================================================================

#[tauri::command]
pub async fn generate_project_memory(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: GenerateMemoryRequest,
) -> StateCommandResult<GenerateMemoryResponse> {
    let services = crate::commands::llm::get_services(&lazy_state);
    let result: Result<GenerateMemoryResponse, VenoreError> = async {
        let (_, llm_gateway) = services?;

        let project_dir = std::path::Path::new(&request.project_path);

        // Gather project context from available files
        let mut context_parts: Vec<String> = Vec::new();

        // 1) .context.md (primary source)
        let context_md = std::fs::read_to_string(project_dir.join(".context.md")).unwrap_or_default();
        if !context_md.trim().is_empty() {
            let truncated = if context_md.len() > CONTEXT_MD_MAX_BYTES {
                format!("{}...\n(truncated)", truncate_utf8(&context_md, CONTEXT_MD_MAX_BYTES))
            } else {
                context_md
            };
            context_parts.push(format!("### .context.md\n{}", truncated));
        }

        // 2) Fallback files — read common project files for context
        let fallback_files = [
            "README.md", "readme.md", "README",
            "package.json", "Cargo.toml", "pyproject.toml", "go.mod",
            "pom.xml", "build.gradle", "composer.json", "Gemfile",
            ".editorconfig", "tsconfig.json", "vite.config.ts",
        ];
        for filename in fallback_files {
            if let Ok(content) = std::fs::read_to_string(project_dir.join(filename)) {
                if !content.trim().is_empty() {
                    // Cap each file at 2000 bytes
                    let truncated = if content.len() > FALLBACK_FILE_MAX_BYTES {
                        format!("{}...(truncated)", truncate_utf8(&content, FALLBACK_FILE_MAX_BYTES))
                    } else {
                        content
                    };
                    context_parts.push(format!("### {}\n{}", filename, truncated));
                }
            }
        }

        // 3) Directory listing (top-level only)
        if let Ok(entries) = std::fs::read_dir(project_dir) {
            let names: Vec<String> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .take(50)
                .collect();
            if !names.is_empty() {
                context_parts.push(format!("### Directory listing\n{}", names.join(", ")));
            }
        }

        // 4) Detected modules from wizard's index pipeline
        if !request.detected_modules.is_empty() {
            let module_list = request.detected_modules.join(", ");
            context_parts.push(format!(
                "### Detected modules ({} total)\n{}",
                request.detected_modules.len(),
                truncate_utf8(&module_list, 3000),
            ));
        }

        // 5) User-supplied hints from wizard Step 1 (treated as ground truth)
        let user_desc = request.user_description.as_deref().unwrap_or("").trim();
        let user_arch = request.user_architecture.as_deref().unwrap_or("").trim();
        let user_debt = request.user_tech_debt.as_deref().unwrap_or("").trim();
        let mut user_hints: Vec<String> = Vec::new();
        if !user_desc.is_empty() { user_hints.push(format!("- Description (user): {}", user_desc)); }
        if !user_arch.is_empty() { user_hints.push(format!("- Architecture (user): {}", user_arch)); }
        if !user_debt.is_empty() { user_hints.push(format!("- Tech debt (user): {}", user_debt)); }
        let has_user_hints = !user_hints.is_empty();
        if has_user_hints {
            context_parts.push(format!(
                "### User-supplied notes (treat as ground truth — preserve these meanings)\n{}",
                user_hints.join("\n"),
            ));
        }

        let has_context = !context_parts.is_empty();
        let all_context = context_parts.join("\n\n");

        // Parse the requested depth level (default to Normal).
        let depth_str = request.depth_level.as_deref().unwrap_or("normal").to_lowercase();
        let (depth_max_tokens, depth_directive): (u32, &str) = match depth_str.as_str() {
            "minimal" => (
                1500,
                "ANALYSIS DEPTH: MINIMAL.\n\
                 Quick scan only. Description is 1 sentence. Architecture states the framework \
                 and basic structure (1 sentence). Skip rationale, edge cases, gotchas, deep \
                 reasoning. projectSummary stays 10-15 lines max."
            ),
            "detailed" => (
                6000,
                "ANALYSIS DEPTH: DETAILED.\n\
                 Go beyond surface-level descriptions. Do NOT assume — every claim about \
                 architecture or tech debt must be grounded in concrete evidence from the files \
                 you've been shown (filenames, manifest entries, directory patterns). \
                 Identify multiple non-obvious gotchas. Mention specific files when relevant. \
                 projectSummary should be 40-60 lines and cover: stack with WHY, top-level modules \
                 with what each does, conventions, anti-patterns or coupling worth flagging."
            ),
            "expert" => (
                10000,
                "ANALYSIS DEPTH: EXPERT.\n\
                 Maximum scrutiny. For each architectural claim, cite the specific files or \
                 manifest entries that support it. Identify hidden coupling between modules, \
                 cross-cutting concerns, anti-patterns the team probably hasn't flagged, edge \
                 cases the README glosses over. Compare what the code does vs what the README \
                 claims. Flag inconsistencies. projectSummary 60+ lines, includes sections for: \
                 Stack & rationale, Top-level modules (cite a key file per module), \
                 Cross-cutting concerns, Risks & anti-patterns, Onboarding gotchas. Do NOT \
                 assume — if evidence is missing, say so explicitly."
            ),
            _ => (
                4000,
                "ANALYSIS DEPTH: NORMAL.\n\
                 Comprehensive analysis with rationale. Identify the framework, structural \
                 pattern, and WHY those choices fit. Note obvious tech debt. projectSummary \
                 20-40 lines covering purpose, stack, structure, conventions, gotchas."
            ),
        };

        // Refinement mode: the user has reviewed a previous draft and wants
        // specific changes. We send the draft + their feedback back to the
        // LLM with a directive to preserve what's correct and only adjust
        // what the user asked. Different prompt shape from the fresh-analysis
        // path because the model needs to RESPECT the prior structure
        // instead of doing another pass from scratch.
        let refinement = match (&request.user_feedback, &request.previous_draft) {
            (Some(fb), Some(draft)) if !fb.trim().is_empty() => Some((fb.clone(), draft)),
            _ => None,
        };

        let prompt = if let Some((feedback, draft)) = refinement {
            let prior_json = serde_json::to_string_pretty(draft).unwrap_or_else(|_| "{}".to_string());
            format!(
                "{}\n\n\
                 You previously produced the following project analysis (JSON below). The user has \
                 reviewed it and asked for specific changes. Refine the analysis: preserve fields \
                 that are correct, fix what the user flagged, and add what they said is missing. \
                 Do NOT rewrite from scratch — keep the parts they didn't complain about.\n\n\
                 USER FEEDBACK:\n{}\n\n\
                 PREVIOUS ANALYSIS:\n{}\n\n\
                 PROJECT EVIDENCE (for cross-reference — same files as the previous run):\n{}\n\n\
                 OUTPUT SHAPE — CRITICAL: return a JSON object with EXACTLY these fields and types:\n\
                 - description (string, plain text — NOT an object)\n\
                 - state (string, one of: planning, active, maintenance, legacy, archived)\n\
                 - goals (array of strings)\n\
                 - architecture (string, plain markdown text — NOT an object, NOT nested)\n\
                 - techDebt (string, plain text)\n\
                 - projectSummary (string, plain markdown text — NOT an object)\n\
                 Every text field is a single string. Do NOT split them into sub-objects, lists, \
                 or nested keys, even if the previous draft you're refining uses formatting inside \
                 the string. Return ONLY the JSON object, no markdown fences.",
                depth_directive, feedback.trim(), prior_json, all_context
            )
        } else if has_context {
            let user_clause = if has_user_hints {
                "\nIMPORTANT: The user has already supplied description/architecture/techDebt notes. \
                 If a user note is present and non-empty, use its meaning verbatim (you may polish wording \
                 but do not contradict). If a user field is empty, fill it from your analysis.\n"
            } else {
                ""
            };
            format!(
                "{}\n\n\
                 Analyze the following project and extract structured information.\n\
                 Return a JSON object with these fields:\n\
                 - \"description\": 1-2 sentence project description\n\
                 - \"state\": one of \"planning\", \"active\", \"maintenance\", \"legacy\", \"archived\"\n\
                 - \"goals\": array, subset of [\"onboarding\", \"understand\", \"document\", \"refactor\", \"audit\", \"maintain\"]\n\
                 - \"architecture\": describe the actual stack and structural pattern. Mention the framework(s), \
                   how the code is organized (monorepo / by feature / by layer / domain-driven / etc.), key folders, \
                   and WHY those choices fit the project.\n\
                 - \"techDebt\": note on known tech debt or pain points (empty string if none obvious)\n\
                 - \"projectSummary\": markdown summary covering purpose, stack, architectural pattern with \
                   rationale, top-level modules and what each does, notable conventions, and any gotchas a new dev should know. \
                   Length and depth follow the ANALYSIS DEPTH directive above.\n\
                 {}\n\
                 Return ONLY valid JSON, no markdown fences.\n\n---\n\n{}",
                depth_directive, user_clause, all_context
            )
        } else {
            let normalized = request.project_path.replace('\\', "/");
            let project_name = normalized.split('/').rfind(|s| !s.is_empty()).unwrap_or("Project");
            format!(
                "I have a project named \"{}\" but no files are available to analyze.\n\
                 Return a JSON object with these fields, using reasonable defaults:\n\
                 - \"description\": \"\" (empty string)\n\
                 - \"state\": \"active\"\n\
                 - \"goals\": [\"understand\", \"document\"]\n\
                 - \"architecture\": \"\" (empty string)\n\
                 - \"techDebt\": \"\" (empty string)\n\
                 - \"projectSummary\": \"\" (empty string)\n\n\
                 Return ONLY valid JSON, no markdown fences.",
                project_name
            )
        };

        let json_schema = JsonSchema {
            name: "generate_memory".into(),
            strict: true,
            schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "description": { "type": "string" },
                    "state": { "type": "string", "enum": ["planning", "active", "maintenance", "legacy", "archived"] },
                    "goals": {
                        "type": "array",
                        "items": { "type": "string", "enum": ["onboarding", "understand", "document", "refactor", "audit", "maintain"] }
                    },
                    "architecture": { "type": "string" },
                    "techDebt": { "type": "string" },
                    "projectSummary": { "type": "string" }
                },
                "required": ["description", "state", "goals", "architecture", "techDebt", "projectSummary"],
                "additionalProperties": false
            }),
        };

        let messages = vec![
            LlmMessage {
                role: MessageRole::User,
                content: prompt,
                tool_call_id: None,
                tool_calls: None,
                content_parts: None,
            },
        ];

        let options = GatewayOptions::for_task(LlmTask::Onboarding);
        let (_provider, model) = llm_gateway.resolve_model(&options).await;

        let llm_request = LlmRequest {
            model,
            messages,
            temperature: Some(0.2),
            max_tokens: Some(depth_max_tokens),
            tools: None,
            json_schema: Some(json_schema),
            timeout_secs: Some(120),
            web_search: false,
        };

        let response = llm_gateway.complete(llm_request, options).await?;

        // Strip markdown fences if present (```json ... ```)
        let raw = response.content.trim();
        let cleaned = if raw.starts_with("```") {
            let without_opening = raw
                .strip_prefix("```json").or_else(|| raw.strip_prefix("```"))
                .unwrap_or(raw)
                .trim_start();
            without_opening.strip_suffix("```").unwrap_or(without_opening).trim()
        } else {
            raw
        };

        tracing::debug!(raw_len = raw.len(), cleaned_preview = truncate_utf8(cleaned, 300), "generate_project_memory LLM response");

        // First pass: parse into a permissive Value so we can coerce fields
        // whose type the model got wrong (Gemini 2.5 occasionally returns
        // nested objects in fields the schema declared as `string` —
        // particularly when given a prior draft as context). Strict
        // deserialization onto GenerateMemoryResponse would reject those
        // and force the user to retry, so we normalize first and only error
        // out if the JSON itself is malformed.
        let value: serde_json::Value = serde_json::from_str(cleaned)
            .map_err(|e| {
                tracing::error!(error = %e, raw = truncate_utf8(raw, 500), "Failed to parse generate-memory JSON");
                VenoreError::LlmInvalidResponse(format!(
                    "Failed to parse generate-memory response: {}",
                    e,
                ))
            })?;

        let parsed = GenerateMemoryResponse {
            description: coerce_to_string(value.get("description")),
            state: coerce_to_state(value.get("state")),
            goals: coerce_to_string_array(value.get("goals")),
            architecture: coerce_to_string(value.get("architecture")),
            tech_debt: coerce_to_string(value.get("techDebt").or_else(|| value.get("tech_debt"))),
            project_summary: coerce_to_string(value.get("projectSummary").or_else(|| value.get("project_summary"))),
        };

        Ok(parsed)
    }
    .await;
    result.into_state()
}

/// Coerce a JSON value into a plain string field.
///
/// Used to normalize LLM output where the model returned the wrong shape
/// (e.g. a JSON object or array in a field the schema declared as `string`).
/// Strict serde rejection would force a retry; we instead degrade gracefully:
/// objects/arrays are re-serialized as pretty markdown-ish text the user can
/// still read and edit. Null and missing → "".
fn coerce_to_string(value: Option<&serde_json::Value>) -> String {
    match value {
        None | Some(serde_json::Value::Null) => String::new(),
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Bool(b)) => b.to_string(),
        Some(serde_json::Value::Number(n)) => n.to_string(),
        Some(serde_json::Value::Array(arr)) => {
            arr.iter()
                .map(|v| match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
        Some(serde_json::Value::Object(map)) => {
            // Render an object as "Key: value" lines so the user gets
            // something readable instead of raw JSON noise.
            let mut lines: Vec<String> = Vec::with_capacity(map.len());
            for (k, v) in map {
                let val_str = match v {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                        serde_json::to_string_pretty(v).unwrap_or_default()
                    }
                    other => other.to_string(),
                };
                lines.push(format!("- **{}**: {}", k, val_str));
            }
            lines.join("\n")
        }
    }
}

/// Coerce a value into the state enum, defaulting to "active" if unknown.
fn coerce_to_state(value: Option<&serde_json::Value>) -> String {
    const VALID: &[&str] = &["planning", "active", "maintenance", "legacy", "archived"];
    let s = match value {
        Some(serde_json::Value::String(s)) => s.to_lowercase(),
        _ => return "active".to_string(),
    };
    if VALID.contains(&s.as_str()) { s } else { "active".to_string() }
}

/// Coerce a value into a `Vec<String>` of goal names. Drops anything that
/// isn't a string.
fn coerce_to_string_array(value: Option<&serde_json::Value>) -> Vec<String> {
    match value {
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect(),
        _ => Vec::new(),
    }
}

// =============================================================================
// Regenerate memory summary (reads .context.md, calls LLM to summarize)
// =============================================================================

#[tauri::command]
pub async fn regenerate_memory_summary(
    lazy_state: tauri::State<'_, LazyAppState>,
    request: RegenerateSummaryRequest,
) -> StateCommandResult<String> {
    let services = crate::commands::llm::get_services(&lazy_state);
    let result: Result<String, VenoreError> = async {
        let (_, llm_gateway) = services?;

        // Read .context.md from project path
        let context_path = std::path::Path::new(&request.project_path).join(".context.md");
        let context_content = std::fs::read_to_string(&context_path)
            .map_err(|e| VenoreError::Io(format!(
                "Failed to read .context.md at {}: {}", context_path.display(), e
            )))?;

        // Truncate if too large (keep first ~8000 bytes)
        let content = if context_content.len() > REGENERATE_MAX_BYTES {
            format!("{}...\n(truncated)", truncate_utf8(&context_content, REGENERATE_MAX_BYTES))
        } else {
            context_content
        };

        let prompt = format!(
            "Summarize the following .context.md file into a compact 30-50 line overview. \
             Focus on: project purpose, key technologies, architecture decisions, \
             important patterns, and critical conventions. \
             Output ONLY the summary text in markdown, no preamble.\n\n---\n\n{}",
            content
        );

        let messages = vec![
            LlmMessage {
                role: MessageRole::User,
                content: prompt,
                tool_call_id: None,
                tool_calls: None,
                content_parts: None,
            },
        ];

        let options = GatewayOptions::for_task(LlmTask::Chat);
        let (_provider, model) = llm_gateway.resolve_model(&options).await;

        let llm_request = LlmRequest {
            model,
            messages,
            temperature: Some(0.2),
            max_tokens: Some(2000),
            tools: None,
            json_schema: None,
            timeout_secs: Some(60),
            web_search: false,
        };

        let response = llm_gateway.complete(llm_request, options).await?;

        Ok(response.content.trim().to_string())
    }
    .await;
    result.into_state()
}
