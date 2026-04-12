use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FCImage {
    pub path: PathBuf,
    pub is_cover: bool,          // 展示图标记
    pub caption: Option<String>, // 图注，后续 AI 可以用
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryTag {
    pub schema_id: Uuid,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: Uuid,
    pub project_id: Uuid,
    pub category_id: Option<Uuid>,
    pub title: String,
    pub summary: Option<String>, // 新增
    pub content: String,
    pub r#type: Option<String>,
    pub tags: Json<Vec<EntryTag>>,
    pub images: Json<Vec<FCImage>>,
    pub cover_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryBrief {
    pub id: Uuid,
    pub project_id: Uuid,
    pub category_id: Option<Uuid>,
    pub title: String,
    pub summary: Option<String>, // 新增
    pub r#type: Option<String>,
    pub cover: Option<PathBuf>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEntry {
    pub project_id: Uuid,
    pub category_id: Option<Uuid>,
    pub title: String,
    pub summary: Option<String>, // 新增
    pub content: Option<String>,
    pub r#type: Option<String>,
    pub tags: Option<Vec<EntryTag>>,
    pub images: Option<Vec<FCImage>>,
}

#[derive(Debug, Clone, Default)]
pub struct EntryFilter<'a> {
    pub category_id: Option<&'a Uuid>,
    pub entry_type: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateEntry {
    pub category_id: Option<Option<Uuid>>,
    pub title: Option<String>,
    pub summary: Option<String>, // 新增
    pub content: Option<String>,
    pub r#type: Option<Option<String>>,
    pub tags: Option<Vec<EntryTag>>,
    pub images: Option<Vec<FCImage>>,
}
