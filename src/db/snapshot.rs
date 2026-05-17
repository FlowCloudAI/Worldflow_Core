use crate::db::csv_bundle::{
    AllCsvBytes, CsvImportMode, CsvImportResult, export_all_to_dir, import_all_bytes,
    read_all_csv_bytes_from_dir,
};
use crate::error::{Result, WorldflowError};
use git2::{BranchType, ErrorCode, IndexAddOption, Repository, Signature, Sort};
use serde::Serialize;
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use tokio::sync::Mutex;

// ═══════════════════════════════════════ 公开类型 ═══════════════════════════════════════════════

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

pub type AppendResult = CsvImportResult;
pub type RestoreMode = CsvImportMode;

// ═══════════════════════════════════════ 内部状态 ═══════════════════════════════════════════════

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

fn load_persisted_active_branch(dir: &Path) -> Result<Option<String>> {
    let path = active_branch_file(dir);
    let branch = match std::fs::read_to_string(path) {
        Ok(branch) => branch,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(WorldflowError::Io(e)),
    };
    let branch = branch.trim();
    if branch.is_empty() {
        Ok(None)
    } else {
        Ok(Some(branch.to_owned()))
    }
}

fn persist_active_branch(dir: &Path, branch_name: &str) -> Result<()> {
    std::fs::create_dir_all(dir).map_err(WorldflowError::Io)?;
    std::fs::write(active_branch_file(dir), branch_name.as_bytes()).map_err(WorldflowError::Io)?;
    Ok(())
}

fn detect_active_branch(dir: &Path) -> Result<String> {
    if let Some(branch) = load_persisted_active_branch(dir)? {
        return Ok(branch);
    }
    let repo = match Repository::open(dir) {
        Ok(repo) => repo,
        Err(e) if e.code() == ErrorCode::NotFound => return Ok("main".to_owned()),
        Err(e) => return Err(WorldflowError::Git(e)),
    };
    let head = match repo.head() {
        Ok(head) => head,
        Err(e) if matches!(e.code(), ErrorCode::NotFound | ErrorCode::UnbornBranch) => {
            return Ok("main".to_owned());
        }
        Err(e) => return Err(WorldflowError::Git(e)),
    };
    head.shorthand()
        .map(str::to_owned)
        .ok_or_else(|| WorldflowError::InvalidInput("无法识别活动快照分支".to_owned()))
}

impl SnapshotState {
    pub fn new(config: SnapshotConfig) -> Result<Self> {
        let active_branch = detect_active_branch(&config.dir)?;
        persist_active_branch(&config.dir, &active_branch)?;
        Ok(Self {
            lock: Mutex::new(SnapshotRuntimeState { active_branch }),
            config,
        })
    }
}

// ═══════════════════════════════════════ Git 操作 ═══════════════════════════════════════════════

fn branch_ref_name(branch_name: &str) -> String {
    format!("refs/heads/{branch_name}")
}

fn sync_git_commit_to_ref(
    dir: &Path,
    message: &str,
    author_name: &str,
    author_email: &str,
    update_ref: &str,
) -> std::result::Result<bool, git2::Error> {
    let repo = Repository::open(dir).or_else(|_| Repository::init(dir))?;
    let sig = Signature::now(author_name, author_email)?;

    let mut index = repo.index()?;
    index.add_all(["*.csv"].iter(), IndexAddOption::DEFAULT, None)?;
    index.write()?;
    let tree_id = index.write_tree()?;

    let parent_commits: Vec<git2::Commit<'_>> = match repo.revparse_single(update_ref) {
        Ok(obj) => vec![obj.peel_to_commit()?],
        Err(_) if update_ref == "HEAD" => match repo.head() {
            Ok(head) => vec![head.peel_to_commit()?],
            Err(_) => vec![],
        },
        Err(_) => vec![],
    };

    // 空提交检测：如果新 tree 与父提交相同，说明没有任何变化
    if let Some(parent) = parent_commits.first() {
        if parent.tree_id() == tree_id {
            return Ok(false);
        }
    }

    let tree = repo.find_tree(tree_id)?;
    let parent_refs: Vec<&git2::Commit<'_>> = parent_commits.iter().collect();
    repo.commit(Some(update_ref), &sig, &sig, message, &tree, &parent_refs)?;
    Ok(true)
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
    let committed = tokio::task::spawn_blocking(move || {
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
    .map_err(WorldflowError::Git)?;

    if !committed {
        return Err(WorldflowError::NoChanges);
    }
    Ok(())
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

// ═══════════════════════════════════════ 胶水层 ═════════════════════════════════════════════════

async fn do_snapshot_to_branch(
    pool: &SqlitePool,
    config: &SnapshotConfig,
    branch_name: &str,
    message: &str,
) -> Result<()> {
    export_all_to_dir(pool, &config.dir).await?;
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

// ═══════════════════════════════════════ SqliteDb 公开实现 ══════════════════════════════════════

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
        match do_snapshot_to_branch(
            &self.pool,
            &state.config,
            &guard.active_branch,
            &format!("pre-switch-to-{branch_name} {}", current_unix_secs()),
        )
        .await
        {
            Ok(()) | Err(WorldflowError::NoChanges) => {}
            Err(e) => return Err(e),
        }
        let bytes =
            git_read_all_from_branch(state.config.dir.clone(), branch_name.to_owned()).await?;
        import_all_bytes(&self.pool, bytes, CsvImportMode::Replace).await?;
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
        // 在整个操作期间持有锁，避免分支切换或回滚交错写入半清空的数据库。
        let guard = state.lock.lock().await;
        match do_snapshot_to_branch(
            &self.pool,
            &state.config,
            &guard.active_branch,
            &format!("pre-rollback {}", current_unix_secs()),
        )
        .await
        {
            Ok(()) | Err(WorldflowError::NoChanges) => {}
            Err(e) => return Err(e),
        }
        let bytes =
            git_read_all_from_commit(state.config.dir.clone(), snapshot_id.to_owned()).await?;
        import_all_bytes(&self.pool, bytes, CsvImportMode::Replace).await?;
        Ok(())
    }

    pub async fn append_from(&self, snapshot_id: &str) -> Result<AppendResult> {
        let state = self
            .snapshot
            .as_ref()
            .ok_or(WorldflowError::SnapshotNotConfigured)?;
        let bytes =
            git_read_all_from_commit(state.config.dir.clone(), snapshot_id.to_owned()).await?;
        import_all_bytes(&self.pool, bytes, CsvImportMode::Merge).await
    }

    pub async fn restore_from_csvs(&self, dir: &Path, mode: RestoreMode) -> Result<AppendResult> {
        let bytes = read_all_csv_bytes_from_dir(dir)?;
        import_all_bytes(&self.pool, bytes, mode).await
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

    #[tokio::test]
    async fn snapshot_no_changes_returns_no_changes() -> Result<()> {
        let (_temp, database_url, snapshot_dir) = test_paths("snapshot_no_changes")?;
        let db = new_test_db(&database_url, &snapshot_dir).await?;

        insert_project(&db, "测试项目").await?;
        db.snapshot_with_message("first").await?;

        // 没有任何修改，再次快照应返回 NoChanges
        let result = db.snapshot_with_message("second").await;
        assert!(
            matches!(result, Err(crate::error::WorldflowError::NoChanges)),
            "expected NoChanges error, got {:?}",
            result
        );

        // 确认分支上仍然只有 1 个提交
        let snapshots = db.list_snapshots().await?;
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].message, "first");
        Ok(())
    }
}
