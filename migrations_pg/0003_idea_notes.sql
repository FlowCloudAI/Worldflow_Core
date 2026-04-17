-- 灵感便签表：独立建模，不污染 entries
CREATE TABLE IF NOT EXISTS idea_notes
(
    id                 UUID PRIMARY KEY,
    project_id         UUID        REFERENCES projects (id) ON DELETE SET NULL,
    content            TEXT        NOT NULL,
    title              TEXT,
    status             TEXT        NOT NULL DEFAULT 'inbox'
        CHECK (status IN ('inbox', 'processed', 'archived')),
    pinned             BOOLEAN     NOT NULL DEFAULT FALSE,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_reviewed_at   TIMESTAMPTZ,
    converted_entry_id UUID        REFERENCES entries (id) ON DELETE SET NULL
);

-- 自动更新 updated_at（使用 BEFORE UPDATE 触发器）
CREATE
OR REPLACE
FUNCTION update_idea_notes_updated_at()
RETURNS TRIGGER AS $$
BEGIN NEW.updated_at = NOW();
RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER idea_notes_updated_at
    BEFORE UPDATE
    ON idea_notes
    FOR EACH ROW EXECUTE FUNCTION update_idea_notes_updated_at();

-- 索引：覆盖常见查询路径
CREATE INDEX IF NOT EXISTS idx_idea_notes_project ON idea_notes (project_id);
CREATE INDEX IF NOT EXISTS idx_idea_notes_status ON idea_notes (status);
CREATE INDEX IF NOT EXISTS idx_idea_notes_updated_at ON idea_notes (updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_idea_notes_pinned ON idea_notes (pinned);
CREATE INDEX IF NOT EXISTS idx_idea_notes_project_status ON idea_notes (project_id, status);
CREATE INDEX IF NOT EXISTS idx_idea_notes_pinned_updated ON idea_notes (pinned DESC, updated_at DESC);
