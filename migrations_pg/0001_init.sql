CREATE TABLE IF NOT EXISTS projects (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT,
    cover_path  TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS categories (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    parent_id   TEXT,
    name        TEXT NOT NULL,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (id != parent_id),
    UNIQUE (project_id, id),
    FOREIGN KEY (project_id, parent_id) REFERENCES categories(project_id, id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS tag_schemas (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    description TEXT,
    type        TEXT NOT NULL CHECK(type IN ('number', 'string', 'boolean')),
    target      TEXT NOT NULL DEFAULT '[]',
    default_val TEXT,
    range_min   DOUBLE PRECISION,
    range_max   DOUBLE PRECISION,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (range_min IS NULL OR range_max IS NULL OR range_min <= range_max)
);

CREATE TABLE IF NOT EXISTS entries (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    category_id TEXT REFERENCES categories(id) ON DELETE SET NULL,
    title       TEXT NOT NULL,
    summary     TEXT,
    content     TEXT NOT NULL DEFAULT '',
    type        TEXT,
    tags        TEXT NOT NULL DEFAULT '[]',
    images      TEXT NOT NULL DEFAULT '[]',
    cover_path  TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS entry_relations (
    id         TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    a_id       TEXT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    b_id       TEXT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    relation   TEXT NOT NULL CHECK(relation IN ('one_way', 'two_way')),
    content    TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (a_id, b_id, content)
);

-- 全文搜索索引（使用 PostgreSQL tsvector）
CREATE INDEX IF NOT EXISTS idx_entries_fts ON entries
    USING GIN (to_tsvector('simple', coalesce(title,'') || ' ' || coalesce(summary,'') || ' ' || coalesce(content,'')));

CREATE INDEX IF NOT EXISTS idx_entries_project_id ON entries(project_id);
CREATE INDEX IF NOT EXISTS idx_entries_category_id ON entries(category_id);
CREATE INDEX IF NOT EXISTS idx_categories_project_id ON categories(project_id);
CREATE INDEX IF NOT EXISTS idx_tag_schemas_project_id ON tag_schemas(project_id);
CREATE INDEX IF NOT EXISTS idx_entry_relations_a_id ON entry_relations(a_id);
CREATE INDEX IF NOT EXISTS idx_entry_relations_b_id ON entry_relations(b_id);
CREATE INDEX IF NOT EXISTS idx_entry_relations_project ON entry_relations(project_id);
