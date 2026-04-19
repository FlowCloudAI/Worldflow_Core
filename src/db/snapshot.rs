use crate::error::{Result, WorldflowError};
use git2::{IndexAddOption, Repository, Signature, Sort};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqliteConnection, SqlitePool};
use std::path::{Path, PathBuf};
use tokio::sync::Mutex;
use uuid::Uuid;

// ═══════════════════════════════════════ Public types ════════════════════════════════════════════

#[derive(Debug)]
pub struct SnapshotConfig {
    pub dir: PathBuf,
    pub author_name: String,
    pub author_email: String,
}

#[derive(Debug, Clone)]
pub struct SnapshotInfo {
    pub id: String,
    pub message: String,
    pub timestamp: i64,
}

#[derive(Debug, Default)]
pub struct AppendResult {
    pub projects: usize,
    pub categories: usize,
    pub entries: usize,
    pub tag_schemas: usize,
    pub relations: usize,
    pub links: usize,
    pub entry_types: usize,
    pub idea_notes: usize,
}

pub enum RestoreMode {
    Replace,
    Merge,
}

// ═══════════════════════════════════════ Internal state ══════════════════════════════════════════

#[derive(Debug)]
pub(super) struct SnapshotState {
    pub config: SnapshotConfig,
    lock: Mutex<()>,
}

impl SnapshotState {
    pub fn new(config: SnapshotConfig) -> Self {
        Self {
            config,
            lock: Mutex::new(()),
        }
    }
}

// ═══════════════════════════════════════ CSV row structs ═════════════════════════════════════════
// All fields are String. "" ↔ None conversion is done explicitly in import/export.

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

// ═══════════════════════════════════════ Conversion helpers ══════════════════════════════════════

fn opt_str(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_owned())
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

// Parents must come before children; cycle guard handles corrupt data gracefully.
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

// ═══════════════════════════════════════ CSV I/O ═════════════════════════════════════════════════

fn write_csv_file<T: Serialize>(path: &Path, rows: &[T]) -> Result<()> {
    let mut wtr = csv::Writer::from_path(path)?;
    for row in rows {
        wtr.serialize(row)?;
    }
    wtr.flush()?;
    Ok(())
}

fn parse_csv_bytes<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<Vec<T>> {
    let mut rdr = csv::Reader::from_reader(bytes);
    let rows: std::result::Result<Vec<T>, csv::Error> = rdr.deserialize().collect();
    Ok(rows?)
}

// ═══════════════════════════════════════ Export ══════════════════════════════════════════════════

pub(super) async fn export_all(pool: &SqlitePool, dir: &Path) -> Result<()> {
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(WorldflowError::Io)?;
    export_projects(pool, dir).await?;
    export_categories(pool, dir).await?;
    export_tag_schemas(pool, dir).await?;
    export_entry_types(pool, dir).await?;
    export_entries(pool, dir).await?;
    export_entry_relations(pool, dir).await?;
    export_entry_links(pool, dir).await?;
    export_idea_notes(pool, dir).await?;
    Ok(())
}

async fn export_projects(pool: &SqlitePool, dir: &Path) -> Result<()> {
    let rows = sqlx::query(
        "SELECT id, name, description, cover_image, created_at, updated_at
         FROM projects ORDER BY created_at",
    )
    .fetch_all(pool)
    .await?;

    let mut csv_rows = Vec::with_capacity(rows.len());
    for row in &rows {
        csv_rows.push(ProjectRow {
            id: row.try_get::<Uuid, _>("id")?.to_string(),
            name: row.try_get("name")?,
            description: row
                .try_get::<Option<String>, _>("description")?
                .unwrap_or_default(),
            cover_image: row
                .try_get::<Option<String>, _>("cover_image")?
                .unwrap_or_default(),
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        });
    }

    let path = dir.join("projects.csv");
    tokio::task::spawn_blocking(move || write_csv_file(&path, &csv_rows))
        .await
        .map_err(|e| WorldflowError::InvalidInput(format!("csv write task failed: {e}")))?
}

async fn export_categories(pool: &SqlitePool, dir: &Path) -> Result<()> {
    let rows = sqlx::query(
        "SELECT id, project_id, parent_id, name, sort_order, created_at, updated_at
         FROM categories ORDER BY created_at",
    )
    .fetch_all(pool)
    .await?;

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

    let path = dir.join("categories.csv");
    tokio::task::spawn_blocking(move || write_csv_file(&path, &csv_rows))
        .await
        .map_err(|e| WorldflowError::InvalidInput(format!("csv write task failed: {e}")))?
}

async fn export_tag_schemas(pool: &SqlitePool, dir: &Path) -> Result<()> {
    let rows = sqlx::query(
        "SELECT id, project_id, name, description, type, target, default_val,
                range_min, range_max, sort_order, created_at, updated_at
         FROM tag_schemas ORDER BY created_at",
    )
    .fetch_all(pool)
    .await?;

    let mut csv_rows = Vec::with_capacity(rows.len());
    for row in &rows {
        csv_rows.push(TagSchemaRow {
            id: row.try_get::<Uuid, _>("id")?.to_string(),
            project_id: row.try_get::<Uuid, _>("project_id")?.to_string(),
            name: row.try_get("name")?,
            description: row
                .try_get::<Option<String>, _>("description")?
                .unwrap_or_default(),
            type_: row.try_get("type")?,
            target: row.try_get("target")?,
            default_val: row
                .try_get::<Option<String>, _>("default_val")?
                .unwrap_or_default(),
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

    let path = dir.join("tag_schemas.csv");
    tokio::task::spawn_blocking(move || write_csv_file(&path, &csv_rows))
        .await
        .map_err(|e| WorldflowError::InvalidInput(format!("csv write task failed: {e}")))?
}

async fn export_entry_types(pool: &SqlitePool, dir: &Path) -> Result<()> {
    let rows = sqlx::query(
        "SELECT id, project_id, name, description, icon, color, created_at, updated_at
         FROM entry_types ORDER BY created_at",
    )
    .fetch_all(pool)
    .await?;

    let mut csv_rows = Vec::with_capacity(rows.len());
    for row in &rows {
        csv_rows.push(EntryTypeRow {
            id: row.try_get::<Uuid, _>("id")?.to_string(),
            project_id: row.try_get::<Uuid, _>("project_id")?.to_string(),
            name: row.try_get("name")?,
            description: row
                .try_get::<Option<String>, _>("description")?
                .unwrap_or_default(),
            icon: row
                .try_get::<Option<String>, _>("icon")?
                .unwrap_or_default(),
            color: row
                .try_get::<Option<String>, _>("color")?
                .unwrap_or_default(),
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        });
    }

    let path = dir.join("entry_types.csv");
    tokio::task::spawn_blocking(move || write_csv_file(&path, &csv_rows))
        .await
        .map_err(|e| WorldflowError::InvalidInput(format!("csv write task failed: {e}")))?
}

async fn export_entries(pool: &SqlitePool, dir: &Path) -> Result<()> {
    let rows = sqlx::query(
        "SELECT id, project_id, category_id, title, summary, content, type,
                tags, images, cover_path, created_at, updated_at
         FROM entries ORDER BY created_at",
    )
    .fetch_all(pool)
    .await?;

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
            summary: row
                .try_get::<Option<String>, _>("summary")?
                .unwrap_or_default(),
            content: row.try_get("content")?,
            type_: row
                .try_get::<Option<String>, _>("type")?
                .unwrap_or_default(),
            tags: row.try_get("tags")?,
            images: row.try_get("images")?,
            cover_path: row
                .try_get::<Option<String>, _>("cover_path")?
                .unwrap_or_default(),
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        });
    }

    let path = dir.join("entries.csv");
    tokio::task::spawn_blocking(move || write_csv_file(&path, &csv_rows))
        .await
        .map_err(|e| WorldflowError::InvalidInput(format!("csv write task failed: {e}")))?
}

async fn export_entry_relations(pool: &SqlitePool, dir: &Path) -> Result<()> {
    let rows = sqlx::query(
        "SELECT id, project_id, a_id, b_id, relation, content, created_at, updated_at
         FROM entry_relations ORDER BY created_at",
    )
    .fetch_all(pool)
    .await?;

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

    let path = dir.join("entry_relations.csv");
    tokio::task::spawn_blocking(move || write_csv_file(&path, &csv_rows))
        .await
        .map_err(|e| WorldflowError::InvalidInput(format!("csv write task failed: {e}")))?
}

async fn export_entry_links(pool: &SqlitePool, dir: &Path) -> Result<()> {
    let rows = sqlx::query(
        "SELECT id, project_id, a_id, b_id, created_at FROM entry_links ORDER BY created_at",
    )
    .fetch_all(pool)
    .await?;

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

    let path = dir.join("entry_links.csv");
    tokio::task::spawn_blocking(move || write_csv_file(&path, &csv_rows))
        .await
        .map_err(|e| WorldflowError::InvalidInput(format!("csv write task failed: {e}")))?
}

async fn export_idea_notes(pool: &SqlitePool, dir: &Path) -> Result<()> {
    let rows = sqlx::query(
        "SELECT id, project_id, content, title, status, pinned,
                created_at, updated_at, last_reviewed_at, converted_entry_id
         FROM idea_notes ORDER BY created_at",
    )
    .fetch_all(pool)
    .await?;

    let mut csv_rows = Vec::with_capacity(rows.len());
    for row in &rows {
        csv_rows.push(IdeaNoteRow {
            id: row.try_get::<Uuid, _>("id")?.to_string(),
            project_id: row
                .try_get::<Option<Uuid>, _>("project_id")?
                .map(|u| u.to_string())
                .unwrap_or_default(),
            content: row.try_get("content")?,
            title: row
                .try_get::<Option<String>, _>("title")?
                .unwrap_or_default(),
            status: row.try_get("status")?,
            pinned: row.try_get::<i64, _>("pinned")?.to_string(),
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
            last_reviewed_at: row
                .try_get::<Option<String>, _>("last_reviewed_at")?
                .unwrap_or_default(),
            converted_entry_id: row
                .try_get::<Option<Uuid>, _>("converted_entry_id")?
                .map(|u| u.to_string())
                .unwrap_or_default(),
        });
    }

    let path = dir.join("idea_notes.csv");
    tokio::task::spawn_blocking(move || write_csv_file(&path, &csv_rows))
        .await
        .map_err(|e| WorldflowError::InvalidInput(format!("csv write task failed: {e}")))?
}

// ═══════════════════════════════════════ Git operations ══════════════════════════════════════════

struct AllCsvBytes {
    projects: Vec<u8>,
    categories: Vec<u8>,
    tag_schemas: Vec<u8>,
    entry_types: Vec<u8>,
    entries: Vec<u8>,
    entry_relations: Vec<u8>,
    entry_links: Vec<u8>,
    idea_notes: Vec<u8>,
}

fn sync_git_commit(
    dir: &Path,
    message: &str,
    author_name: &str,
    author_email: &str,
) -> std::result::Result<(), git2::Error> {
    let repo = Repository::open(dir).or_else(|_| Repository::init(dir))?;
    let sig = Signature::now(author_name, author_email)?;

    let mut index = repo.index()?;
    index.add_all(["*.csv"].iter(), IndexAddOption::DEFAULT, None)?;
    index.write()?;
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;

    let parent_commits: Vec<git2::Commit<'_>> = match repo.head() {
        Ok(head) => vec![head.peel_to_commit()?],
        Err(_) => vec![],
    };
    let parent_refs: Vec<&git2::Commit<'_>> = parent_commits.iter().collect();

    repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)?;
    Ok(())
}

fn sync_git_list(dir: &Path) -> std::result::Result<Vec<SnapshotInfo>, git2::Error> {
    let repo = match Repository::open(dir) {
        Ok(r) => r,
        Err(_) => return Ok(vec![]),
    };

    let mut walk = repo.revwalk()?;
    if walk.push_head().is_err() {
        return Ok(vec![]);
    }
    walk.set_sorting(Sort::TIME)?;

    let mut results = Vec::new();
    for oid in walk {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        results.push(SnapshotInfo {
            id: oid.to_string(),
            message: commit.summary().unwrap_or("").to_string(),
            timestamp: commit.time().seconds(),
        });
    }
    Ok(results)
}

fn sync_git_read_all(dir: &Path, commit_id: &str) -> std::result::Result<AllCsvBytes, git2::Error> {
    let repo = Repository::open(dir)?;
    let commit = repo.revparse_single(commit_id)?.peel_to_commit()?;
    let tree = commit.tree()?;

    let read_blob = |name: &str| -> std::result::Result<Vec<u8>, git2::Error> {
        let entry = tree.get_name(name).ok_or_else(|| {
            git2::Error::from_str(&format!("{name} not found in commit {commit_id}"))
        })?;
        let blob = repo.find_blob(entry.id())?;
        Ok(blob.content().to_vec())
    };

    Ok(AllCsvBytes {
        projects: read_blob("projects.csv")?,
        categories: read_blob("categories.csv")?,
        tag_schemas: read_blob("tag_schemas.csv")?,
        entry_types: read_blob("entry_types.csv")?,
        entries: read_blob("entries.csv")?,
        entry_relations: read_blob("entry_relations.csv")?,
        entry_links: read_blob("entry_links.csv")?,
        idea_notes: read_blob("idea_notes.csv")?,
    })
}

async fn git_commit_snapshot(
    dir: PathBuf,
    message: String,
    author_name: String,
    author_email: String,
) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        sync_git_commit(&dir, &message, &author_name, &author_email)
    })
    .await
    .map_err(|e| WorldflowError::InvalidInput(format!("git task panicked: {e}")))?
    .map_err(WorldflowError::Git)
}

async fn git_list_commits(dir: PathBuf) -> Result<Vec<SnapshotInfo>> {
    tokio::task::spawn_blocking(move || sync_git_list(&dir))
        .await
        .map_err(|e| WorldflowError::InvalidInput(format!("git task panicked: {e}")))?
        .map_err(WorldflowError::Git)
}

async fn git_read_all_from_commit(dir: PathBuf, commit_id: String) -> Result<AllCsvBytes> {
    tokio::task::spawn_blocking(move || sync_git_read_all(&dir, &commit_id))
        .await
        .map_err(|e| WorldflowError::InvalidInput(format!("git task panicked: {e}")))?
        .map_err(WorldflowError::Git)
}

// ═══════════════════════════════════════ Import / restore ════════════════════════════════════════

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

async fn apply_csv_bytes(
    pool: &SqlitePool,
    bytes: AllCsvBytes,
    mode: RestoreMode,
) -> Result<AppendResult> {
    let parsed = parse_all_csv(&bytes)?;

    let mut conn = pool.acquire().await?;
    // FK enforcement must be toggled outside a transaction in SQLite
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *conn)
        .await?;
    sqlx::query("BEGIN").execute(&mut *conn).await?;

    let result = do_apply(&mut *conn, parsed, &mode).await;

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
    mode: &RestoreMode,
) -> Result<AppendResult> {
    if matches!(mode, RestoreMode::Replace) {
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

    let mut r = AppendResult::default();
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
        .bind(opt_str(&row.description))
        .bind(opt_str(&row.cover_image))
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
        .bind(opt_str(&row.description))
        .bind(&row.type_)
        .bind(&row.target)
        .bind(opt_str(&row.default_val))
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
        .bind(opt_str(&row.description))
        .bind(opt_str(&row.icon))
        .bind(opt_str(&row.color))
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
        .bind(opt_str(&row.summary))
        .bind(&row.content)
        .bind(opt_str(&row.type_))
        .bind(&row.tags)
        .bind(&row.images)
        .bind(opt_str(&row.cover_path))
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
        .bind(opt_str(&row.title))
        .bind(&row.status)
        .bind(pinned)
        .bind(&row.created_at)
        .bind(&row.updated_at)
        .bind(opt_str(&row.last_reviewed_at))
        .bind(converted_entry_id)
        .execute(&mut *conn)
        .await?
        .rows_affected();
        count += n as usize;
    }
    Ok(count)
}

// ═══════════════════════════════════════ Glue ════════════════════════════════════════════════════

async fn do_snapshot(pool: &SqlitePool, config: &SnapshotConfig, message: &str) -> Result<()> {
    export_all(pool, &config.dir).await?;
    git_commit_snapshot(
        config.dir.clone(),
        message.to_owned(),
        config.author_name.clone(),
        config.author_email.clone(),
    )
    .await
}

fn current_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ═══════════════════════════════════════ Public impl on SqliteDb ══════════════════════════════════

use crate::db::SqliteDb;

impl SqliteDb {
    pub async fn snapshot(&self) -> Result<()> {
        let state = self
            .snapshot
            .as_ref()
            .ok_or(WorldflowError::SnapshotNotConfigured)?;
        let _guard = state.lock.lock().await;
        do_snapshot(
            &self.pool,
            &state.config,
            &format!("manual {}", current_unix_secs()),
        )
        .await
    }

    pub async fn list_snapshots(&self) -> Result<Vec<SnapshotInfo>> {
        let state = self
            .snapshot
            .as_ref()
            .ok_or(WorldflowError::SnapshotNotConfigured)?;
        git_list_commits(state.config.dir.clone()).await
    }

    pub async fn rollback_to(&self, snapshot_id: &str) -> Result<()> {
        let state = self
            .snapshot
            .as_ref()
            .ok_or(WorldflowError::SnapshotNotConfigured)?;
        // Hold lock for the entire operation: prevents auto-snapshots from
        // capturing a half-cleared database during the replace.
        let _guard = state.lock.lock().await;
        do_snapshot(
            &self.pool,
            &state.config,
            &format!("pre-rollback {}", current_unix_secs()),
        )
        .await?;
        let bytes =
            git_read_all_from_commit(state.config.dir.clone(), snapshot_id.to_owned()).await?;
        apply_csv_bytes(&self.pool, bytes, RestoreMode::Replace).await?;
        Ok(())
    }

    pub async fn append_from(&self, snapshot_id: &str) -> Result<AppendResult> {
        let state = self
            .snapshot
            .as_ref()
            .ok_or(WorldflowError::SnapshotNotConfigured)?;
        let bytes =
            git_read_all_from_commit(state.config.dir.clone(), snapshot_id.to_owned()).await?;
        apply_csv_bytes(&self.pool, bytes, RestoreMode::Merge).await
    }

    pub async fn restore_from_csvs(&self, dir: &Path, mode: RestoreMode) -> Result<AppendResult> {
        let read_file = |name: &str| -> Result<Vec<u8>> {
            std::fs::read(dir.join(name)).map_err(WorldflowError::Io)
        };
        let bytes = AllCsvBytes {
            projects: read_file("projects.csv")?,
            categories: read_file("categories.csv")?,
            tag_schemas: read_file("tag_schemas.csv")?,
            entry_types: read_file("entry_types.csv")?,
            entries: read_file("entries.csv")?,
            entry_relations: read_file("entry_relations.csv")?,
            entry_links: read_file("entry_links.csv")?,
            idea_notes: read_file("idea_notes.csv")?,
        };
        apply_csv_bytes(&self.pool, bytes, mode).await
    }

    pub(crate) fn trigger_snapshot(&self) {
        let Some(state) = self.snapshot.clone() else {
            return;
        };
        let pool = self.pool.clone();
        tokio::spawn(async move {
            let _guard = state.lock.lock().await;
            if let Err(e) = do_snapshot(
                &pool,
                &state.config,
                &format!("auto {}", current_unix_secs()),
            )
            .await
            {
                eprintln!("[worldflow] snapshot error: {e}");
            }
        });
    }
}
