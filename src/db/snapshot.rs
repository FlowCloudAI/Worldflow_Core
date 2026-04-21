use crate::error::{Result, WorldflowError};
use git2::{BranchType, IndexAddOption, Repository, Signature, Sort};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqliteConnection, SqlitePool};
use std::path::{Path, PathBuf};
use tokio::sync::Mutex;
use uuid::Uuid;

// ═══════════════════════════════════════ Public types ════════════════════════════════════════════

#[derive(Debug, Serialize)]
pub struct SnapshotConfig {
    pub dir: PathBuf,
    pub author_name: String,
    pub author_email: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SnapshotInfo {
    pub id: String,
    pub message: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SnapshotBranchInfo {
    pub name: String,
    pub head: Option<String>,
    pub is_current: bool,
    pub is_active: bool,
}

#[derive(Debug, Default, Serialize)]
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

#[derive(Debug, Serialize)]
pub enum RestoreMode {
    Replace,
    Merge,
}

// ═══════════════════════════════════════ Internal state ══════════════════════════════════════════

#[derive(Debug)]
pub(super) struct SnapshotState {
    pub config: SnapshotConfig,
    lock: Mutex<SnapshotRuntimeState>,
}

#[derive(Debug)]
struct SnapshotRuntimeState {
    active_branch: String,
}

const ACTIVE_BRANCH_FILE: &str = ".worldflow-active-branch";

fn active_branch_file(dir: &Path) -> PathBuf {
    dir.join(ACTIVE_BRANCH_FILE)
}

fn load_persisted_active_branch(dir: &Path) -> Option<String> {
    let path = active_branch_file(dir);
    let branch = std::fs::read_to_string(path).ok()?;
    let branch = branch.trim();
    if branch.is_empty() {
        None
    } else {
        Some(branch.to_owned())
    }
}

fn persist_active_branch(dir: &Path, branch_name: &str) -> Result<()> {
    std::fs::create_dir_all(dir).map_err(WorldflowError::Io)?;
    std::fs::write(active_branch_file(dir), branch_name.as_bytes()).map_err(WorldflowError::Io)?;
    Ok(())
}

fn detect_active_branch(dir: &Path) -> String {
    if let Some(branch) = load_persisted_active_branch(dir) {
        return branch;
    }
    let Ok(repo) = Repository::open(dir) else {
        return "main".to_owned();
    };
    let Ok(head) = repo.head() else {
        return "main".to_owned();
    };
    head.shorthand().unwrap_or("main").to_owned()
}

impl SnapshotState {
    pub fn new(config: SnapshotConfig) -> Self {
        let active_branch = detect_active_branch(&config.dir);
        let _ = persist_active_branch(&config.dir, &active_branch);
        Self {
            lock: Mutex::new(SnapshotRuntimeState { active_branch }),
            config,
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

fn branch_ref_name(branch_name: &str) -> String {
    format!("refs/heads/{branch_name}")
}

fn sync_git_commit_to_ref(
    dir: &Path,
    message: &str,
    author_name: &str,
    author_email: &str,
    update_ref: &str,
) -> std::result::Result<(), git2::Error> {
    let repo = Repository::open(dir).or_else(|_| Repository::init(dir))?;
    let sig = Signature::now(author_name, author_email)?;

    let mut index = repo.index()?;
    index.add_all(["*.csv"].iter(), IndexAddOption::DEFAULT, None)?;
    index.write()?;
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;

    let parent_commits: Vec<git2::Commit<'_>> = match repo.revparse_single(update_ref) {
        Ok(obj) => vec![obj.peel_to_commit()?],
        Err(_) if update_ref == "HEAD" => match repo.head() {
            Ok(head) => vec![head.peel_to_commit()?],
            Err(_) => vec![],
        },
        Err(_) => vec![],
    };
    let parent_refs: Vec<&git2::Commit<'_>> = parent_commits.iter().collect();

    repo.commit(Some(update_ref), &sig, &sig, message, &tree, &parent_refs)?;
    Ok(())
}

fn sync_git_list_ref(
    dir: &Path,
    git_ref: &str,
) -> std::result::Result<Vec<SnapshotInfo>, git2::Error> {
    let repo = match Repository::open(dir) {
        Ok(r) => r,
        Err(_) => return Ok(vec![]),
    };

    let mut walk = repo.revwalk()?;
    let pushed = if git_ref == "HEAD" {
        walk.push_head()
    } else {
        walk.push_ref(git_ref)
    };
    if pushed.is_err() {
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

fn sync_git_list_branches(
    dir: &Path,
    active_branch: &str,
) -> std::result::Result<Vec<SnapshotBranchInfo>, git2::Error> {
    let repo = match Repository::open(dir) {
        Ok(r) => r,
        Err(_) => return Ok(vec![]),
    };

    let mut results = Vec::new();
    for branch in repo.branches(Some(BranchType::Local))? {
        let (branch, _) = branch?;
        let name = branch
            .name()?
            .ok_or_else(|| git2::Error::from_str("invalid utf-8 branch name"))?
            .to_string();
        let head = branch.get().target().map(|oid| oid.to_string());
        let is_current = branch.is_head();
        results.push(SnapshotBranchInfo {
            is_active: active_branch == name,
            name,
            head,
            is_current,
        });
    }
    Ok(results)
}

fn sync_git_create_branch(
    dir: &Path,
    branch_name: &str,
    from_ref: Option<&str>,
) -> std::result::Result<(), git2::Error> {
    let repo = Repository::open(dir).or_else(|_| Repository::init(dir))?;
    let start_ref = from_ref.unwrap_or("HEAD");
    let commit = repo.revparse_single(start_ref)?.peel_to_commit()?;
    repo.branch(branch_name, &commit, false)?;
    Ok(())
}

fn sync_git_set_head(dir: &Path, branch_name: &str) -> std::result::Result<(), git2::Error> {
    let repo = Repository::open(dir)?;
    repo.set_head(&format!("refs/heads/{branch_name}"))?;
    Ok(())
}

fn sync_git_branch_exists(dir: &Path, branch_name: &str) -> std::result::Result<bool, git2::Error> {
    let repo = match Repository::open(dir) {
        Ok(r) => r,
        Err(_) => return Ok(false),
    };
    Ok(repo.find_branch(branch_name, BranchType::Local).is_ok())
}

fn sync_git_ref_exists(dir: &Path, git_ref: &str) -> std::result::Result<bool, git2::Error> {
    let repo = match Repository::open(dir) {
        Ok(r) => r,
        Err(_) => return Ok(false),
    };
    Ok(repo.revparse_single(git_ref).is_ok())
}

fn sync_git_read_all_ref(
    dir: &Path,
    git_ref: &str,
    display_name: &str,
) -> std::result::Result<AllCsvBytes, git2::Error> {
    let repo = Repository::open(dir)?;
    let commit = repo.revparse_single(git_ref)?.peel_to_commit()?;
    let tree = commit.tree()?;

    let read_blob = |name: &str| -> std::result::Result<Vec<u8>, git2::Error> {
        let entry = tree
            .get_name(name)
            .ok_or_else(|| git2::Error::from_str(&format!("{name} not found in {display_name}")))?;
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

fn sync_git_read_all(dir: &Path, commit_id: &str) -> std::result::Result<AllCsvBytes, git2::Error> {
    sync_git_read_all_ref(dir, commit_id, &format!("commit {commit_id}"))
}

async fn git_commit_snapshot_to_branch(
    dir: PathBuf,
    branch_name: String,
    message: String,
    author_name: String,
    author_email: String,
) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        sync_git_commit_to_ref(
            &dir,
            &message,
            &author_name,
            &author_email,
            &branch_ref_name(&branch_name),
        )
    })
    .await
    .map_err(|e| WorldflowError::InvalidInput(format!("git task panicked: {e}")))?
    .map_err(WorldflowError::Git)
}

async fn git_list_commits_in_branch(
    dir: PathBuf,
    branch_name: String,
) -> Result<Vec<SnapshotInfo>> {
    tokio::task::spawn_blocking(move || sync_git_list_ref(&dir, &branch_ref_name(&branch_name)))
        .await
        .map_err(|e| WorldflowError::InvalidInput(format!("git task panicked: {e}")))?
        .map_err(WorldflowError::Git)
}

async fn git_list_branches(dir: PathBuf, active_branch: String) -> Result<Vec<SnapshotBranchInfo>> {
    tokio::task::spawn_blocking(move || sync_git_list_branches(&dir, &active_branch))
        .await
        .map_err(|e| WorldflowError::InvalidInput(format!("git task panicked: {e}")))?
        .map_err(WorldflowError::Git)
}

async fn git_create_branch(
    dir: PathBuf,
    branch_name: String,
    from_ref: Option<String>,
) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        sync_git_create_branch(&dir, &branch_name, from_ref.as_deref())
    })
    .await
    .map_err(|e| WorldflowError::InvalidInput(format!("git task panicked: {e}")))?
    .map_err(WorldflowError::Git)
}

async fn git_set_head(dir: PathBuf, branch_name: String) -> Result<()> {
    tokio::task::spawn_blocking(move || sync_git_set_head(&dir, &branch_name))
        .await
        .map_err(|e| WorldflowError::InvalidInput(format!("git task panicked: {e}")))?
        .map_err(WorldflowError::Git)
}

async fn git_branch_exists(dir: PathBuf, branch_name: String) -> Result<bool> {
    tokio::task::spawn_blocking(move || sync_git_branch_exists(&dir, &branch_name))
        .await
        .map_err(|e| WorldflowError::InvalidInput(format!("git task panicked: {e}")))?
        .map_err(WorldflowError::Git)
}

async fn git_ref_exists(dir: PathBuf, git_ref: String) -> Result<bool> {
    tokio::task::spawn_blocking(move || sync_git_ref_exists(&dir, &git_ref))
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

async fn git_read_all_from_branch(dir: PathBuf, branch_name: String) -> Result<AllCsvBytes> {
    tokio::task::spawn_blocking(move || {
        let git_ref = branch_ref_name(&branch_name);
        sync_git_read_all_ref(&dir, &git_ref, &format!("branch {branch_name}"))
    })
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

async fn do_snapshot_to_branch(
    pool: &SqlitePool,
    config: &SnapshotConfig,
    branch_name: &str,
    message: &str,
) -> Result<()> {
    export_all(pool, &config.dir).await?;
    git_commit_snapshot_to_branch(
        config.dir.clone(),
        branch_name.to_owned(),
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
        self.snapshot_with_message(&format!("manual {}", current_unix_secs()))
            .await
    }

    pub async fn snapshot_with_message(&self, message: &str) -> Result<()> {
        let state = self
            .snapshot
            .as_ref()
            .ok_or(WorldflowError::SnapshotNotConfigured)?;
        let guard = state.lock.lock().await;
        do_snapshot_to_branch(&self.pool, &state.config, &guard.active_branch, message).await
    }

    pub async fn list_snapshots(&self) -> Result<Vec<SnapshotInfo>> {
        let state = self
            .snapshot
            .as_ref()
            .ok_or(WorldflowError::SnapshotNotConfigured)?;
        let active_branch = {
            let guard = state.lock.lock().await;
            guard.active_branch.clone()
        };
        git_list_commits_in_branch(state.config.dir.clone(), active_branch).await
    }

    pub async fn list_branches(&self) -> Result<Vec<SnapshotBranchInfo>> {
        let state = self
            .snapshot
            .as_ref()
            .ok_or(WorldflowError::SnapshotNotConfigured)?;
        let active_branch = {
            let guard = state.lock.lock().await;
            guard.active_branch.clone()
        };
        git_list_branches(state.config.dir.clone(), active_branch).await
    }

    pub async fn active_branch(&self) -> Result<String> {
        let state = self
            .snapshot
            .as_ref()
            .ok_or(WorldflowError::SnapshotNotConfigured)?;
        let guard = state.lock.lock().await;
        Ok(guard.active_branch.clone())
    }

    pub async fn create_branch(&self, branch_name: &str, from_ref: Option<&str>) -> Result<()> {
        let state = self
            .snapshot
            .as_ref()
            .ok_or(WorldflowError::SnapshotNotConfigured)?;
        let guard = state.lock.lock().await;
        let start_ref = from_ref
            .map(str::to_owned)
            .unwrap_or_else(|| branch_ref_name(&guard.active_branch));
        if !git_ref_exists(state.config.dir.clone(), start_ref.clone()).await? {
            return Err(WorldflowError::InvalidInput(
                "当前没有基线提交，不能创建分支".to_owned(),
            ));
        }
        git_create_branch(
            state.config.dir.clone(),
            branch_name.to_owned(),
            Some(start_ref),
        )
        .await
    }

    pub async fn switch_branch(&self, branch_name: &str) -> Result<()> {
        let state = self
            .snapshot
            .as_ref()
            .ok_or(WorldflowError::SnapshotNotConfigured)?;
        let mut guard = state.lock.lock().await;
        if guard.active_branch == branch_name {
            return Ok(());
        }
        if !git_branch_exists(state.config.dir.clone(), branch_name.to_owned()).await? {
            return Err(WorldflowError::NotFound(format!(
                "分支不存在: {branch_name}"
            )));
        }
        do_snapshot_to_branch(
            &self.pool,
            &state.config,
            &guard.active_branch,
            &format!("pre-switch-to-{branch_name} {}", current_unix_secs()),
        )
        .await?;
        let bytes =
            git_read_all_from_branch(state.config.dir.clone(), branch_name.to_owned()).await?;
        apply_csv_bytes(&self.pool, bytes, RestoreMode::Replace).await?;
        git_set_head(state.config.dir.clone(), branch_name.to_owned()).await?;
        persist_active_branch(&state.config.dir, branch_name)?;
        guard.active_branch = branch_name.to_owned();
        Ok(())
    }

    pub async fn list_snapshots_in_branch(&self, branch_name: &str) -> Result<Vec<SnapshotInfo>> {
        let state = self
            .snapshot
            .as_ref()
            .ok_or(WorldflowError::SnapshotNotConfigured)?;
        git_list_commits_in_branch(state.config.dir.clone(), branch_name.to_owned()).await
    }

    pub async fn snapshot_to_branch(&self, branch_name: &str, message: &str) -> Result<()> {
        let state = self
            .snapshot
            .as_ref()
            .ok_or(WorldflowError::SnapshotNotConfigured)?;
        let _guard = state.lock.lock().await;
        do_snapshot_to_branch(&self.pool, &state.config, branch_name, message).await
    }

    pub async fn rollback_to(&self, snapshot_id: &str) -> Result<()> {
        let state = self
            .snapshot
            .as_ref()
            .ok_or(WorldflowError::SnapshotNotConfigured)?;
        // Hold lock for the entire operation: prevents auto-snapshots from
        // capturing a half-cleared database during the replace.
        let guard = state.lock.lock().await;
        do_snapshot_to_branch(
            &self.pool,
            &state.config,
            &guard.active_branch,
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
            let guard = state.lock.lock().await;
            if let Err(e) = do_snapshot_to_branch(
                &pool,
                &state.config,
                &guard.active_branch,
                &format!("auto {}", current_unix_secs()),
            )
            .await
            {
                eprintln!("[worldflow] snapshot error: {e}");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{ACTIVE_BRANCH_FILE, SnapshotConfig};
    use crate::db::SqliteDb;
    use crate::error::Result;
    use sqlx::Row;
    use std::path::Path;
    use tempfile::TempDir;
    use uuid::Uuid;

    fn test_paths(prefix: &str) -> Result<(TempDir, String, std::path::PathBuf)> {
        let temp = tempfile::tempdir()?;
        let db_path = temp.path().join(format!("{prefix}.db"));
        let snapshot_dir = temp.path().join("snapshots");
        Ok((
            temp,
            format!(
                "sqlite:{}?mode=rwc",
                db_path.to_string_lossy().replace('\\', "/")
            ),
            snapshot_dir,
        ))
    }

    async fn new_test_db(database_url: &str, snapshot_dir: &Path) -> Result<SqliteDb> {
        SqliteDb::new_with_snapshot(
            database_url,
            SnapshotConfig {
                dir: snapshot_dir.to_path_buf(),
                author_name: "测试".to_owned(),
                author_email: "test@example.com".to_owned(),
            },
        )
        .await
    }

    async fn insert_project(db: &SqliteDb, name: &str) -> Result<Uuid> {
        let id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO projects (id, name, description, cover_image)
             VALUES (?, ?, NULL, NULL)",
        )
        .bind(id)
        .bind(name)
        .execute(&db.pool)
        .await?;
        Ok(id)
    }

    async fn update_project_name(db: &SqliteDb, id: Uuid, name: &str) -> Result<()> {
        sqlx::query("UPDATE projects SET name = ? WHERE id = ?")
            .bind(name)
            .bind(id)
            .execute(&db.pool)
            .await?;
        Ok(())
    }

    async fn get_project_name(db: &SqliteDb, id: Uuid) -> Result<String> {
        let row = sqlx::query("SELECT name FROM projects WHERE id = ?")
            .bind(id)
            .fetch_one(&db.pool)
            .await?;
        Ok(row.try_get("name")?)
    }

    #[tokio::test]
    async fn active_branch_persists_across_reopen() -> Result<()> {
        let (_temp, database_url, snapshot_dir) = test_paths("persist_active_branch")?;
        let db = new_test_db(&database_url, &snapshot_dir).await?;

        let project_id = insert_project(&db, "主线项目").await?;
        db.snapshot_with_message("main init").await?;
        db.create_branch("feature", None).await?;
        db.switch_branch("feature").await?;
        update_project_name(&db, project_id, "分支项目").await?;
        db.snapshot_with_message("feature update").await?;

        let persisted = std::fs::read_to_string(snapshot_dir.join(ACTIVE_BRANCH_FILE))?;
        assert_eq!(persisted.trim(), "feature");

        {
            let repo = git2::Repository::open(&snapshot_dir)?;
            assert_eq!(repo.head()?.shorthand(), Some("feature"));
        }

        drop(db);

        let reopened = new_test_db(&database_url, &snapshot_dir).await?;
        assert_eq!(reopened.active_branch().await?, "feature");

        let snapshots = reopened.list_snapshots().await?;
        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].message, "feature update");
        Ok(())
    }

    #[tokio::test]
    async fn switch_branch_restores_database_to_branch_tip() -> Result<()> {
        let (_temp, database_url, snapshot_dir) = test_paths("switch_branch_restore")?;
        let db = new_test_db(&database_url, &snapshot_dir).await?;

        let project_id = insert_project(&db, "主线项目").await?;
        db.snapshot_with_message("main init").await?;
        db.create_branch("feature", None).await?;

        db.switch_branch("feature").await?;
        update_project_name(&db, project_id, "特性版本").await?;
        db.snapshot_with_message("feature update").await?;

        db.switch_branch("main").await?;
        assert_eq!(db.active_branch().await?, "main");
        assert_eq!(get_project_name(&db, project_id).await?, "主线项目");
        {
            let repo = git2::Repository::open(&snapshot_dir)?;
            assert_eq!(repo.head()?.shorthand(), Some("main"));
        }

        db.switch_branch("feature").await?;
        assert_eq!(db.active_branch().await?, "feature");
        assert_eq!(get_project_name(&db, project_id).await?, "特性版本");
        {
            let repo = git2::Repository::open(&snapshot_dir)?;
            assert_eq!(repo.head()?.shorthand(), Some("feature"));
        }
        Ok(())
    }
}
