CREATE TABLE IF NOT EXISTS entry_links
(
    id         UUID PRIMARY KEY,
    project_id UUID        NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    a_id       UUID        NOT NULL REFERENCES entries (id) ON DELETE CASCADE,
    b_id       UUID        NOT NULL REFERENCES entries (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (project_id, a_id, b_id),
    CHECK (a_id != b_id)
);

CREATE
OR REPLACE
FUNCTION check_entry_link_same_project()
    RETURNS TRIGGER AS $$
BEGIN IF (SELECT project_id FROM entries WHERE id = NEW.a_id) != NEW.project_id THEN
        RAISE EXCEPTION 'a_id must belong to same project';
END IF;
IF (SELECT project_id FROM entries WHERE id = NEW.b_id) != NEW.project_id THEN
        RAISE EXCEPTION 'b_id must belong to same project';
END IF;
RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER entry_links_same_project
    BEFORE INSERT OR UPDATE ON entry_links
    FOR EACH ROW EXECUTE FUNCTION check_entry_link_same_project();

CREATE INDEX IF NOT EXISTS idx_entry_links_a_id ON entry_links (a_id);
CREATE INDEX IF NOT EXISTS idx_entry_links_b_id ON entry_links (b_id);
CREATE INDEX IF NOT EXISTS idx_entry_links_project ON entry_links (project_id);
