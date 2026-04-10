// worldflow_core/src/models/tag_schema.rs
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 标签定义
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TagSchema {
    /// 标签ID
    pub id: Uuid,

    /// 项目ID
    pub project_id: Uuid,

    /// 标签名称
    pub name: String,

    /// 标签描述
    pub description: Option<String>,

    /// 标签类型 (number | string | boolean)
    pub r#type: String,          // "number" | "string" | "boolean"

    /// 标签目标对象
    pub target: String,          // JSON数组原文，如 ["character", "item"]

    /// 默认值
    pub default_val: Option<String>,

    /// 范围最小值
    pub range_min: Option<f64>,

    /// 范围最大值
    pub range_max: Option<f64>,

    /// 排序
    pub sort_order: i64,

    /// 创建时间
    pub created_at: String,

    /// 更新时间
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTagSchema {
    pub project_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub r#type: String,
    pub target: Vec<String>,
    pub default_val: Option<String>,
    pub range_min: Option<f64>,
    pub range_max: Option<f64>,
    pub sort_order: Option<i64>,
}
