use worldflow_core::{SqliteDb, models::*, ProjectOps, CategoryOps, EntryOps, TagSchemaOps, EntryRelationOps, EntryTypeOps};
use worldflow_core::models::EntryFilter;
use std::{collections::HashMap, time::Instant};

async fn setup() -> SqliteDb {
    let db_path = format!(
        "sqlite:{}/worldflow_dev.db?mode=rwc",
        env!("CARGO_MANIFEST_DIR").replace('\\', "/")
    );
    SqliteDb::new(&db_path).await.unwrap()
}

fn random_string(prefix: &str, i: usize) -> String {
    format!("{prefix}_{i}_{}", i * 7919 % 9973)
}

#[tokio::test]
async fn stress_write_and_query() {
    let db = setup().await;

    const N_PROJECTS:              usize = 50;
    const N_CATEGORIES:            usize = 100;
    const N_SCHEMAS:               usize = 15;
    const N_ENTRIES:               usize = 500;
    const N_RELATIONS:             usize = 10;
    const N_TYPE_PROJECTS:         usize = 10;
    const CUSTOM_TYPES_PER_PROJECT: usize = 5;
    const ENTRIES_PER_TYPE:        usize = 20;

    let mut project_ids = vec![];
    let t0 = Instant::now();

    for pi in 0..N_PROJECTS {
        let project = db.create_project(CreateProject {
            name: random_string("世界观", pi),
            description: Some(format!("压力测试项目 {pi}")),
        }).await.unwrap();
        project_ids.push(project.id.clone());

        let mut category_ids = vec![];
        for ci in 0..N_CATEGORIES / 2 {
            let root = db.create_category(CreateCategory {
                project_id: project.id.clone(),
                parent_id: None,
                name: random_string("分类", ci),
                sort_order: Some(ci as i64),
            }).await.unwrap();
            let child = db.create_category(CreateCategory {
                project_id: project.id.clone(),
                parent_id: Some(root.id.clone()),
                name: random_string("子分类", ci),
                sort_order: Some(0),
            }).await.unwrap();
            category_ids.push(root.id);
            category_ids.push(child.id);
        }

        let mut schema_ids = vec![];
        for si in 0..N_SCHEMAS {
            let schema = db.create_tag_schema(CreateTagSchema {
                project_id: project.id.clone(),
                name: random_string("属性", si),
                description: Some(format!("测试属性 {si}")),
                r#type: if si % 2 == 0 { "number" } else { "string" }.to_string(),
                target: vec!["character".to_string(), "item".to_string()],
                default_val: Some("0".to_string()),
                range_min: Some(0.0),
                range_max: Some(1000.0),
                sort_order: Some(si as i64),
            }).await.unwrap();
            schema_ids.push(schema.id);
        }

        let entry_types = ["character", "location", "item", "event", "concept"];
        let bulk_inputs: Vec<CreateEntry> = (0..N_ENTRIES).map(|ei| {
            let tags: Vec<EntryTag> = schema_ids.iter().enumerate().map(|(i, sid)| EntryTag {
                schema_id: sid.clone(),
                value: if i % 2 == 0 {
                    serde_json::json!(ei * 3 % 1000)
                } else {
                    serde_json::json!(random_string("值", ei))
                },
            }).collect();

            let fake_images: Vec<FCImage> = (0..3).map(|ii| FCImage {
                path:     std::path::PathBuf::from(format!(
                    "C:/Users/test/worldflow/images/entry_{}_img_{}.png",
                    pi * N_ENTRIES + ei, ii
                )),
                is_cover: ii == 0,
                caption:  if ii == 0 { Some(format!("封面图_{ei}")) } else { None },
            }).collect();

            CreateEntry {
                project_id:  project.id.clone(),
                category_id: category_ids.get(ei % category_ids.len()).cloned(),
                title:       random_string("词条", pi * N_ENTRIES + ei),
                summary:     Some(format!("这是第 {} 个词条的简要说明，属于项目 {}。", ei, pi)),
                content:     Some(format!(
                    "# {}\n\n这是第 {} 个词条的内容，属于项目 {}。\n\n详细描述内容在这里。",
                    random_string("词条", ei), ei, pi,
                )),
                r#type: Some(entry_types[ei % entry_types.len()].to_string()),
                tags:   Some(tags),
                images: Some(fake_images),
            }
        }).collect();
        db.create_entries_bulk(bulk_inputs).await.unwrap();
    }
    db.optimize_fts().await.unwrap();

    let write_elapsed = t0.elapsed();
    let total_written = N_PROJECTS * (N_CATEGORIES + N_SCHEMAS + N_ENTRIES);
    println!(
        "\n写入完成: {} 条记录，耗时 {:.2}s，平均 {:.2}ms/条",
        total_written,
        write_elapsed.as_secs_f64(),
        write_elapsed.as_millis() as f64 / total_written as f64,
    );

    // ── 关系写入 ──────────────────────────────────────────────
    let t_rel_write = Instant::now();
    let mut relation_samples: Vec<(String, String)> = vec![];

    for pid in &project_ids {
        let entries = db.list_entries(pid, EntryFilter::default(), N_ENTRIES, 0).await.unwrap();
        let directions = [RelationDirection::OneWay, RelationDirection::TwoWay];

        for ri in 0..N_RELATIONS {
            let a = &entries[ri * 7 % entries.len()];
            let b = &entries[(ri * 13 + 3) % entries.len()];
            if a.id == b.id { continue; }

            let rel = db.create_relation(CreateEntryRelation {
                project_id: pid.clone(),
                a_id:       a.id.clone(),
                b_id:       b.id.clone(),
                relation:   directions[ri % 2].clone(),
                content:    format!("关系内容_{ri}"),
            }).await;

            if let Ok(r) = rel {
                relation_samples.push((a.id.clone(), r.id.clone()));
            }
        }
    }
    println!("关系写入 x{}: {:.2}ms", N_PROJECTS * N_RELATIONS, t_rel_write.elapsed().as_millis());

    // ── 自定义类型写入 ──────────────────────────────────────────
    let mut type_project_ids = vec![];
    let mut type_ids = vec![];

    let t_type_create = Instant::now();
    for pi in 0..N_TYPE_PROJECTS {
        let project = db.create_project(CreateProject {
            name: random_string("类型项目", pi),
            description: None,
        }).await.unwrap();
        type_project_ids.push(project.id.clone());

        for ti in 0..CUSTOM_TYPES_PER_PROJECT {
            let custom_type = db.create_entry_type(CreateCustomEntryType {
                project_id: project.id.clone(),
                name: format!("custom_{}_{}", pi, ti),
                description: Some(format!("自定义类型 {} in project {}", ti, pi)),
                icon: Some("🎨".to_string()),
                color: Some(format!("#{:06X}", (pi * 256 + ti * 10) % 0xFFFFFF)),
            }).await.unwrap();
            type_ids.push(custom_type.id.clone());
        }
    }
    println!("create {} type-projects with {} custom types each: {:.2}ms",
             N_TYPE_PROJECTS, CUSTOM_TYPES_PER_PROJECT, t_type_create.elapsed().as_millis());

    let t_type_entries = Instant::now();
    let mut type_entry_count = 0;
    for (pi, pid) in type_project_ids.iter().enumerate() {
        let project_types: Vec<String> = db.list_custom_entry_types(pid)
            .await
            .unwrap_or_default()
            .iter()
            .map(|ct| ct.id.clone())
            .collect();

        for (ti, type_id) in project_types.iter().enumerate() {
            for ei in 0..ENTRIES_PER_TYPE {
                db.create_entry(CreateEntry {
                    project_id:  pid.clone(),
                    category_id: None,
                    title:       random_string(&format!("entry_p{}_t{}", pi, ti), ei),
                    summary:     None,
                    content:     Some(format!("使用自定义类型的词条 {}", ei)),
                    r#type:      Some(type_id.clone()),
                    tags:        None,
                    images:      None,
                }).await.unwrap();
                type_entry_count += 1;
            }
        }
    }
    println!("create {} entries with custom types: {:.2}ms", type_entry_count, t_type_entries.elapsed().as_millis());

    // ── 普通查询 ──────────────────────────────────────────────
    let t1 = Instant::now();
    for _ in 0..100 {
        let list = db.list_projects().await.unwrap();
        assert!(list.len() >= N_PROJECTS);
    }
    println!("list_projects x100: {:.2}ms", t1.elapsed().as_millis());

    let t2 = Instant::now();
    for _ in 0..50 {
        for pid in &project_ids {
            let cats = db.list_categories(pid).await.unwrap();
            assert_eq!(cats.len(), N_CATEGORIES);
        }
    }
    println!("list_categories x{}: {:.2}ms", 50 * N_PROJECTS, t2.elapsed().as_millis());

    let t3 = Instant::now();
    for _ in 0..20 {
        for pid in &project_ids {
            let entries = db.list_entries(pid, EntryFilter::default(), N_ENTRIES, 0).await.unwrap();
            assert_eq!(entries.len(), N_ENTRIES);
        }
    }
    println!("list_entries full x{}: {:.2}ms", 20 * N_PROJECTS, t3.elapsed().as_millis());

    let mut first_cat: HashMap<String, String> = HashMap::new();
    for pid in &project_ids {
        let cats = db.list_categories(pid).await.unwrap();
        if let Some(cat) = cats.first() {
            first_cat.insert(pid.clone(), cat.id.clone());
        }
    }
    let t4 = Instant::now();
    for _ in 0..50 {
        for pid in &project_ids {
            if let Some(cid) = first_cat.get(pid) {
                let _ = db.list_entries(pid, EntryFilter { category_id: Some(cid), ..Default::default() }, 100, 0).await.unwrap();
            }
        }
    }
    println!("list_entries by_category x{}: {:.2}ms", 50 * N_PROJECTS, t4.elapsed().as_millis());

    let hit_title = random_string("词条", 42);
    let t5 = Instant::now();
    for _ in 0..50 {
        for pid in &project_ids {
            let results = db.search_entries(pid, &hit_title, EntryFilter::default(), 50).await.unwrap();
            assert!(!results.is_empty());
        }
    }
    println!("search_entries hit x{}: {:.2}ms", 50 * N_PROJECTS, t5.elapsed().as_millis());

    let t6 = Instant::now();
    for _ in 0..50 {
        for pid in &project_ids {
            let results = db.search_entries(pid, "xyznotexist999", EntryFilter::default(), 50).await.unwrap();
            assert!(results.is_empty());
        }
    }
    println!("search_entries miss x{}: {:.2}ms", 50 * N_PROJECTS, t6.elapsed().as_millis());

    let t7 = Instant::now();
    let all_entries = db.list_entries(&project_ids[0], EntryFilter::default(), N_ENTRIES, 0).await.unwrap();
    for i in 0..200 {
        let entry = &all_entries[i % all_entries.len()];
        let fetched = db.get_entry(&entry.id).await.unwrap();
        assert_eq!(fetched.id, entry.id);
    }
    println!("get_entry x200: {:.2}ms", t7.elapsed().as_millis());

    let sample = db.list_entries(&project_ids[0], EntryFilter::default(), 5, 0).await.unwrap();
    for brief in &sample {
        assert!(brief.cover.is_some(), "封面图应当存在");
    }
    println!("cover 提取验证: {} 条均有封面", sample.len());

    // ── 自定义类型查询 ────────────────────────────────────────
    let t_type_filter = Instant::now();
    let mut filtered_count = 0;
    for type_id in &type_ids {
        if let Ok(custom_type) = db.get_entry_type(type_id).await {
            let entries = db.list_entries(
                &custom_type.project_id,
                EntryFilter { entry_type: Some(type_id), ..Default::default() },
                1000,
                0,
            ).await.unwrap();
            filtered_count += entries.len();
        }
    }
    println!("filter entries by {} custom types: {:.2}ms (found {} entries)",
             type_ids.len(), t_type_filter.elapsed().as_millis(), filtered_count);

    let t_list_all = Instant::now();
    for pid in &type_project_ids {
        let all_types = db.list_all_entry_types(pid).await.unwrap();
        assert_eq!(all_types.len(), 9 + CUSTOM_TYPES_PER_PROJECT);
    }
    println!("list_all_entry_types x{}: {:.2}ms", N_TYPE_PROJECTS, t_list_all.elapsed().as_millis());

    // ── 关系查询 ──────────────────────────────────────────────
    let t_rel_read = Instant::now();
    for _ in 0..100 {
        for (entry_id, _) in &relation_samples {
            let _ = db.list_relations_for_entry(entry_id).await.unwrap();
        }
    }
    println!(
        "list_relations_for_entry x{}: {:.2}ms",
        100 * relation_samples.len(),
        t_rel_read.elapsed().as_millis()
    );

    let t_rel_proj = Instant::now();
    for _ in 0..50 {
        for pid in &project_ids {
            let _ = db.list_relations_for_project(pid).await.unwrap();
        }
    }
    println!(
        "list_relations_for_project x{}: {:.2}ms",
        50 * N_PROJECTS,
        t_rel_proj.elapsed().as_millis()
    );

    let t_rel_update = Instant::now();
    for (_, rel_id) in &relation_samples {
        db.update_relation(rel_id, UpdateEntryRelation {
            relation: Some(RelationDirection::TwoWay),
            content:  Some("更新后的关系内容".to_string()),
        }).await.unwrap();
    }
    println!(
        "update_relation x{}: {:.2}ms",
        relation_samples.len(),
        t_rel_update.elapsed().as_millis()
    );

    // ── 清理 ──────────────────────────────────────────────────
    let t8 = Instant::now();
    for pid in project_ids.iter().chain(type_project_ids.iter()) {
        db.delete_project(pid).await.unwrap();
    }
    println!("delete {} projects (cascade): {:.2}ms", N_PROJECTS + N_TYPE_PROJECTS, t8.elapsed().as_millis());

    for pid in project_ids.iter().chain(type_project_ids.iter()) {
        assert!(db.list_entries(pid, EntryFilter::default(), 1, 0).await.unwrap().is_empty());
        assert!(db.list_categories(pid).await.unwrap().is_empty());
        assert!(db.list_relations_for_project(pid).await.unwrap().is_empty());
    }

    println!("\n总写入: {} 词条 across {} 项目", N_PROJECTS * N_ENTRIES + type_entry_count, N_PROJECTS + N_TYPE_PROJECTS);
}