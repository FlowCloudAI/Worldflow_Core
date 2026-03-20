use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorldflowError {
    #[error("数据库错误: {0}")]
    Database(#[from] sqlx::Error),

    #[error("迁移错误: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("记录不存在: {0}")]
    NotFound(String),

    #[error("序列化错误: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("参数错误: {0}")]
    InvalidInput(String),
}

pub type Result<T> = std::result::Result<T, WorldflowError>;