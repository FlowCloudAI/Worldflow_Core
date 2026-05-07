-- 将 summary 纳入 SQLite FTS，保持与 PostgreSQL 搜索字段一致。
DROP TRIGGER IF EXISTS entries_fts_insert;
DROP TRIGGER IF EXISTS entries_fts_update;
DROP TRIGGER IF EXISTS entries_fts_delete;

DROP TABLE IF EXISTS entries_fts;

CREATE VIRTUAL TABLE IF NOT EXISTS entries_fts USING fts5(
    title,
    summary,
    content,
    project_id UNINDEXED,
    tokenize='trigram'
);

INSERT INTO entries_fts(rowid, title, summary, content, project_id)
SELECT _rowid, title, summary, content, project_id
FROM entries;

CREATE TRIGGER IF NOT EXISTS entries_fts_insert AFTER INSERT ON entries BEGIN
    INSERT INTO entries_fts(rowid, title, summary, content, project_id)
    VALUES (new._rowid, new.title, new.summary, new.content, new.project_id);
END;

CREATE TRIGGER IF NOT EXISTS entries_fts_update AFTER UPDATE ON entries BEGIN
    DELETE FROM entries_fts WHERE rowid = old._rowid;
    INSERT INTO entries_fts(rowid, title, summary, content, project_id)
    VALUES (new._rowid, new.title, new.summary, new.content, new.project_id);
END;

CREATE TRIGGER IF NOT EXISTS entries_fts_delete AFTER DELETE ON entries BEGIN
    DELETE FROM entries_fts WHERE rowid = old._rowid;
END;
