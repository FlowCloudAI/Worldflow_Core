use crate::db::{SQLITE_MIGRATOR, SqliteDb};
use crate::error::{Result, WorldflowError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqliteConnection, SqlitePool};
use std::collections::HashMap;
use uuid::Uuid;

// ═══════════════════════════════════════ 公开类型 ═══════════════════════════════════════════════

const TABLE_ORDER: [WorldflowCsvTable; 8] = [
    WorldflowCsvTable::Projects,
    WorldflowCsvTable::Categories,
    WorldflowCsvTable::TagSchemas,
    WorldflowCsvTable::EntryTypes,
    WorldflowCsvTable::Entries,
    WorldflowCsvTable::EntryRelations,
    WorldflowCsvTable::EntryLinks,
    WorldflowCsvTable::IdeaNotes,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorldflowCsvTable {
    Projects,
    Categories,
    TagSchemas,
    EntryTypes,
    Entries,
    EntryRelations,
    EntryLinks,
    IdeaNotes,
}

impl WorldflowCsvTable {
    pub fn ordered() -> &'static [WorldflowCsvTable] {
        &TABLE_ORDER
    }

    pub fn file_name(self) -> &'static str {
        match self {
            WorldflowCsvTable::Projects => "projects.csv",
            WorldflowCsvTable::Categories => "categories.csv",
            WorldflowCsvTable::TagSchemas => "tag_schemas.csv",
            WorldflowCsvTable::EntryTypes => "entry_types.csv",
            WorldflowCsvTable::Entries => "entries.csv",
            WorldflowCsvTable::EntryRelations => "entry_relations.csv",
            WorldflowCsvTable::EntryLinks => "entry_links.csv",
            WorldflowCsvTable::IdeaNotes => "idea_notes.csv",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CsvExportScope {
    All,
    Project { project_id: Uuid },
}

impl CsvExportScope {
    fn project_id(&self) -> Option<&Uuid> {
        match self {
            CsvExportScope::All => None,
            CsvExportScope::Project { project_id } => Some(project_id),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvExportItem {
    pub table: WorldflowCsvTable,
    pub file_name: String,
    pub row_count: usize,
    pub content: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectCsvExport {
    pub project_id: Uuid,
    pub schema_version: u32,
    pub items: Vec<CsvExportItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvImportItem {
    pub table: WorldflowCsvTable,
    pub file_name: String,
    pub content: String,
}

impl From<CsvExportItem> for CsvImportItem {
    fn from(item: CsvExportItem) -> Self {
        Self {
            table: item.table,
            file_name: item.file_name,
            content: item.content,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvImportBundle {
    pub items: Vec<CsvImportItem>,
}

impl CsvImportBundle {
    pub fn from_export_items(items: Vec<CsvExportItem>) -> Self {
        Self {
            items: items.into_iter().map(CsvImportItem::from).collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CsvImportMode {
    Replace,
    Merge,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsvImportResult {
    pub projects: usize,
    pub categories: usize,
    pub entries: usize,
    pub tag_schemas: usize,
    pub relations: usize,
    pub links: usize,
    pub entry_types: usize,
    pub idea_notes: usize,
}

// ═══════════════════════════════════════ CSV 行结构 ══════════════════════════════════════════════
// 所有字段均为 String。"" ↔ None 的转换在导入/导出中显式处理。

#[derive(Debug, Serialize, Deserialize)]
struct ProjectRow {
    id: String,
    name: String,
    description: String,
    cover_image: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CategoryRow {
    id: String,
    project_id: String,
    parent_id: String,
    name: String,
    sort_order: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TagSchemaRow {
    id: String,
    project_id: String,
    name: String,
    description: String,
    #[serde(rename = "type")]
    type_: String,
    target: String,
    default_val: String,
    range_min: String,
    range_max: String,
    sort_order: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct EntryRow {
    id: String,
    project_id: String,
    category_id: String,
    title: String,
    summary: String,
    content: String,
    #[serde(rename = "type")]
    type_: String,
    tags: String,
    images: String,
    cover_path: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct EntryRelationRow {
    id: String,
    project_id: String,
    a_id: String,
    b_id: String,
    relation: String,
    content: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct EntryLinkRow {
    id: String,
    project_id: String,
    a_id: String,
    b_id: String,
    created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct EntryTypeRow {
    id: String,
    project_id: String,
    name: String,
    description: String,
    icon: String,
    color: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct IdeaNoteRow {
    id: String,
    project_id: String,
    content: String,
    title: String,
    status: String,
    pinned: String,
    created_at: String,
    updated_at: String,
    last_reviewed_at: String,
    converted_entry_id: String,
}

#[derive(Debug)]
struct ParsedCsvData {
    projects: Vec<ProjectRow>,
    categories: Vec<CategoryRow>,
    tag_schemas: Vec<TagSchemaRow>,
    entry_types: Vec<EntryTypeRow>,
    entries: Vec<EntryRow>,
    entry_relations: Vec<EntryRelationRow>,
    entry_links: Vec<EntryLinkRow>,
    idea_notes: Vec<IdeaNoteRow>,
}

pub(in crate::db) struct AllCsvBytes {
    pub(in crate::db) projects: Vec<u8>,
    pub(in crate::db) categories: Vec<u8>,
    pub(in crate::db) tag_schemas: Vec<u8>,
    pub(in crate::db) entry_types: Vec<u8>,
    pub(in crate::db) entries: Vec<u8>,
    pub(in crate::db) entry_relations: Vec<u8>,
    pub(in crate::db) entry_links: Vec<u8>,
    pub(in crate::db) idea_notes: Vec<u8>,
}

// ═══════════════════════════════════════ 转换辅助 ═══════════════════════════════════════════════

fn encode_opt_str(s: Option<String>) -> Result<String> {
    serde_json::to_string(&s).map_err(WorldflowError::Serialization)
}

fn opt_str(s: &str) -> Result<Option<String>> {
    let trimmed = s.trim();
    if trimmed == "null" || trimmed.starts_with('"') {
        return serde_json::from_str::<Option<String>>(trimmed)
            .map_err(WorldflowError::Serialization);
    }
    if s.is_empty() {
        Ok(None)
    } else {
        Ok(Some(s.to_owned()))
    }
}

fn parse_uuid(s: &str) -> Result<Uuid> {
    Uuid::parse_str(s).map_err(|e| WorldflowError::InvalidInput(format!("invalid UUID '{s}': {e}")))
}

fn parse_opt_uuid(s: &str) -> Result<Option<Uuid>> {
    if s.is_empty() {
        return Ok(None);
    }
    parse_uuid(s).map(Some)
}

fn parse_i64(s: &str) -> Result<i64> {
    s.parse()
        .map_err(|e| WorldflowError::InvalidInput(format!("invalid number '{s}': {e}")))
}

fn parse_opt_f64(s: &str) -> Result<Option<f64>> {
    if s.is_empty() {
        return Ok(None);
    }
    s.parse::<f64>()
        .map(Some)
        .map_err(|e| WorldflowError::InvalidInput(format!("invalid float '{s}': {e}")))
}

// 父节点必须先于子节点出现；循环守卫可优雅处理损坏数据。
fn sort_categories_topological(rows: Vec<CategoryRow>) -> Vec<CategoryRow> {
    let mut sorted: Vec<CategoryRow> = Vec::with_capacity(rows.len());
    let mut remaining = rows;
    loop {
        if remaining.is_empty() {
            break;
        }
        let sorted_ids: std::collections::HashSet<&str> =
            sorted.iter().map(|r| r.id.as_str()).collect();
        let (can_add, rest): (Vec<_>, Vec<_>) = remaining
            .into_iter()
            .partition(|r| r.parent_id.is_empty() || sorted_ids.contains(r.parent_id.as_str()));
        if can_add.is_empty() {
            sorted.extend(rest);
            break;
        }
        sorted.extend(can_add);
        remaining = rest;
    }
    sorted
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

fn write_csv_string<T: Serialize>(rows: &[T]) -> Result<String> {
    let mut wtr = csv::Writer::from_writer(Vec::<u8>::new());
    for row in rows {
        wtr.serialize(row)?;
    }
    wtr.flush()?;
    let bytes = wtr
        .into_inner()
        .map_err(|e| WorldflowError::InvalidInput(format!("csv finalize failed: {e}")))?;
    String::from_utf8(bytes)
        .map_err(|e| WorldflowError::InvalidInput(format!("csv is not utf-8: {e}")))
}

fn parse_csv_bytes<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<Vec<T>> {
    let mut rdr = csv::Reader::from_reader(bytes);
    let rows: std::result::Result<Vec<T>, csv::Error> = rdr.deserialize().collect();
    Ok(rows?)
}

fn build_export_item<T: Serialize>(
    table: WorldflowCsvTable,
    rows: &[T],
) -> Result<CsvExportItem> {
    let content = write_csv_string(rows)?;
    Ok(CsvExportItem {
        table,
        file_name: table.file_name().to_owned(),
        row_count: rows.len(),
        sha256: sha256_hex(content.as_bytes()),
        content,
    })
}

fn schema_version_from_migrator() -> u32 {
    SQLITE_MIGRATOR
        .migrations
        .iter()
        .filter_map(|migration| u32::try_from(migration.version).ok())
        .max()
        .unwrap_or(0)
}

// ═══════════════════════════════════════ 导出 ═══════════════════════════════════════════════════

async fn ensure_project_exists(pool: &SqlitePool, project_id: &Uuid) -> Result<()> {
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(1) FROM projects WHERE id = ?")
        .bind(project_id)
        .fetch_one(pool)
        .await?;
    if exists == 0 {
        return Err(WorldflowError::NotFound(format!("project {project_id}")));
    }
    Ok(())
}

async fn fetch_project_rows(pool: &SqlitePool, scope: &CsvExportScope) -> Result<Vec<ProjectRow>> {
    let rows = match scope.project_id() {
        Some(project_id) => {
            sqlx::query(
                "SELECT id, name, description, cover_image, created_at, updated_at
                 FROM projects WHERE id = ? ORDER BY created_at",
            )
            .bind(project_id)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query(
                "SELECT id, name, description, cover_image, created_at, updated_at
                 FROM projects ORDER BY created_at",
            )
            .fetch_all(pool)
            .await?
        }
    };

    let mut csv_rows = Vec::with_capacity(rows.len());
    for row in &rows {
        csv_rows.push(ProjectRow {
            id: row.try_get::<Uuid, _>("id")?.to_string(),
            name: row.try_get("name")?,
            description: encode_opt_str(row.try_get::<Option<String>, _>("description")?)?,
            cover_image: encode_opt_str(row.try_get::<Option<String>, _>("cover_image")?)?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        });
    }
    Ok(csv_rows)
}

async fn fetch_category_rows(
    pool: &SqlitePool,
    scope: &CsvExportScope,
) -> Result<Vec<CategoryRow>> {
    let rows = match scope.project_id() {
        Some(project_id) => {
            sqlx::query(
                "SELECT id, project_id, parent_id, name, sort_order, created_at, updated_at
                 FROM categories WHERE project_id = ? ORDER BY created_at",
            )
            .bind(project_id)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query(
                "SELECT id, project_id, parent_id, name, sort_order, created_at, updated_at
                 FROM categories ORDER BY created_at",
            )
            .fetch_all(pool)
            .await?
        }
    };

    let mut csv_rows = Vec::with_capacity(rows.len());
    for row in &rows {
        csv_rows.push(CategoryRow {
            id: row.try_get::<Uuid, _>("id")?.to_string(),
            project_id: row.try_get::<Uuid, _>("project_id")?.to_string(),
            parent_id: row
                .try_get::<Option<Uuid>, _>("parent_id")?
                .map(|u| u.to_string())
                .unwrap_or_default(),
            name: row.try_get("name")?,
            sort_order: row.try_get::<i64, _>("sort_order")?.to_string(),
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        });
    }
    Ok(csv_rows)
}

async fn fetch_tag_schema_rows(
    pool: &SqlitePool,
    scope: &CsvExportScope,
) -> Result<Vec<TagSchemaRow>> {
    let rows = match scope.project_id() {
        Some(project_id) => {
            sqlx::query(
                "SELECT id, project_id, name, description, type, target, default_val,
                        range_min, range_max, sort_order, created_at, updated_at
                 FROM tag_schemas WHERE project_id = ? ORDER BY created_at",
            )
            .bind(project_id)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query(
                "SELECT id, project_id, name, description, type, target, default_val,
                        range_min, range_max, sort_order, created_at, updated_at
                 FROM tag_schemas ORDER BY created_at",
            )
            .fetch_all(pool)
            .await?
        }
    };

    let mut csv_rows = Vec::with_capacity(rows.len());
    for row in &rows {
        csv_rows.push(TagSchemaRow {
            id: row.try_get::<Uuid, _>("id")?.to_string(),
            project_id: row.try_get::<Uuid, _>("project_id")?.to_string(),
            name: row.try_get("name")?,
            description: encode_opt_str(row.try_get::<Option<String>, _>("description")?)?,
            type_: row.try_get("type")?,
            target: row.try_get("target")?,
            default_val: encode_opt_str(row.try_get::<Option<String>, _>("default_val")?)?,
            range_min: row
                .try_get::<Option<f64>, _>("range_min")?
                .map(|v| v.to_string())
                .unwrap_or_default(),
            range_max: row
                .try_get::<Option<f64>, _>("range_max")?
                .map(|v| v.to_string())
                .unwrap_or_default(),
            sort_order: row.try_get::<i64, _>("sort_order")?.to_string(),
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        });
    }
    Ok(csv_rows)
}

async fn fetch_entry_type_rows(
    pool: &SqlitePool,
    scope: &CsvExportScope,
) -> Result<Vec<EntryTypeRow>> {
    let rows = match scope.project_id() {
        Some(project_id) => {
            sqlx::query(
                "SELECT id, project_id, name, description, icon, color, created_at, updated_at
                 FROM entry_types WHERE project_id = ? ORDER BY created_at",
            )
            .bind(project_id)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query(
                "SELECT id, project_id, name, description, icon, color, created_at, updated_at
                 FROM entry_types ORDER BY created_at",
            )
            .fetch_all(pool)
            .await?
        }
    };

    let mut csv_rows = Vec::with_capacity(rows.len());
    for row in &rows {
        csv_rows.push(EntryTypeRow {
            id: row.try_get::<Uuid, _>("id")?.to_string(),
            project_id: row.try_get::<Uuid, _>("project_id")?.to_string(),
            name: row.try_get("name")?,
            description: encode_opt_str(row.try_get::<Option<String>, _>("description")?)?,
            icon: encode_opt_str(row.try_get::<Option<String>, _>("icon")?)?,
            color: encode_opt_str(row.try_get::<Option<String>, _>("color")?)?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        });
    }
    Ok(csv_rows)
}

async fn fetch_entry_rows(pool: &SqlitePool, scope: &CsvExportScope) -> Result<Vec<EntryRow>> {
    let rows = match scope.project_id() {
        Some(project_id) => {
            sqlx::query(
                "SELECT id, project_id, category_id, title, summary, content, type,
                        tags, images, cover_path, created_at, updated_at
                 FROM entries WHERE project_id = ? ORDER BY created_at",
            )
            .bind(project_id)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query(
                "SELECT id, project_id, category_id, title, summary, content, type,
                        tags, images, cover_path, created_at, updated_at
                 FROM entries ORDER BY created_at",
            )
            .fetch_all(pool)
            .await?
        }
    };

    let mut csv_rows = Vec::with_capacity(rows.len());
    for row in &rows {
        csv_rows.push(EntryRow {
            id: row.try_get::<Uuid, _>("id")?.to_string(),
            project_id: row.try_get::<Uuid, _>("project_id")?.to_string(),
            category_id: row
                .try_get::<Option<Uuid>, _>("category_id")?
                .map(|u| u.to_string())
                .unwrap_or_default(),
            title: row.try_get("title")?,
            summary: encode_opt_str(row.try_get::<Option<String>, _>("summary")?)?,
            content: row.try_get("content")?,
            type_: encode_opt_str(row.try_get::<Option<String>, _>("type")?)?,
            tags: row.try_get("tags")?,
            images: row.try_get("images")?,
            cover_path: encode_opt_str(row.try_get::<Option<String>, _>("cover_path")?)?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        });
    }
    Ok(csv_rows)
}

async fn fetch_entry_relation_rows(
    pool: &SqlitePool,
    scope: &CsvExportScope,
) -> Result<Vec<EntryRelationRow>> {
    let rows = match scope.project_id() {
        Some(project_id) => {
            sqlx::query(
                "SELECT id, project_id, a_id, b_id, relation, content, created_at, updated_at
                 FROM entry_relations WHERE project_id = ? ORDER BY created_at",
            )
            .bind(project_id)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query(
                "SELECT id, project_id, a_id, b_id, relation, content, created_at, updated_at
                 FROM entry_relations ORDER BY created_at",
            )
            .fetch_all(pool)
            .await?
        }
    };

    let mut csv_rows = Vec::with_capacity(rows.len());
    for row in &rows {
        csv_rows.push(EntryRelationRow {
            id: row.try_get::<Uuid, _>("id")?.to_string(),
            project_id: row.try_get::<Uuid, _>("project_id")?.to_string(),
            a_id: row.try_get::<Uuid, _>("a_id")?.to_string(),
            b_id: row.try_get::<Uuid, _>("b_id")?.to_string(),
            relation: row.try_get("relation")?,
            content: row.try_get("content")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        });
    }
    Ok(csv_rows)
}

async fn fetch_entry_link_rows(
    pool: &SqlitePool,
    scope: &CsvExportScope,
) -> Result<Vec<EntryLinkRow>> {
    let rows = match scope.project_id() {
        Some(project_id) => {
            sqlx::query(
                "SELECT id, project_id, a_id, b_id, created_at
                 FROM entry_links WHERE project_id = ? ORDER BY created_at",
            )
            .bind(project_id)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query(
                "SELECT id, project_id, a_id, b_id, created_at
                 FROM entry_links ORDER BY created_at",
            )
            .fetch_all(pool)
            .await?
        }
    };

    let mut csv_rows = Vec::with_capacity(rows.len());
    for row in &rows {
        csv_rows.push(EntryLinkRow {
            id: row.try_get::<Uuid, _>("id")?.to_string(),
            project_id: row.try_get::<Uuid, _>("project_id")?.to_string(),
            a_id: row.try_get::<Uuid, _>("a_id")?.to_string(),
            b_id: row.try_get::<Uuid, _>("b_id")?.to_string(),
            created_at: row.try_get("created_at")?,
        });
    }
    Ok(csv_rows)
}

async fn fetch_idea_note_rows(
    pool: &SqlitePool,
    scope: &CsvExportScope,
) -> Result<Vec<IdeaNoteRow>> {
    let rows = match scope.project_id() {
        Some(project_id) => {
            sqlx::query(
                "SELECT id, project_id, content, title, status, pinned,
                        created_at, updated_at, last_reviewed_at, converted_entry_id
                 FROM idea_notes WHERE project_id = ? ORDER BY created_at",
            )
            .bind(project_id)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query(
                "SELECT id, project_id, content, title, status, pinned,
                        created_at, updated_at, last_reviewed_at, converted_entry_id
                 FROM idea_notes ORDER BY created_at",
            )
            .fetch_all(pool)
            .await?
        }
    };

    let mut csv_rows = Vec::with_capacity(rows.len());
    for row in &rows {
        csv_rows.push(IdeaNoteRow {
            id: row.try_get::<Uuid, _>("id")?.to_string(),
            project_id: row
                .try_get::<Option<Uuid>, _>("project_id")?
                .map(|u| u.to_string())
                .unwrap_or_default(),
            content: row.try_get("content")?,
            title: encode_opt_str(row.try_get::<Option<String>, _>("title")?)?,
            status: row.try_get("status")?,
            pinned: row.try_get::<i64, _>("pinned")?.to_string(),
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
            last_reviewed_at: encode_opt_str(
                row.try_get::<Option<String>, _>("last_reviewed_at")?,
            )?,
            converted_entry_id: row
                .try_get::<Option<Uuid>, _>("converted_entry_id")?
                .map(|u| u.to_string())
                .unwrap_or_default(),
        });
    }
    Ok(csv_rows)
}

pub(in crate::db) async fn export_csv_item(
    pool: &SqlitePool,
    table: WorldflowCsvTable,
    scope: &CsvExportScope,
) -> Result<CsvExportItem> {
    match table {
        WorldflowCsvTable::Projects => {
            build_export_item(table, &fetch_project_rows(pool, scope).await?)
        }
        WorldflowCsvTable::Categories => {
            build_export_item(table, &fetch_category_rows(pool, scope).await?)
        }
        WorldflowCsvTable::TagSchemas => {
            build_export_item(table, &fetch_tag_schema_rows(pool, scope).await?)
        }
        WorldflowCsvTable::EntryTypes => {
            build_export_item(table, &fetch_entry_type_rows(pool, scope).await?)
        }
        WorldflowCsvTable::Entries => {
            build_export_item(table, &fetch_entry_rows(pool, scope).await?)
        }
        WorldflowCsvTable::EntryRelations => {
            build_export_item(table, &fetch_entry_relation_rows(pool, scope).await?)
        }
        WorldflowCsvTable::EntryLinks => {
            build_export_item(table, &fetch_entry_link_rows(pool, scope).await?)
        }
        WorldflowCsvTable::IdeaNotes => {
            build_export_item(table, &fetch_idea_note_rows(pool, scope).await?)
        }
    }
}

async fn export_csv_items(
    pool: &SqlitePool,
    scope: &CsvExportScope,
) -> Result<Vec<CsvExportItem>> {
    let mut items = Vec::with_capacity(TABLE_ORDER.len());
    for table in TABLE_ORDER {
        items.push(export_csv_item(pool, table, scope).await?);
    }
    Ok(items)
}

#[cfg(feature = "snapshot")]
pub(in crate::db) async fn export_all_to_dir(
    pool: &SqlitePool,
    dir: &std::path::Path,
) -> Result<()> {
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(WorldflowError::Io)?;
    for item in export_csv_items(pool, &CsvExportScope::All).await? {
        tokio::fs::write(dir.join(&item.file_name), item.content)
            .await
            .map_err(WorldflowError::Io)?;
    }
    Ok(())
}

// ═══════════════════════════════════════ 导入 ═══════════════════════════════════════════════════

fn parse_all_csv(bytes: &AllCsvBytes) -> Result<ParsedCsvData> {
    Ok(ParsedCsvData {
        projects: parse_csv_bytes(&bytes.projects)?,
        categories: parse_csv_bytes(&bytes.categories)?,
        tag_schemas: parse_csv_bytes(&bytes.tag_schemas)?,
        entry_types: parse_csv_bytes(&bytes.entry_types)?,
        entries: parse_csv_bytes(&bytes.entries)?,
        entry_relations: parse_csv_bytes(&bytes.entry_relations)?,
        entry_links: parse_csv_bytes(&bytes.entry_links)?,
        idea_notes: parse_csv_bytes(&bytes.idea_notes)?,
    })
}

fn all_csv_bytes_from_bundle(bundle: CsvImportBundle) -> Result<AllCsvBytes> {
    let mut by_table = HashMap::<WorldflowCsvTable, Vec<u8>>::new();
    for item in bundle.items {
        if by_table
            .insert(item.table, item.content.into_bytes())
            .is_some()
        {
            return Err(WorldflowError::InvalidInput(format!(
                "重复的 CSV 表: {:?}",
                item.table
            )));
        }
    }

    let mut take = |table: WorldflowCsvTable| -> Result<Vec<u8>> {
        by_table
            .remove(&table)
            .ok_or_else(|| WorldflowError::InvalidInput(format!("缺少 CSV 表: {:?}", table)))
    };

    Ok(AllCsvBytes {
        projects: take(WorldflowCsvTable::Projects)?,
        categories: take(WorldflowCsvTable::Categories)?,
        tag_schemas: take(WorldflowCsvTable::TagSchemas)?,
        entry_types: take(WorldflowCsvTable::EntryTypes)?,
        entries: take(WorldflowCsvTable::Entries)?,
        entry_relations: take(WorldflowCsvTable::EntryRelations)?,
        entry_links: take(WorldflowCsvTable::EntryLinks)?,
        idea_notes: take(WorldflowCsvTable::IdeaNotes)?,
    })
}

#[cfg(feature = "snapshot")]
pub(in crate::db) fn read_all_csv_bytes_from_dir(dir: &std::path::Path) -> Result<AllCsvBytes> {
    let read_file = |table: WorldflowCsvTable| -> Result<Vec<u8>> {
        std::fs::read(dir.join(table.file_name())).map_err(WorldflowError::Io)
    };

    Ok(AllCsvBytes {
        projects: read_file(WorldflowCsvTable::Projects)?,
        categories: read_file(WorldflowCsvTable::Categories)?,
        tag_schemas: read_file(WorldflowCsvTable::TagSchemas)?,
        entry_types: read_file(WorldflowCsvTable::EntryTypes)?,
        entries: read_file(WorldflowCsvTable::Entries)?,
        entry_relations: read_file(WorldflowCsvTable::EntryRelations)?,
        entry_links: read_file(WorldflowCsvTable::EntryLinks)?,
        idea_notes: read_file(WorldflowCsvTable::IdeaNotes)?,
    })
}

pub(in crate::db) async fn import_all_bytes(
    pool: &SqlitePool,
    bytes: AllCsvBytes,
    mode: CsvImportMode,
) -> Result<CsvImportResult> {
    let parsed = parse_all_csv(&bytes)?;

    let mut conn = pool.acquire().await?;
    // SQLite 中外键约束必须在事务外切换。
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *conn)
        .await?;
    sqlx::query("BEGIN").execute(&mut *conn).await?;

    let result = do_apply(&mut conn, parsed, mode).await;

    match result {
        Ok(r) => {
            sqlx::query("COMMIT").execute(&mut *conn).await?;
            sqlx::query("PRAGMA foreign_keys = ON")
                .execute(&mut *conn)
                .await?;
            Ok(r)
        }
        Err(e) => {
            let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
            sqlx::query("PRAGMA foreign_keys = ON")
                .execute(&mut *conn)
                .await?;
            Err(e)
        }
    }
}

async fn do_apply(
    conn: &mut SqliteConnection,
    mut parsed: ParsedCsvData,
    mode: CsvImportMode,
) -> Result<CsvImportResult> {
    if matches!(mode, CsvImportMode::Replace) {
        for table in &[
            "idea_notes",
            "entry_links",
            "entry_relations",
            "entries",
            "entry_types",
            "tag_schemas",
            "categories",
            "projects",
        ] {
            sqlx::query(&format!("DELETE FROM {table}"))
                .execute(&mut *conn)
                .await?;
        }
    }

    parsed.categories = sort_categories_topological(parsed.categories);

    let mut r = CsvImportResult::default();
    r.projects = insert_projects(conn, &parsed.projects).await?;
    r.categories = insert_categories(conn, &parsed.categories).await?;
    r.tag_schemas = insert_tag_schemas(conn, &parsed.tag_schemas).await?;
    r.entry_types = insert_entry_types(conn, &parsed.entry_types).await?;
    r.entries = insert_entries(conn, &parsed.entries).await?;
    r.relations = insert_entry_relations(conn, &parsed.entry_relations).await?;
    r.links = insert_entry_links(conn, &parsed.entry_links).await?;
    r.idea_notes = insert_idea_notes(conn, &parsed.idea_notes).await?;
    Ok(r)
}

async fn insert_projects(conn: &mut SqliteConnection, rows: &[ProjectRow]) -> Result<usize> {
    let mut count = 0usize;
    for row in rows {
        let id = parse_uuid(&row.id)?;
        let n = sqlx::query(
            "INSERT OR IGNORE INTO projects
             (id, name, description, cover_image, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&row.name)
        .bind(opt_str(&row.description)?)
        .bind(opt_str(&row.cover_image)?)
        .bind(&row.created_at)
        .bind(&row.updated_at)
        .execute(&mut *conn)
        .await?
        .rows_affected();
        count += n as usize;
    }
    Ok(count)
}

async fn insert_categories(conn: &mut SqliteConnection, rows: &[CategoryRow]) -> Result<usize> {
    let mut count = 0usize;
    for row in rows {
        let id = parse_uuid(&row.id)?;
        let project_id = parse_uuid(&row.project_id)?;
        let parent_id = parse_opt_uuid(&row.parent_id)?;
        let sort_order = parse_i64(&row.sort_order)?;
        let n = sqlx::query(
            "INSERT OR IGNORE INTO categories
             (id, project_id, parent_id, name, sort_order, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&project_id)
        .bind(parent_id)
        .bind(&row.name)
        .bind(sort_order)
        .bind(&row.created_at)
        .bind(&row.updated_at)
        .execute(&mut *conn)
        .await?
        .rows_affected();
        count += n as usize;
    }
    Ok(count)
}

async fn insert_tag_schemas(conn: &mut SqliteConnection, rows: &[TagSchemaRow]) -> Result<usize> {
    let mut count = 0usize;
    for row in rows {
        let id = parse_uuid(&row.id)?;
        let project_id = parse_uuid(&row.project_id)?;
        let sort_order = parse_i64(&row.sort_order)?;
        let range_min = parse_opt_f64(&row.range_min)?;
        let range_max = parse_opt_f64(&row.range_max)?;
        let n = sqlx::query(
            "INSERT OR IGNORE INTO tag_schemas
             (id, project_id, name, description, type, target, default_val,
              range_min, range_max, sort_order, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&project_id)
        .bind(&row.name)
        .bind(opt_str(&row.description)?)
        .bind(&row.type_)
        .bind(&row.target)
        .bind(opt_str(&row.default_val)?)
        .bind(range_min)
        .bind(range_max)
        .bind(sort_order)
        .bind(&row.created_at)
        .bind(&row.updated_at)
        .execute(&mut *conn)
        .await?
        .rows_affected();
        count += n as usize;
    }
    Ok(count)
}

async fn insert_entry_types(conn: &mut SqliteConnection, rows: &[EntryTypeRow]) -> Result<usize> {
    let mut count = 0usize;
    for row in rows {
        let id = parse_uuid(&row.id)?;
        let project_id = parse_uuid(&row.project_id)?;
        let n = sqlx::query(
            "INSERT OR IGNORE INTO entry_types
             (id, project_id, name, description, icon, color, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&project_id)
        .bind(&row.name)
        .bind(opt_str(&row.description)?)
        .bind(opt_str(&row.icon)?)
        .bind(opt_str(&row.color)?)
        .bind(&row.created_at)
        .bind(&row.updated_at)
        .execute(&mut *conn)
        .await?
        .rows_affected();
        count += n as usize;
    }
    Ok(count)
}

async fn insert_entries(conn: &mut SqliteConnection, rows: &[EntryRow]) -> Result<usize> {
    let mut count = 0usize;
    for row in rows {
        let id = parse_uuid(&row.id)?;
        let project_id = parse_uuid(&row.project_id)?;
        let category_id = parse_opt_uuid(&row.category_id)?;
        let n = sqlx::query(
            "INSERT OR IGNORE INTO entries
             (id, project_id, category_id, title, summary, content, type,
              tags, images, cover_path, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&project_id)
        .bind(category_id)
        .bind(&row.title)
        .bind(opt_str(&row.summary)?)
        .bind(&row.content)
        .bind(opt_str(&row.type_)?)
        .bind(&row.tags)
        .bind(&row.images)
        .bind(opt_str(&row.cover_path)?)
        .bind(&row.created_at)
        .bind(&row.updated_at)
        .execute(&mut *conn)
        .await?
        .rows_affected();
        count += n as usize;
    }
    Ok(count)
}

async fn insert_entry_relations(
    conn: &mut SqliteConnection,
    rows: &[EntryRelationRow],
) -> Result<usize> {
    let mut count = 0usize;
    for row in rows {
        let id = parse_uuid(&row.id)?;
        let project_id = parse_uuid(&row.project_id)?;
        let a_id = parse_uuid(&row.a_id)?;
        let b_id = parse_uuid(&row.b_id)?;
        let n = sqlx::query(
            "INSERT OR IGNORE INTO entry_relations
             (id, project_id, a_id, b_id, relation, content, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&project_id)
        .bind(&a_id)
        .bind(&b_id)
        .bind(&row.relation)
        .bind(&row.content)
        .bind(&row.created_at)
        .bind(&row.updated_at)
        .execute(&mut *conn)
        .await?
        .rows_affected();
        count += n as usize;
    }
    Ok(count)
}

async fn insert_entry_links(conn: &mut SqliteConnection, rows: &[EntryLinkRow]) -> Result<usize> {
    let mut count = 0usize;
    for row in rows {
        let id = parse_uuid(&row.id)?;
        let project_id = parse_uuid(&row.project_id)?;
        let a_id = parse_uuid(&row.a_id)?;
        let b_id = parse_uuid(&row.b_id)?;
        let n = sqlx::query(
            "INSERT OR IGNORE INTO entry_links
             (id, project_id, a_id, b_id, created_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&project_id)
        .bind(&a_id)
        .bind(&b_id)
        .bind(&row.created_at)
        .execute(&mut *conn)
        .await?
        .rows_affected();
        count += n as usize;
    }
    Ok(count)
}

async fn insert_idea_notes(conn: &mut SqliteConnection, rows: &[IdeaNoteRow]) -> Result<usize> {
    let mut count = 0usize;
    for row in rows {
        let id = parse_uuid(&row.id)?;
        let project_id = parse_opt_uuid(&row.project_id)?;
        let converted_entry_id = parse_opt_uuid(&row.converted_entry_id)?;
        let pinned = parse_i64(&row.pinned)?;
        let n = sqlx::query(
            "INSERT OR IGNORE INTO idea_notes
             (id, project_id, content, title, status, pinned,
              created_at, updated_at, last_reviewed_at, converted_entry_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(project_id)
        .bind(&row.content)
        .bind(opt_str(&row.title)?)
        .bind(&row.status)
        .bind(pinned)
        .bind(&row.created_at)
        .bind(&row.updated_at)
        .bind(opt_str(&row.last_reviewed_at)?)
        .bind(converted_entry_id)
        .execute(&mut *conn)
        .await?
        .rows_affected();
        count += n as usize;
    }
    Ok(count)
}

// ═══════════════════════════════════════ SqliteDb 公开实现 ══════════════════════════════════════

impl SqliteDb {
    pub async fn export_project_csvs(&self, project_id: Uuid) -> Result<ProjectCsvExport> {
        ensure_project_exists(&self.pool, &project_id).await?;
        Ok(ProjectCsvExport {
            project_id,
            schema_version: self.worldflow_schema_version(),
            items: export_csv_items(&self.pool, &CsvExportScope::Project { project_id }).await?,
        })
    }

    pub async fn export_csv_table(
        &self,
        table: WorldflowCsvTable,
        scope: CsvExportScope,
    ) -> Result<CsvExportItem> {
        if let CsvExportScope::Project { project_id } = &scope {
            ensure_project_exists(&self.pool, project_id).await?;
        }
        export_csv_item(&self.pool, table, &scope).await
    }

    pub async fn export_all_csvs(&self) -> Result<Vec<CsvExportItem>> {
        export_csv_items(&self.pool, &CsvExportScope::All).await
    }

    pub async fn import_csvs(
        &self,
        bundle: CsvImportBundle,
        mode: CsvImportMode,
    ) -> Result<CsvImportResult> {
        import_all_bytes(&self.pool, all_csv_bytes_from_bundle(bundle)?, mode).await
    }

    pub fn worldflow_schema_version(&self) -> u32 {
        schema_version_from_migrator()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::traits::{EntryOps, IdeaNoteOps, ProjectOps};
    use crate::models::{CreateEntry, CreateIdeaNote, CreateProject, UpdateProject};
    use tempfile::TempDir;

    async fn new_test_db(prefix: &str) -> Result<(TempDir, SqliteDb)> {
        let temp = tempfile::tempdir()?;
        let db_path = temp.path().join(format!("{prefix}.db"));
        let database_url = format!(
            "sqlite:{}?mode=rwc",
            db_path.to_string_lossy().replace('\\', "/")
        );
        let db = SqliteDb::new(&database_url).await?;
        Ok((temp, db))
    }

    async fn seed_project(db: &SqliteDb, name: &str) -> Result<Uuid> {
        Ok(db
            .create_project(CreateProject {
                name: name.to_owned(),
                description: Some(format!("{name} 描述")),
                cover_image: None,
            })
            .await?
            .id)
    }

    async fn seed_entry(db: &SqliteDb, project_id: Uuid, title: &str) -> Result<Uuid> {
        Ok(db
            .create_entry(CreateEntry {
                project_id,
                category_id: None,
                title: title.to_owned(),
                summary: Some(format!("{title} 摘要")),
                content: Some(format!("{title} 正文")),
                r#type: Some("character".to_owned()),
                tags: None,
                images: None,
                cover_path: None,
            })
            .await?
            .id)
    }

    fn bundle_from_items(items: Vec<CsvExportItem>) -> CsvImportBundle {
        CsvImportBundle::from_export_items(items)
    }

    #[tokio::test]
    async fn project_export_filters_project_rows_and_keeps_order() -> Result<()> {
        let (_temp, db) = new_test_db("project_export_filters").await?;
        let project_a = seed_project(&db, "项目甲").await?;
        let project_b = seed_project(&db, "项目乙").await?;
        seed_entry(&db, project_a, "甲词条").await?;
        seed_entry(&db, project_b, "乙词条").await?;
        db.create_idea_note(CreateIdeaNote {
            project_id: Some(project_a),
            content: "甲便签".to_owned(),
            title: Some("甲标题".to_owned()),
            pinned: Some(false),
        })
        .await?;
        db.create_idea_note(CreateIdeaNote {
            project_id: None,
            content: "全局便签".to_owned(),
            title: None,
            pinned: Some(false),
        })
        .await?;

        let export = db.export_project_csvs(project_a).await?;
        assert_eq!(export.project_id, project_a);
        assert_eq!(export.schema_version, db.worldflow_schema_version());
        assert_eq!(
            export
                .items
                .iter()
                .map(|item| item.table)
                .collect::<Vec<_>>(),
            WorldflowCsvTable::ordered()
        );
        assert_eq!(
            export
                .items
                .iter()
                .map(|item| item.file_name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "projects.csv",
                "categories.csv",
                "tag_schemas.csv",
                "entry_types.csv",
                "entries.csv",
                "entry_relations.csv",
                "entry_links.csv",
                "idea_notes.csv",
            ]
        );

        let projects = export
            .items
            .iter()
            .find(|item| item.table == WorldflowCsvTable::Projects)
            .expect("projects item");
        assert_eq!(projects.row_count, 1);
        assert!(projects.content.contains("项目甲"));
        assert!(!projects.content.contains("项目乙"));
        assert_eq!(projects.sha256, sha256_hex(projects.content.as_bytes()));

        let entries = export
            .items
            .iter()
            .find(|item| item.table == WorldflowCsvTable::Entries)
            .expect("entries item");
        assert_eq!(entries.row_count, 1);
        assert!(entries.content.contains("甲词条"));
        assert!(!entries.content.contains("乙词条"));

        let notes = export
            .items
            .iter()
            .find(|item| item.table == WorldflowCsvTable::IdeaNotes)
            .expect("idea notes item");
        assert_eq!(notes.row_count, 1);
        assert!(notes.content.contains("甲便签"));
        assert!(!notes.content.contains("全局便签"));
        Ok(())
    }

    #[tokio::test]
    async fn import_csvs_merge_only_adds_missing_rows() -> Result<()> {
        let (_source_temp, source) = new_test_db("merge_source").await?;
        let project_id = seed_project(&source, "源项目").await?;
        seed_entry(&source, project_id, "源词条").await?;
        let bundle = bundle_from_items(source.export_all_csvs().await?);

        let (_target_temp, target) = new_test_db("merge_target").await?;
        let first = target.import_csvs(bundle.clone(), CsvImportMode::Merge).await?;
        assert_eq!(first.projects, 1);
        assert_eq!(first.entries, 1);

        target
            .update_project(
                &project_id,
                UpdateProject {
                    name: Some("本地改名".to_owned()),
                    description: None,
                    cover_image: None,
                },
            )
            .await?;
        let second = target.import_csvs(bundle, CsvImportMode::Merge).await?;
        assert_eq!(second.projects, 0);
        assert_eq!(second.entries, 0);
        assert_eq!(target.get_project(&project_id).await?.name, "本地改名");
        Ok(())
    }

    #[tokio::test]
    async fn import_csvs_replace_replaces_business_tables() -> Result<()> {
        let (_source_temp, source) = new_test_db("replace_source").await?;
        let source_project = seed_project(&source, "替换源项目").await?;
        seed_entry(&source, source_project, "替换源词条").await?;
        let bundle = bundle_from_items(source.export_all_csvs().await?);

        let (_target_temp, target) = new_test_db("replace_target").await?;
        seed_project(&target, "应被清空的项目").await?;
        let result = target.import_csvs(bundle, CsvImportMode::Replace).await?;
        assert_eq!(result.projects, 1);
        assert_eq!(result.entries, 1);

        let projects = target.list_projects().await?;
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].id, source_project);
        assert_eq!(projects[0].name, "替换源项目");
        Ok(())
    }
}
