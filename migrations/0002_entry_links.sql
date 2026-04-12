CREATE TABLE IF NOT EXISTS entry_links
(
    id         BLOB PRIMARY KEY CHECK (length(id) = 16),
    project_id BLOB NOT NULL REFERENCES projects (id) ON DELETE CASCADE CHECK (length(project_id) = 16),
    a_id       BLOB NOT NULL REFERENCES entries (id) ON DELETE CASCADE CHECK (length(a_id) = 16),
    b_id       BLOB NOT NULL REFERENCES entries (id) ON DELETE CASCADE CHECK (length(b_id) = 16),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (project_id, a_id, b_id),
    CHECK (a_id != b_id)
);

CREATE TRIGGER IF NOT EXISTS entry_links_same_project_insert
    BEFORE INSERT
    ON entry_links
BEGIN
    SELECT RAISE(ABORT, 'a_id must belong to same project')
    WHERE (SELECT project_id FROM entries WHERE id = NEW.a_id) != NEW.project_id;
    SELECT RAISE(ABORT, 'b_id must belong to same project')
    WHERE (SELECT project_id FROM entries WHERE id = NEW.b_id) != NEW.project_id;
END;

CREATE TRIGGER IF NOT EXISTS entry_links_same_project_update
    BEFORE UPDATE
    ON entry_links
BEGIN
    SELECT RAISE(ABORT, 'a_id must belong to same project')
    WHERE (SELECT project_id FROM entries WHERE id = NEW.a_id) != NEW.project_id;
    SELECT RAISE(ABORT, 'b_id must belong to same project')
    WHERE (SELECT project_id FROM entries WHERE id = NEW.b_id) != NEW.project_id;
END;

CREATE INDEX IF NOT EXISTS idx_entry_links_a ON entry_links (a_id);
CREATE INDEX IF NOT EXISTS idx_entry_links_b ON entry_links (b_id);
CREATE INDEX IF NOT EXISTS idx_entry_links_project ON entry_links (project_id);
