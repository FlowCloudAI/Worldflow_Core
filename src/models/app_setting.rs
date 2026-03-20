// worldflow_core/src/models/app_setting.rs
use serde::{Deserialize, Serialize};

/// 应用程序全局设置
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AppSetting {
    /// 键
    pub key: String,
    
    /// 值
    pub value: String,
    
    /// 更新时间
    pub updated_at: String,
}