-- API 用量统计表：记录每次 AI 调用的 token 消耗
CREATE TABLE IF NOT EXISTS api_usage_log
(
    id                 TEXT PRIMARY KEY,
    session_id         TEXT    NOT NULL,
    model              TEXT    NOT NULL,
    provider           TEXT    NOT NULL,
    modality           TEXT    NOT NULL CHECK (modality IN ('llm', 'image', 'tts')),
    prompt_tokens      INTEGER NOT NULL DEFAULT 0,
    completion_tokens  INTEGER NOT NULL DEFAULT 0,
    total_tokens       INTEGER NOT NULL DEFAULT 0,
    created_at         TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_api_usage_session ON api_usage_log (session_id);
CREATE INDEX IF NOT EXISTS idx_api_usage_model ON api_usage_log (model);
CREATE INDEX IF NOT EXISTS idx_api_usage_created ON api_usage_log (created_at);
CREATE INDEX IF NOT EXISTS idx_api_usage_provider ON api_usage_log (provider);
CREATE INDEX IF NOT EXISTS idx_api_usage_modality ON api_usage_log (modality);
