use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use std::path::PathBuf;
use uuid::Uuid;

use super::{EntryLink, EntryRelation, RelationDirection};

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
    /// 词条封面展示图路径。None 时兼容旧逻辑，从 images 中的 is_cover 图片推导。
    pub cover_path: Option<String>,
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
    /// None = 不更新；Some(None) = 清空；Some(Some(s)) = 更新
    pub summary: Option<Option<String>>,
    pub content: Option<String>,
    pub r#type: Option<Option<String>>,
    pub tags: Option<Vec<EntryTag>>,
    pub images: Option<Vec<FCImage>>,
    /// None = 不主动设置；Some(None) = 清空；Some(Some(s)) = 更新为指定展示图路径。
    /// 当该字段为 None 且 images 为 Some 时，兼容旧逻辑，从 images 中的 is_cover 图片推导。
    pub cover_path: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveEntryRelationPatch {
    pub id: Option<Uuid>,
    pub a_id: Uuid,
    pub b_id: Uuid,
    pub relation: RelationDirection,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveEntryLinkTarget {
    pub entry_id: Option<Uuid>,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveEntryBundle {
    pub project_id: Uuid,
    pub entry_id: Uuid,
    pub category_id: Option<Uuid>,
    pub title: String,
    pub summary: Option<String>,
    pub content: String,
    pub r#type: Option<String>,
    pub tags: Option<Vec<EntryTag>>,
    pub images: Option<Vec<FCImage>>,
    pub cover_path: Option<Option<String>>,
    pub outgoing_link_targets: Vec<SaveEntryLinkTarget>,
    pub relation_patches: Vec<SaveEntryRelationPatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveEntryBundleResult {
    pub entry: Entry,
    pub outgoing_links: Vec<EntryLink>,
    pub incoming_links: Vec<EntryLink>,
    pub relations: Vec<EntryRelation>,
}
