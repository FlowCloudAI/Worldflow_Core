use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use sqlx::types::Json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FCImage {
    pub path:     PathBuf,
    pub is_cover: bool,      // 展示图标记
    pub caption:  Option<String>,  // 图注，后续 AI 可以用
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryTag {
    pub schema_id: String,
    pub value:     serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id:          String,
    pub project_id:  String,
    pub category_id: Option<String>,
    pub title:       String,
    pub summary:     Option<String>,  // 新增
    pub content:     String,
    pub r#type:      Option<String>,
    pub tags:        Json<Vec<EntryTag>>,
    pub images:      Json<Vec<FCImage>>,
    pub cover_path:  Option<String>,
    pub created_at:  String,
    pub updated_at:  String,
}

// 列表用：不含 content / tags，减少反序列化开销

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryBrief {
    pub id:          String,
    pub project_id:  String,
    pub category_id: Option<String>,
    pub title:       String,
    pub summary:     Option<String>,  // 新增
    pub r#type:      Option<String>,
    pub cover:       Option<PathBuf>,
    pub updated_at:  String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEntry {
    pub project_id:  String,
    pub category_id: Option<String>,
    pub title:       String,
    pub summary:     Option<String>,  // 新增
    pub content:     Option<String>,
    pub r#type:      Option<String>,
    pub tags:        Option<Vec<EntryTag>>,
    pub images:      Option<Vec<FCImage>>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateEntry {
    pub category_id: Option<Option<String>>,
    pub title:       Option<String>,
    pub summary:     Option<String>,  // 新增
    pub content:     Option<String>,
    pub r#type:      Option<Option<String>>,
    pub tags:        Option<Vec<EntryTag>>,
    pub images:      Option<Vec<FCImage>>,
}