CREATE INDEX IF NOT EXISTS idx_entries_project_category_updated
    ON entries(project_id, category_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_entries_project_type_updated
    ON entries(project_id, type, updated_at DESC);
