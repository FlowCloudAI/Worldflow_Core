// worldflow_core/src/models/category.rs
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 词条树状分类节点
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Category {
    /// 分类ID
    pub id: Uuid,

    /// 所属项目ID
    pub project_id: Uuid,

    /// 父级分类ID
    pub parent_id: Option<Uuid>,

    /// 分类名称
    pub name: String,

    /// 排序
    pub sort_order: i64,

    /// 创建时间
    pub created_at: String,

    /// 更新时间
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCategory {
    pub project_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub sort_order: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCategory {
    pub parent_id: Option<Option<Uuid>>, // None = 不更新, Some(None) = 移到根节点
    pub name: Option<String>,
    pub sort_order: Option<i64>,
}
