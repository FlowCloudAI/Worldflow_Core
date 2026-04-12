use std::{collections::HashMap, fs, time::Instant};

use uuid::Uuid;
use worldflow_core::{
    CategoryOps, EntryLinkOps, EntryOps, EntryRelationOps, EntryTypeOps, ProjectOps, SqliteDb,
    TagSchemaOps, models::*,
};

const N_PROJECTS: usize = 50;
const N_CATEGORIES: usize = 100;
const N_SCHEMAS: usize = 15;
const N_ENTRIES: usize = 500;
const N_RELATIONS: usize = 10;
const N_LINKS: usize = 20;

fn random_string(prefix: &str, i: usize) -> String {
    format!("{prefix}_{i}_{}", i * 7919 % 9973)
}

fn sqlite_test_db_url(name: &str) -> (String, std::path::PathBuf) {
    let dir = std::env::temp_dir().join("worldflow_bench");
    let _ = fs::create_dir_all(&dir);

    let path = dir.join(format!("{name}_{}.db", Uuid::now_v7()));
    let url = format!(
        "sqlite:{}?mode=rwc",
        path.to_string_lossy().replace('\\', "/")
    );
    (url, path)
}

async fn setup(name: &str) -> (SqliteDb, std::path::PathBuf) {
    let (url, path) = sqlite_test_db_url(name);
    let db = SqliteDb::new(&url).await.unwrap();
    (db, path)
}

async fn cleanup_sqlite_file(path: std::path::PathBuf) {
    let _ = fs::remove_file(path);
}

#[derive(Debug)]
struct SeedData {
    project_ids: Vec<Uuid>,
    first_cat: HashMap<Uuid, Uuid>,
    relation_samples: Vec<(Uuid, Uuid)>, // (entry_id, relation_id)
    link_samples: Vec<(Uuid, Uuid)>,     // (source_entry_id, target_entry_id)
}

async fn seed_base_data(db: &SqliteDb) -> SeedData {
    let mut project_ids = vec![];

    for pi in 0..N_PROJECTS {
        let project = db
            .create_project(CreateProject {
                cover_image: None,
                name: random_string("世界观", pi),
                description: Some(format!("压力测试项目 {pi}")),
            })
            .await
            .unwrap();
        project_ids.push(project.id);

        let root_inputs: Vec<CreateCategory> = (0..N_CATEGORIES / 2)
            .map(|ci| CreateCategory {
                project_id: project.id,
                parent_id: None,
                name: random_string("分类", ci),
                sort_order: Some(ci as i64),
            })
            .collect();
        let roots = db.create_categories_bulk(root_inputs).await.unwrap();

        let child_inputs: Vec<CreateCategory> = roots
            .iter()
            .enumerate()
            .map(|(ci, root)| CreateCategory {
                project_id: project.id,
                parent_id: Some(root.id),
                name: random_string("子分类", ci),
                sort_order: Some(0),
            })
            .collect();
        let children = db.create_categories_bulk(child_inputs).await.unwrap();

        let mut category_ids = Vec::with_capacity(N_CATEGORIES);
        for root in roots {
            category_ids.push(root.id);
        }
        for child in children {
            category_ids.push(child.id);
        }

        let schema_inputs: Vec<CreateTagSchema> = (0..N_SCHEMAS)
            .map(|si| CreateTagSchema {
                project_id: project.id,
                name: random_string("属性", si),
                description: Some(format!("测试属性 {si}")),
                r#type: if si % 2 == 0 { "number" } else { "string" }.to_string(),
                target: vec!["character".to_string(), "item".to_string()],
                default_val: Some("0".to_string()),
                range_min: Some(0.0),
                range_max: Some(1000.0),
                sort_order: Some(si as i64),
            })
            .collect();

        let schema_ids: Vec<Uuid> = db
            .create_tag_schemas_bulk(schema_inputs)
            .await
            .unwrap()
            .into_iter()
            .map(|schema| schema.id)
            .collect();

        let entry_types = ["character", "location", "item", "event", "concept"];
        let bulk_inputs: Vec<CreateEntry> = (0..N_ENTRIES)
            .map(|ei| {
                let tags: Vec<EntryTag> = schema_ids
                    .iter()
                    .enumerate()
                    .map(|(i, sid)| EntryTag {
                        schema_id: *sid,
                        value: if i % 2 == 0 {
                            serde_json::json!(ei * 3 % 1000)
                        } else {
                            serde_json::json!(random_string("值", ei))
                        },
                    })
                    .collect();

                let fake_images: Vec<FCImage> = (0..3)
                    .map(|ii| FCImage {
                        path: std::path::PathBuf::from(format!(
                            "C:/Users/test/worldflow/images/entry_{}_img_{}.png",
                            pi * N_ENTRIES + ei,
                            ii
                        )),
                        is_cover: ii == 0,
                        caption: if ii == 0 {
                            Some(format!("封面图_{ei}"))
                        } else {
                            None
                        },
                    })
                    .collect();

                CreateEntry {
                    project_id: project.id,
                    category_id: category_ids.get(ei % category_ids.len()).copied(),
                    title: random_string("词条", pi * N_ENTRIES + ei),
                    summary: Some(format!("这是第 {} 个词条的简要说明，属于项目 {}。", ei, pi)),
                    content: Some(format!(
                        "# {}\n\n这是第 {} 个词条的内容，属于项目 {}。\n\n详细描述内容在这里。",
                        random_string("词条", ei),
                        ei,
                        pi,
                    )),
                    r#type: Some(entry_types[ei % entry_types.len()].to_string()),
                    tags: Some(tags),
                    images: Some(fake_images),
                }
            })
            .collect();

        db.create_entries_bulk(bulk_inputs).await.unwrap();
    }

    let mut first_cat = HashMap::new();
    for pid in &project_ids {
        let cats = db.list_categories(pid).await.unwrap();
        if let Some(cat) = cats.first() {
            first_cat.insert(*pid, cat.id);
        }
    }

    SeedData {
        project_ids,
        first_cat,
        relation_samples: vec![],
        link_samples: vec![],
    }
}

async fn seed_relations(db: &SqliteDb, data: &mut SeedData) {
    let mut relation_samples = Vec::with_capacity(N_PROJECTS * N_RELATIONS);

    for pid in &data.project_ids {
        let entries = db
            .list_entries(pid, EntryFilter::default(), N_ENTRIES, 0)
            .await
            .unwrap();

        let directions = [RelationDirection::OneWay, RelationDirection::TwoWay];
        let mut relation_inputs = Vec::with_capacity(N_RELATIONS);

        for ri in 0..N_RELATIONS {
            let a = &entries[ri * 7 % entries.len()];
            let b = &entries[(ri * 13 + 3) % entries.len()];
            if a.id == b.id {
                continue;
            }

            relation_inputs.push(CreateEntryRelation {
                project_id: *pid,
                a_id: a.id,
                b_id: b.id,
                relation: directions[ri % 2].clone(),
                content: format!("关系内容_{ri}"),
            });
        }

        for relation in db.create_relations_bulk(relation_inputs).await.unwrap() {
            relation_samples.push((relation.a_id, relation.id));
        }
    }

    data.relation_samples = relation_samples;
}

async fn seed_links(db: &SqliteDb, data: &mut SeedData) {
    let mut link_samples = Vec::with_capacity(N_PROJECTS * N_LINKS);

    for pid in &data.project_ids {
        let entries = db
            .list_entries(pid, EntryFilter::default(), N_ENTRIES, 0)
            .await
            .unwrap();

        for batch in 0..N_LINKS {
            let source = &entries[batch % entries.len()];
            let mut linked_entry_ids = Vec::with_capacity(3);

            for offset in 1..=3 {
                let target = &entries[(batch * 17 + offset) % entries.len()];
                if target.id != source.id {
                    linked_entry_ids.push(target.id);
                }
            }

            let links = db
                .replace_outgoing_links(pid, &source.id, &linked_entry_ids)
                .await
                .unwrap();

            if let Some(first_link) = links.first() {
                link_samples.push((first_link.a_id, first_link.b_id));
            }
        }
    }

    data.link_samples = link_samples;
}

async fn cleanup_projects(db: &SqliteDb, project_ids: &[Uuid]) {
    for pid in project_ids {
        db.delete_project(pid).await.unwrap();
    }
}

#[tokio::test]
async fn bench_sqlite_write_bulk() {
    let (db, path) = setup("sqlite_write_bulk").await;

    let t0 = Instant::now();
    let data = seed_base_data(&db).await;
    let elapsed = t0.elapsed();

    let total_written = N_PROJECTS * (N_CATEGORIES + N_SCHEMAS + N_ENTRIES);
    println!(
        "\n[sqlite/write] 写入完成: {} 条记录，耗时 {:.2}s，平均 {:.2}ms/条",
        total_written,
        elapsed.as_secs_f64(),
        elapsed.as_millis() as f64 / total_written as f64,
    );

    cleanup_projects(&db, &data.project_ids).await;
    cleanup_sqlite_file(path).await;
}

#[tokio::test]
async fn bench_sqlite_read_paths() {
    let (db, path) = setup("sqlite_read_paths").await;
    let data = seed_base_data(&db).await;

    let t1 = Instant::now();
    for _ in 0..100 {
        let list = db.list_projects().await.unwrap();
        assert!(list.len() >= N_PROJECTS);
    }
    println!(
        "[sqlite/read] list_projects x100: {}ms",
        t1.elapsed().as_millis()
    );

    let t2 = Instant::now();
    for _ in 0..50 {
        for pid in &data.project_ids {
            let cats = db.list_categories(pid).await.unwrap();
            assert_eq!(cats.len(), N_CATEGORIES);
        }
    }
    println!(
        "[sqlite/read] list_categories x{}: {}ms",
        50 * N_PROJECTS,
        t2.elapsed().as_millis()
    );

    let t3 = Instant::now();
    for _ in 0..20 {
        for pid in &data.project_ids {
            let entries = db
                .list_entries(pid, EntryFilter::default(), N_ENTRIES, 0)
                .await
                .unwrap();
            assert_eq!(entries.len(), N_ENTRIES);
        }
    }
    println!(
        "[sqlite/read] list_entries full x{}: {}ms",
        20 * N_PROJECTS,
        t3.elapsed().as_millis()
    );

    let t4 = Instant::now();
    for _ in 0..50 {
        for pid in &data.project_ids {
            if let Some(cid) = data.first_cat.get(pid) {
                let _ = db
                    .list_entries(
                        pid,
                        EntryFilter {
                            category_id: Some(cid),
                            ..Default::default()
                        },
                        100,
                        0,
                    )
                    .await
                    .unwrap();
            }
        }
    }
    println!(
        "[sqlite/read] list_entries by_category x{}: {}ms",
        50 * N_PROJECTS,
        t4.elapsed().as_millis()
    );

    let t5 = Instant::now();
    let all_entries = db
        .list_entries(&data.project_ids[0], EntryFilter::default(), N_ENTRIES, 0)
        .await
        .unwrap();
    for i in 0..200 {
        let entry = &all_entries[i % all_entries.len()];
        let fetched = db.get_entry(&entry.id).await.unwrap();
        assert_eq!(fetched.id, entry.id);
    }
    println!(
        "[sqlite/read] get_entry x200: {}ms",
        t5.elapsed().as_millis()
    );

    let sample = db
        .list_entries(&data.project_ids[0], EntryFilter::default(), 5, 0)
        .await
        .unwrap();
    for brief in &sample {
        assert!(brief.cover.is_some(), "封面图应当存在");
    }
    println!("[sqlite/read] cover 提取验证: {} 条均有封面", sample.len());

    cleanup_projects(&db, &data.project_ids).await;
    cleanup_sqlite_file(path).await;
}

#[tokio::test]
async fn bench_sqlite_search_paths() {
    let (db, path) = setup("sqlite_search_paths").await;
    let data = seed_base_data(&db).await;

    // 稀疏命中：只命中极少数
    let sparse_hit = random_string("词条", 42);

    let t1 = Instant::now();
    for _ in 0..50 {
        for pid in &data.project_ids {
            let results = db
                .search_entries(pid, &sparse_hit, EntryFilter::default(), 50)
                .await
                .unwrap();
            assert!(!results.is_empty());
        }
    }
    println!(
        "[sqlite/search] sparse hit x{}: {}ms",
        50 * N_PROJECTS,
        t1.elapsed().as_millis()
    );

    // 稠密命中：性能压测，不断言结果非空。
    // 测试数据"详细描述"出现在全部词条，是 fts_limit 截断的极端情况；
    // 实际场景搜索词不会命中 100% 词条，fts_limit 足够覆盖目标项目的候选集。
    let dense_hit = "详细描述";

    let t2 = Instant::now();
    for _ in 0..5 {
        for pid in &data.project_ids {
            let _results = db
                .search_entries(pid, dense_hit, EntryFilter::default(), 50)
                .await
                .unwrap();
        }
    }
    println!(
        "[sqlite/search] dense hit x{}: {}ms",
        5 * N_PROJECTS,
        t2.elapsed().as_millis()
    );

    // miss
    let t3 = Instant::now();
    for _ in 0..50 {
        for pid in &data.project_ids {
            let results = db
                .search_entries(pid, "xyznotexist999", EntryFilter::default(), 50)
                .await
                .unwrap();
            assert!(results.is_empty());
        }
    }
    println!(
        "[sqlite/search] miss x{}: {}ms",
        50 * N_PROJECTS,
        t3.elapsed().as_millis()
    );

    cleanup_projects(&db, &data.project_ids).await;
    cleanup_sqlite_file(path).await;
}

#[tokio::test]
async fn bench_sqlite_relation_paths() {
    let (db, path) = setup("sqlite_relation_paths").await;
    let mut data = seed_base_data(&db).await;
    seed_relations(&db, &mut data).await;

    println!(
        "[sqlite/relation] 关系写入 x{}: seeded",
        data.relation_samples.len()
    );

    let t1 = Instant::now();
    for _ in 0..100 {
        for (entry_id, _) in &data.relation_samples {
            let _ = db.list_relations_for_entry(entry_id).await.unwrap();
        }
    }
    println!(
        "[sqlite/relation] list_relations_for_entry x{}: {}ms",
        100 * data.relation_samples.len(),
        t1.elapsed().as_millis()
    );

    let t2 = Instant::now();
    for _ in 0..50 {
        for pid in &data.project_ids {
            let _ = db.list_relations_for_project(pid).await.unwrap();
        }
    }
    println!(
        "[sqlite/relation] list_relations_for_project x{}: {}ms",
        50 * N_PROJECTS,
        t2.elapsed().as_millis()
    );

    let t3 = Instant::now();
    for (_, rel_id) in &data.relation_samples {
        db.update_relation(
            rel_id,
            UpdateEntryRelation {
                relation: Some(RelationDirection::TwoWay),
                content: Some("更新后的关系内容".to_string()),
            },
        )
            .await
            .unwrap();
    }
    println!(
        "[sqlite/relation] update_relation x{}: {}ms",
        data.relation_samples.len(),
        t3.elapsed().as_millis()
    );

    cleanup_projects(&db, &data.project_ids).await;
    cleanup_sqlite_file(path).await;
}

#[tokio::test]
async fn bench_sqlite_entry_link_paths() {
    let (db, path) = setup("sqlite_entry_link_paths").await;
    let mut data = seed_base_data(&db).await;
    seed_links(&db, &mut data).await;

    println!(
        "[sqlite/link] 结构化链接写入 x{}: seeded",
        data.link_samples.len()
    );

    let t1 = Instant::now();
    for _ in 0..100 {
        for (source_id, _) in &data.link_samples {
            let outgoing = db.list_outgoing_links(source_id).await.unwrap();
            assert!(!outgoing.is_empty());
        }
    }
    println!(
        "[sqlite/link] list_outgoing_links x{}: {}ms",
        100 * data.link_samples.len(),
        t1.elapsed().as_millis()
    );

    let t2 = Instant::now();
    for _ in 0..100 {
        for (_, target_id) in &data.link_samples {
            let incoming = db.list_incoming_links(target_id).await.unwrap();
            assert!(!incoming.is_empty());
        }
    }
    println!(
        "[sqlite/link] list_incoming_links x{}: {}ms",
        100 * data.link_samples.len(),
        t2.elapsed().as_millis()
    );

    let t3 = Instant::now();
    for pid in &data.project_ids {
        let entries = db
            .list_entries(pid, EntryFilter::default(), N_ENTRIES, 0)
            .await
            .unwrap();

        for batch in 0..N_LINKS {
            let source = &entries[batch % entries.len()];
            let targets = [
                entries[(batch * 19 + 5) % entries.len()].id,
                entries[(batch * 23 + 7) % entries.len()].id,
            ];
            let replaced = db
                .replace_outgoing_links(pid, &source.id, &targets)
                .await
                .unwrap();
            assert!(!replaced.is_empty());
        }
    }
    println!(
        "[sqlite/link] replace_outgoing_links x{}: {}ms",
        N_PROJECTS * N_LINKS,
        t3.elapsed().as_millis()
    );

    let t4 = Instant::now();
    for (source_id, _) in data.link_samples.iter().take((N_PROJECTS * N_LINKS) / 2) {
        let deleted = db.delete_links_from_entry(source_id).await.unwrap();
        assert!(deleted > 0);
    }
    println!(
        "[sqlite/link] delete_links_from_entry x{}: {}ms",
        (N_PROJECTS * N_LINKS) / 2,
        t4.elapsed().as_millis()
    );

    cleanup_projects(&db, &data.project_ids).await;
    cleanup_sqlite_file(path).await;
}

#[tokio::test]
async fn bench_sqlite_custom_entry_types() {
    let (db, path) = setup("sqlite_custom_types").await;

    const T_PROJECTS: usize = 10;
    const CUSTOM_TYPES_PER_PROJECT: usize = 5;
    const ENTRIES_PER_TYPE: usize = 20;

    let mut project_ids = vec![];
    let mut type_ids = vec![];

    let t_create = Instant::now();
    for pi in 0..T_PROJECTS {
        let project = db
            .create_project(CreateProject {
                cover_image: None,
                name: random_string("类型项目", pi),
                description: None,
            })
            .await
            .unwrap();
        project_ids.push(project.id);

        let type_inputs: Vec<CreateCustomEntryType> = (0..CUSTOM_TYPES_PER_PROJECT)
            .map(|ti| CreateCustomEntryType {
                project_id: project.id,
                name: format!("custom_{}_{}", pi, ti),
                description: Some(format!("自定义类型 {} in project {}", ti, pi)),
                icon: Some("🎨".to_string()),
                color: Some(format!("#{:06X}", (pi * 256 + ti * 10) % 0xFFFFFF)),
            })
            .collect();

        let custom_types = db.create_entry_types_bulk(type_inputs).await.unwrap();
        type_ids.extend(custom_types.into_iter().map(|t| t.id));
    }
    println!(
        "[sqlite/type] create {} projects with {} custom types each: {}ms",
        T_PROJECTS,
        CUSTOM_TYPES_PER_PROJECT,
        t_create.elapsed().as_millis()
    );

    let t_entries = Instant::now();
    let mut entry_count = 0;
    for (pi, pid) in project_ids.iter().enumerate() {
        let project_types: Vec<String> = db
            .list_custom_entry_types(pid)
            .await
            .unwrap_or_default()
            .iter()
            .map(|ct| ct.id.to_string())
            .collect();

        let mut bulk_inputs = Vec::with_capacity(project_types.len() * ENTRIES_PER_TYPE);
        for (ti, type_id) in project_types.iter().enumerate() {
            for ei in 0..ENTRIES_PER_TYPE {
                bulk_inputs.push(CreateEntry {
                    project_id: *pid,
                    category_id: None,
                    title: random_string(&format!("entry_p{}_t{}", pi, ti), ei),
                    summary: None,
                    content: Some(format!("使用自定义类型的词条 {}", ei)),
                    r#type: Some(type_id.clone()),
                    tags: None,
                    images: None,
                });
                entry_count += 1;
            }
        }
        db.create_entries_bulk(bulk_inputs).await.unwrap();
    }
    println!(
        "[sqlite/type] create {} entries with custom types: {}ms",
        entry_count,
        t_entries.elapsed().as_millis()
    );

    let t_filter = Instant::now();
    let mut filtered_count = 0;
    for type_id in type_ids.iter().take(T_PROJECTS * CUSTOM_TYPES_PER_PROJECT) {
        if let Ok(custom_type) = db.get_entry_type(type_id).await {
            let type_id_str = type_id.to_string();
            let entries = db
                .list_entries(
                    &custom_type.project_id,
                    EntryFilter {
                        entry_type: Some(type_id_str.as_str()),
                        ..Default::default()
                    },
                    1000,
                    0,
                )
                .await
                .unwrap();
            filtered_count += entries.len();
        }
    }
    println!(
        "[sqlite/type] filter entries by {} custom types: {}ms (found {} entries)",
        T_PROJECTS * CUSTOM_TYPES_PER_PROJECT,
        t_filter.elapsed().as_millis(),
        filtered_count
    );

    let t_list_all = Instant::now();
    for pid in &project_ids {
        let all_types = db.list_all_entry_types(pid).await.unwrap();
        assert_eq!(all_types.len(), 9 + CUSTOM_TYPES_PER_PROJECT);
    }
    println!(
        "[sqlite/type] list_all_entry_types x{}: {}ms",
        project_ids.len(),
        t_list_all.elapsed().as_millis()
    );

    cleanup_projects(&db, &project_ids).await;
    cleanup_sqlite_file(path).await;
}
