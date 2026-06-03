-- Create task_settings table
-- Stores custom LLM task configuration per task type

CREATE TABLE IF NOT EXISTS task_settings (
    task TEXT PRIMARY KEY NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    temperature REAL,
    max_tokens INTEGER,
    timeout_ms INTEGER,
    streaming INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Index for quick lookups
CREATE INDEX IF NOT EXISTS idx_task_settings_task ON task_settings(task);

-- Trigger to automatically update updated_at timestamp
CREATE TRIGGER IF NOT EXISTS update_task_settings_timestamp
    AFTER UPDATE ON task_settings
    FOR EACH ROW
BEGIN
    UPDATE task_settings
    SET updated_at = datetime('now')
    WHERE task = OLD.task;
END;
