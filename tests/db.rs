use worldflow_core::{models::*, CategoryOps, EntryOps, ProjectOps, SqliteDb, TagSchemaOps};

async fn setup() -> SqliteDb {
    let db_path = format!(
        "sqlite:{}/worldflow_dev.db?mode=rwc",
        env!("CARGO_MANIFEST_DIR").replace('\\', "/")
    );
    SqliteDb::new(&db_path).await.unwrap()
}

#[tokio::test]
async fn test_project_crud() {
    let db = setup().await;

    let project = db.create_project(CreateProject {
        name: "测试世界观".to_string(),
        description: Some("这是一个测试".to_string()),
    }).await.unwrap();

    assert_eq!(project.name, "测试世界观");
    assert!(!project.id.is_empty());

    let fetched = db.get_project(&project.id).await.unwrap();
    assert_eq!(fetched.id, project.id);

    let updated = db.update_project(&project.id, UpdateProject {
        name: Some("改名后的世界观".to_string()),
        description: None,
    }).await.unwrap();
    assert_eq!(updated.name, "改名后的世界观");
    assert_eq!(updated.description, Some("这是一个测试".to_string()));

    let list = db.list_projects().await.unwrap();
    assert!(list.iter().any(|p| p.id == project.id));

    db.delete_project(&project.id).await.unwrap();
    assert!(db.get_project(&project.id).await.is_err());
}

#[tokio::test]
async fn test_category_tree() {
    let db = setup().await;

    let project = db.create_project(CreateProject {
        name: "分类树测试".to_string(),
        description: None,
    }).await.unwrap();

    let root = db.create_category(CreateCategory {
        project_id: project.id.clone(),
        parent_id: None,
        name: "人物".to_string(),
        sort_order: None,
    }).await.unwrap();
    assert!(root.parent_id.is_none());

    let child = db.create_category(CreateCategory {
        project_id: project.id.clone(),
        parent_id: Some(root.id.clone()),
        name: "主角".to_string(),
        sort_order: None,
    }).await.unwrap();
    assert_eq!(child.parent_id, Some(root.id.clone()));

    assert_eq!(db.list_categories(&project.id).await.unwrap().len(), 2);

    let moved = db.update_category(&child.id, UpdateCategory {
        parent_id: Some(None),
        name: None,
        sort_order: None,
    }).await.unwrap();
    assert!(moved.parent_id.is_none());

    db.delete_project(&project.id).await.unwrap();
    assert!(db.list_categories(&project.id).await.unwrap().is_empty());
}

#[tokio::test]
async fn test_entry_with_tags() {
    let db = setup().await;

    let project = db.create_project(CreateProject {
        name: "词条测试".to_string(),
        description: None,
    }).await.unwrap();

    let schema = db.create_tag_schema(CreateTagSchema {
        project_id: project.id.clone(),
        name: "魔法值".to_string(),
        description: Some("角色魔力上限".to_string()),
        r#type: "number".to_string(),
        target: vec!["character".to_string()],
        default_val: Some("0".to_string()),
        range_min: Some(0.0),
        range_max: Some(100.0),
        sort_order: None,
    }).await.unwrap();

    let entry = db.create_entry(CreateEntry {
        project_id: project.id.clone(),
        category_id: None,
        title: "艾莉丝".to_string(),
        summary: None,
        content: Some("# 艾莉丝\n\n主角，魔法少女。".to_string()),
        r#type: Some("character".to_string()),
        tags: Some(vec![EntryTag {
            schema_id: schema.id.clone(),
            value: serde_json::json!(85),
        }]),
        images: None,
    }).await.unwrap();

    assert_eq!(entry.tags.0.len(), 1);
    assert_eq!(entry.tags.0[0].value, serde_json::json!(85));

    assert!(!db.search_entries(&project.id, "艾莉丝", 50).await.unwrap().is_empty());
    assert!(!db.search_entries(&project.id, "魔法少女", 50).await.unwrap().is_empty());
    assert!(db.search_entries(&project.id, "不存在的关键词xyz", 50).await.unwrap().is_empty());

    let updated = db.update_entry(&entry.id, UpdateEntry {
        title: None,
        summary: None,
        content: None, category_id: None, r#type: None,
        tags: Some(vec![EntryTag {
            schema_id: schema.id.clone(),
            value: serde_json::json!(99),
        }]),
        images: None,
    }).await.unwrap();
    assert_eq!(updated.tags.0[0].value, serde_json::json!(99));

    db.delete_project(&project.id).await.unwrap();
}

#[tokio::test]
async fn test_inspect_data() {
    let db = setup().await;

    let project = db.create_project(CreateProject {
        name: "银河帝国".to_string(),
        description: Some("阿西莫夫风格的科幻世界观".to_string()),
    }).await.unwrap();

    let cat_people = db.create_category(CreateCategory {
        project_id: project.id.clone(),
        parent_id: None,
        name: "人物".to_string(),
        sort_order: Some(0),
    }).await.unwrap();

    let cat_hero = db.create_category(CreateCategory {
        project_id: project.id.clone(),
        parent_id: Some(cat_people.id.clone()),
        name: "主角".to_string(),
        sort_order: Some(0),
    }).await.unwrap();

    let schema_power = db.create_tag_schema(CreateTagSchema {
        project_id: project.id.clone(),
        name: "心理史学等级".to_string(),
        description: Some("掌握心理史学的程度".to_string()),
        r#type: "number".to_string(),
        target: vec!["character".to_string()],
        default_val: Some("0".to_string()),
        range_min: Some(0.0),
        range_max: Some(10.0),
        sort_order: Some(0),
    }).await.unwrap();

    let entry = db.create_entry(CreateEntry {
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
    }).await.unwrap();

    println!("\n=== 写入数据 ===");
    println!("project.id  = {}", project.id);
    println!("category.id = {}", cat_hero.id);
    println!("entry.id    = {}", entry.id);

    let results = db.search_entries(&project.id, "心理史学", 50).await.unwrap();
    println!("\n=== 搜索\"心理史学\" ===");
    for r in &results {
        println!("  - {} ({})", r.title, r.id);
    }
    println!("\n数据已保留在 worldflow_dev.db，可在 RustRover 中查看");
}