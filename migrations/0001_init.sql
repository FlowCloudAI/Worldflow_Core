PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA recursive_triggers = OFF;

CREATE TABLE IF NOT EXISTS projects (
                                        id BLOB PRIMARY KEY CHECK (length(id) = 16),
                                        name        TEXT NOT NULL,
                                        description TEXT,
                                        cover_image TEXT,
                                        created_at  TEXT NOT NULL DEFAULT (datetime('now')),
                                        updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS categories (
                                          id         BLOB PRIMARY KEY CHECK (length(id) = 16),
                                          project_id BLOB NOT NULL REFERENCES projects (id) ON DELETE CASCADE CHECK (length(project_id) = 16),
                                          parent_id  BLOB CHECK (parent_id IS NULL OR length(parent_id) = 16),
                                          name        TEXT NOT NULL,
                                          sort_order  INTEGER NOT NULL DEFAULT 0,
                                          created_at  TEXT NOT NULL DEFAULT (datetime('now')),
                                          updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
                                          CHECK (id != parent_id),
                                          UNIQUE (project_id, id),
                                          FOREIGN KEY (project_id, parent_id) REFERENCES categories(project_id, id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS tag_schemas (
                                           id         BLOB PRIMARY KEY CHECK (length(id) = 16),
                                           project_id BLOB NOT NULL REFERENCES projects (id) ON DELETE CASCADE CHECK (length(project_id) = 16),
                                           name        TEXT NOT NULL,
                                           description TEXT,
                                           type        TEXT NOT NULL CHECK(type IN ('number', 'string', 'boolean')),
                                           target      TEXT NOT NULL DEFAULT '[]',
                                           default_val TEXT,
                                           range_min   REAL,
                                           range_max   REAL,
                                           sort_order  INTEGER NOT NULL DEFAULT 0,
                                           created_at  TEXT NOT NULL DEFAULT (datetime('now')),
                                           updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
                                           CHECK (range_min IS NULL OR range_max IS NULL OR range_min <= range_max)
);

CREATE TABLE IF NOT EXISTS entries (
                                       _rowid      INTEGER PRIMARY KEY AUTOINCREMENT,
                                       id          BLOB UNIQUE NOT NULL CHECK (length(id) = 16),
                                       project_id  BLOB        NOT NULL REFERENCES projects (id) ON DELETE CASCADE CHECK (length(project_id) = 16),
                                       category_id BLOB        REFERENCES categories (id) ON DELETE SET NULL CHECK (category_id IS NULL OR length(category_id) = 16),
                                       title       TEXT        NOT NULL,
                                       summary     TEXT,
                                       content     TEXT        NOT NULL DEFAULT '',
                                       type        TEXT,
                                       tags        TEXT        NOT NULL DEFAULT '[]',
                                       images      TEXT        NOT NULL DEFAULT '[]',
                                       cover_path  TEXT,
                                       created_at  TEXT        NOT NULL DEFAULT (datetime('now')),
                                       updated_at  TEXT        NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS entry_relations (
                                               id         BLOB PRIMARY KEY CHECK (length(id) = 16),
                                               project_id BLOB NOT NULL REFERENCES projects (id) ON DELETE CASCADE CHECK (length(project_id) = 16),
                                               a_id       BLOB NOT NULL REFERENCES entries (id) ON DELETE CASCADE CHECK (length(a_id) = 16),
                                               b_id       BLOB NOT NULL REFERENCES entries (id) ON DELETE CASCADE CHECK (length(b_id) = 16),
                                               relation   TEXT NOT NULL CHECK(relation IN ('one_way', 'two_way')),
                                               content    TEXT NOT NULL,
                                               created_at TEXT NOT NULL DEFAULT (datetime('now')),
                                               updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                                               UNIQUE (a_id, b_id, content),
                                               CHECK (a_id != b_id)
);

-- entries: 防止 category_id 跨项目（INSERT）
CREATE TRIGGER IF NOT EXISTS entries_category_same_project_insert
    BEFORE INSERT ON entries
    WHEN NEW.category_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'category_id must belong to same project')
    WHERE (SELECT project_id FROM categories WHERE id = NEW.category_id) != NEW.project_id;
END;

-- entries: 防止 category_id 跨项目（UPDATE category_id）
CREATE TRIGGER IF NOT EXISTS entries_category_same_project_update_cat
    BEFORE UPDATE OF category_id ON entries
    WHEN NEW.category_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'category_id must belong to same project')
    WHERE (SELECT project_id FROM categories WHERE id = NEW.category_id) != NEW.project_id;
END;

-- entries: 防止 project_id 变更导致 category_id 跨项目（UPDATE project_id）
CREATE TRIGGER IF NOT EXISTS entries_category_same_project_update_proj
    BEFORE UPDATE OF project_id ON entries
    WHEN NEW.category_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'category_id must belong to same project')
    WHERE (SELECT project_id FROM categories WHERE id = NEW.category_id) != NEW.project_id;
END;

-- entry_relations: 防止 a_id/b_id 跨项目
CREATE TRIGGER IF NOT EXISTS relations_same_project_insert
    BEFORE INSERT ON entry_relations
BEGIN
    SELECT RAISE(ABORT, 'a_id must belong to same project')
    WHERE (SELECT project_id FROM entries WHERE id = NEW.a_id) != NEW.project_id;
    SELECT RAISE(ABORT, 'b_id must belong to same project')
    WHERE (SELECT project_id FROM entries WHERE id = NEW.b_id) != NEW.project_id;
END;

-- entry_relations: two_way 关系必须 a_id < b_id（应用层已规范化，此为安全网）
CREATE TRIGGER IF NOT EXISTS relations_two_way_order_insert
    BEFORE INSERT ON entry_relations
    WHEN NEW.relation = 'two_way' AND NEW.a_id > NEW.b_id
BEGIN
    SELECT RAISE(ABORT, 'two_way relation must have a_id < b_id');
END;

CREATE TRIGGER IF NOT EXISTS relations_two_way_order_update
    BEFORE UPDATE ON entry_relations
    WHEN NEW.relation = 'two_way' AND NEW.a_id > NEW.b_id
BEGIN
    SELECT RAISE(ABORT, 'two_way relation must have a_id < b_id');
END;

-- updated_at triggers
CREATE TRIGGER IF NOT EXISTS projects_updated_at
    AFTER UPDATE ON projects
BEGIN UPDATE projects SET updated_at = datetime('now') WHERE id = NEW.id; END;

CREATE TRIGGER IF NOT EXISTS categories_updated_at
    AFTER UPDATE ON categories
BEGIN UPDATE categories SET updated_at = datetime('now') WHERE id = NEW.id; END;

CREATE TRIGGER IF NOT EXISTS tag_schemas_updated_at
    AFTER UPDATE ON tag_schemas
BEGIN UPDATE tag_schemas SET updated_at = datetime('now') WHERE id = NEW.id; END;

CREATE TRIGGER IF NOT EXISTS entries_updated_at
    AFTER UPDATE ON entries
BEGIN
    UPDATE entries SET updated_at = datetime('now') WHERE _rowid = NEW._rowid;
END;

CREATE TRIGGER IF NOT EXISTS entry_relations_updated_at
    AFTER UPDATE ON entry_relations
BEGIN UPDATE entry_relations SET updated_at = datetime('now') WHERE id = NEW.id; END;

-- 普通索引
CREATE INDEX IF NOT EXISTS idx_entries_project         ON entries(project_id);
CREATE INDEX IF NOT EXISTS idx_entries_category        ON entries(category_id);
CREATE INDEX IF NOT EXISTS idx_entries_type            ON entries(type);
CREATE INDEX IF NOT EXISTS idx_entries_project_updated ON entries(project_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_entries_project_category_updated ON entries (project_id, category_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_entries_project_type_updated ON entries (project_id, type, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_categories_project      ON categories(project_id);
CREATE INDEX IF NOT EXISTS idx_categories_parent       ON categories(parent_id);
CREATE INDEX IF NOT EXISTS idx_tag_schemas_project     ON tag_schemas(project_id);
CREATE INDEX IF NOT EXISTS idx_relations_a             ON entry_relations(a_id);
CREATE INDEX IF NOT EXISTS idx_relations_b             ON entry_relations(b_id);
CREATE INDEX IF NOT EXISTS idx_relations_project       ON entry_relations(project_id);

-- FTS5 全文搜索
-- 不使用 content=entries，让 project_id UNINDEXED 直接存储在 FTS5 内部，
-- 避免高频词命中时大量回表读取 project_id 导致的随机 I/O 爆炸。
-- entries 表使用 INTEGER PRIMARY KEY (_rowid) 作为稳定 rowid，FTS5 通过触发器同步 _rowid，
-- 解决了 UUID 主键下隐式 rowid 不稳定导致的 FTS5 关联失效问题。
CREATE VIRTUAL TABLE IF NOT EXISTS entries_fts USING fts5(
                                                             title,
                                                             content,
                                                             project_id UNINDEXED,
                                                             tokenize='trigram'
);

CREATE TRIGGER IF NOT EXISTS entries_fts_insert AFTER INSERT ON entries BEGIN
    INSERT INTO entries_fts(rowid, title, content, project_id)
    VALUES (new._rowid, new.title, new.content, new.project_id);
END;

CREATE TRIGGER IF NOT EXISTS entries_fts_update AFTER UPDATE ON entries BEGIN
    DELETE FROM entries_fts WHERE rowid = old._rowid;
    INSERT INTO entries_fts(rowid, title, content, project_id)
    VALUES (new._rowid, new.title, new.content, new.project_id);
END;

CREATE TRIGGER IF NOT EXISTS entries_fts_delete AFTER DELETE ON entries BEGIN
    DELETE FROM entries_fts WHERE rowid = old._rowid;
END;

-- ── entry_types 表（自定义词条类型）──────────────────
CREATE TABLE IF NOT EXISTS entry_types (
                                           id         BLOB PRIMARY KEY CHECK (length(id) = 16),
                                           project_id BLOB NOT NULL REFERENCES projects (id) ON DELETE CASCADE CHECK (length(project_id) = 16),
    name        TEXT NOT NULL,
    description TEXT,
    icon        TEXT,
    color       TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (project_id, name),
    CHECK (name != '')
);

-- 自动更新 updated_at
CREATE TRIGGER IF NOT EXISTS entry_types_updated_at
    AFTER UPDATE ON entry_types
BEGIN UPDATE entry_types SET updated_at = datetime('now') WHERE id = NEW.id; END;

-- 索引
CREATE INDEX IF NOT EXISTS idx_entry_types_project ON entry_types(project_id);
CREATE INDEX IF NOT EXISTS idx_entry_types_name ON entry_types(project_id, name);
