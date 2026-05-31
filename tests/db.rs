use sqlx::Row;
use std::{env, path::PathBuf};
use uuid::Uuid;
use worldflow_core::models::EntryFilter;
use worldflow_core::{
    CategoryOps, EntryLinkOps, EntryOps, EntryRelationOps, EntryTypeOps, IdeaNoteOps, ProjectOps,
    SqliteDb, TagSchemaOps, models::*,
};

async fn setup() -> SqliteDb {
    let db_path = env::temp_dir()
        .join(format!("worldflow-test-{}.db", Uuid::now_v7()))
        .to_string_lossy()
        .replace('\\', "/");
    let db_url = format!("sqlite:{db_path}?mode=rwc");
    SqliteDb::new(&db_url).await.unwrap()
}

fn quote_ident(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

async fn table_columns(db: &SqliteDb, table: &str) -> Vec<String> {
    let sql = format!("PRAGMA table_info({})", quote_ident(table));
    sqlx::query(&sql)
        .fetch_all(&db.pool)
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.try_get::<String, _>("name").unwrap())
        .collect()
}

async fn assert_table_columns(db: &SqliteDb, table: &str, expected: &[&str]) {
    let columns = table_columns(db, table).await;
    for column in expected {
        assert!(
            columns.iter().any(|existing| existing == column),
            "{table} 缺少列 {column}，当前列: {columns:?}"
        );
    }
}

async fn assert_schema_object_exists(db: &SqliteDb, kind: &str, name: &str) {
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(1) FROM sqlite_master WHERE type = ? AND name = ?")
            .bind(kind)
            .bind(name)
            .fetch_one(&db.pool)
            .await
            .unwrap();

    assert_eq!(count, 1, "缺少 {kind} {name}");
}

async fn assert_representative_query(db: &SqliteDb, sql: &str) {
    sqlx::query(sql).fetch_all(&db.pool).await.unwrap();
}

#[tokio::test]
async fn test_sqlite_schema_smoke_matches_hot_queries() {
    let db = setup().await;

    assert_table_columns(
        &db,
        "projects",
        &[
            "id",
            "name",
            "description",
            "cover_image",
            "created_at",
            "updated_at",
        ],
    )
    .await;
    assert_table_columns(
        &db,
        "categories",
        &[
            "id",
            "project_id",
            "parent_id",
            "name",
            "sort_order",
            "created_at",
            "updated_at",
        ],
    )
    .await;
    assert_table_columns(
        &db,
        "tag_schemas",
        &[
            "id",
            "project_id",
            "name",
            "description",
            "type",
            "target",
            "default_val",
            "range_min",
            "range_max",
            "sort_order",
            "created_at",
            "updated_at",
        ],
    )
    .await;
    assert_table_columns(
        &db,
        "entries",
        &[
            "_rowid",
            "id",
            "project_id",
            "category_id",
            "title",
            "summary",
            "content",
            "type",
            "tags",
            "images",
            "cover_path",
            "created_at",
            "updated_at",
        ],
    )
    .await;
    assert_table_columns(
        &db,
        "entries_fts",
        &["title", "summary", "content", "project_id"],
    )
    .await;
    assert_table_columns(
        &db,
        "entry_relations",
        &[
            "id",
            "project_id",
            "a_id",
            "b_id",
            "relation",
            "content",
            "created_at",
            "updated_at",
        ],
    )
    .await;
    assert_table_columns(
        &db,
        "entry_types",
        &[
            "id",
            "project_id",
            "name",
            "description",
            "icon",
            "color",
            "created_at",
            "updated_at",
        ],
    )
    .await;
    assert_table_columns(
        &db,
        "entry_links",
        &["id", "project_id", "a_id", "b_id", "created_at"],
    )
    .await;
    assert_table_columns(
        &db,
        "idea_notes",
        &[
            "id",
            "project_id",
            "content",
            "title",
            "status",
            "pinned",
            "created_at",
            "updated_at",
            "last_reviewed_at",
            "converted_entry_id",
        ],
    )
    .await;
    assert_table_columns(
        &db,
        "api_usage_log",
        &[
            "id",
            "session_id",
            "model",
            "provider",
            "modality",
            "prompt_tokens",
            "completion_tokens",
            "total_tokens",
            "created_at",
        ],
    )
    .await;

    for index in [
        "idx_entries_project",
        "idx_entries_category",
        "idx_entries_type",
        "idx_entries_project_updated",
        "idx_entries_project_category_updated",
        "idx_entries_project_type_updated",
        "idx_categories_project",
        "idx_categories_parent",
        "idx_tag_schemas_project",
        "idx_relations_a",
        "idx_relations_b",
        "idx_relations_project",
        "idx_entry_types_project",
        "idx_entry_types_name",
        "idx_entry_links_a",
        "idx_entry_links_b",
        "idx_entry_links_project",
        "idx_idea_notes_project",
        "idx_idea_notes_status",
        "idx_idea_notes_updated_at",
        "idx_idea_notes_pinned",
        "idx_idea_notes_project_status",
        "idx_idea_notes_pinned_updated",
        "idx_api_usage_session",
        "idx_api_usage_model",
        "idx_api_usage_created",
        "idx_api_usage_provider",
        "idx_api_usage_modality",
    ] {
        assert_schema_object_exists(&db, "index", index).await;
    }

    for sql in [
        "SELECT id, name, description, cover_image, created_at, updated_at FROM projects ORDER BY updated_at DESC LIMIT 1",
        "SELECT id, project_id, parent_id, name, sort_order, created_at, updated_at FROM categories WHERE project_id = zeroblob(16) ORDER BY sort_order, name",
        "SELECT id, project_id, name, description, type, target, default_val, range_min, range_max, sort_order, created_at, updated_at FROM tag_schemas WHERE project_id = zeroblob(16) ORDER BY sort_order, name",
        "SELECT _rowid, id, project_id, category_id, title, summary, content, type, tags, images, cover_path, created_at, updated_at FROM entries WHERE project_id = zeroblob(16) ORDER BY updated_at DESC LIMIT 1",
        "SELECT rowid, title, summary, content, project_id FROM entries_fts WHERE entries_fts MATCH 'schema' LIMIT 1",
        "SELECT id, project_id, name, description, icon, color, created_at, updated_at FROM entry_types WHERE project_id = zeroblob(16) ORDER BY name",
        "SELECT id, project_id, a_id, b_id, created_at FROM entry_links WHERE project_id = zeroblob(16) LIMIT 1",
        "SELECT id, project_id, a_id, b_id, relation, content, created_at, updated_at FROM entry_relations WHERE project_id = zeroblob(16) LIMIT 1",
        "SELECT id, project_id, content, title, status, pinned, created_at, updated_at, last_reviewed_at, converted_entry_id FROM idea_notes ORDER BY pinned DESC, updated_at DESC LIMIT 1",
        "SELECT id, session_id, model, provider, modality, prompt_tokens, completion_tokens, total_tokens, created_at FROM api_usage_log ORDER BY created_at DESC LIMIT 1",
    ] {
        assert_representative_query(&db, sql).await;
    }
}

#[tokio::test]
async fn test_project_crud() {
    let db = setup().await;

    let project = db
        .create_project(CreateProject {
            cover_image: None,
            name: "测试世界观".to_string(),
            description: Some("这是一个测试".to_string()),
        })
        .await
        .unwrap();

    assert_eq!(project.name, "测试世界观");
    assert_ne!(project.id, Uuid::nil());

    let fetched = db.get_project(&project.id).await.unwrap();
    assert_eq!(fetched.id, project.id);

    let updated = db
        .update_project(
            &project.id,
            UpdateProject {
                name: Some("改名后的世界观".to_string()),
                description: None,
                cover_image: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.name, "改名后的世界观");
    assert_eq!(updated.description, Some("这是一个测试".to_string()));

    let cleared = db
        .update_project(
            &project.id,
            UpdateProject {
                name: None,
                description: Some(None),
                cover_image: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(cleared.description, None);

    let list = db.list_projects().await.unwrap();
    assert!(list.iter().any(|p| p.id == project.id));

    db.delete_project(&project.id).await.unwrap();
    assert!(db.get_project(&project.id).await.is_err());
}

#[tokio::test]
async fn test_category_tree() {
    let db = setup().await;

    let project = db
        .create_project(CreateProject {
            cover_image: None,
            name: "分类树测试".to_string(),
            description: None,
        })
        .await
        .unwrap();

    let root = db
        .create_category(CreateCategory {
            project_id: project.id.clone(),
            parent_id: None,
            name: "人物".to_string(),
            sort_order: None,
        })
        .await
        .unwrap();
    assert!(root.parent_id.is_none());

    let child = db
        .create_category(CreateCategory {
            project_id: project.id.clone(),
            parent_id: Some(root.id.clone()),
            name: "主角".to_string(),
            sort_order: None,
        })
        .await
        .unwrap();
    assert_eq!(child.parent_id, Some(root.id.clone()));

    assert_eq!(db.list_categories(&project.id).await.unwrap().len(), 2);

    let moved = db
        .update_category(
            &child.id,
            UpdateCategory {
                parent_id: Some(None),
                name: None,
                sort_order: None,
            },
        )
        .await
        .unwrap();
    assert!(moved.parent_id.is_none());

    db.delete_project(&project.id).await.unwrap();
    assert!(db.list_categories(&project.id).await.unwrap().is_empty());
}

#[tokio::test]
async fn test_entry_with_tags() {
    let db = setup().await;

    let project = db
        .create_project(CreateProject {
            cover_image: None,
            name: "词条测试".to_string(),
            description: None,
        })
        .await
        .unwrap();

    let schema = db
        .create_tag_schema(CreateTagSchema {
            project_id: project.id.clone(),
            name: "魔法值".to_string(),
            description: Some("角色魔力上限".to_string()),
            r#type: "number".to_string(),
            target: vec!["character".to_string()],
            default_val: Some("0".to_string()),
            range_min: Some(0.0),
            range_max: Some(100.0),
            sort_order: None,
        })
        .await
        .unwrap();

    let entry = db
        .create_entry(CreateEntry {
            project_id: project.id.clone(),
            category_id: None,
            title: "艾莉丝".to_string(),
            summary: Some("初始摘要".to_string()),
            content: Some("# 艾莉丝\n\n主角，魔法少女。".to_string()),
            r#type: Some("character".to_string()),
            tags: Some(vec![EntryTag {
                schema_id: schema.id.clone(),
                value: serde_json::json!(85),
            }]),
            images: None,
            cover_path: None,
        })
        .await
        .unwrap();

    assert_eq!(entry.tags.0.len(), 1);
    assert_eq!(entry.tags.0[0].value, serde_json::json!(85));

    assert!(
        !db.search_entries(&project.id, "艾莉丝", EntryFilter::default(), 50)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        !db.search_entries(&project.id, "魔法少女", EntryFilter::default(), 50)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        db.search_entries(&project.id, "不存在的关键词xyz", EntryFilter::default(), 50)
            .await
            .unwrap()
            .is_empty()
    );

    let updated = db
        .update_entry(
            &entry.id,
            UpdateEntry {
                title: None,
                summary: None,
                content: None,
                category_id: None,
                r#type: None,
                tags: Some(vec![EntryTag {
                    schema_id: schema.id.clone(),
                    value: serde_json::json!(99),
                }]),
                images: None,
                cover_path: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.tags.0[0].value, serde_json::json!(99));
    assert_eq!(updated.summary, Some("初始摘要".to_string()));

    let cleared = db
        .update_entry(
            &entry.id,
            UpdateEntry {
                title: None,
                summary: Some(None),
                content: None,
                category_id: None,
                r#type: None,
                tags: None,
                images: None,
                cover_path: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(cleared.summary, None);

    db.delete_project(&project.id).await.unwrap();
}

#[tokio::test]
async fn test_entry_cover_path_explicit_and_fallback() {
    let db = setup().await;

    let project = db
        .create_project(CreateProject {
            cover_image: None,
            name: "封面路径测试".to_string(),
            description: None,
        })
        .await
        .unwrap();

    let original_cover = PathBuf::from("images/project/original.jpg");
    let thumb_cover = "images/project/thumbs/original_cover.jpg".to_string();
    let entry = db
        .create_entry(CreateEntry {
            project_id: project.id,
            category_id: None,
            title: "显式封面词条".to_string(),
            summary: None,
            content: None,
            r#type: None,
            tags: None,
            images: Some(vec![FCImage {
                path: original_cover.clone(),
                is_cover: true,
                caption: None,
            }]),
            cover_path: Some(thumb_cover.clone()),
        })
        .await
        .unwrap();
    assert_eq!(entry.cover_path, Some(thumb_cover.clone()));
    let listed = db
        .list_entries(&project.id, EntryFilter::default(), 10, 0)
        .await
        .unwrap();
    assert_eq!(listed[0].cover, Some(PathBuf::from(&thumb_cover)));

    let fallback = db
        .create_entry(CreateEntry {
            project_id: project.id,
            category_id: None,
            title: "兼容封面词条".to_string(),
            summary: None,
            content: None,
            r#type: None,
            tags: None,
            images: Some(vec![FCImage {
                path: original_cover.clone(),
                is_cover: true,
                caption: None,
            }]),
            cover_path: None,
        })
        .await
        .unwrap();
    assert_eq!(
        fallback.cover_path,
        Some(original_cover.to_string_lossy().to_string())
    );

    let updated_thumb = "images/project/thumbs/updated_cover.jpg".to_string();
    let updated = db
        .update_entry(
            &entry.id,
            UpdateEntry {
                title: None,
                summary: None,
                content: None,
                category_id: None,
                r#type: None,
                tags: None,
                images: None,
                cover_path: Some(Some(updated_thumb.clone())),
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.cover_path, Some(updated_thumb));

    let cleared = db
        .update_entry(
            &entry.id,
            UpdateEntry {
                title: None,
                summary: None,
                content: None,
                category_id: None,
                r#type: None,
                tags: None,
                images: None,
                cover_path: Some(None),
            },
        )
        .await
        .unwrap();
    assert_eq!(cleared.cover_path, None);
}

#[tokio::test]
async fn test_search_entries_matches_summary_with_fts() {
    let db = setup().await;

    let project = db
        .create_project(CreateProject {
            cover_image: None,
            name: "摘要搜索测试".to_string(),
            description: None,
        })
        .await
        .unwrap();

    let entry = db
        .create_entry(CreateEntry {
            project_id: project.id,
            category_id: None,
            title: "无关标题".to_string(),
            summary: Some("星际航线枢纽".to_string()),
            content: Some("正文不包含目标关键词".to_string()),
            r#type: None,
            tags: None,
            images: None,
            cover_path: None,
        })
        .await
        .unwrap();

    let results = db
        .search_entries(&project.id, "航线枢纽", EntryFilter::default(), 20)
        .await
        .unwrap();

    assert!(
        results.iter().any(|item| item.id == entry.id),
        "FTS 长查询应能命中 summary"
    );

    let overflow = db
        .search_entries(&project.id, "航线枢纽", EntryFilter::default(), usize::MAX)
        .await;
    assert!(overflow.is_err(), "过大的 limit 应被提前拒绝");
}

#[tokio::test]
async fn test_search_entries_treats_fts_special_chars_as_text() {
    let db = setup().await;

    let project = db
        .create_project(CreateProject {
            cover_image: None,
            name: "FTS 特殊字符项目".to_string(),
            description: None,
        })
        .await
        .unwrap();

    let entry = db
        .create_entry(CreateEntry {
            project_id: project.id,
            category_id: None,
            title: "foo\"bar a:b AND 星*门".to_string(),
            summary: Some("包含 FTS5 语法字符的摘要".to_string()),
            content: Some("普通正文".to_string()),
            r#type: None,
            tags: None,
            images: None,
            cover_path: None,
        })
        .await
        .unwrap();

    for query in ["foo\"bar", "a:b", "AND", "星*门", "*"] {
        let results = db
            .search_entries(&project.id, query, EntryFilter::default(), 20)
            .await
            .unwrap_or_else(|error| panic!("查询 {query:?} 不应触发 FTS5 语法错误: {error}"));

        assert!(
            results.iter().any(|item| item.id == entry.id),
            "查询 {query:?} 应按普通文本命中词条"
        );
    }
}

#[tokio::test]
async fn test_inspect_data() {
    let db = setup().await;

    let project = db
        .create_project(CreateProject {
            cover_image: None,
            name: "银河帝国".to_string(),
            description: Some("阿西莫夫风格的科幻世界观".to_string()),
        })
        .await
        .unwrap();

    let cat_people = db
        .create_category(CreateCategory {
            project_id: project.id.clone(),
            parent_id: None,
            name: "人物".to_string(),
            sort_order: Some(0),
        })
        .await
        .unwrap();

    let cat_hero = db
        .create_category(CreateCategory {
            project_id: project.id.clone(),
            parent_id: Some(cat_people.id.clone()),
            name: "主角".to_string(),
            sort_order: Some(0),
        })
        .await
        .unwrap();

    let schema_power = db
        .create_tag_schema(CreateTagSchema {
            project_id: project.id.clone(),
            name: "心理史学等级".to_string(),
            description: Some("掌握心理史学的程度".to_string()),
            r#type: "number".to_string(),
            target: vec!["character".to_string()],
            default_val: Some("0".to_string()),
            range_min: Some(0.0),
            range_max: Some(10.0),
            sort_order: Some(0),
        })
        .await
        .unwrap();

    let entry = db
        .create_entry(CreateEntry {
            project_id: project.id.clone(),
            category_id: Some(cat_hero.id.clone()),
            title: "哈里·谢顿".to_string(),
            summary: None,
            content: Some("# 哈里·谢顿\n\n心理史学的创始人，银河帝国末期的数学家。".to_string()),
            r#type: Some("character".to_string()),
            tags: Some(vec![EntryTag {
                schema_id: schema_power.id.clone(),
                value: serde_json::json!(10),
            }]),
            images: None,
            cover_path: None,
        })
        .await
        .unwrap();

    println!("\n=== 写入数据 ===");
    println!("project.id  = {}", project.id);
    println!("category.id = {}", cat_hero.id);
    println!("entry.id    = {}", entry.id);

    let results = db
        .search_entries(&project.id, "心理史学", EntryFilter::default(), 50)
        .await
        .unwrap();
    println!("\n=== 搜索\"心理史学\" ===");
    for r in &results {
        println!("  - {} ({})", r.title, r.id);
    }
    println!("\n数据已保留在 worldflow_dev.db，可在 RustRover 中查看");
}

#[tokio::test]
async fn test_entry_links_replace_and_query_by_id() {
    let db = setup().await;

    let project = db
        .create_project(CreateProject {
            cover_image: None,
            name: "链接测试".to_string(),
            description: None,
        })
        .await
        .unwrap();

    let source = db
        .create_entry(CreateEntry {
            project_id: project.id,
            category_id: None,
            title: "源词条".to_string(),
            summary: None,
            content: Some("旧正文仍可保留 [[目标一]] 这类按标题写法".to_string()),
            r#type: None,
            tags: None,
            images: None,
            cover_path: None,
        })
        .await
        .unwrap();

    let target1 = db
        .create_entry(CreateEntry {
            project_id: project.id,
            category_id: None,
            title: "目标一".to_string(),
            summary: None,
            content: Some("内容1".to_string()),
            r#type: None,
            tags: None,
            images: None,
            cover_path: None,
        })
        .await
        .unwrap();

    let target2 = db
        .create_entry(CreateEntry {
            project_id: project.id,
            category_id: None,
            title: "目标二".to_string(),
            summary: None,
            content: Some("内容2".to_string()),
            r#type: None,
            tags: None,
            images: None,
            cover_path: None,
        })
        .await
        .unwrap();

    let links = db
        .replace_outgoing_links(
            &project.id,
            &source.id,
            &[target1.id, target2.id, target1.id, source.id],
        )
        .await
        .unwrap();
    assert_eq!(links.len(), 2, "重复目标和自链接应被过滤");

    let outgoing = db.list_outgoing_links(&source.id).await.unwrap();
    assert_eq!(outgoing.len(), 2);
    assert!(outgoing.iter().any(|link| link.b_id == target1.id));
    assert!(outgoing.iter().any(|link| link.b_id == target2.id));

    let incoming_target1 = db.list_incoming_links(&target1.id).await.unwrap();
    assert_eq!(incoming_target1.len(), 1);
    assert_eq!(incoming_target1[0].a_id, source.id);

    let renamed = db
        .update_entry(
            &target1.id,
            UpdateEntry {
                title: Some("目标一（改名后）".to_string()),
                summary: None,
                content: None,
                category_id: None,
                r#type: None,
                tags: None,
                images: None,
                cover_path: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(renamed.title, "目标一（改名后）");

    let outgoing_after_rename = db.list_outgoing_links(&source.id).await.unwrap();
    assert!(
        outgoing_after_rename
            .iter()
            .any(|link| link.b_id == target1.id),
        "改名后链接仍应按 id 保持有效"
    );

    let replaced = db
        .replace_outgoing_links(&project.id, &source.id, &[target2.id])
        .await
        .unwrap();
    assert_eq!(replaced.len(), 1);
    assert_eq!(replaced[0].b_id, target2.id);
    assert!(
        db.list_incoming_links(&target1.id)
            .await
            .unwrap()
            .is_empty(),
        "被替换掉的旧链接应被移除"
    );

    let deleted = db.delete_links_from_entry(&source.id).await.unwrap();
    assert_eq!(deleted, 1);
    assert!(db.list_outgoing_links(&source.id).await.unwrap().is_empty());
}

#[tokio::test]
async fn test_entry_links_foreign_key_rejects_missing_target() {
    let db = setup().await;

    let project = db
        .create_project(CreateProject {
            cover_image: None,
            name: "链接约束测试".to_string(),
            description: None,
        })
        .await
        .unwrap();

    let source = db
        .create_entry(CreateEntry {
            project_id: project.id,
            category_id: None,
            title: "源词条".to_string(),
            summary: None,
            content: Some("内容".to_string()),
            r#type: None,
            tags: None,
            images: None,
            cover_path: None,
        })
        .await
        .unwrap();

    let result = db
        .create_link(CreateEntryLink {
            project_id: project.id,
            a_id: source.id,
            b_id: Uuid::now_v7(),
        })
        .await;

    assert!(result.is_err(), "不存在的 b_id 应被外键拒绝");
}

#[tokio::test]
async fn test_save_entry_bundle_updates_entry_links_and_relations_in_transaction() {
    let db = setup().await;

    let project = db
        .create_project(CreateProject {
            cover_image: None,
            name: "词条聚合保存测试".to_string(),
            description: None,
        })
        .await
        .unwrap();

    let source = db
        .create_entry(CreateEntry {
            project_id: project.id,
            category_id: None,
            title: "源词条".to_string(),
            summary: None,
            content: Some("旧正文".to_string()),
            r#type: None,
            tags: None,
            images: None,
            cover_path: None,
        })
        .await
        .unwrap();

    let target1 = db
        .create_entry(CreateEntry {
            project_id: project.id,
            category_id: None,
            title: "目标一".to_string(),
            summary: None,
            content: Some("内容1".to_string()),
            r#type: None,
            tags: None,
            images: None,
            cover_path: None,
        })
        .await
        .unwrap();

    let target2 = db
        .create_entry(CreateEntry {
            project_id: project.id,
            category_id: None,
            title: "目标二".to_string(),
            summary: None,
            content: Some("内容2".to_string()),
            r#type: None,
            tags: None,
            images: None,
            cover_path: None,
        })
        .await
        .unwrap();

    let kept_relation = db
        .create_relation(CreateEntryRelation {
            project_id: project.id,
            a_id: source.id,
            b_id: target1.id,
            relation: RelationDirection::OneWay,
            content: "旧关系".to_string(),
        })
        .await
        .unwrap();
    let stale_relation = db
        .create_relation(CreateEntryRelation {
            project_id: project.id,
            a_id: source.id,
            b_id: target2.id,
            relation: RelationDirection::OneWay,
            content: "待删除关系".to_string(),
        })
        .await
        .unwrap();

    let result = db
        .save_entry_bundle(SaveEntryBundle {
            project_id: project.id,
            entry_id: source.id,
            category_id: None,
            title: "源词条更新".to_string(),
            summary: Some("新摘要".to_string()),
            content: "新正文".to_string(),
            r#type: Some("character".to_string()),
            tags: None,
            images: None,
            cover_path: Some(None),
            outgoing_link_targets: vec![
                SaveEntryLinkTarget {
                    entry_id: None,
                    title: "目标二".to_string(),
                },
                SaveEntryLinkTarget {
                    entry_id: Some(target1.id),
                    title: "目标一".to_string(),
                },
                SaveEntryLinkTarget {
                    entry_id: Some(source.id),
                    title: "自链接应过滤".to_string(),
                },
            ],
            relation_patches: vec![
                SaveEntryRelationPatch {
                    id: Some(kept_relation.id),
                    a_id: source.id,
                    b_id: target1.id,
                    relation: RelationDirection::OneWay,
                    content: "新关系".to_string(),
                },
                SaveEntryRelationPatch {
                    id: None,
                    a_id: source.id,
                    b_id: target2.id,
                    relation: RelationDirection::TwoWay,
                    content: "双向关系".to_string(),
                },
            ],
        })
        .await
        .unwrap();

    assert_eq!(result.entry.title, "源词条更新");
    assert_eq!(result.entry.summary.as_deref(), Some("新摘要"));
    assert_eq!(
        result.outgoing_links.len(),
        2,
        "标题链接、id 链接应去重并过滤自链接"
    );
    assert!(
        result
            .outgoing_links
            .iter()
            .any(|link| link.b_id == target1.id)
    );
    assert!(
        result
            .outgoing_links
            .iter()
            .any(|link| link.b_id == target2.id)
    );
    assert!(result.incoming_links.is_empty());

    let stored_relations = db.list_relations_for_entry(&source.id).await.unwrap();
    assert_eq!(stored_relations.len(), 2);
    assert!(
        stored_relations
            .iter()
            .any(|relation| relation.id == kept_relation.id && relation.content == "新关系")
    );
    assert!(
        stored_relations
            .iter()
            .any(|relation| relation.relation == RelationDirection::TwoWay
                && relation.content == "双向关系")
    );
    assert!(
        !stored_relations
            .iter()
            .any(|relation| relation.id == stale_relation.id),
        "草稿中不存在的旧关系应被删除"
    );

    let rollback_result = db
        .save_entry_bundle(SaveEntryBundle {
            project_id: project.id,
            entry_id: source.id,
            category_id: None,
            title: "不应落库".to_string(),
            summary: Some("不应落库".to_string()),
            content: "不应落库".to_string(),
            r#type: Some("character".to_string()),
            tags: None,
            images: None,
            cover_path: None,
            outgoing_link_targets: vec![SaveEntryLinkTarget {
                entry_id: Some(Uuid::now_v7()),
                title: "不存在目标".to_string(),
            }],
            relation_patches: Vec::new(),
        })
        .await;
    assert!(rollback_result.is_err(), "无效链接目标应触发事务回滚");

    let source_after_rollback = db.get_entry(&source.id).await.unwrap();
    assert_eq!(source_after_rollback.title, "源词条更新");
    assert_eq!(source_after_rollback.content, "新正文");
    assert_eq!(db.list_outgoing_links(&source.id).await.unwrap().len(), 2);
}

// ======================== 词条类型测试 ========================

#[tokio::test]
async fn test_builtin_entry_types_constant() {
    use worldflow_core::models::BUILTIN_ENTRY_TYPES;

    assert_eq!(BUILTIN_ENTRY_TYPES.len(), 9, "Should have 9 builtin types");

    let keys: Vec<&str> = BUILTIN_ENTRY_TYPES.iter().map(|t| t.key).collect();
    let mut unique_keys = keys.clone();
    unique_keys.sort();
    unique_keys.dedup();
    assert_eq!(
        keys.len(),
        unique_keys.len(),
        "All builtin type keys should be unique"
    );

    // 验证包含所有9个类型
    assert!(BUILTIN_ENTRY_TYPES.iter().any(|t| t.key == "character"));
    assert!(BUILTIN_ENTRY_TYPES.iter().any(|t| t.key == "organization"));
    assert!(BUILTIN_ENTRY_TYPES.iter().any(|t| t.key == "location"));
    assert!(BUILTIN_ENTRY_TYPES.iter().any(|t| t.key == "item"));
    assert!(BUILTIN_ENTRY_TYPES.iter().any(|t| t.key == "creature"));
    assert!(BUILTIN_ENTRY_TYPES.iter().any(|t| t.key == "event"));
    assert!(BUILTIN_ENTRY_TYPES.iter().any(|t| t.key == "concept"));
    assert!(BUILTIN_ENTRY_TYPES.iter().any(|t| t.key == "culture"));
    assert!(BUILTIN_ENTRY_TYPES.iter().any(|t| t.key == "else"));
}

#[tokio::test]
async fn test_is_builtin_type_function() {
    use worldflow_core::models::is_builtin_type;

    assert!(is_builtin_type("character"), "short key should be builtin");
    assert!(
        is_builtin_type("organization"),
        "short key should be builtin"
    );
    assert!(
        !is_builtin_type("018f0d4e-6b30-7c2a-9f65-8d7b3a1c2e4f"),
        "UUID should not be builtin"
    );
}

#[tokio::test]
async fn test_get_builtin_type_function() {
    use worldflow_core::models::get_builtin_type;

    let ct = get_builtin_type("character");
    assert!(ct.is_some());
    assert_eq!(ct.unwrap().name, "人物");

    let invalid = get_builtin_type("invalid_type");
    assert!(invalid.is_none());
}

#[tokio::test]
async fn test_create_custom_entry_type() {
    let db = setup().await;

    let project = db
        .create_project(CreateProject {
            cover_image: None,
            name: "自定义类型测试".to_string(),
            description: None,
        })
        .await
        .unwrap();

    let custom_type = db
        .create_entry_type(CreateCustomEntryType {
            project_id: project.id.clone(),
            name: "自定义类型1".to_string(),
            description: Some("这是一个自定义类型".to_string()),
            icon: Some("🎨".to_string()),
            color: Some("#FF5733".to_string()),
        })
        .await
        .unwrap();

    assert_ne!(custom_type.id, Uuid::nil());
    assert_eq!(
        custom_type.id.to_string().len(),
        36,
        "ID should be UUID format"
    );
    assert_eq!(custom_type.name, "自定义类型1");
    assert_eq!(
        custom_type.description,
        Some("这是一个自定义类型".to_string())
    );
    assert!(!custom_type.created_at.is_empty());
    assert!(!custom_type.updated_at.is_empty());
}

#[tokio::test]
async fn test_custom_entry_type_unique_per_project() {
    let db = setup().await;

    let proj1 = db
        .create_project(CreateProject {
            cover_image: None,
            name: "项目1".to_string(),
            description: None,
        })
        .await
        .unwrap();

    let proj2 = db
        .create_project(CreateProject {
            cover_image: None,
            name: "项目2".to_string(),
            description: None,
        })
        .await
        .unwrap();

    // 在proj1中创建类型
    let _type1 = db
        .create_entry_type(CreateCustomEntryType {
            project_id: proj1.id.clone(),
            name: "Custom1".to_string(),
            description: None,
            icon: None,
            color: None,
        })
        .await
        .unwrap();

    // 在proj1中用相同名称创建应该失败
    let result = db
        .create_entry_type(CreateCustomEntryType {
            project_id: proj1.id.clone(),
            name: "Custom1".to_string(),
            description: None,
            icon: None,
            color: None,
        })
        .await;
    assert!(result.is_err(), "Same name in same project should fail");

    // 在proj2中用相同名称创建应该成功
    let _type2 = db
        .create_entry_type(CreateCustomEntryType {
            project_id: proj2.id.clone(),
            name: "Custom1".to_string(),
            description: None,
            icon: None,
            color: None,
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn test_update_custom_entry_type() {
    let db = setup().await;

    let project = db
        .create_project(CreateProject {
            cover_image: None,
            name: "更新测试".to_string(),
            description: None,
        })
        .await
        .unwrap();

    let custom_type = db
        .create_entry_type(CreateCustomEntryType {
            project_id: project.id.clone(),
            name: "原始名称".to_string(),
            description: Some("原始描述".to_string()),
            icon: Some("📦".to_string()),
            color: Some("#000000".to_string()),
        })
        .await
        .unwrap();

    // 更新名称
    let updated = db
        .update_entry_type(
            &custom_type.id,
            UpdateCustomEntryType {
                name: Some("新名称".to_string()),
                description: None,
                icon: None,
                color: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(updated.name, "新名称");
    assert_eq!(
        updated.description,
        Some("原始描述".to_string()),
        "description should remain"
    );

    // 清空描述
    let updated2 = db
        .update_entry_type(
            &custom_type.id,
            UpdateCustomEntryType {
                name: None,
                description: Some(None),
                icon: None,
                color: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(updated2.description, None, "description should be cleared");
}

#[tokio::test]
async fn test_delete_custom_entry_type_unused() {
    let db = setup().await;

    let project = db
        .create_project(CreateProject {
            cover_image: None,
            name: "删除测试".to_string(),
            description: None,
        })
        .await
        .unwrap();

    let custom_type = db
        .create_entry_type(CreateCustomEntryType {
            project_id: project.id.clone(),
            name: "待删除类型".to_string(),
            description: None,
            icon: None,
            color: None,
        })
        .await
        .unwrap();

    // 删除未使用的类型应该成功
    db.delete_entry_type(&custom_type.id).await.unwrap();

    // 验证已删除
    let result = db.get_entry_type(&custom_type.id).await;
    assert!(result.is_err(), "Deleted type should not exist");
}

#[tokio::test]
async fn test_delete_custom_entry_type_in_use() {
    let db = setup().await;

    let project = db
        .create_project(CreateProject {
            cover_image: None,
            name: "使用中删除测试".to_string(),
            description: None,
        })
        .await
        .unwrap();

    let custom_type = db
        .create_entry_type(CreateCustomEntryType {
            project_id: project.id.clone(),
            name: "被使用的类型".to_string(),
            description: None,
            icon: None,
            color: None,
        })
        .await
        .unwrap();

    // 创建使用该类型的entry
    let _entry = db
        .create_entry(CreateEntry {
            project_id: project.id.clone(),
            category_id: None,
            title: "测试词条".to_string(),
            summary: None,
            content: Some("内容".to_string()),
            r#type: Some(custom_type.id.to_string()),
            tags: None,
            images: None,
            cover_path: None,
        })
        .await
        .unwrap();

    // 尝试删除应该失败
    let result = db.delete_entry_type(&custom_type.id).await;
    assert!(result.is_err(), "Cannot delete type in use");

    // 验证类型仍存在
    let still_exists = db.get_entry_type(&custom_type.id).await;
    assert!(still_exists.is_ok(), "Type should still exist");
}

#[tokio::test]
async fn test_list_all_entry_types_structure() {
    let db = setup().await;

    let project = db
        .create_project(CreateProject {
            cover_image: None,
            name: "列表测试".to_string(),
            description: None,
        })
        .await
        .unwrap();

    // 先列出，应该只有9个内置类型
    let all_types = db.list_all_entry_types(&project.id).await.unwrap();
    assert_eq!(all_types.len(), 9, "Should have 9 builtin types initially");

    // 验证都是Builtin类型
    for t in &all_types {
        match t {
            EntryTypeView::Builtin { .. } => {}
            EntryTypeView::Custom(_) => panic!("Should not have custom types yet"),
        }
    }

    // 创建2个自定义类型
    let _custom1 = db
        .create_entry_type(CreateCustomEntryType {
            project_id: project.id.clone(),
            name: "自定义1".to_string(),
            description: None,
            icon: None,
            color: None,
        })
        .await
        .unwrap();

    let _custom2 = db
        .create_entry_type(CreateCustomEntryType {
            project_id: project.id.clone(),
            name: "自定义2".to_string(),
            description: None,
            icon: None,
            color: None,
        })
        .await
        .unwrap();

    // 重新列出，应该有9个内置+2个自定义
    let all_types = db.list_all_entry_types(&project.id).await.unwrap();
    assert_eq!(
        all_types.len(),
        11,
        "Should have 11 types after adding 2 custom"
    );

    // 验证内置类型在前
    let first_9_builtin = all_types[0..9]
        .iter()
        .all(|t| matches!(t, EntryTypeView::Builtin { .. }));
    assert!(first_9_builtin, "First 9 should be builtin");

    let last_2_custom = all_types[9..11]
        .iter()
        .all(|t| matches!(t, EntryTypeView::Custom(_)));
    assert!(last_2_custom, "Last 2 should be custom");
}

#[tokio::test]
async fn test_list_custom_entry_types() {
    let db = setup().await;

    let project = db
        .create_project(CreateProject {
            cover_image: None,
            name: "自定义类型列表测试".to_string(),
            description: None,
        })
        .await
        .unwrap();

    // 创建3个自定义类型
    for i in 1..=3 {
        db.create_entry_type(CreateCustomEntryType {
            project_id: project.id.clone(),
            name: format!("类型{}", i),
            description: None,
            icon: None,
            color: None,
        })
        .await
        .unwrap();
    }

    let custom = db.list_custom_entry_types(&project.id).await.unwrap();
    assert_eq!(custom.len(), 3, "Should have 3 custom types");
}

#[tokio::test]
async fn test_filter_entries_by_builtin_type() {
    let db = setup().await;

    let project = db
        .create_project(CreateProject {
            cover_image: None,
            name: "内置类型过滤测试".to_string(),
            description: None,
        })
        .await
        .unwrap();

    // 创建不同类型的词条
    let entry1 = db
        .create_entry(CreateEntry {
            project_id: project.id.clone(),
            category_id: None,
            title: "人物1".to_string(),
            summary: None,
            content: Some("人物描述".to_string()),
            r#type: Some("character".to_string()),
            tags: None,
            images: None,
            cover_path: None,
        })
        .await
        .unwrap();

    let entry2 = db
        .create_entry(CreateEntry {
            project_id: project.id.clone(),
            category_id: None,
            title: "地点1".to_string(),
            summary: None,
            content: Some("地点描述".to_string()),
            r#type: Some("location".to_string()),
            tags: None,
            images: None,
            cover_path: None,
        })
        .await
        .unwrap();

    let entry3 = db
        .create_entry(CreateEntry {
            project_id: project.id.clone(),
            category_id: None,
            title: "物品1".to_string(),
            summary: None,
            content: Some("物品描述".to_string()),
            r#type: Some("item".to_string()),
            tags: None,
            images: None,
            cover_path: None,
        })
        .await
        .unwrap();

    // 按character类型过滤
    let character_entries = db
        .list_entries(
            &project.id,
            EntryFilter {
                entry_type: Some("character"),
                ..Default::default()
            },
            100,
            0,
        )
        .await
        .unwrap();

    assert!(character_entries.iter().any(|e| e.id == entry1.id));
    assert!(!character_entries.iter().any(|e| e.id == entry2.id));
    assert!(!character_entries.iter().any(|e| e.id == entry3.id));

    // 按location类型过滤
    let location_entries = db
        .list_entries(
            &project.id,
            EntryFilter {
                entry_type: Some("location"),
                ..Default::default()
            },
            100,
            0,
        )
        .await
        .unwrap();

    assert!(!location_entries.iter().any(|e| e.id == entry1.id));
    assert!(location_entries.iter().any(|e| e.id == entry2.id));
    assert!(!location_entries.iter().any(|e| e.id == entry3.id));
}

#[tokio::test]
async fn test_filter_entries_by_custom_type_uuid() {
    let db = setup().await;

    let project = db
        .create_project(CreateProject {
            cover_image: None,
            name: "自定义类型过滤测试".to_string(),
            description: None,
        })
        .await
        .unwrap();

    // 创建自定义类型
    let custom_type = db
        .create_entry_type(CreateCustomEntryType {
            project_id: project.id.clone(),
            name: "自定义类型".to_string(),
            description: None,
            icon: None,
            color: None,
        })
        .await
        .unwrap();

    // 创建使用自定义类型的词条
    let entry = db
        .create_entry(CreateEntry {
            project_id: project.id.clone(),
            category_id: None,
            title: "使用自定义类型的词条".to_string(),
            summary: None,
            content: Some("内容".to_string()),
            r#type: Some(custom_type.id.to_string()),
            tags: None,
            images: None,
            cover_path: None,
        })
        .await
        .unwrap();

    // 按自定义类型UUID过滤
    let custom_type_id = custom_type.id.to_string();
    let filtered = db
        .list_entries(
            &project.id,
            EntryFilter {
                entry_type: Some(custom_type_id.as_str()),
                ..Default::default()
            },
            100,
            0,
        )
        .await
        .unwrap();

    assert!(
        filtered.iter().any(|e| e.id == entry.id),
        "Should find entry with custom type"
    );
}

#[tokio::test]
async fn test_entry_type_validation_rejects_invalid_and_cross_project() {
    let db = setup().await;

    let project1 = db
        .create_project(CreateProject {
            cover_image: None,
            name: "类型校验项目1".to_string(),
            description: None,
        })
        .await
        .unwrap();
    let project2 = db
        .create_project(CreateProject {
            cover_image: None,
            name: "类型校验项目2".to_string(),
            description: None,
        })
        .await
        .unwrap();

    let invalid_builtin = db
        .create_entry(CreateEntry {
            project_id: project1.id,
            category_id: None,
            title: "非法类型词条".to_string(),
            summary: None,
            content: None,
            r#type: Some("unknown_type".to_string()),
            tags: None,
            images: None,
            cover_path: None,
        })
        .await;
    assert!(invalid_builtin.is_err(), "未知内置类型 key 应被拒绝");

    let custom_type = db
        .create_entry_type(CreateCustomEntryType {
            project_id: project1.id,
            name: "项目1专属类型".to_string(),
            description: None,
            icon: None,
            color: None,
        })
        .await
        .unwrap();

    let cross_project = db
        .create_entry(CreateEntry {
            project_id: project2.id,
            category_id: None,
            title: "跨项目类型词条".to_string(),
            summary: None,
            content: None,
            r#type: Some(custom_type.id.to_string()),
            tags: None,
            images: None,
            cover_path: None,
        })
        .await;
    assert!(cross_project.is_err(), "跨项目自定义类型应被拒绝");

    let valid = db
        .create_entry(CreateEntry {
            project_id: project1.id,
            category_id: None,
            title: "合法自定义类型词条".to_string(),
            summary: None,
            content: None,
            r#type: Some(custom_type.id.to_string()),
            tags: None,
            images: None,
            cover_path: None,
        })
        .await
        .unwrap();
    assert_eq!(valid.r#type, Some(custom_type.id.to_string()));
}

#[tokio::test]
async fn test_create_entries_bulk_validates_types_by_project_and_type() {
    let db = setup().await;

    let project1 = db
        .create_project(CreateProject {
            cover_image: None,
            name: "批量类型校验项目1".to_string(),
            description: None,
        })
        .await
        .unwrap();
    let project2 = db
        .create_project(CreateProject {
            cover_image: None,
            name: "批量类型校验项目2".to_string(),
            description: None,
        })
        .await
        .unwrap();

    let custom_type = db
        .create_entry_type(CreateCustomEntryType {
            project_id: project1.id,
            name: "批量专属类型".to_string(),
            description: None,
            icon: None,
            color: None,
        })
        .await
        .unwrap();

    let invalid_builtin = db
        .create_entries_bulk(vec![CreateEntry {
            project_id: project1.id,
            category_id: None,
            title: "批量非法内置类型".to_string(),
            summary: None,
            content: None,
            r#type: Some("unknown_type".to_string()),
            tags: None,
            images: None,
            cover_path: None,
        }])
        .await;
    assert!(invalid_builtin.is_err(), "批量创建应拒绝未知内置类型 key");

    let cross_project = db
        .create_entries_bulk(vec![CreateEntry {
            project_id: project2.id,
            category_id: None,
            title: "批量跨项目类型".to_string(),
            summary: None,
            content: None,
            r#type: Some(custom_type.id.to_string()),
            tags: None,
            images: None,
            cover_path: None,
        }])
        .await;
    assert!(cross_project.is_err(), "批量创建应拒绝其他项目的自定义类型");

    let custom_type_id = custom_type.id.to_string();
    let created_count = db
        .create_entries_bulk(vec![
            CreateEntry {
                project_id: project1.id,
                category_id: None,
                title: "批量自定义类型1".to_string(),
                summary: None,
                content: None,
                r#type: Some(custom_type_id.clone()),
                tags: None,
                images: None,
                cover_path: None,
            },
            CreateEntry {
                project_id: project1.id,
                category_id: None,
                title: "批量自定义类型2".to_string(),
                summary: None,
                content: None,
                r#type: Some(custom_type_id.clone()),
                tags: None,
                images: None,
                cover_path: None,
            },
            CreateEntry {
                project_id: project1.id,
                category_id: None,
                title: "批量内置类型".to_string(),
                summary: None,
                content: None,
                r#type: Some("character".to_string()),
                tags: None,
                images: None,
                cover_path: None,
            },
            CreateEntry {
                project_id: project1.id,
                category_id: None,
                title: "批量无类型".to_string(),
                summary: None,
                content: None,
                r#type: None,
                tags: None,
                images: None,
                cover_path: None,
            },
        ])
        .await
        .unwrap();
    assert_eq!(created_count, 4);

    let custom_entries = db
        .list_entries(
            &project1.id,
            EntryFilter {
                entry_type: Some(custom_type_id.as_str()),
                ..Default::default()
            },
            100,
            0,
        )
        .await
        .unwrap();
    assert_eq!(custom_entries.len(), 2);
}

#[tokio::test]
async fn test_cascade_delete_on_project_delete() {
    let db = setup().await;

    let project = db
        .create_project(CreateProject {
            cover_image: None,
            name: "级联删除测试".to_string(),
            description: None,
        })
        .await
        .unwrap();

    // 创建自定义类型
    let _custom1 = db
        .create_entry_type(CreateCustomEntryType {
            project_id: project.id.clone(),
            name: "级联删除类型1".to_string(),
            description: None,
            icon: None,
            color: None,
        })
        .await
        .unwrap();

    let _custom2 = db
        .create_entry_type(CreateCustomEntryType {
            project_id: project.id.clone(),
            name: "级联删除类型2".to_string(),
            description: None,
            icon: None,
            color: None,
        })
        .await
        .unwrap();

    // 删除项目
    db.delete_project(&project.id).await.unwrap();

    // 验证自定义类型已被级联删除
    let remaining_types = db.list_custom_entry_types(&project.id).await.unwrap();
    assert!(
        remaining_types.is_empty(),
        "Custom types should be deleted with project"
    );
}

// ======================== 灵感笔记测试 ========================

/// 辅助：创建测试便签
async fn make_note(db: &SqliteDb, content: &str, project_id: Option<uuid::Uuid>) -> IdeaNote {
    db.create_idea_note(CreateIdeaNote {
        project_id,
        content: content.to_string(),
        title: None,
        pinned: None,
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn test_idea_note_create_and_get() {
    // 创建便签、读取、验证字段默认值
    let db = setup().await;

    let note = db
        .create_idea_note(CreateIdeaNote {
            project_id: None,
            content: "突然想到一个设定：世界末日是循环时间".to_string(),
            title: Some("时间循环".to_string()),
            pinned: Some(true),
        })
        .await
        .unwrap();

    assert_eq!(note.content, "突然想到一个设定：世界末日是循环时间");
    assert_eq!(note.title.as_deref(), Some("时间循环"));
    assert_eq!(note.status, IdeaNoteStatus::Inbox);
    assert!(note.pinned);
    assert!(note.project_id.is_none());
    assert!(note.last_reviewed_at.is_none());
    assert!(note.converted_entry_id.is_none());

    // 通过 id 获取
    let fetched = db.get_idea_note(&note.id).await.unwrap();
    assert_eq!(fetched.id, note.id);
    assert_eq!(fetched.content, note.content);
}

#[tokio::test]
async fn test_idea_note_no_title_required() {
    // content 是唯一必填字段，title 允许为空
    let db = setup().await;

    let note = db
        .create_idea_note(CreateIdeaNote {
            project_id: None,
            content: "无标题灵感".to_string(),
            title: None,
            pinned: None,
        })
        .await
        .unwrap();

    assert!(note.title.is_none());
    assert!(!note.pinned);
    assert_eq!(note.status, IdeaNoteStatus::Inbox);
}

#[tokio::test]
async fn test_idea_note_get_not_found() {
    // 查询不存在的 id 应返回 NotFound
    let db = setup().await;
    let result = db.get_idea_note(&Uuid::now_v7()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_idea_note_update_status_and_pinned() {
    // 更新状态与置顶
    let db = setup().await;

    let note = make_note(&db, "需要归类的灵感", None).await;
    assert_eq!(note.status, IdeaNoteStatus::Inbox);
    assert!(!note.pinned);

    let updated = db
        .update_idea_note(
            &note.id,
            UpdateIdeaNote {
                status: Some(IdeaNoteStatus::Processed),
                pinned: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    assert_eq!(updated.status, IdeaNoteStatus::Processed);
    assert!(updated.pinned);
    assert_eq!(updated.content, note.content);
}

#[tokio::test]
async fn test_idea_note_update_title_clear() {
    // 测试 Option<Option<T>> 模式：Some(None) 清空标题
    let db = setup().await;

    let note = db
        .create_idea_note(CreateIdeaNote {
            project_id: None,
            content: "有标题的灵感".to_string(),
            title: Some("初始标题".to_string()),
            pinned: None,
        })
        .await
        .unwrap();

    assert!(note.title.is_some());

    // 更新内容
    let updated = db
        .update_idea_note(
            &note.id,
            UpdateIdeaNote {
                content: Some("更新后的内容".to_string()),
                title: Some(None), // 清空标题
                ..Default::default()
            },
        )
        .await
        .unwrap();

    assert_eq!(updated.content, "更新后的内容");
    assert!(updated.title.is_none());
}

#[tokio::test]
async fn test_idea_note_update_empty_noop() {
    // 空 UpdateIdeaNote 不修改任何字段
    let db = setup().await;

    let note = make_note(&db, "不变的灵感", None).await;
    let updated = db
        .update_idea_note(&note.id, UpdateIdeaNote::default())
        .await
        .unwrap();

    assert_eq!(updated.content, note.content);
    assert_eq!(updated.status, note.status);
}

#[tokio::test]
async fn test_idea_note_delete() {
    // 删除后查询应返回 NotFound
    let db = setup().await;

    let note = make_note(&db, "待删除的灵感", None).await;
    db.delete_idea_note(&note.id).await.unwrap();

    assert!(db.get_idea_note(&note.id).await.is_err());
}

#[tokio::test]
async fn test_idea_note_delete_not_found() {
    // 删除不存在的 id 应返回 NotFound
    let db = setup().await;
    let result = db.delete_idea_note(&Uuid::now_v7()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_idea_note_list_filter_by_project() {
    // 按 project_id 过滤
    let db = setup().await;

    let project = db
        .create_project(CreateProject {
            cover_image: None,
            name: "便签测试项目".to_string(),
            description: None,
        })
        .await
        .unwrap();

    // 绑定项目的便签 ×2，全局便签 ×1
    make_note(&db, "项目内灵感A", Some(project.id)).await;
    make_note(&db, "项目内灵感B", Some(project.id)).await;
    make_note(&db, "全局灵感，无项目", None).await;

    let in_project = db
        .list_idea_notes(
            IdeaNoteFilter {
                project_id: Some(&project.id),
                ..Default::default()
            },
            100,
            0,
        )
        .await
        .unwrap();

    assert_eq!(in_project.len(), 2);
    assert!(in_project.iter().all(|n| n.project_id == Some(project.id)));
}

#[tokio::test]
async fn test_idea_note_list_no_project_filter() {
    // project_id 为 None 时返回全部便签
    let db = setup().await;

    let project = db
        .create_project(CreateProject {
            cover_image: None,
            name: "便签全量测试".to_string(),
            description: None,
        })
        .await
        .unwrap();

    make_note(&db, "项目便签", Some(project.id)).await;
    make_note(&db, "全局便签", None).await;

    let all = db
        .list_idea_notes(IdeaNoteFilter::default(), 100, 0)
        .await
        .unwrap();

    assert_eq!(all.len(), 2);
}

#[tokio::test]
async fn test_idea_note_list_filter_by_status() {
    // 按状态过滤
    let db = setup().await;

    let n1 = make_note(&db, "收件箱便签", None).await;
    let n2 = make_note(&db, "另一条收件箱", None).await;
    let _n3 = make_note(&db, "已处理便签", None).await;

    // 把 n3 更新为 Processed
    db.update_idea_note(
        &_n3.id,
        UpdateIdeaNote {
            status: Some(IdeaNoteStatus::Processed),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let inbox = db
        .list_idea_notes(
            IdeaNoteFilter {
                status: Some(&IdeaNoteStatus::Inbox),
                ..Default::default()
            },
            100,
            0,
        )
        .await
        .unwrap();

    assert_eq!(inbox.len(), 2);
    assert!(inbox.iter().any(|n| n.id == n1.id));
    assert!(inbox.iter().any(|n| n.id == n2.id));

    let processed = db
        .list_idea_notes(
            IdeaNoteFilter {
                status: Some(&IdeaNoteStatus::Processed),
                ..Default::default()
            },
            100,
            0,
        )
        .await
        .unwrap();

    assert_eq!(processed.len(), 1);
    assert_eq!(processed[0].id, _n3.id);
}

#[tokio::test]
async fn test_idea_note_list_pinned_first() {
    // 置顶便签排在前面
    let db = setup().await;

    make_note(&db, "普通便签", None).await;

    let pinned = db
        .create_idea_note(CreateIdeaNote {
            project_id: None,
            content: "置顶便签".to_string(),
            title: None,
            pinned: Some(true),
        })
        .await
        .unwrap();

    let list = db
        .list_idea_notes(IdeaNoteFilter::default(), 10, 0)
        .await
        .unwrap();

    assert_eq!(list.len(), 2);
    assert_eq!(list[0].id, pinned.id, "置顶便签应排在最前");
}

#[tokio::test]
async fn test_idea_note_list_pagination() {
    // 分页：limit / offset
    let db = setup().await;

    for i in 0..5 {
        make_note(&db, &format!("便签 {i}"), None).await;
    }

    let page1 = db
        .list_idea_notes(IdeaNoteFilter::default(), 3, 0)
        .await
        .unwrap();
    let page2 = db
        .list_idea_notes(IdeaNoteFilter::default(), 3, 3)
        .await
        .unwrap();

    assert_eq!(page1.len(), 3);
    assert_eq!(page2.len(), 2);

    // 两页 id 不重叠
    for n1 in &page1 {
        assert!(!page2.iter().any(|n2| n2.id == n1.id));
    }
}

#[tokio::test]
async fn test_idea_note_without_project_works() {
    // project_id 为 None 的全局便签应正常 CRUD
    let db = setup().await;

    let note = db
        .create_idea_note(CreateIdeaNote {
            project_id: None,
            content: "无项目的全局灵感".to_string(),
            title: None,
            pinned: None,
        })
        .await
        .unwrap();

    assert!(note.project_id.is_none());

    let fetched = db.get_idea_note(&note.id).await.unwrap();
    assert!(fetched.project_id.is_none());

    let updated = db
        .update_idea_note(
            &note.id,
            UpdateIdeaNote {
                content: Some("更新后的全局灵感".to_string()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    assert_eq!(updated.content, "更新后的全局灵感");
    assert!(updated.project_id.is_none());

    db.delete_idea_note(&note.id).await.unwrap();
    assert!(db.get_idea_note(&note.id).await.is_err());
}

#[tokio::test]
async fn test_idea_note_set_null_on_project_delete() {
    // 删除项目时，绑定该项目的便签应降级为全局便签（project_id 变 NULL），而非被删除
    let db = setup().await;

    let project = db
        .create_project(CreateProject {
            cover_image: None,
            name: "SET NULL 测试".to_string(),
            description: None,
        })
        .await
        .unwrap();

    let note = make_note(&db, "项目被删后变为全局便签", Some(project.id)).await;
    assert_eq!(note.project_id, Some(project.id));

    db.delete_project(&project.id).await.unwrap();

    // 便签本身应仍然存在
    let orphaned = db.get_idea_note(&note.id).await.expect("便签不应被删除");
    assert!(
        orphaned.project_id.is_none(),
        "project_id 应被置 NULL，便签降级为全局便签"
    );
    assert_eq!(orphaned.content, note.content);
}

#[tokio::test]
async fn test_idea_note_status_parse() {
    // 枚举字符串互转
    use std::str::FromStr;
    assert_eq!(
        IdeaNoteStatus::from_str("inbox").unwrap(),
        IdeaNoteStatus::Inbox
    );
    assert_eq!(
        IdeaNoteStatus::from_str("processed").unwrap(),
        IdeaNoteStatus::Processed
    );
    assert_eq!(
        IdeaNoteStatus::from_str("archived").unwrap(),
        IdeaNoteStatus::Archived
    );
    assert!(IdeaNoteStatus::from_str("unknown").is_err());

    assert_eq!(IdeaNoteStatus::Inbox.as_str(), "inbox");
    assert_eq!(IdeaNoteStatus::Processed.as_str(), "processed");
    assert_eq!(IdeaNoteStatus::Archived.as_str(), "archived");
}

#[tokio::test]
async fn test_idea_note_list_only_global() {
    // only_global = true 只返回 project_id IS NULL 的便签，不包含有项目的便签
    let db = setup().await;

    let project = db
        .create_project(CreateProject {
            cover_image: None,
            name: "全局筛选测试".to_string(),
            description: None,
        })
        .await
        .unwrap();

    let global1 = make_note(&db, "全局便签1", None).await;
    let global2 = make_note(&db, "全局便签2", None).await;
    make_note(&db, "项目便签，不应出现", Some(project.id)).await;

    let result = db
        .list_idea_notes(
            IdeaNoteFilter {
                only_global: true,
                ..Default::default()
            },
            100,
            0,
        )
        .await
        .unwrap();

    assert_eq!(result.len(), 2);
    assert!(result.iter().all(|n| n.project_id.is_none()));
    assert!(result.iter().any(|n| n.id == global1.id));
    assert!(result.iter().any(|n| n.id == global2.id));
}

#[tokio::test]
async fn test_idea_note_list_only_global_after_project_delete() {
    // 项目删除后，原属该项目的便签降级为全局便签，only_global 应能查到它们
    let db = setup().await;

    let project = db
        .create_project(CreateProject {
            cover_image: None,
            name: "项目删除后全局可见".to_string(),
            description: None,
        })
        .await
        .unwrap();

    let note = make_note(&db, "原属项目、删项目后变全局", Some(project.id)).await;

    // 删项目前，only_global 查不到它
    let before = db
        .list_idea_notes(
            IdeaNoteFilter {
                only_global: true,
                ..Default::default()
            },
            100,
            0,
        )
        .await
        .unwrap();
    assert!(!before.iter().any(|n| n.id == note.id));

    db.delete_project(&project.id).await.unwrap();

    // 删项目后，only_global 可以查到它（已降级为全局便签）
    let after = db
        .list_idea_notes(
            IdeaNoteFilter {
                only_global: true,
                ..Default::default()
            },
            100,
            0,
        )
        .await
        .unwrap();
    assert!(
        after.iter().any(|n| n.id == note.id),
        "降级后的便签应出现在 only_global 结果中"
    );
}

#[tokio::test]
async fn test_idea_note_list_conflict_error() {
    // only_global 和 project_id 同时设置应返回 InvalidInput 错误
    let db = setup().await;
    let some_id = Uuid::now_v7();

    let result = db
        .list_idea_notes(
            IdeaNoteFilter {
                only_global: true,
                project_id: Some(&some_id),
                ..Default::default()
            },
            100,
            0,
        )
        .await;

    assert!(
        result.is_err(),
        "同时设置 only_global 和 project_id 应返回错误"
    );
}
