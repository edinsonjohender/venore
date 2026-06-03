//! Agent repository — SQLite CRUD for profiles and teams

use sqlx::sqlite::SqlitePool;
use sqlx::Row;

use crate::{MapDbErr, Result, VenoreError};
use crate::tools::names as N;
use super::models::{AgentProfile, AgentTeam, AgentRule, AgentStage, Severity, ToolCategory, ToolDefinition, ChatMode};
use super::pipeline::{PipelineRun, PipelineRunStatus, PipelineStep, PipelineStepStatus};
use super::snapshot::{CategorySnapshot, AuthorStats, CategoryAverage};

pub struct AgentRepository {
    pool: SqlitePool,
}

impl AgentRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Access the SQLite pool (for cross-file impl blocks in this crate).
    pub(crate) fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    // =========================================================================
    // Initialize
    // =========================================================================

    pub async fn initialize(&self) -> Result<()> {
        self.create_tables().await?;
        tracing::info!("Agent repository initialized");
        Ok(())
    }

    async fn create_tables(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS agent_profiles (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                stage TEXT NOT NULL,
                system_prompt TEXT NOT NULL DEFAULT '',
                provider TEXT NOT NULL DEFAULT 'anthropic',
                model TEXT NOT NULL DEFAULT 'claude-sonnet-4-5',
                temperature REAL NOT NULL DEFAULT 0.3,
                is_template INTEGER NOT NULL DEFAULT 0,
                is_enabled INTEGER NOT NULL DEFAULT 1,
                rules_json TEXT NOT NULL DEFAULT '[]',
                criteria_json TEXT NOT NULL DEFAULT '[]',
                tools_json TEXT NOT NULL DEFAULT '[]',
                max_tokens_per_run INTEGER NOT NULL DEFAULT 30000,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )"
        )
        .execute(&self.pool)
        .await
        .db_err("Failed to create agent_profiles")?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS agent_teams (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                profile_ids TEXT NOT NULL DEFAULT '[]',
                is_template INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )"
        )
        .execute(&self.pool)
        .await
        .db_err("Failed to create agent_teams")?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS agent_rules (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                scope TEXT NOT NULL DEFAULT '[]',
                severity TEXT NOT NULL DEFAULT 'warning',
                is_active INTEGER NOT NULL DEFAULT 1,
                is_template INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )"
        )
        .execute(&self.pool)
        .await
        .db_err("Failed to create agent_rules")?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS pipeline_runs (
                id TEXT PRIMARY KEY,
                team_id TEXT NOT NULL,
                team_name TEXT NOT NULL,
                task_type TEXT NOT NULL DEFAULT 'pr-analysis',
                title TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'running',
                pr_number INTEGER,
                project_path TEXT NOT NULL,
                started_at TEXT NOT NULL,
                finished_at TEXT,
                duration_ms INTEGER NOT NULL DEFAULT 0,
                total_tokens INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            )"
        )
        .execute(&self.pool)
        .await
        .db_err("Failed to create pipeline_runs")?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS pipeline_steps (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL REFERENCES pipeline_runs(id),
                profile_id TEXT NOT NULL,
                profile_name TEXT NOT NULL,
                stage TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                input_context TEXT NOT NULL DEFAULT '',
                output TEXT NOT NULL DEFAULT '',
                provider TEXT NOT NULL DEFAULT '',
                model TEXT NOT NULL DEFAULT '',
                prompt_tokens INTEGER NOT NULL DEFAULT 0,
                completion_tokens INTEGER NOT NULL DEFAULT 0,
                total_tokens INTEGER NOT NULL DEFAULT 0,
                duration_ms INTEGER NOT NULL DEFAULT 0,
                error TEXT,
                step_order INTEGER NOT NULL DEFAULT 0,
                started_at TEXT NOT NULL,
                finished_at TEXT
            )"
        )
        .execute(&self.pool)
        .await
        .db_err("Failed to create pipeline_steps")?;

        // Migrate: add PR metadata columns to pipeline_runs
        for col in [
            "pr_author TEXT",
            "pr_author_avatar TEXT",
            "pr_additions INTEGER",
            "pr_deletions INTEGER",
            "pr_changed_files INTEGER",
            "depth_level TEXT",
        ] {
            let _ = sqlx::query(&format!("ALTER TABLE pipeline_runs ADD COLUMN {col}"))
                .execute(&self.pool)
                .await;
        }

        // Migrate: add tools_json to agent_profiles
        let _ = sqlx::query("ALTER TABLE agent_profiles ADD COLUMN tools_json TEXT NOT NULL DEFAULT '[]'")
            .execute(&self.pool).await;

        // Snapshot tables for historical tracking
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS pr_category_snapshots (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL REFERENCES pipeline_runs(id),
                project_path TEXT NOT NULL,
                author_login TEXT NOT NULL,
                category_name TEXT NOT NULL,
                score INTEGER NOT NULL,
                status TEXT NOT NULL,
                findings_count INTEGER NOT NULL DEFAULT 0,
                overall_score INTEGER NOT NULL,
                created_at TEXT NOT NULL
            )"
        )
        .execute(&self.pool)
        .await
        .db_err("Failed to create pr_category_snapshots")?;

        let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_snapshots_project ON pr_category_snapshots(project_path)")
            .execute(&self.pool).await;
        let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_snapshots_author ON pr_category_snapshots(author_login, project_path)")
            .execute(&self.pool).await;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS pr_authors (
                login TEXT NOT NULL,
                project_path TEXT NOT NULL,
                avatar_url TEXT NOT NULL DEFAULT '',
                total_runs INTEGER NOT NULL DEFAULT 0,
                avg_overall_score REAL NOT NULL DEFAULT 0.0,
                last_overall_score INTEGER NOT NULL DEFAULT 0,
                last_run_at TEXT NOT NULL,
                PRIMARY KEY (login, project_path)
            )"
        )
        .execute(&self.pool)
        .await
        .db_err("Failed to create pr_authors")?;

        // Tool categories
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS tool_categories (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                icon TEXT NOT NULL DEFAULT '',
                color TEXT NOT NULL DEFAULT '#6b7280',
                display_order INTEGER NOT NULL DEFAULT 0,
                is_template INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )"
        )
        .execute(&self.pool)
        .await
        .db_err("Failed to create tool_categories")?;

        // Tool definitions
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS tool_definitions (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                description TEXT NOT NULL,
                category_id TEXT NOT NULL,
                parameters_json TEXT NOT NULL DEFAULT '{}',
                is_read_only INTEGER NOT NULL DEFAULT 0,
                is_enabled INTEGER NOT NULL DEFAULT 1,
                is_template INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )"
        )
        .execute(&self.pool)
        .await
        .db_err("Failed to create tool_definitions")?;

        // Chat modes — named bundles of (categories, tools, sub-agents, rules,
        // prompt) that decide what the chat sees per project kind.
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS chat_modes (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                category_ids_json TEXT NOT NULL DEFAULT '[]',
                tool_ids_json TEXT NOT NULL DEFAULT '[]',
                sub_agent_ids_json TEXT NOT NULL DEFAULT '[]',
                rule_ids_json TEXT NOT NULL DEFAULT '[]',
                prompt_id TEXT,
                is_template INTEGER NOT NULL DEFAULT 0,
                is_default_for_kind TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )"
        )
        .execute(&self.pool)
        .await
        .db_err("Failed to create chat_modes")?;

        Ok(())
    }

    // =========================================================================
    // Profile CRUD
    // =========================================================================

    pub async fn list_profiles(&self) -> Result<Vec<AgentProfile>> {
        let rows = sqlx::query(
            "SELECT id, name, description, stage, system_prompt,
                    provider, model, temperature, is_template, is_enabled,
                    rules_json, criteria_json, tools_json, max_tokens_per_run,
                    created_at, updated_at
             FROM agent_profiles ORDER BY created_at ASC"
        )
        .fetch_all(&self.pool)
        .await
        .db_err("Failed to list profiles")?;

        Ok(rows.iter().map(row_to_profile).collect())
    }

    pub async fn get_profile(&self, id: &str) -> Result<AgentProfile> {
        let row = sqlx::query(
            "SELECT id, name, description, stage, system_prompt,
                    provider, model, temperature, is_template, is_enabled,
                    rules_json, criteria_json, tools_json, max_tokens_per_run,
                    created_at, updated_at
             FROM agent_profiles WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .db_err("Failed to get profile")?;

        match row {
            Some(r) => Ok(row_to_profile(&r)),
            None => Err(VenoreError::NotFound(format!("Agent profile '{}'", id))),
        }
    }

    pub async fn create_profile(&self, profile: &AgentProfile) -> Result<()> {
        sqlx::query(
            "INSERT INTO agent_profiles
                (id, name, description, stage, system_prompt,
                 provider, model, temperature, is_template, is_enabled,
                 rules_json, criteria_json, tools_json, max_tokens_per_run,
                 created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&profile.id)
        .bind(&profile.name)
        .bind(&profile.description)
        .bind(profile.stage.as_str())
        .bind(&profile.system_prompt)
        .bind(&profile.provider)
        .bind(&profile.model)
        .bind(profile.temperature)
        .bind(profile.is_template)
        .bind(profile.is_enabled)
        .bind(&profile.rules_json)
        .bind(&profile.criteria_json)
        .bind(&profile.tools_json)
        .bind(profile.max_tokens_per_run)
        .bind(&profile.created_at)
        .bind(&profile.updated_at)
        .execute(&self.pool)
        .await
        .db_err("Failed to create profile")?;

        tracing::info!(id = %profile.id, name = %profile.name, "Agent profile created");
        Ok(())
    }

    pub async fn update_profile(&self, profile: &AgentProfile) -> Result<()> {
        let result = sqlx::query(
            "UPDATE agent_profiles SET
                name = ?, description = ?, stage = ?,
                system_prompt = ?, provider = ?, model = ?, temperature = ?,
                is_template = ?, is_enabled = ?, rules_json = ?, criteria_json = ?,
                tools_json = ?, max_tokens_per_run = ?,
                updated_at = ?
             WHERE id = ?"
        )
        .bind(&profile.name)
        .bind(&profile.description)
        .bind(profile.stage.as_str())
        .bind(&profile.system_prompt)
        .bind(&profile.provider)
        .bind(&profile.model)
        .bind(profile.temperature)
        .bind(profile.is_template)
        .bind(profile.is_enabled)
        .bind(&profile.rules_json)
        .bind(&profile.criteria_json)
        .bind(&profile.tools_json)
        .bind(profile.max_tokens_per_run)
        .bind(&profile.updated_at)
        .bind(&profile.id)
        .execute(&self.pool)
        .await
        .db_err("Failed to update profile")?;

        if result.rows_affected() == 0 {
            return Err(VenoreError::NotFound(format!("Agent profile '{}'", profile.id)));
        }

        tracing::info!(id = %profile.id, "Agent profile updated");
        Ok(())
    }

    pub async fn delete_profile(&self, id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM agent_profiles WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .db_err("Failed to delete profile")?;

        if result.rows_affected() == 0 {
            return Err(VenoreError::NotFound(format!("Agent profile '{}'", id)));
        }

        tracing::info!(id = %id, "Agent profile deleted");
        Ok(())
    }

    // =========================================================================
    // Team CRUD
    // =========================================================================

    pub async fn list_teams(&self) -> Result<Vec<AgentTeam>> {
        let rows = sqlx::query(
            "SELECT id, name, description, profile_ids, is_template, created_at, updated_at
             FROM agent_teams ORDER BY created_at ASC"
        )
        .fetch_all(&self.pool)
        .await
        .db_err("Failed to list teams")?;

        Ok(rows.iter().map(row_to_team).collect())
    }

    pub async fn get_team(&self, id: &str) -> Result<AgentTeam> {
        let row = sqlx::query(
            "SELECT id, name, description, profile_ids, is_template, created_at, updated_at
             FROM agent_teams WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .db_err("Failed to get team")?;

        match row {
            Some(r) => Ok(row_to_team(&r)),
            None => Err(VenoreError::NotFound(format!("Agent team '{}'", id))),
        }
    }

    pub async fn create_team(&self, team: &AgentTeam) -> Result<()> {
        let profile_ids_json = serde_json::to_string(&team.profile_ids)
            .db_err("Failed to serialize profile_ids")?;

        sqlx::query(
            "INSERT INTO agent_teams (id, name, description, profile_ids, is_template, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&team.id)
        .bind(&team.name)
        .bind(&team.description)
        .bind(&profile_ids_json)
        .bind(team.is_template)
        .bind(&team.created_at)
        .bind(&team.updated_at)
        .execute(&self.pool)
        .await
        .db_err("Failed to create team")?;

        tracing::info!(id = %team.id, name = %team.name, "Agent team created");
        Ok(())
    }

    pub async fn update_team(&self, team: &AgentTeam) -> Result<()> {
        let profile_ids_json = serde_json::to_string(&team.profile_ids)
            .db_err("Failed to serialize profile_ids")?;

        let result = sqlx::query(
            "UPDATE agent_teams SET
                name = ?, description = ?, profile_ids = ?, is_template = ?,
                updated_at = ?
             WHERE id = ?"
        )
        .bind(&team.name)
        .bind(&team.description)
        .bind(&profile_ids_json)
        .bind(team.is_template)
        .bind(&team.updated_at)
        .bind(&team.id)
        .execute(&self.pool)
        .await
        .db_err("Failed to update team")?;

        if result.rows_affected() == 0 {
            return Err(VenoreError::NotFound(format!("Agent team '{}'", team.id)));
        }

        tracing::info!(id = %team.id, "Agent team updated");
        Ok(())
    }

    pub async fn delete_team(&self, id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM agent_teams WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .db_err("Failed to delete team")?;

        if result.rows_affected() == 0 {
            return Err(VenoreError::NotFound(format!("Agent team '{}'", id)));
        }

        tracing::info!(id = %id, "Agent team deleted");
        Ok(())
    }

    // =========================================================================
    // Seed check
    // =========================================================================

    pub async fn count_template_profiles(&self) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) as cnt FROM agent_profiles WHERE is_template = 1")
            .fetch_one(&self.pool)
            .await
            .db_err("Failed to count templates")?;

        Ok(row.get::<i64, _>("cnt"))
    }

    // =========================================================================
    // Rule CRUD
    // =========================================================================

    pub async fn list_rules(&self) -> Result<Vec<AgentRule>> {
        let rows = sqlx::query(
            "SELECT id, name, description, scope, severity, is_active, is_template,
                    created_at, updated_at
             FROM agent_rules ORDER BY created_at ASC"
        )
        .fetch_all(&self.pool)
        .await
        .db_err("Failed to list rules")?;

        Ok(rows.iter().map(row_to_rule).collect())
    }

    pub async fn get_rule(&self, id: &str) -> Result<AgentRule> {
        let row = sqlx::query(
            "SELECT id, name, description, scope, severity, is_active, is_template,
                    created_at, updated_at
             FROM agent_rules WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .db_err("Failed to get rule")?;

        match row {
            Some(r) => Ok(row_to_rule(&r)),
            None => Err(VenoreError::NotFound(format!("Agent rule '{}'", id))),
        }
    }

    pub async fn create_rule(&self, rule: &AgentRule) -> Result<()> {
        let scope_json = serde_json::to_string(&rule.scope)
            .db_err("Failed to serialize scope")?;

        sqlx::query(
            "INSERT INTO agent_rules
                (id, name, description, scope, severity, is_active, is_template,
                 created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&rule.id)
        .bind(&rule.name)
        .bind(&rule.description)
        .bind(&scope_json)
        .bind(rule.severity.as_str())
        .bind(rule.is_active)
        .bind(rule.is_template)
        .bind(&rule.created_at)
        .bind(&rule.updated_at)
        .execute(&self.pool)
        .await
        .db_err("Failed to create rule")?;

        tracing::info!(id = %rule.id, name = %rule.name, "Agent rule created");
        Ok(())
    }

    pub async fn update_rule(&self, rule: &AgentRule) -> Result<()> {
        let scope_json = serde_json::to_string(&rule.scope)
            .db_err("Failed to serialize scope")?;

        let result = sqlx::query(
            "UPDATE agent_rules SET
                name = ?, description = ?, scope = ?, severity = ?,
                is_active = ?, is_template = ?, updated_at = ?
             WHERE id = ?"
        )
        .bind(&rule.name)
        .bind(&rule.description)
        .bind(&scope_json)
        .bind(rule.severity.as_str())
        .bind(rule.is_active)
        .bind(rule.is_template)
        .bind(&rule.updated_at)
        .bind(&rule.id)
        .execute(&self.pool)
        .await
        .db_err("Failed to update rule")?;

        if result.rows_affected() == 0 {
            return Err(VenoreError::NotFound(format!("Agent rule '{}'", rule.id)));
        }

        tracing::info!(id = %rule.id, "Agent rule updated");
        Ok(())
    }

    pub async fn delete_rule(&self, id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM agent_rules WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .db_err("Failed to delete rule")?;

        if result.rows_affected() == 0 {
            return Err(VenoreError::NotFound(format!("Agent rule '{}'", id)));
        }

        tracing::info!(id = %id, "Agent rule deleted");
        Ok(())
    }

    pub async fn count_template_rules(&self) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) as cnt FROM agent_rules WHERE is_template = 1")
            .fetch_one(&self.pool)
            .await
            .db_err("Failed to count template rules")?;

        Ok(row.get::<i64, _>("cnt"))
    }

    // =========================================================================
    // Pipeline Run CRUD
    // =========================================================================

    pub async fn create_pipeline_run(&self, run: &PipelineRun) -> Result<()> {
        sqlx::query(
            "INSERT INTO pipeline_runs
                (id, team_id, team_name, task_type, title, status,
                 pr_number, project_path, started_at, finished_at,
                 duration_ms, total_tokens, created_at,
                 pr_author, pr_author_avatar, pr_additions, pr_deletions, pr_changed_files,
                 depth_level)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&run.id)
        .bind(&run.team_id)
        .bind(&run.team_name)
        .bind(&run.task_type)
        .bind(&run.title)
        .bind(run.status.as_str())
        .bind(run.pr_number.map(|n| n as i64))
        .bind(&run.project_path)
        .bind(&run.started_at)
        .bind(&run.finished_at)
        .bind(run.duration_ms as i64)
        .bind(run.total_tokens as i32)
        .bind(&run.created_at)
        .bind(&run.pr_author)
        .bind(&run.pr_author_avatar)
        .bind(run.pr_additions.map(|n| n as i64))
        .bind(run.pr_deletions.map(|n| n as i64))
        .bind(run.pr_changed_files.map(|n| n as i64))
        .bind(&run.depth_level)
        .execute(&self.pool)
        .await
        .db_err("Failed to create pipeline run")?;

        tracing::info!(id = %run.id, title = %run.title, "Pipeline run created");
        Ok(())
    }

    pub async fn update_pipeline_run(&self, run: &PipelineRun) -> Result<()> {
        let result = sqlx::query(
            "UPDATE pipeline_runs SET
                status = ?, finished_at = ?, duration_ms = ?, total_tokens = ?
             WHERE id = ?"
        )
        .bind(run.status.as_str())
        .bind(&run.finished_at)
        .bind(run.duration_ms as i64)
        .bind(run.total_tokens as i32)
        .bind(&run.id)
        .execute(&self.pool)
        .await
        .db_err("Failed to update pipeline run")?;

        if result.rows_affected() == 0 {
            return Err(VenoreError::NotFound(format!("Pipeline run '{}'", run.id)));
        }

        tracing::info!(id = %run.id, status = run.status.as_str(), "Pipeline run updated");
        Ok(())
    }

    pub async fn get_pipeline_run(&self, id: &str) -> Result<PipelineRun> {
        let row = sqlx::query(
            "SELECT id, team_id, team_name, task_type, title, status,
                    pr_number, project_path, started_at, finished_at,
                    duration_ms, total_tokens, created_at,
                    pr_author, pr_author_avatar, pr_additions, pr_deletions, pr_changed_files,
                    depth_level
             FROM pipeline_runs WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .db_err("Failed to get pipeline run")?;

        match row {
            Some(r) => Ok(row_to_pipeline_run(&r)),
            None => Err(VenoreError::NotFound(format!("Pipeline run '{}'", id))),
        }
    }

    pub async fn list_pipeline_runs(&self) -> Result<Vec<PipelineRun>> {
        let rows = sqlx::query(
            "SELECT id, team_id, team_name, task_type, title, status,
                    pr_number, project_path, started_at, finished_at,
                    duration_ms, total_tokens, created_at,
                    pr_author, pr_author_avatar, pr_additions, pr_deletions, pr_changed_files,
                    depth_level
             FROM pipeline_runs ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await
        .db_err("Failed to list pipeline runs")?;

        Ok(rows.iter().map(row_to_pipeline_run).collect())
    }

    // =========================================================================
    // Pipeline Step CRUD
    // =========================================================================

    pub async fn create_pipeline_step(&self, step: &PipelineStep) -> Result<()> {
        sqlx::query(
            "INSERT INTO pipeline_steps
                (id, run_id, profile_id, profile_name, stage, status,
                 input_context, output, provider, model,
                 prompt_tokens, completion_tokens, total_tokens,
                 duration_ms, error, step_order, started_at, finished_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&step.id)
        .bind(&step.run_id)
        .bind(&step.profile_id)
        .bind(&step.profile_name)
        .bind(&step.stage)
        .bind(step.status.as_str())
        .bind(&step.input_context)
        .bind(&step.output)
        .bind(&step.provider)
        .bind(&step.model)
        .bind(step.prompt_tokens as i32)
        .bind(step.completion_tokens as i32)
        .bind(step.total_tokens as i32)
        .bind(step.duration_ms as i64)
        .bind(&step.error)
        .bind(step.step_order as i32)
        .bind(&step.started_at)
        .bind(&step.finished_at)
        .execute(&self.pool)
        .await
        .db_err("Failed to create pipeline step")?;

        tracing::debug!(id = %step.id, profile = %step.profile_name, "Pipeline step created");
        Ok(())
    }

    pub async fn update_pipeline_step(&self, step: &PipelineStep) -> Result<()> {
        let result = sqlx::query(
            "UPDATE pipeline_steps SET
                status = ?, output = ?, provider = ?, model = ?,
                prompt_tokens = ?, completion_tokens = ?, total_tokens = ?,
                duration_ms = ?, error = ?, finished_at = ?
             WHERE id = ?"
        )
        .bind(step.status.as_str())
        .bind(&step.output)
        .bind(&step.provider)
        .bind(&step.model)
        .bind(step.prompt_tokens as i32)
        .bind(step.completion_tokens as i32)
        .bind(step.total_tokens as i32)
        .bind(step.duration_ms as i64)
        .bind(&step.error)
        .bind(&step.finished_at)
        .bind(&step.id)
        .execute(&self.pool)
        .await
        .db_err("Failed to update pipeline step")?;

        if result.rows_affected() == 0 {
            return Err(VenoreError::NotFound(format!("Pipeline step '{}'", step.id)));
        }

        tracing::debug!(id = %step.id, status = step.status.as_str(), "Pipeline step updated");
        Ok(())
    }

    pub async fn list_pipeline_steps(&self, run_id: &str) -> Result<Vec<PipelineStep>> {
        let rows = sqlx::query(
            "SELECT id, run_id, profile_id, profile_name, stage, status,
                    input_context, output, provider, model,
                    prompt_tokens, completion_tokens, total_tokens,
                    duration_ms, error, step_order, started_at, finished_at
             FROM pipeline_steps WHERE run_id = ? ORDER BY step_order ASC"
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await
        .db_err("Failed to list pipeline steps")?;

        Ok(rows.iter().map(row_to_pipeline_step).collect())
    }

    // =========================================================================
    // Snapshot & Author Stats
    // =========================================================================

    pub async fn save_category_snapshots(&self, snapshots: &[CategorySnapshot]) -> Result<()> {
        for snap in snapshots {
            sqlx::query(
                "INSERT INTO pr_category_snapshots
                    (id, run_id, project_path, author_login, category_name,
                     score, status, findings_count, overall_score, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(&snap.id)
            .bind(&snap.run_id)
            .bind(&snap.project_path)
            .bind(&snap.author_login)
            .bind(&snap.category_name)
            .bind(snap.score as i32)
            .bind(&snap.status)
            .bind(snap.findings_count as i32)
            .bind(snap.overall_score as i32)
            .bind(&snap.created_at)
            .execute(&self.pool)
            .await
            .db_err("Failed to save snapshot")?;
        }

        tracing::info!(count = snapshots.len(), "Saved analysis snapshots");
        Ok(())
    }

    pub async fn upsert_author_stats(&self, stats: &AuthorStats) -> Result<()> {
        sqlx::query(
            "INSERT INTO pr_authors (login, project_path, avatar_url, total_runs, avg_overall_score, last_overall_score, last_run_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (login, project_path) DO UPDATE SET
                avatar_url = excluded.avatar_url,
                total_runs = excluded.total_runs,
                avg_overall_score = excluded.avg_overall_score,
                last_overall_score = excluded.last_overall_score,
                last_run_at = excluded.last_run_at"
        )
        .bind(&stats.login)
        .bind(&stats.project_path)
        .bind(&stats.avatar_url)
        .bind(stats.total_runs as i32)
        .bind(stats.avg_overall_score)
        .bind(stats.last_overall_score as i32)
        .bind(&stats.last_run_at)
        .execute(&self.pool)
        .await
        .db_err("Failed to upsert author stats")?;

        tracing::debug!(login = %stats.login, "Author stats upserted");
        Ok(())
    }

    pub async fn get_author_stats(&self, login: &str, project_path: &str) -> Result<Option<AuthorStats>> {
        let row = sqlx::query(
            "SELECT login, project_path, avatar_url, total_runs, avg_overall_score, last_overall_score, last_run_at
             FROM pr_authors WHERE login = ? AND project_path = ?"
        )
        .bind(login)
        .bind(project_path)
        .fetch_optional(&self.pool)
        .await
        .db_err("Failed to get author stats")?;

        Ok(row.map(|r| AuthorStats {
            login: r.get("login"),
            project_path: r.get("project_path"),
            avatar_url: r.get("avatar_url"),
            total_runs: r.get::<i32, _>("total_runs") as u32,
            avg_overall_score: r.get("avg_overall_score"),
            last_overall_score: r.get::<i32, _>("last_overall_score") as u32,
            last_run_at: r.get("last_run_at"),
        }))
    }

    pub async fn get_project_category_averages(&self, project_path: &str) -> Result<Vec<CategoryAverage>> {
        let rows = sqlx::query(
            "SELECT category_name, AVG(score) as avg_score, COUNT(*) as run_count
             FROM pr_category_snapshots WHERE project_path = ?
             GROUP BY category_name"
        )
        .bind(project_path)
        .fetch_all(&self.pool)
        .await
        .db_err("Failed to get project averages")?;

        Ok(rows.iter().map(|r| CategoryAverage {
            category_name: r.get("category_name"),
            avg_score: r.get("avg_score"),
            run_count: r.get::<i32, _>("run_count") as u32,
        }).collect())
    }

    pub async fn get_author_category_averages(&self, login: &str, project_path: &str) -> Result<Vec<CategoryAverage>> {
        let rows = sqlx::query(
            "SELECT category_name, AVG(score) as avg_score, COUNT(*) as run_count
             FROM pr_category_snapshots WHERE author_login = ? AND project_path = ?
             GROUP BY category_name"
        )
        .bind(login)
        .bind(project_path)
        .fetch_all(&self.pool)
        .await
        .db_err("Failed to get author averages")?;

        Ok(rows.iter().map(|r| CategoryAverage {
            category_name: r.get("category_name"),
            avg_score: r.get("avg_score"),
            run_count: r.get::<i32, _>("run_count") as u32,
        }).collect())
    }

    // =========================================================================
    // Tool Category CRUD
    // =========================================================================

    pub async fn list_tool_categories(&self) -> Result<Vec<ToolCategory>> {
        let rows = sqlx::query(
            "SELECT id, name, description, icon, color, display_order, is_template,
                    created_at, updated_at
             FROM tool_categories ORDER BY display_order ASC"
        )
        .fetch_all(&self.pool)
        .await
        .db_err("Failed to list tool categories")?;

        Ok(rows.iter().map(row_to_tool_category).collect())
    }

    pub async fn get_tool_category(&self, id: &str) -> Result<ToolCategory> {
        let row = sqlx::query(
            "SELECT id, name, description, icon, color, display_order, is_template,
                    created_at, updated_at
             FROM tool_categories WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .db_err("Failed to get tool category")?;

        match row {
            Some(r) => Ok(row_to_tool_category(&r)),
            None => Err(VenoreError::NotFound(format!("Tool category '{}'", id))),
        }
    }

    pub async fn create_tool_category(&self, category: &ToolCategory) -> Result<()> {
        sqlx::query(
            "INSERT INTO tool_categories
                (id, name, description, icon, color, display_order, is_template,
                 created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&category.id)
        .bind(&category.name)
        .bind(&category.description)
        .bind(&category.icon)
        .bind(&category.color)
        .bind(category.display_order)
        .bind(category.is_template)
        .bind(&category.created_at)
        .bind(&category.updated_at)
        .execute(&self.pool)
        .await
        .db_err("Failed to create tool category")?;

        tracing::info!(id = %category.id, name = %category.name, "Tool category created");
        Ok(())
    }

    pub async fn update_tool_category(&self, category: &ToolCategory) -> Result<()> {
        let result = sqlx::query(
            "UPDATE tool_categories SET
                name = ?, description = ?, icon = ?, color = ?,
                display_order = ?, is_template = ?, updated_at = ?
             WHERE id = ?"
        )
        .bind(&category.name)
        .bind(&category.description)
        .bind(&category.icon)
        .bind(&category.color)
        .bind(category.display_order)
        .bind(category.is_template)
        .bind(&category.updated_at)
        .bind(&category.id)
        .execute(&self.pool)
        .await
        .db_err("Failed to update tool category")?;

        if result.rows_affected() == 0 {
            return Err(VenoreError::NotFound(format!("Tool category '{}'", category.id)));
        }

        tracing::info!(id = %category.id, "Tool category updated");
        Ok(())
    }

    pub async fn delete_tool_category(&self, id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM tool_categories WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .db_err("Failed to delete tool category")?;

        if result.rows_affected() == 0 {
            return Err(VenoreError::NotFound(format!("Tool category '{}'", id)));
        }

        tracing::info!(id = %id, "Tool category deleted");
        Ok(())
    }

    pub async fn count_template_tool_categories(&self) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) as cnt FROM tool_categories WHERE is_template = 1")
            .fetch_one(&self.pool)
            .await
            .db_err("Failed to count template tool categories")?;

        Ok(row.get::<i64, _>("cnt"))
    }

    // =========================================================================
    // Tool Definition CRUD
    // =========================================================================

    pub async fn list_tool_definitions(&self) -> Result<Vec<ToolDefinition>> {
        let rows = sqlx::query(
            "SELECT id, name, description, category_id, parameters_json,
                    is_read_only, is_enabled, is_template, created_at, updated_at
             FROM tool_definitions ORDER BY name ASC"
        )
        .fetch_all(&self.pool)
        .await
        .db_err("Failed to list tool definitions")?;

        Ok(rows.iter().map(row_to_tool_definition).collect())
    }

    pub async fn get_tool_definition(&self, id: &str) -> Result<ToolDefinition> {
        let row = sqlx::query(
            "SELECT id, name, description, category_id, parameters_json,
                    is_read_only, is_enabled, is_template, created_at, updated_at
             FROM tool_definitions WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .db_err("Failed to get tool definition")?;

        match row {
            Some(r) => Ok(row_to_tool_definition(&r)),
            None => Err(VenoreError::NotFound(format!("Tool definition '{}'", id))),
        }
    }

    pub async fn create_tool_definition(&self, tool: &ToolDefinition) -> Result<()> {
        sqlx::query(
            "INSERT INTO tool_definitions
                (id, name, description, category_id, parameters_json,
                 is_read_only, is_enabled, is_template, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&tool.id)
        .bind(&tool.name)
        .bind(&tool.description)
        .bind(&tool.category_id)
        .bind(&tool.parameters_json)
        .bind(tool.is_read_only)
        .bind(tool.is_enabled)
        .bind(tool.is_template)
        .bind(&tool.created_at)
        .bind(&tool.updated_at)
        .execute(&self.pool)
        .await
        .db_err("Failed to create tool definition")?;

        tracing::info!(id = %tool.id, name = %tool.name, "Tool definition created");
        Ok(())
    }

    pub async fn update_tool_definition(&self, tool: &ToolDefinition) -> Result<()> {
        let result = sqlx::query(
            "UPDATE tool_definitions SET
                name = ?, description = ?, category_id = ?, parameters_json = ?,
                is_read_only = ?, is_enabled = ?, is_template = ?, updated_at = ?
             WHERE id = ?"
        )
        .bind(&tool.name)
        .bind(&tool.description)
        .bind(&tool.category_id)
        .bind(&tool.parameters_json)
        .bind(tool.is_read_only)
        .bind(tool.is_enabled)
        .bind(tool.is_template)
        .bind(&tool.updated_at)
        .bind(&tool.id)
        .execute(&self.pool)
        .await
        .db_err("Failed to update tool definition")?;

        if result.rows_affected() == 0 {
            return Err(VenoreError::NotFound(format!("Tool definition '{}'", tool.id)));
        }

        tracing::info!(id = %tool.id, "Tool definition updated");
        Ok(())
    }

    pub async fn delete_tool_definition(&self, id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM tool_definitions WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .db_err("Failed to delete tool definition")?;

        if result.rows_affected() == 0 {
            return Err(VenoreError::NotFound(format!("Tool definition '{}'", id)));
        }

        tracing::info!(id = %id, "Tool definition deleted");
        Ok(())
    }

    pub async fn count_template_tool_definitions(&self) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) as cnt FROM tool_definitions WHERE is_template = 1")
            .fetch_one(&self.pool)
            .await
            .db_err("Failed to count template tool definitions")?;

        Ok(row.get::<i64, _>("cnt"))
    }

    // =========================================================================
    // DB → LlmTool conversion
    // =========================================================================

    /// Load enabled tools by IDs → Vec<LlmTool>. Empty `tool_ids` = all enabled tools.
    pub async fn load_llm_tools(&self, tool_ids: &[String]) -> Result<Vec<crate::llm::types::LlmTool>> {
        let all_defs = self.list_tool_definitions().await?;
        let tools = all_defs
            .into_iter()
            .filter(|d| d.is_enabled)
            .filter(|d| tool_ids.is_empty() || tool_ids.contains(&d.id))
            .filter_map(def_to_llm_tool)
            .collect();
        Ok(tools)
    }

    /// Load read-only tools (for plan mode) → Vec<LlmTool>. Empty `tool_ids` = all enabled read-only.
    /// Always includes `submit_plan` (from hardcoded fallback if not in DB).
    pub async fn load_read_only_llm_tools(&self, tool_ids: &[String]) -> Result<Vec<crate::llm::types::LlmTool>> {
        let all_defs = self.list_tool_definitions().await?;
        let mut tools: Vec<crate::llm::types::LlmTool> = all_defs
            .into_iter()
            .filter(|d| d.is_enabled && d.is_read_only)
            .filter(|d| tool_ids.is_empty() || tool_ids.contains(&d.id))
            .filter_map(def_to_llm_tool)
            .collect();

        // Ensure submit_plan is always present
        if !tools.iter().any(|t| t.name == N::SUBMIT_PLAN) {
            let plan_tools = crate::tools::plan_tools();
            if let Some(submit) = plan_tools.into_iter().find(|t| t.name == N::SUBMIT_PLAN) {
                tools.push(submit);
            }
        }

        Ok(tools)
    }

    /// Load enabled tools whose category matches one of the given ids.
    /// Empty `category_ids` = all categories (equivalent to `load_llm_tools`).
    pub async fn load_llm_tools_by_categories(
        &self,
        category_ids: &[String],
    ) -> Result<Vec<crate::llm::types::LlmTool>> {
        let all_defs = self.list_tool_definitions().await?;
        let tools = all_defs
            .into_iter()
            .filter(|d| d.is_enabled)
            .filter(|d| category_ids.is_empty() || category_ids.contains(&d.category_id))
            .filter_map(def_to_llm_tool)
            .collect();
        Ok(tools)
    }

    /// Load enabled tools matching an arbitrary predicate over `ToolDefinition`.
    /// Useful when a caller needs read_only + category combined or other ad-hoc
    /// filtering without piling more flags onto the public API.
    pub async fn load_llm_tools_filtered<F>(
        &self,
        predicate: F,
    ) -> Result<Vec<crate::llm::types::LlmTool>>
    where
        F: Fn(&ToolDefinition) -> bool,
    {
        let all_defs = self.list_tool_definitions().await?;
        let tools = all_defs
            .into_iter()
            .filter(|d| d.is_enabled && predicate(d))
            .filter_map(def_to_llm_tool)
            .collect();
        Ok(tools)
    }

    // =========================================================================
    // ChatMode CRUD
    // =========================================================================

    pub async fn list_chat_modes(&self) -> Result<Vec<ChatMode>> {
        let rows = sqlx::query(
            "SELECT id, name, description, category_ids_json, tool_ids_json,
                    sub_agent_ids_json, rule_ids_json, prompt_id, is_template,
                    is_default_for_kind, created_at, updated_at
             FROM chat_modes ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await
        .db_err("Failed to list chat modes")?;
        Ok(rows.iter().map(row_to_chat_mode).collect())
    }

    pub async fn get_chat_mode(&self, id: &str) -> Result<ChatMode> {
        let row = sqlx::query(
            "SELECT id, name, description, category_ids_json, tool_ids_json,
                    sub_agent_ids_json, rule_ids_json, prompt_id, is_template,
                    is_default_for_kind, created_at, updated_at
             FROM chat_modes WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .db_err("Failed to get chat mode")?;
        match row {
            Some(r) => Ok(row_to_chat_mode(&r)),
            None => Err(VenoreError::NotFound(format!("Chat mode '{}'", id))),
        }
    }

    /// Find the seeded default mode for a project kind ("code" | "knowledge").
    /// Returns `None` if no template default is set up for that kind.
    pub async fn get_default_chat_mode_for_kind(
        &self,
        kind: &str,
    ) -> Result<Option<ChatMode>> {
        let row = sqlx::query(
            "SELECT id, name, description, category_ids_json, tool_ids_json,
                    sub_agent_ids_json, rule_ids_json, prompt_id, is_template,
                    is_default_for_kind, created_at, updated_at
             FROM chat_modes WHERE is_default_for_kind = ? LIMIT 1",
        )
        .bind(kind)
        .fetch_optional(&self.pool)
        .await
        .db_err("Failed to query default chat mode")?;
        Ok(row.as_ref().map(row_to_chat_mode))
    }

    pub async fn create_chat_mode(&self, mode: &ChatMode) -> Result<()> {
        sqlx::query(
            "INSERT INTO chat_modes
                (id, name, description, category_ids_json, tool_ids_json,
                 sub_agent_ids_json, rule_ids_json, prompt_id, is_template,
                 is_default_for_kind, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&mode.id)
        .bind(&mode.name)
        .bind(&mode.description)
        .bind(serde_json::to_string(&mode.category_ids).unwrap_or_else(|_| "[]".into()))
        .bind(serde_json::to_string(&mode.tool_ids).unwrap_or_else(|_| "[]".into()))
        .bind(serde_json::to_string(&mode.sub_agent_ids).unwrap_or_else(|_| "[]".into()))
        .bind(serde_json::to_string(&mode.rule_ids).unwrap_or_else(|_| "[]".into()))
        .bind(&mode.prompt_id)
        .bind(mode.is_template as i64)
        .bind(&mode.is_default_for_kind)
        .bind(&mode.created_at)
        .bind(&mode.updated_at)
        .execute(&self.pool)
        .await
        .db_err("Failed to create chat mode")?;
        Ok(())
    }

    pub async fn update_chat_mode(&self, mode: &ChatMode) -> Result<()> {
        sqlx::query(
            "UPDATE chat_modes SET
                name = ?, description = ?,
                category_ids_json = ?, tool_ids_json = ?,
                sub_agent_ids_json = ?, rule_ids_json = ?,
                prompt_id = ?, is_default_for_kind = ?,
                updated_at = ?
             WHERE id = ?",
        )
        .bind(&mode.name)
        .bind(&mode.description)
        .bind(serde_json::to_string(&mode.category_ids).unwrap_or_else(|_| "[]".into()))
        .bind(serde_json::to_string(&mode.tool_ids).unwrap_or_else(|_| "[]".into()))
        .bind(serde_json::to_string(&mode.sub_agent_ids).unwrap_or_else(|_| "[]".into()))
        .bind(serde_json::to_string(&mode.rule_ids).unwrap_or_else(|_| "[]".into()))
        .bind(&mode.prompt_id)
        .bind(&mode.is_default_for_kind)
        .bind(&mode.updated_at)
        .bind(&mode.id)
        .execute(&self.pool)
        .await
        .db_err("Failed to update chat mode")?;
        Ok(())
    }

    pub async fn delete_chat_mode(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM chat_modes WHERE id = ? AND is_template = 0")
            .bind(id)
            .execute(&self.pool)
            .await
            .db_err("Failed to delete chat mode")?;
        Ok(())
    }

    pub async fn count_template_chat_modes(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM chat_modes WHERE is_template = 1")
            .fetch_one(&self.pool)
            .await
            .db_err("Failed to count template chat modes")?;
        Ok(row.0)
    }

    /// Resolve the LlmTool list a chat session should expose given a project
    /// kind. Looks up the default mode, intersects with enabled tools.
    /// Falls back to "all enabled" if no mode is set for that kind.
    pub async fn load_llm_tools_for_kind(
        &self,
        kind: &str,
    ) -> Result<Vec<crate::llm::types::LlmTool>> {
        let mode = self.get_default_chat_mode_for_kind(kind).await?;
        match mode {
            Some(m) if !m.category_ids.is_empty() || !m.tool_ids.is_empty() => {
                let all_defs = self.list_tool_definitions().await?;
                let tools = all_defs
                    .into_iter()
                    .filter(|d| d.is_enabled)
                    .filter(|d| {
                        let cat_ok = m.category_ids.is_empty()
                            || m.category_ids.contains(&d.category_id);
                        let tool_ok = m.tool_ids.is_empty()
                            || m.tool_ids.contains(&d.id);
                        cat_ok && tool_ok
                    })
                    .filter_map(def_to_llm_tool)
                    .collect();
                Ok(tools)
            }
            _ => self.load_llm_tools(&[]).await,
        }
    }

}

// =============================================================================
// Row mapping helpers
// =============================================================================

fn row_to_profile(row: &sqlx::sqlite::SqliteRow) -> AgentProfile {
    let stage_str: String = row.get("stage");
    let stage = AgentStage::from_str(&stage_str).unwrap_or_else(|| {
        tracing::warn!(value = %stage_str, "Unknown stage in agent profile, defaulting to Specialist");
        AgentStage::Specialist
    });

    AgentProfile {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        stage,
        system_prompt: row.get("system_prompt"),
        provider: row.get("provider"),
        model: row.get("model"),
        temperature: row.get::<f64, _>("temperature") as f32,
        is_template: row.get::<bool, _>("is_template"),
        is_enabled: row.get::<bool, _>("is_enabled"),
        rules_json: row.get("rules_json"),
        criteria_json: row.get("criteria_json"),
        tools_json: row.get("tools_json"),
        max_tokens_per_run: row.get::<i32, _>("max_tokens_per_run") as u32,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn row_to_rule(row: &sqlx::sqlite::SqliteRow) -> AgentRule {
    let scope_str: String = row.get("scope");
    let scope: Vec<String> = serde_json::from_str(&scope_str).unwrap_or_else(|e| {
        tracing::warn!(scope = %scope_str, error = %e, "Corrupt scope JSON in agent rule, using empty");
        Vec::new()
    });

    let severity_str: String = row.get("severity");
    let severity = Severity::from_str(&severity_str).unwrap_or_else(|| {
        tracing::warn!(value = %severity_str, "Unknown severity in agent rule, defaulting to Warning");
        Severity::Warning
    });

    AgentRule {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        scope,
        severity,
        is_active: row.get::<bool, _>("is_active"),
        is_template: row.get::<bool, _>("is_template"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn row_to_team(row: &sqlx::sqlite::SqliteRow) -> AgentTeam {
    let profile_ids_str: String = row.get("profile_ids");
    let profile_ids: Vec<String> = serde_json::from_str(&profile_ids_str).unwrap_or_else(|e| {
        tracing::warn!(json = %profile_ids_str, error = %e, "Corrupt profile_ids JSON in team, using empty");
        Vec::new()
    });

    AgentTeam {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        profile_ids,
        is_template: row.get::<bool, _>("is_template"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn row_to_pipeline_run(row: &sqlx::sqlite::SqliteRow) -> PipelineRun {
    let status_str: String = row.get("status");
    let status = PipelineRunStatus::from_str(&status_str).unwrap_or_else(|| {
        tracing::warn!(value = %status_str, "Unknown pipeline run status, defaulting to Failed");
        PipelineRunStatus::Failed
    });

    PipelineRun {
        id: row.get("id"),
        team_id: row.get("team_id"),
        team_name: row.get("team_name"),
        task_type: row.get("task_type"),
        title: row.get("title"),
        status,
        pr_number: row.get::<Option<i64>, _>("pr_number").map(|n| n as u64),
        project_path: row.get("project_path"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
        duration_ms: row.get::<i64, _>("duration_ms") as u64,
        total_tokens: row.get::<i32, _>("total_tokens") as u32,
        created_at: row.get("created_at"),
        pr_author: row.get("pr_author"),
        pr_author_avatar: row.get("pr_author_avatar"),
        pr_additions: row.get::<Option<i64>, _>("pr_additions").map(|n| n as u64),
        pr_deletions: row.get::<Option<i64>, _>("pr_deletions").map(|n| n as u64),
        pr_changed_files: row.get::<Option<i64>, _>("pr_changed_files").map(|n| n as u64),
        depth_level: row.get("depth_level"),
    }
}

fn row_to_pipeline_step(row: &sqlx::sqlite::SqliteRow) -> PipelineStep {
    let status_str: String = row.get("status");
    let status = PipelineStepStatus::from_str(&status_str).unwrap_or_else(|| {
        tracing::warn!(value = %status_str, "Unknown pipeline step status, defaulting to Failed");
        PipelineStepStatus::Failed
    });

    PipelineStep {
        id: row.get("id"),
        run_id: row.get("run_id"),
        profile_id: row.get("profile_id"),
        profile_name: row.get("profile_name"),
        stage: row.get("stage"),
        status,
        input_context: row.get("input_context"),
        output: row.get("output"),
        provider: row.get("provider"),
        model: row.get("model"),
        prompt_tokens: row.get::<i32, _>("prompt_tokens") as u32,
        completion_tokens: row.get::<i32, _>("completion_tokens") as u32,
        total_tokens: row.get::<i32, _>("total_tokens") as u32,
        duration_ms: row.get::<i64, _>("duration_ms") as u64,
        error: row.get("error"),
        step_order: row.get::<i32, _>("step_order") as u32,
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
    }
}

fn row_to_tool_category(row: &sqlx::sqlite::SqliteRow) -> ToolCategory {
    ToolCategory {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        icon: row.get("icon"),
        color: row.get("color"),
        display_order: row.get::<i32, _>("display_order") as u32,
        is_template: row.get::<bool, _>("is_template"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn def_to_llm_tool(d: ToolDefinition) -> Option<crate::llm::types::LlmTool> {
    match serde_json::from_str(&d.parameters_json) {
        Ok(parameters) => Some(crate::llm::types::LlmTool {
            name: d.name,
            description: d.description,
            parameters,
        }),
        Err(e) => {
            tracing::warn!(tool = %d.name, error = %e, "Skipping tool with invalid parameters JSON");
            None
        }
    }
}

fn row_to_tool_definition(row: &sqlx::sqlite::SqliteRow) -> ToolDefinition {
    ToolDefinition {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        category_id: row.get("category_id"),
        parameters_json: row.get("parameters_json"),
        is_read_only: row.get::<bool, _>("is_read_only"),
        is_enabled: row.get::<bool, _>("is_enabled"),
        is_template: row.get::<bool, _>("is_template"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn row_to_chat_mode(row: &sqlx::sqlite::SqliteRow) -> ChatMode {
    let category_ids_json: String = row.get("category_ids_json");
    let tool_ids_json: String = row.get("tool_ids_json");
    let sub_agent_ids_json: String = row.get("sub_agent_ids_json");
    let rule_ids_json: String = row.get("rule_ids_json");
    ChatMode {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        category_ids: serde_json::from_str(&category_ids_json).unwrap_or_default(),
        tool_ids: serde_json::from_str(&tool_ids_json).unwrap_or_default(),
        sub_agent_ids: serde_json::from_str(&sub_agent_ids_json).unwrap_or_default(),
        rule_ids: serde_json::from_str(&rule_ids_json).unwrap_or_default(),
        prompt_id: row.get("prompt_id"),
        is_template: row.get::<bool, _>("is_template"),
        is_default_for_kind: row.get("is_default_for_kind"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}
