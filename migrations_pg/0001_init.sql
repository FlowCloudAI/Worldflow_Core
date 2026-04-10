CREATE TABLE IF NOT EXISTS projects (
                                        id          TEXT PRIMARY KEY,
                                        name        TEXT NOT NULL,
                                        description TEXT,
                                        cover_image TEXT,
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
                                               UNIQUE (a_id, b_id, content),
                                               CHECK (a_id != b_id)
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

-- ── entry_types 表（自定义词条类型）──────────────────
CREATE TABLE IF NOT EXISTS entry_types (
                                           id          TEXT PRIMARY KEY,
                                           project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                                           name        TEXT NOT NULL,
                                           description TEXT,
                                           icon        TEXT,
                                           color       TEXT,
                                           created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                                           updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                                           UNIQUE (project_id, name),
                                           CHECK (name != '')
);

-- ── 跨项目一致性约束 ─────────────────────────────────

-- entries: 防止 category_id 跨项目
CREATE OR REPLACE FUNCTION check_entry_category_project()
    RETURNS TRIGGER AS $$
BEGIN
    IF NEW.category_id IS NOT NULL THEN
        IF (SELECT project_id FROM categories WHERE id = NEW.category_id) != NEW.project_id THEN
            RAISE EXCEPTION 'category_id must belong to same project';
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER entries_category_same_project
    BEFORE INSERT OR UPDATE ON entries
    FOR EACH ROW EXECUTE FUNCTION check_entry_category_project();

-- entry_relations: 防止 a_id/b_id 跨项目
CREATE OR REPLACE FUNCTION check_relation_same_project()
    RETURNS TRIGGER AS $$
BEGIN
    IF (SELECT project_id FROM entries WHERE id = NEW.a_id) != NEW.project_id THEN
        RAISE EXCEPTION 'a_id must belong to same project';
    END IF;
    IF (SELECT project_id FROM entries WHERE id = NEW.b_id) != NEW.project_id THEN
        RAISE EXCEPTION 'b_id must belong to same project';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER relations_same_project
    BEFORE INSERT ON entry_relations
    FOR EACH ROW EXECUTE FUNCTION check_relation_same_project();

-- entry_relations: two_way 关系规范化 a_id < b_id
CREATE OR REPLACE FUNCTION normalize_relation_order()
    RETURNS TRIGGER AS $$
DECLARE
    tmp TEXT;
BEGIN
    IF NEW.relation = 'two_way' AND NEW.a_id > NEW.b_id THEN
        tmp := NEW.a_id;
        NEW.a_id := NEW.b_id;
        NEW.b_id := tmp;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER relations_normalize_order
    BEFORE INSERT OR UPDATE ON entry_relations
    FOR EACH ROW EXECUTE FUNCTION normalize_relation_order();

-- 通用函数：自动更新 timestamp
CREATE OR REPLACE FUNCTION update_timestamp()
    RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- 触发器：自动更新 timestamp on update (all tables)
CREATE TRIGGER IF NOT EXISTS projects_updated_at
    BEFORE UPDATE ON projects
               FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TRIGGER IF NOT EXISTS categories_updated_at
    BEFORE UPDATE ON categories
               FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TRIGGER IF NOT EXISTS tag_schemas_updated_at
    BEFORE UPDATE ON tag_schemas
               FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TRIGGER IF NOT EXISTS entries_updated_at
    BEFORE UPDATE ON entries
               FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TRIGGER IF NOT EXISTS entry_relations_updated_at
    BEFORE UPDATE ON entry_relations
               FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TRIGGER IF NOT EXISTS entry_types_updated_at
    BEFORE UPDATE ON entry_types
               FOR EACH ROW EXECUTE FUNCTION update_timestamp();

-- 索引
CREATE INDEX IF NOT EXISTS idx_entry_types_project ON entry_types(project_id);
CREATE INDEX IF NOT EXISTS idx_entry_types_name ON entry_types(project_id, name);
