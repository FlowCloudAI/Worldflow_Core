PRAGMA foreign_keys = OFF;

DROP TRIGGER IF EXISTS entries_category_same_project_insert;
DROP TRIGGER IF EXISTS entries_category_same_project_update_cat;
DROP TRIGGER IF EXISTS entries_category_same_project_update_proj;
DROP TRIGGER IF EXISTS relations_same_project_insert;
DROP TRIGGER IF EXISTS relations_two_way_order_insert;
DROP TRIGGER IF EXISTS relations_two_way_order_update;
DROP TRIGGER IF EXISTS projects_updated_at;
DROP TRIGGER IF EXISTS categories_updated_at;
DROP TRIGGER IF EXISTS tag_schemas_updated_at;
DROP TRIGGER IF EXISTS entries_updated_at;
DROP TRIGGER IF EXISTS entry_relations_updated_at;
DROP TRIGGER IF EXISTS entry_types_updated_at;
DROP TRIGGER IF EXISTS entries_fts_insert;
DROP TRIGGER IF EXISTS entries_fts_update;
DROP TRIGGER IF EXISTS entries_fts_delete;

DROP TABLE IF EXISTS entries_fts;

ALTER TABLE projects RENAME TO projects_old;
ALTER TABLE categories RENAME TO categories_old;
ALTER TABLE tag_schemas RENAME TO tag_schemas_old;
ALTER TABLE entries RENAME TO entries_old;
ALTER TABLE entry_relations RENAME TO entry_relations_old;
ALTER TABLE entry_types RENAME TO entry_types_old;

CREATE TABLE projects (
    id          BLOB PRIMARY KEY CHECK(length(id) = 16),
    name        TEXT NOT NULL,
    description TEXT,
    cover_image TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE categories (
    id          BLOB PRIMARY KEY CHECK(length(id) = 16),
    project_id  BLOB NOT NULL REFERENCES projects(id) ON DELETE CASCADE CHECK(length(project_id) = 16),
    parent_id   BLOB CHECK(parent_id IS NULL OR length(parent_id) = 16),
    name        TEXT NOT NULL,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
    CHECK (id != parent_id),
    UNIQUE (project_id, id),
    FOREIGN KEY (project_id, parent_id) REFERENCES categories(project_id, id) ON DELETE CASCADE
);

CREATE TABLE tag_schemas (
    id          BLOB PRIMARY KEY CHECK(length(id) = 16),
    project_id  BLOB NOT NULL REFERENCES projects(id) ON DELETE CASCADE CHECK(length(project_id) = 16),
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

CREATE TABLE entries (
    id          BLOB PRIMARY KEY CHECK(length(id) = 16),
    project_id  BLOB NOT NULL REFERENCES projects(id) ON DELETE CASCADE CHECK(length(project_id) = 16),
    category_id BLOB REFERENCES categories(id) ON DELETE SET NULL CHECK(category_id IS NULL OR length(category_id) = 16),
    title       TEXT NOT NULL,
    summary     TEXT,
    content     TEXT NOT NULL DEFAULT '',
    type        TEXT,
    tags        TEXT NOT NULL DEFAULT '[]',
    images      TEXT NOT NULL DEFAULT '[]',
    cover_path  TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE entry_relations (
    id         BLOB PRIMARY KEY CHECK(length(id) = 16),
    project_id BLOB NOT NULL REFERENCES projects(id) ON DELETE CASCADE CHECK(length(project_id) = 16),
    a_id       BLOB NOT NULL REFERENCES entries(id) ON DELETE CASCADE CHECK(length(a_id) = 16),
    b_id       BLOB NOT NULL REFERENCES entries(id) ON DELETE CASCADE CHECK(length(b_id) = 16),
    relation   TEXT NOT NULL CHECK(relation IN ('one_way', 'two_way')),
    content    TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (a_id, b_id, content),
    CHECK (a_id != b_id)
);

CREATE TABLE entry_types (
    id          BLOB PRIMARY KEY CHECK(length(id) = 16),
    project_id  BLOB NOT NULL REFERENCES projects(id) ON DELETE CASCADE CHECK(length(project_id) = 16),
    name        TEXT NOT NULL,
    description TEXT,
    icon        TEXT,
    color       TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (project_id, name),
    CHECK (name != '')
);

INSERT INTO projects (id, name, description, cover_image, created_at, updated_at)
SELECT unhex(replace(id, '-', '')), name, description, cover_image, created_at, updated_at
FROM projects_old;

INSERT INTO categories (id, project_id, parent_id, name, sort_order, created_at, updated_at)
SELECT
    unhex(replace(id, '-', '')),
    unhex(replace(project_id, '-', '')),
    CASE WHEN parent_id IS NULL THEN NULL ELSE unhex(replace(parent_id, '-', '')) END,
    name,
    sort_order,
    created_at,
    updated_at
FROM categories_old;

INSERT INTO tag_schemas (id, project_id, name, description, type, target, default_val, range_min, range_max, sort_order, created_at, updated_at)
SELECT
    unhex(replace(id, '-', '')),
    unhex(replace(project_id, '-', '')),
    name,
    description,
    type,
    target,
    default_val,
    range_min,
    range_max,
    sort_order,
    created_at,
    updated_at
FROM tag_schemas_old;

INSERT INTO entries (id, project_id, category_id, title, summary, content, type, tags, images, cover_path, created_at, updated_at)
SELECT
    unhex(replace(id, '-', '')),
    unhex(replace(project_id, '-', '')),
    CASE WHEN category_id IS NULL THEN NULL ELSE unhex(replace(category_id, '-', '')) END,
    title,
    summary,
    content,
    type,
    tags,
    images,
    cover_path,
    created_at,
    updated_at
FROM entries_old;

INSERT INTO entry_relations (id, project_id, a_id, b_id, relation, content, created_at, updated_at)
SELECT
    unhex(replace(id, '-', '')),
    unhex(replace(project_id, '-', '')),
    unhex(replace(a_id, '-', '')),
    unhex(replace(b_id, '-', '')),
    relation,
    content,
    created_at,
    updated_at
FROM entry_relations_old;

INSERT INTO entry_types (id, project_id, name, description, icon, color, created_at, updated_at)
SELECT
    unhex(replace(id, '-', '')),
    unhex(replace(project_id, '-', '')),
    name,
    description,
    icon,
    color,
    created_at,
    updated_at
FROM entry_types_old;

DROP TABLE entry_relations_old;
DROP TABLE entries_old;
DROP TABLE entry_types_old;
DROP TABLE tag_schemas_old;
DROP TABLE categories_old;
DROP TABLE projects_old;

CREATE TRIGGER entries_category_same_project_insert
    BEFORE INSERT ON entries
    WHEN NEW.category_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'category_id must belong to same project')
    WHERE (SELECT project_id FROM categories WHERE id = NEW.category_id) != NEW.project_id;
END;

CREATE TRIGGER entries_category_same_project_update_cat
    BEFORE UPDATE OF category_id ON entries
    WHEN NEW.category_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'category_id must belong to same project')
    WHERE (SELECT project_id FROM categories WHERE id = NEW.category_id) != NEW.project_id;
END;

CREATE TRIGGER entries_category_same_project_update_proj
    BEFORE UPDATE OF project_id ON entries
    WHEN NEW.category_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'category_id must belong to same project')
    WHERE (SELECT project_id FROM categories WHERE id = NEW.category_id) != NEW.project_id;
END;

CREATE TRIGGER relations_same_project_insert
    BEFORE INSERT ON entry_relations
BEGIN
    SELECT RAISE(ABORT, 'a_id must belong to same project')
    WHERE (SELECT project_id FROM entries WHERE id = NEW.a_id) != NEW.project_id;
    SELECT RAISE(ABORT, 'b_id must belong to same project')
    WHERE (SELECT project_id FROM entries WHERE id = NEW.b_id) != NEW.project_id;
END;

CREATE TRIGGER relations_two_way_order_insert
    BEFORE INSERT ON entry_relations
    WHEN NEW.relation = 'two_way' AND NEW.a_id > NEW.b_id
BEGIN
    SELECT RAISE(ABORT, 'two_way relation must have a_id < b_id');
END;

CREATE TRIGGER relations_two_way_order_update
    BEFORE UPDATE ON entry_relations
    WHEN NEW.relation = 'two_way' AND NEW.a_id > NEW.b_id
BEGIN
    SELECT RAISE(ABORT, 'two_way relation must have a_id < b_id');
END;

CREATE TRIGGER projects_updated_at
    AFTER UPDATE ON projects
BEGIN UPDATE projects SET updated_at = datetime('now') WHERE id = NEW.id; END;

CREATE TRIGGER categories_updated_at
    AFTER UPDATE ON categories
BEGIN UPDATE categories SET updated_at = datetime('now') WHERE id = NEW.id; END;

CREATE TRIGGER tag_schemas_updated_at
    AFTER UPDATE ON tag_schemas
BEGIN UPDATE tag_schemas SET updated_at = datetime('now') WHERE id = NEW.id; END;

CREATE TRIGGER entries_updated_at
    AFTER UPDATE ON entries
BEGIN UPDATE entries SET updated_at = datetime('now') WHERE id = NEW.id; END;

CREATE TRIGGER entry_relations_updated_at
    AFTER UPDATE ON entry_relations
BEGIN UPDATE entry_relations SET updated_at = datetime('now') WHERE id = NEW.id; END;

CREATE TRIGGER entry_types_updated_at
    AFTER UPDATE ON entry_types
BEGIN UPDATE entry_types SET updated_at = datetime('now') WHERE id = NEW.id; END;

CREATE INDEX idx_entries_project         ON entries(project_id);
CREATE INDEX idx_entries_category        ON entries(category_id);
CREATE INDEX idx_entries_type            ON entries(type);
CREATE INDEX idx_entries_project_updated ON entries(project_id, updated_at DESC);
CREATE INDEX idx_categories_project      ON categories(project_id);
CREATE INDEX idx_categories_parent       ON categories(parent_id);
CREATE INDEX idx_tag_schemas_project     ON tag_schemas(project_id);
CREATE INDEX idx_relations_a             ON entry_relations(a_id);
CREATE INDEX idx_relations_b             ON entry_relations(b_id);
CREATE INDEX idx_relations_project       ON entry_relations(project_id);
CREATE INDEX idx_entry_types_project     ON entry_types(project_id);
CREATE INDEX idx_entry_types_name        ON entry_types(project_id, name);

CREATE VIRTUAL TABLE entries_fts USING fts5(
    title,
    content,
    content=entries,
    content_rowid=rowid,
    tokenize='trigram'
);

CREATE TRIGGER entries_fts_insert AFTER INSERT ON entries BEGIN
    INSERT INTO entries_fts(rowid, title, content)
    VALUES (new.rowid, new.title, new.content);
END;

CREATE TRIGGER entries_fts_update AFTER UPDATE ON entries BEGIN
    INSERT INTO entries_fts(entries_fts, rowid, title, content)
    VALUES ('delete', old.rowid, old.title, old.content);
    INSERT INTO entries_fts(rowid, title, content)
    VALUES (new.rowid, new.title, new.content);
END;

CREATE TRIGGER entries_fts_delete AFTER DELETE ON entries BEGIN
    INSERT INTO entries_fts(entries_fts, rowid, title, content)
    VALUES ('delete', old.rowid, old.title, old.content);
END;

INSERT INTO entries_fts(rowid, title, content)
SELECT rowid, title, content FROM entries;

PRAGMA foreign_keys = ON;
