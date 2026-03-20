PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA recursive_triggers = OFF;

CREATE TABLE IF NOT EXISTS projects (
                                        id          TEXT PRIMARY KEY,
                                        name        TEXT NOT NULL,
                                        description TEXT,
                                        created_at  TEXT NOT NULL DEFAULT (datetime('now')),
                                        updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS categories (
                                          id          TEXT PRIMARY KEY,
                                          project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                                          parent_id   TEXT,
                                          name        TEXT NOT NULL,
                                          sort_order  INTEGER NOT NULL DEFAULT 0,
                                          created_at  TEXT NOT NULL DEFAULT (datetime('now')),
                                          updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
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
                                           range_min   REAL,
                                           range_max   REAL,
                                           sort_order  INTEGER NOT NULL DEFAULT 0,
                                           created_at  TEXT NOT NULL DEFAULT (datetime('now')),
                                           updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
                                           CHECK (range_min IS NULL OR range_max IS NULL OR range_min <= range_max)
);

CREATE TABLE IF NOT EXISTS entries (
                                       id          TEXT PRIMARY KEY,
                                       project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                                       category_id TEXT REFERENCES categories(id) ON DELETE SET NULL,
                                       title       TEXT NOT NULL,
                                       content     TEXT NOT NULL DEFAULT '',
                                       type        TEXT,
                                       tags        TEXT NOT NULL DEFAULT '[]',
                                       images      TEXT NOT NULL DEFAULT '[]',
                                       cover_path  TEXT,
                                       created_at  TEXT NOT NULL DEFAULT (datetime('now')),
                                       updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS app_settings (
                                            key        TEXT PRIMARY KEY,
                                            value      TEXT NOT NULL,
                                            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
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
BEGIN UPDATE entries SET updated_at = datetime('now') WHERE id = NEW.id; END;

CREATE TRIGGER IF NOT EXISTS app_settings_updated_at
    AFTER UPDATE ON app_settings
BEGIN UPDATE app_settings SET updated_at = datetime('now') WHERE key = NEW.key; END;

-- 普通索引
CREATE INDEX IF NOT EXISTS idx_entries_project         ON entries(project_id);
CREATE INDEX IF NOT EXISTS idx_entries_category        ON entries(category_id);
CREATE INDEX IF NOT EXISTS idx_entries_type            ON entries(type);
CREATE INDEX IF NOT EXISTS idx_entries_project_updated ON entries(project_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_categories_project      ON categories(project_id);
CREATE INDEX IF NOT EXISTS idx_categories_parent       ON categories(parent_id);
CREATE INDEX IF NOT EXISTS idx_tag_schemas_project     ON tag_schemas(project_id);

-- FTS5 全文搜索
CREATE VIRTUAL TABLE IF NOT EXISTS entries_fts USING fts5(
                                                             project_id UNINDEXED,
                                                             title,
                                                             content,
                                                             content=entries,
                                                             content_rowid=rowid,
                                                             tokenize='trigram'
);

CREATE TRIGGER IF NOT EXISTS entries_fts_insert AFTER INSERT ON entries BEGIN
    INSERT INTO entries_fts(rowid, project_id, title, content)
    VALUES (new.rowid, new.project_id, new.title, new.content);
END;

CREATE TRIGGER IF NOT EXISTS entries_fts_update AFTER UPDATE ON entries BEGIN
    INSERT INTO entries_fts(entries_fts, rowid, project_id, title, content)
    VALUES ('delete', old.rowid, old.project_id, old.title, old.content);
    INSERT INTO entries_fts(rowid, project_id, title, content)
    VALUES (new.rowid, new.project_id, new.title, new.content);
END;

CREATE TRIGGER IF NOT EXISTS entries_fts_delete AFTER DELETE ON entries BEGIN
    INSERT INTO entries_fts(entries_fts, rowid, project_id, title, content)
    VALUES ('delete', old.rowid, old.project_id, old.title, old.content);
END;