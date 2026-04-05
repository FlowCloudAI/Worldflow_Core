use worldflow_core::{models::*, CategoryOps, EntryOps, ProjectOps, SqliteDb, TagSchemaOps, EntryTypeOps};
use worldflow_core::models::EntryFilter;

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

    assert!(!db.search_entries(&project.id, "艾莉丝", EntryFilter::default(), 50).await.unwrap().is_empty());
    assert!(!db.search_entries(&project.id, "魔法少女", EntryFilter::default(), 50).await.unwrap().is_empty());
    assert!(db.search_entries(&project.id, "不存在的关键词xyz", EntryFilter::default(), 50).await.unwrap().is_empty());

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

    let results = db.search_entries(&project.id, "心理史学", EntryFilter::default(), 50).await.unwrap();
    println!("\n=== 搜索\"心理史学\" ===");
    for r in &results {
        println!("  - {} ({})", r.title, r.id);
    }
    println!("\n数据已保留在 worldflow_dev.db，可在 RustRover 中查看");
}

// ======================== EntryType Tests ========================

#[tokio::test]
async fn test_builtin_entry_types_constant() {
    use worldflow_core::models::BUILTIN_ENTRY_TYPES;

    assert_eq!(BUILTIN_ENTRY_TYPES.len(), 9, "Should have 9 builtin types");

    let keys: Vec<&str> = BUILTIN_ENTRY_TYPES.iter().map(|t| t.key).collect();
    let mut unique_keys = keys.clone();
    unique_keys.sort();
    unique_keys.dedup();
    assert_eq!(keys.len(), unique_keys.len(), "All builtin type keys should be unique");

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
    assert!(is_builtin_type("organization"), "short key should be builtin");
    assert!(!is_builtin_type("550e8400-e29b-41d4-a716-446655440000"), "UUID should not be builtin");
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

    let project = db.create_project(CreateProject {
        name: "自定义类型测试".to_string(),
        description: None,
    }).await.unwrap();

    let custom_type = db.create_entry_type(CreateCustomEntryType {
        project_id: project.id.clone(),
        name: "自定义类型1".to_string(),
        description: Some("这是一个自定义类型".to_string()),
        icon: Some("🎨".to_string()),
        color: Some("#FF5733".to_string()),
    }).await.unwrap();

    assert!(!custom_type.id.is_empty());
    assert_eq!(custom_type.id.len(), 36, "ID should be UUID format");
    assert_eq!(custom_type.name, "自定义类型1");
    assert_eq!(custom_type.description, Some("这是一个自定义类型".to_string()));
    assert!(!custom_type.created_at.is_empty());
    assert!(!custom_type.updated_at.is_empty());
}

#[tokio::test]
async fn test_custom_entry_type_unique_per_project() {
    let db = setup().await;

    let proj1 = db.create_project(CreateProject {
        name: "项目1".to_string(),
        description: None,
    }).await.unwrap();

    let proj2 = db.create_project(CreateProject {
        name: "项目2".to_string(),
        description: None,
    }).await.unwrap();

    // 在proj1中创建类型
    let _type1 = db.create_entry_type(CreateCustomEntryType {
        project_id: proj1.id.clone(),
        name: "Custom1".to_string(),
        description: None,
        icon: None,
        color: None,
    }).await.unwrap();

    // 在proj1中用相同名称创建应该失败
    let result = db.create_entry_type(CreateCustomEntryType {
        project_id: proj1.id.clone(),
        name: "Custom1".to_string(),
        description: None,
        icon: None,
        color: None,
    }).await;
    assert!(result.is_err(), "Same name in same project should fail");

    // 在proj2中用相同名称创建应该成功
    let _type2 = db.create_entry_type(CreateCustomEntryType {
        project_id: proj2.id.clone(),
        name: "Custom1".to_string(),
        description: None,
        icon: None,
        color: None,
    }).await.unwrap();
}

#[tokio::test]
async fn test_update_custom_entry_type() {
    let db = setup().await;

    let project = db.create_project(CreateProject {
        name: "更新测试".to_string(),
        description: None,
    }).await.unwrap();

    let custom_type = db.create_entry_type(CreateCustomEntryType {
        project_id: project.id.clone(),
        name: "原始名称".to_string(),
        description: Some("原始描述".to_string()),
        icon: Some("📦".to_string()),
        color: Some("#000000".to_string()),
    }).await.unwrap();

    // 更新名称
    let updated = db.update_entry_type(&custom_type.id, UpdateCustomEntryType {
        name: Some("新名称".to_string()),
        description: None,
        icon: None,
        color: None,
    }).await.unwrap();

    assert_eq!(updated.name, "新名称");
    assert_eq!(updated.description, Some("原始描述".to_string()), "description should remain");

    // 清空描述
    let updated2 = db.update_entry_type(&custom_type.id, UpdateCustomEntryType {
        name: None,
        description: Some(None),
        icon: None,
        color: None,
    }).await.unwrap();

    assert_eq!(updated2.description, None, "description should be cleared");
}

#[tokio::test]
async fn test_delete_custom_entry_type_unused() {
    let db = setup().await;

    let project = db.create_project(CreateProject {
        name: "删除测试".to_string(),
        description: None,
    }).await.unwrap();

    let custom_type = db.create_entry_type(CreateCustomEntryType {
        project_id: project.id.clone(),
        name: "待删除类型".to_string(),
        description: None,
        icon: None,
        color: None,
    }).await.unwrap();

    // 删除未使用的类型应该成功
    db.delete_entry_type(&custom_type.id).await.unwrap();

    // 验证已删除
    let result = db.get_entry_type(&custom_type.id).await;
    assert!(result.is_err(), "Deleted type should not exist");
}

#[tokio::test]
async fn test_delete_custom_entry_type_in_use() {
    let db = setup().await;

    let project = db.create_project(CreateProject {
        name: "使用中删除测试".to_string(),
        description: None,
    }).await.unwrap();

    let custom_type = db.create_entry_type(CreateCustomEntryType {
        project_id: project.id.clone(),
        name: "被使用的类型".to_string(),
        description: None,
        icon: None,
        color: None,
    }).await.unwrap();

    // 创建使用该类型的entry
    let _entry = db.create_entry(CreateEntry {
        project_id: project.id.clone(),
        category_id: None,
        title: "测试词条".to_string(),
        summary: None,
        content: Some("内容".to_string()),
        r#type: Some(custom_type.id.clone()),
        tags: None,
        images: None,
    }).await.unwrap();

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

    let project = db.create_project(CreateProject {
        name: "列表测试".to_string(),
        description: None,
    }).await.unwrap();

    // 先列出，应该只有9个内置类型
    let all_types = db.list_all_entry_types(&project.id).await.unwrap();
    assert_eq!(all_types.len(), 9, "Should have 9 builtin types initially");

    // 验证都是Builtin类型
    for t in &all_types {
        match t {
            EntryTypeView::Builtin { .. } => {},
            EntryTypeView::Custom(_) => panic!("Should not have custom types yet"),
        }
    }

    // 创建2个自定义类型
    let _custom1 = db.create_entry_type(CreateCustomEntryType {
        project_id: project.id.clone(),
        name: "自定义1".to_string(),
        description: None,
        icon: None,
        color: None,
    }).await.unwrap();

    let _custom2 = db.create_entry_type(CreateCustomEntryType {
        project_id: project.id.clone(),
        name: "自定义2".to_string(),
        description: None,
        icon: None,
        color: None,
    }).await.unwrap();

    // 重新列出，应该有9个内置+2个自定义
    let all_types = db.list_all_entry_types(&project.id).await.unwrap();
    assert_eq!(all_types.len(), 11, "Should have 11 types after adding 2 custom");

    // 验证内置类型在前
    let first_9_builtin = all_types[0..9].iter().all(|t| matches!(t, EntryTypeView::Builtin { .. }));
    assert!(first_9_builtin, "First 9 should be builtin");

    let last_2_custom = all_types[9..11].iter().all(|t| matches!(t, EntryTypeView::Custom(_)));
    assert!(last_2_custom, "Last 2 should be custom");
}

#[tokio::test]
async fn test_list_custom_entry_types() {
    let db = setup().await;

    let project = db.create_project(CreateProject {
        name: "自定义类型列表测试".to_string(),
        description: None,
    }).await.unwrap();

    // 创建3个自定义类型
    for i in 1..=3 {
        db.create_entry_type(CreateCustomEntryType {
            project_id: project.id.clone(),
            name: format!("类型{}", i),
            description: None,
            icon: None,
            color: None,
        }).await.unwrap();
    }

    let custom = db.list_custom_entry_types(&project.id).await.unwrap();
    assert_eq!(custom.len(), 3, "Should have 3 custom types");
}

#[tokio::test]
async fn test_filter_entries_by_builtin_type() {
    let db = setup().await;

    let project = db.create_project(CreateProject {
        name: "内置类型过滤测试".to_string(),
        description: None,
    }).await.unwrap();

    // 创建不同类型的词条
    let entry1 = db.create_entry(CreateEntry {
        project_id: project.id.clone(),
        category_id: None,
        title: "人物1".to_string(),
        summary: None,
        content: Some("人物描述".to_string()),
        r#type: Some("character".to_string()),
        tags: None,
        images: None,
    }).await.unwrap();

    let entry2 = db.create_entry(CreateEntry {
        project_id: project.id.clone(),
        category_id: None,
        title: "地点1".to_string(),
        summary: None,
        content: Some("地点描述".to_string()),
        r#type: Some("location".to_string()),
        tags: None,
        images: None,
    }).await.unwrap();

    let entry3 = db.create_entry(CreateEntry {
        project_id: project.id.clone(),
        category_id: None,
        title: "物品1".to_string(),
        summary: None,
        content: Some("物品描述".to_string()),
        r#type: Some("item".to_string()),
        tags: None,
        images: None,
    }).await.unwrap();

    // 按character类型过滤
    let character_entries = db.list_entries(
        &project.id,
        EntryFilter { entry_type: Some("character"), ..Default::default() },
        100,
        0
    ).await.unwrap();

    assert!(character_entries.iter().any(|e| e.id == entry1.id));
    assert!(!character_entries.iter().any(|e| e.id == entry2.id));
    assert!(!character_entries.iter().any(|e| e.id == entry3.id));

    // 按location类型过滤
    let location_entries = db.list_entries(
        &project.id,
        EntryFilter { entry_type: Some("location"), ..Default::default() },
        100,
        0
    ).await.unwrap();

    assert!(!location_entries.iter().any(|e| e.id == entry1.id));
    assert!(location_entries.iter().any(|e| e.id == entry2.id));
    assert!(!location_entries.iter().any(|e| e.id == entry3.id));
}

#[tokio::test]
async fn test_filter_entries_by_custom_type_uuid() {
    let db = setup().await;

    let project = db.create_project(CreateProject {
        name: "自定义类型过滤测试".to_string(),
        description: None,
    }).await.unwrap();

    // 创建自定义类型
    let custom_type = db.create_entry_type(CreateCustomEntryType {
        project_id: project.id.clone(),
        name: "自定义类型".to_string(),
        description: None,
        icon: None,
        color: None,
    }).await.unwrap();

    // 创建使用自定义类型的词条
    let entry = db.create_entry(CreateEntry {
        project_id: project.id.clone(),
        category_id: None,
        title: "使用自定义类型的词条".to_string(),
        summary: None,
        content: Some("内容".to_string()),
        r#type: Some(custom_type.id.clone()),
        tags: None,
        images: None,
    }).await.unwrap();

    // 按自定义类型UUID过滤
    let filtered = db.list_entries(
        &project.id,
        EntryFilter { entry_type: Some(&custom_type.id), ..Default::default() },
        100,
        0
    ).await.unwrap();

    assert!(filtered.iter().any(|e| e.id == entry.id), "Should find entry with custom type");
}

#[tokio::test]
async fn test_cascade_delete_on_project_delete() {
    let db = setup().await;

    let project = db.create_project(CreateProject {
        name: "级联删除测试".to_string(),
        description: None,
    }).await.unwrap();

    // 创建自定义类型
    let _custom1 = db.create_entry_type(CreateCustomEntryType {
        project_id: project.id.clone(),
        name: "级联删除类型1".to_string(),
        description: None,
        icon: None,
        color: None,
    }).await.unwrap();

    let _custom2 = db.create_entry_type(CreateCustomEntryType {
        project_id: project.id.clone(),
        name: "级联删除类型2".to_string(),
        description: None,
        icon: None,
        color: None,
    }).await.unwrap();

    // 删除项目
    db.delete_project(&project.id).await.unwrap();

    // 验证自定义类型已被级联删除
    let remaining_types = db.list_custom_entry_types(&project.id).await.unwrap();
    assert!(remaining_types.is_empty(), "Custom types should be deleted with project");
}