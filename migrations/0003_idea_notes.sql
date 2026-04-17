-- 灵感便签表：独立建模，不污染 entries
CREATE TABLE IF NOT EXISTS idea_notes
(
    id                 BLOB PRIMARY KEY CHECK (length(id) = 16),
    project_id         BLOB    REFERENCES projects (id) ON DELETE SET NULL
        CHECK (project_id IS NULL OR length(project_id) = 16),
    content            TEXT    NOT NULL,
    title              TEXT,
    status             TEXT    NOT NULL DEFAULT 'inbox'
        CHECK (status IN ('inbox', 'processed', 'archived')),
    pinned             INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0, 1)),
    created_at         TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at         TEXT    NOT NULL DEFAULT (datetime('now')),
    last_reviewed_at   TEXT,
    converted_entry_id BLOB    REFERENCES entries (id) ON DELETE SET NULL
        CHECK (converted_entry_id IS NULL OR length(converted_entry_id) = 16)
);

-- 自动更新 updated_at
CREATE TRIGGER IF NOT EXISTS idea_notes_updated_at
    AFTER UPDATE
    ON idea_notes
BEGIN
    UPDATE idea_notes SET updated_at = datetime('now') WHERE id = NEW.id;
END;

-- 索引：覆盖常见查询路径
CREATE INDEX IF NOT EXISTS idx_idea_notes_project ON idea_notes (project_id);
CREATE INDEX IF NOT EXISTS idx_idea_notes_status ON idea_notes (status);
CREATE INDEX IF NOT EXISTS idx_idea_notes_updated_at ON idea_notes (updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_idea_notes_pinned ON idea_notes (pinned);
CREATE INDEX IF NOT EXISTS idx_idea_notes_project_status ON idea_notes (project_id, status);
CREATE INDEX IF NOT EXISTS idx_idea_notes_pinned_updated ON idea_notes (pinned DESC, updated_at DESC);
