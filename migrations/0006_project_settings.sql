-- 项目级键值设置表：存放与具体实体无关的项目视图/偏好配置（如关系图谱布局参数）。
-- 采用通用 KV 形态，后续其他视图偏好可直接复用，无需每次新增迁移。
CREATE TABLE IF NOT EXISTS project_settings
(
    project_id BLOB NOT NULL REFERENCES projects (id) ON DELETE CASCADE
        CHECK (length(project_id) = 16),
    key        TEXT NOT NULL,
    value      TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (project_id, key)
);

-- 自动更新 updated_at
CREATE TRIGGER IF NOT EXISTS project_settings_updated_at
    AFTER UPDATE
    ON project_settings
BEGIN
    UPDATE project_settings
    SET updated_at = datetime('now')
    WHERE project_id = NEW.project_id
      AND key = NEW.key;
END;
