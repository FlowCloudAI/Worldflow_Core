// worldflow_core/src/models/project.rs
use serde::{Deserialize, Serialize};

/// 项目条目
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Project {
    /// 项目ID
    pub id: String,

    /// 项目名称(世界观名称)
    pub name: String,

    /// 项目描述
    pub description: Option<String>,

    /// 创建时间
    pub created_at: String,

    /// 更新时间
    pub updated_at: String,
}

/// 创建项目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProject {
    pub name: String,
    pub description: Option<String>,
}

/// 更新项目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProject {
    pub name: Option<String>,
    pub description: Option<String>,
}