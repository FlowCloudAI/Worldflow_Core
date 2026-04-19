use std::env;
use uuid::Uuid;
use worldflow_core::{
    AppendResult, EntryOps, ProjectOps, RestoreMode, SnapshotConfig, SqliteDb,
    models::{CreateEntry, CreateProject},
};

// ─── helpers ─────────────────────────────────────────────────────────────────

async fn setup_with_snapshot() -> (SqliteDb, tempfile::TempDir) {
    let snap_dir = tempfile::tempdir().expect("tempdir");
    let db_path = env::temp_dir()
        .join(format!("worldflow-snap-{}.db", Uuid::now_v7()))
        .to_string_lossy()
        .replace('\\', "/");
    let db_url = format!("sqlite:{db_path}?mode=rwc");
    let db = SqliteDb::new_with_snapshot(
        &db_url,
        SnapshotConfig {
            dir: snap_dir.path().to_path_buf(),
            author_name: "Test Bot".to_string(),
            author_email: "test@example.com".to_string(),
        },
    )
    .await
    .unwrap();
    (db, snap_dir)
}

async fn setup_plain() -> SqliteDb {
    let db_path = env::temp_dir()
        .join(format!("worldflow-plain-{}.db", Uuid::now_v7()))
        .to_string_lossy()
        .replace('\\', "/");
    let db_url = format!("sqlite:{db_path}?mode=rwc");
    SqliteDb::new(&db_url).await.unwrap()
}

async fn seed_project(db: &SqliteDb, name: &str) -> worldflow_core::models::Project {
    db.create_project(CreateProject {
        name: name.to_string(),
        description: Some(format!("{name} 的描述")),
        cover_image: None,
    })
    .await
    .unwrap()
}

async fn seed_entry(db: &SqliteDb, project_id: Uuid, title: &str) -> worldflow_core::models::Entry {
    db.create_entry(CreateEntry {
        project_id,
        category_id: None,
        title: title.to_string(),
        summary: None,
        content: Some(format!("{title} 的内容")),
        r#type: None,
        tags: None,
        images: None,
    })
    .await
    .unwrap()
}

// ─── 手动快照 ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_manual_snapshot_creates_csv_files() {
    let (db, snap_dir) = setup_with_snapshot().await;
    seed_project(&db, "世界一").await;

    db.snapshot().await.unwrap();

    for name in &[
        "projects.csv",
        "categories.csv",
        "entries.csv",
        "tag_schemas.csv",
        "entry_relations.csv",
        "entry_links.csv",
        "entry_types.csv",
        "idea_notes.csv",
    ] {
        assert!(
            snap_dir.path().join(name).exists(),
            "{name} should exist after snapshot"
        );
    }
}

#[tokio::test]
async fn test_manual_snapshot_csv_contains_data() {
    let (db, snap_dir) = setup_with_snapshot().await;
    let project = seed_project(&db, "CSV 内容测试").await;
    seed_entry(&db, project.id, "词条A").await;

    db.snapshot().await.unwrap();

    let projects_csv = std::fs::read_to_string(snap_dir.path().join("projects.csv")).unwrap();
    assert!(projects_csv.contains("CSV 内容测试"), "project name in csv");

    let entries_csv = std::fs::read_to_string(snap_dir.path().join("entries.csv")).unwrap();
    assert!(entries_csv.contains("词条A"), "entry title in csv");
}

// ─── 版本列表 ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_list_snapshots_empty_before_any_snapshot() {
    let (db, _dir) = setup_with_snapshot().await;
    let list = db.list_snapshots().await.unwrap();
    assert!(list.is_empty(), "no snapshots before first commit");
}

#[tokio::test]
async fn test_list_snapshots_grows_with_each_commit() {
    let (db, _dir) = setup_with_snapshot().await;

    seed_project(&db, "版本1").await;
    db.snapshot().await.unwrap();

    seed_project(&db, "版本2").await;
    db.snapshot().await.unwrap();

    let list = db.list_snapshots().await.unwrap();
    assert!(
        list.len() >= 2,
        "at least two snapshots after two manual commits"
    );
    // newest first
    assert!(list[0].timestamp >= list[1].timestamp);
}

#[tokio::test]
async fn test_snapshot_info_fields() {
    let (db, _dir) = setup_with_snapshot().await;
    seed_project(&db, "字段测试").await;
    db.snapshot().await.unwrap();

    let list = db.list_snapshots().await.unwrap();
    let info = &list[0];
    assert!(!info.id.is_empty(), "commit id");
    assert!(!info.message.is_empty(), "commit message");
    assert!(info.timestamp > 0, "unix timestamp");
}

// ─── 快照未配置时返回 Err ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_snapshot_not_configured_error() {
    let db = setup_plain().await;
    assert!(db.snapshot().await.is_err());
    assert!(db.list_snapshots().await.is_err());
    assert!(db.rollback_to("abc").await.is_err());
    assert!(db.append_from("abc").await.is_err());
}

// ─── rollback_to ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_rollback_restores_to_snapshot_state() {
    let (db, _dir) = setup_with_snapshot().await;

    // v1: 只有一个项目
    let p1 = seed_project(&db, "项目Alpha").await;
    db.snapshot().await.unwrap();

    let list_v1 = db.list_snapshots().await.unwrap();
    let snap_v1_id = list_v1[0].id.clone();

    // v2: 再加一个项目
    seed_project(&db, "项目Beta").await;
    db.snapshot().await.unwrap();

    assert_eq!(db.list_projects().await.unwrap().len(), 2);

    // 回退到 v1
    db.rollback_to(&snap_v1_id).await.unwrap();

    let projects_after = db.list_projects().await.unwrap();
    assert_eq!(projects_after.len(), 1, "only one project after rollback");
    assert_eq!(projects_after[0].id, p1.id);
}

#[tokio::test]
async fn test_rollback_creates_pre_rollback_snapshot() {
    let (db, _dir) = setup_with_snapshot().await;

    seed_project(&db, "原始项目").await;
    db.snapshot().await.unwrap();

    let snap_id = db.list_snapshots().await.unwrap()[0].id.clone();

    // 回退前有 1 个快照，回退后应有 2 个（pre-rollback + 原来的）
    db.rollback_to(&snap_id).await.unwrap();

    let list = db.list_snapshots().await.unwrap();
    assert!(list.len() >= 2, "pre-rollback snapshot was created");
    assert!(
        list.iter().any(|s| s.message.contains("pre-rollback")),
        "one commit is labeled pre-rollback"
    );
}

// ─── append_from ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_append_from_restores_deleted_entries() {
    let (db, _dir) = setup_with_snapshot().await;

    let project = seed_project(&db, "追加测试项目").await;
    let entry = seed_entry(&db, project.id, "被删词条").await;

    db.snapshot().await.unwrap();
    let snap_id = db.list_snapshots().await.unwrap()[0].id.clone();

    // 删掉词条
    db.delete_entry(&entry.id).await.unwrap();
    // 等 auto-snapshot 完成（delete 触发了一个 fire-and-forget）
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // 确认已删
    assert!(db.get_entry(&entry.id).await.is_err());

    // 从历史快照追加恢复
    let result: AppendResult = db.append_from(&snap_id).await.unwrap();
    assert_eq!(result.entries, 1, "one entry restored");

    // 词条回来了
    let restored = db.get_entry(&entry.id).await.unwrap();
    assert_eq!(restored.title, "被删词条");
}

#[tokio::test]
async fn test_append_from_does_not_duplicate_existing() {
    let (db, _dir) = setup_with_snapshot().await;

    let project = seed_project(&db, "去重测试").await;
    seed_entry(&db, project.id, "现存词条").await;

    db.snapshot().await.unwrap();
    let snap_id = db.list_snapshots().await.unwrap()[0].id.clone();

    // 词条仍然存在，再次 append 不应新增
    let result = db.append_from(&snap_id).await.unwrap();
    assert_eq!(result.projects, 0, "project already exists, not duplicated");
    assert_eq!(result.entries, 0, "entry already exists, not duplicated");
}

#[tokio::test]
async fn test_append_from_is_additive_not_destructive() {
    let (db, _dir) = setup_with_snapshot().await;

    let project = seed_project(&db, "基础项目").await;
    seed_entry(&db, project.id, "旧词条").await;
    db.snapshot().await.unwrap();
    let snap_id = db.list_snapshots().await.unwrap()[0].id.clone();

    // 当前新增一条词条（快照里没有）
    let new_entry = seed_entry(&db, project.id, "新词条").await;

    db.append_from(&snap_id).await.unwrap();

    // 新词条不应被删除
    assert!(
        db.get_entry(&new_entry.id).await.is_ok(),
        "new entry survives append_from"
    );
}

// ─── restore_from_csvs ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_restore_from_csvs_replace_mode() {
    let (db, snap_dir) = setup_with_snapshot().await;

    let p = seed_project(&db, "CSV 恢复项目").await;
    seed_entry(&db, p.id, "CSV词条").await;
    db.snapshot().await.unwrap();

    // 清掉所有数据
    db.delete_project(&p.id).await.unwrap();
    assert!(db.list_projects().await.unwrap().is_empty());

    // 用最新 CSV 文件做 Replace 恢复
    let result = db
        .restore_from_csvs(snap_dir.path(), RestoreMode::Replace)
        .await
        .unwrap();

    assert_eq!(result.projects, 1);
    assert_eq!(result.entries, 1);
    let projects = db.list_projects().await.unwrap();
    assert_eq!(projects[0].name, "CSV 恢复项目");
}

#[tokio::test]
async fn test_restore_from_csvs_merge_mode() {
    let (db, snap_dir) = setup_with_snapshot().await;

    let p = seed_project(&db, "Merge项目").await;
    seed_entry(&db, p.id, "Merge词条").await;
    db.snapshot().await.unwrap();

    // 删除词条
    let entries = db
        .list_entries(&p.id, Default::default(), 100, 0)
        .await
        .unwrap();
    for e in &entries {
        db.delete_entry(&e.id).await.unwrap();
    }
    assert_eq!(
        db.list_entries(&p.id, Default::default(), 100, 0)
            .await
            .unwrap()
            .len(),
        0
    );

    // Merge 只补充缺失
    let result = db
        .restore_from_csvs(snap_dir.path(), RestoreMode::Merge)
        .await
        .unwrap();

    assert_eq!(result.entries, 1, "deleted entry restored via merge");
    // 项目本身没被重复插入
    assert_eq!(result.projects, 0);
}

// ─── 自动快照触发 ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_auto_snapshot_triggered_on_write() {
    let (db, _dir) = setup_with_snapshot().await;

    // 写一条数据，自动触发快照
    seed_project(&db, "自动快照项目").await;

    // fire-and-forget，给后台任务一点时间
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let list = db.list_snapshots().await.unwrap();
    assert!(!list.is_empty(), "auto-snapshot created after write");
    assert!(
        list[0].message.starts_with("auto "),
        "auto-snapshot message prefix"
    );
}
