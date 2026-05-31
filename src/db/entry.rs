use super::entry_link::{normalize_linked_entry_ids, row_to_link};
use super::entry_relation::row_to_relation;
use super::traits::EntryOps;
use crate::{
    db::SqliteDb,
    error::{Result, WorldflowError},
    models::{
        CreateEntry, Entry, EntryBrief, EntryFilter, FCImage, RelationDirection, SaveEntryBundle,
        SaveEntryBundleResult, UpdateEntry, validate_builtin_type_key,
    },
};
use sqlx::{Row, Sqlite, Transaction};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    path::PathBuf,
};
use uuid::Uuid;

const MAX_ENTRY_TYPE_VALIDATION_PAIRS_PER_QUERY: usize = 400;

fn row_to_entry(row: &sqlx::sqlite::SqliteRow) -> Result<Entry> {
    let tags_str: String = row.try_get("tags")?;
    let images_str: String = row.try_get("images")?;
    Ok(Entry {
        id: row.try_get("id")?,
        project_id: row.try_get("project_id")?,
        category_id: row.try_get("category_id")?,
        title: row.try_get("title")?,
        summary: row.try_get("summary")?,
        content: row.try_get("content")?,
        r#type: row.try_get("type")?,
        tags: sqlx::types::Json(serde_json::from_str(&tags_str)?),
        images: sqlx::types::Json(serde_json::from_str(&images_str)?),
        cover_path: row.try_get("cover_path")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn row_to_entry_brief(row: &sqlx::sqlite::SqliteRow) -> Result<EntryBrief> {
    let cover_str: Option<String> = row.try_get("cover_path")?;
    Ok(EntryBrief {
        id: row.try_get("id")?,
        project_id: row.try_get("project_id")?,
        category_id: row.try_get("category_id")?,
        title: row.try_get("title")?,
        summary: row.try_get("summary")?,
        r#type: row.try_get("type")?,
        cover: cover_str.map(PathBuf::from),
        updated_at: row.try_get("updated_at")?,
    })
}

fn cover_path_from_images(images: &[FCImage]) -> Option<String> {
    images
        .iter()
        .find(|i| i.is_cover)
        .map(|i| i.path.to_string_lossy().to_string())
}

fn escape_like_pattern(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn escape_fts5_phrase(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(format!("\"{}\"", trimmed.replace('"', "\"\"")))
}

fn custom_entry_type_not_found_error() -> WorldflowError {
    WorldflowError::InvalidInput("自定义词条类型不存在或不属于当前项目".to_string())
}

fn normalize_entry_compare_text(value: &str) -> String {
    value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim()
        .to_string()
}

fn normalize_entry_lookup_title(value: &str) -> String {
    value.trim().to_lowercase()
}

fn normalize_relation_endpoints(
    a_id: Uuid,
    b_id: Uuid,
    relation: &RelationDirection,
) -> (Uuid, Uuid) {
    if *relation == RelationDirection::TwoWay && a_id > b_id {
        (b_id, a_id)
    } else {
        (a_id, b_id)
    }
}

async fn validate_entry_type(db: &SqliteDb, project_id: &Uuid, typ: Option<&str>) -> Result<()> {
    let Some(typ) = typ else {
        return Ok(());
    };
    let Ok(type_id) = Uuid::parse_str(typ) else {
        return validate_builtin_type_key(typ);
    };
    let row =
        sqlx::query("SELECT COUNT(*) as cnt FROM entry_types WHERE id = ? AND project_id = ?")
            .bind(type_id)
            .bind(project_id)
            .fetch_one(&db.pool)
            .await?;
    let cnt: i64 = row.try_get("cnt")?;
    if cnt == 0 {
        return Err(custom_entry_type_not_found_error());
    }
    Ok(())
}

async fn validate_entry_type_in_tx(
    tx: &mut Transaction<'_, Sqlite>,
    project_id: &Uuid,
    typ: Option<&str>,
) -> Result<()> {
    let Some(typ) = typ else {
        return Ok(());
    };
    let Ok(type_id) = Uuid::parse_str(typ) else {
        return validate_builtin_type_key(typ);
    };
    let row =
        sqlx::query("SELECT COUNT(*) as cnt FROM entry_types WHERE id = ? AND project_id = ?")
            .bind(type_id)
            .bind(project_id)
            .fetch_one(&mut **tx)
            .await?;
    let cnt: i64 = row.try_get("cnt")?;
    if cnt == 0 {
        return Err(custom_entry_type_not_found_error());
    }
    Ok(())
}

async fn validate_entry_types_bulk(db: &SqliteDb, inputs: &[CreateEntry]) -> Result<()> {
    let mut custom_types = HashSet::new();

    for input in inputs {
        let Some(typ) = input.r#type.as_deref() else {
            continue;
        };
        let Ok(type_id) = Uuid::parse_str(typ) else {
            validate_builtin_type_key(typ)?;
            continue;
        };

        custom_types.insert((input.project_id, type_id));
    }

    if custom_types.is_empty() {
        return Ok(());
    }

    let custom_type_pairs = custom_types.into_iter().collect::<Vec<_>>();
    let mut found_types: HashSet<(Uuid, Uuid)> = HashSet::with_capacity(custom_type_pairs.len());

    for chunk in custom_type_pairs.chunks(MAX_ENTRY_TYPE_VALIDATION_PAIRS_PER_QUERY) {
        let mut sql = String::from("SELECT project_id, id FROM entry_types WHERE ");
        for index in 0..chunk.len() {
            if index > 0 {
                sql.push_str(" OR ");
            }
            sql.push_str("(project_id = ? AND id = ?)");
        }

        let mut query = sqlx::query(&sql);
        for pair in chunk {
            query = query.bind(&pair.0).bind(&pair.1);
        }

        let rows = query.fetch_all(&db.pool).await?;
        for row in rows {
            let project_id: Uuid = row.try_get("project_id")?;
            let type_id: Uuid = row.try_get("id")?;
            found_types.insert((project_id, type_id));
        }
    }

    if found_types.len() != custom_type_pairs.len() {
        return Err(custom_entry_type_not_found_error());
    }

    Ok(())
}

impl EntryOps for SqliteDb {
    async fn count_entries(&self, project_id: &Uuid, filter: EntryFilter<'_>) -> Result<i64> {
        let mut sql = "SELECT COUNT(*) as cnt FROM entries WHERE project_id = ?".to_string();
        if filter.category_id.is_some() {
            sql.push_str(" AND category_id = ?");
        }
        if filter.entry_type.is_some() {
            sql.push_str(" AND type = ?");
        }

        let mut q = sqlx::query(&sql).bind(project_id);
        if let Some(cid) = filter.category_id {
            q = q.bind(cid);
        }
        if let Some(t) = filter.entry_type {
            q = q.bind(t);
        }

        let row = q.fetch_one(&self.pool).await?;
        Ok(row.try_get("cnt")?)
    }

    async fn create_entry(&self, input: CreateEntry) -> Result<Entry> {
        validate_entry_type(self, &input.project_id, input.r#type.as_deref()).await?;

        let id = Uuid::now_v7();
        let tags = serde_json::to_string(&input.tags.unwrap_or_default())?;
        let images = serde_json::to_string(&input.images.as_deref().unwrap_or_default())?;
        let cover_path = input
            .cover_path
            .clone()
            .or_else(|| input.images.as_deref().and_then(cover_path_from_images));

        let row = sqlx::query(
            "INSERT INTO entries (id, project_id, category_id, title, summary, content, type, tags, images, cover_path)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         RETURNING id, project_id, category_id, title, summary, content, type, tags, images, cover_path, created_at, updated_at"
        )
            .bind(&id)
            .bind(&input.project_id)
            .bind(&input.category_id)
            .bind(&input.title)
            .bind(&input.summary)
            .bind(input.content.unwrap_or_default())
            .bind(&input.r#type)
            .bind(&tags)
            .bind(&images)
            .bind(&cover_path)
            .fetch_one(&self.pool)
            .await?;

        let result = row_to_entry(&row)?;
        Ok(result)
    }

    async fn get_entry(&self, id: &Uuid) -> Result<Entry> {
        let row = sqlx::query(
            "SELECT id, project_id, category_id, title, summary, content, type, tags, images, cover_path, created_at, updated_at
            FROM entries WHERE id = ?"
        )
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| WorldflowError::NotFound(format!("entry {id}")))?;

        row_to_entry(&row)
    }

    async fn list_entries(
        &self,
        project_id: &Uuid,
        filter: EntryFilter<'_>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<EntryBrief>> {
        let (limit, offset) = super::checked_pagination(limit, offset)?;
        let mut sql =
            "SELECT id, project_id, category_id, title, summary, type, cover_path, updated_at
                       FROM entries WHERE project_id = ?"
                .to_string();
        if filter.category_id.is_some() {
            sql.push_str(" AND category_id = ?");
        }
        if filter.entry_type.is_some() {
            sql.push_str(" AND type = ?");
        }
        sql.push_str(" ORDER BY updated_at DESC LIMIT ? OFFSET ?");

        let mut q = sqlx::query(&sql).bind(project_id);
        if let Some(cid) = filter.category_id {
            q = q.bind(cid);
        }
        if let Some(t) = filter.entry_type {
            q = q.bind(t);
        }
        let rows = q.bind(limit).bind(offset).fetch_all(&self.pool).await?;

        rows.iter().map(row_to_entry_brief).collect()
    }

    async fn search_entries(
        &self,
        project_id: &Uuid,
        query: &str,
        filter: EntryFilter<'_>,
        limit: usize,
    ) -> Result<Vec<EntryBrief>> {
        let query = query.trim();
        let query_char_len = query.chars().count();
        let result_limit = super::checked_limit(limit)?;

        if query_char_len < 3 {
            let like_query = format!("%{}%", escape_like_pattern(query));
            let mut sql =
                "SELECT id, project_id, category_id, title, summary, type, cover_path, updated_at
                           FROM entries
                           WHERE project_id = ?
                             AND (
                                 title LIKE ? ESCAPE '\\'
                                 OR COALESCE(summary, '') LIKE ? ESCAPE '\\'
                                 OR COALESCE(content, '') LIKE ? ESCAPE '\\'
                             )"
                .to_string();
            if filter.category_id.is_some() {
                sql.push_str(" AND category_id = ?");
            }
            if filter.entry_type.is_some() {
                sql.push_str(" AND type = ?");
            }
            sql.push_str(" ORDER BY updated_at DESC LIMIT ?");

            let mut q = sqlx::query(&sql)
                .bind(project_id)
                .bind(&like_query)
                .bind(&like_query)
                .bind(&like_query);
            if let Some(cid) = filter.category_id {
                q = q.bind(cid);
            }
            if let Some(t) = filter.entry_type {
                q = q.bind(t);
            }
            let rows = q.bind(result_limit).fetch_all(&self.pool).await?;

            return rows.iter().map(row_to_entry_brief).collect();
        }

        // FTS 子查询只做 MATCH + LIMIT，LIMIT 才能真正在扫描阶段提前截断候选集。
        // project_id 过滤留在外层 WHERE，走主表 B-tree 索引。
        // 4 倍于最终结果数，上限 500，兼顾 sparse（不过度扫描）和 dense（不爆炸）。
        let Some(fts_query) = escape_fts5_phrase(query) else {
            return Ok(Vec::new());
        };
        let fts_limit = super::checked_scaled_limit(limit, 4, 0, 500)?;
        let mut sql = "SELECT id, project_id, category_id, title, summary, type, cover_path, updated_at
                       FROM entries
                       WHERE project_id = ?
                         AND rowid IN (SELECT rowid FROM entries_fts WHERE entries_fts MATCH ? LIMIT ?)".to_string();
        if filter.category_id.is_some() {
            sql.push_str(" AND category_id = ?");
        }
        if filter.entry_type.is_some() {
            sql.push_str(" AND type = ?");
        }
        sql.push_str(" ORDER BY updated_at DESC LIMIT ?");

        let mut q = sqlx::query(&sql)
            .bind(project_id)
            .bind(fts_query)
            .bind(fts_limit);
        if let Some(cid) = filter.category_id {
            q = q.bind(cid);
        }
        if let Some(t) = filter.entry_type {
            q = q.bind(t);
        }
        let rows = q.bind(result_limit).fetch_all(&self.pool).await?;

        rows.iter().map(row_to_entry_brief).collect()
    }

    async fn update_entry(&self, id: &Uuid, input: UpdateEntry) -> Result<Entry> {
        let existing = self.get_entry(id).await?;
        if let Some(Some(typ)) = input.r#type.as_ref() {
            validate_entry_type(self, &existing.project_id, Some(typ)).await?;
        }

        let tags_json = input.tags.map(|t| serde_json::to_string(&t)).transpose()?;

        let new_cover_path = input
            .cover_path
            .clone()
            .or_else(|| input.images.as_deref().map(cover_path_from_images));
        let cover_path_is_some = new_cover_path.is_some();
        let images_json = input
            .images
            .map(|i| serde_json::to_string(&i))
            .transpose()?;

        let row = sqlx::query(
            "UPDATE entries
         SET title       = COALESCE(?, title),
             summary     = CASE WHEN ? THEN ? ELSE summary END,
             content     = COALESCE(?, content),
             category_id = CASE WHEN ? THEN ? ELSE category_id END,
             type        = CASE WHEN ? THEN ? ELSE type END,
             tags        = COALESCE(?, tags),
             images      = COALESCE(?, images),
             cover_path  = CASE WHEN ? THEN ? ELSE cover_path END
         WHERE id = ?
         RETURNING id, project_id, category_id, title, summary, content, type, tags, images, cover_path, created_at, updated_at"
        )
            .bind(&input.title)
            .bind(input.summary.is_some())
            .bind(input.summary.flatten())
            .bind(&input.content)
            .bind(input.category_id.is_some())
            .bind(input.category_id.flatten())
            .bind(input.r#type.is_some())
            .bind(input.r#type.flatten())
            .bind(tags_json)
            .bind(images_json)
            .bind(cover_path_is_some)
            .bind(new_cover_path.flatten())
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| super::map_row_not_found(e, format!("entry {id}")))?;

        let result = row_to_entry(&row)?;
        Ok(result)
    }

    async fn delete_entry(&self, id: &Uuid) -> Result<()> {
        let result = sqlx::query("DELETE FROM entries WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(WorldflowError::NotFound(format!("entry {id}")));
        }
        Ok(())
    }

    async fn create_entries_bulk(&self, inputs: Vec<CreateEntry>) -> Result<usize> {
        validate_entry_types_bulk(self, &inputs).await?;

        let mut tx = self.pool.begin().await?;
        let mut count = 0;

        for input in inputs {
            let id = Uuid::now_v7();
            let tags = serde_json::to_string(&input.tags.unwrap_or_default())?;
            let images = serde_json::to_string(&input.images.as_deref().unwrap_or_default())?;
            let cover_path = input
                .cover_path
                .clone()
                .or_else(|| input.images.as_deref().and_then(cover_path_from_images));

            sqlx::query(
                "INSERT INTO entries (id, project_id, category_id, title, summary, content, type, tags, images, cover_path)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
                .bind(&id)
                .bind(&input.project_id)
                .bind(&input.category_id)
                .bind(&input.title)
                .bind(&input.summary)
                .bind(input.content.unwrap_or_default())
                .bind(&input.r#type)
                .bind(&tags)
                .bind(&images)
                .bind(&cover_path)
                .execute(&mut *tx)
                .await?;

            count += 1;
        }

        tx.commit().await?;
        Ok(count)
    }

    async fn save_entry_bundle(&self, input: SaveEntryBundle) -> Result<SaveEntryBundleResult> {
        let mut tx = self.pool.begin().await?;

        let existing_row = sqlx::query(
            "SELECT id, project_id, category_id, title, summary, content, type, tags, images, cover_path, created_at, updated_at
             FROM entries WHERE id = ?",
        )
        .bind(input.entry_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| WorldflowError::NotFound(format!("entry {}", input.entry_id)))?;
        let existing_entry = row_to_entry(&existing_row)?;
        if existing_entry.project_id != input.project_id {
            return Err(WorldflowError::InvalidInput(
                "词条不属于当前项目".to_string(),
            ));
        }

        validate_entry_type_in_tx(&mut tx, &input.project_id, input.r#type.as_deref()).await?;

        let same_category_rows = if let Some(category_id) = input.category_id {
            sqlx::query(
                "SELECT id, title
                 FROM entries
                 WHERE project_id = ? AND category_id = ?",
            )
            .bind(input.project_id)
            .bind(category_id)
            .fetch_all(&mut *tx)
            .await?
        } else {
            sqlx::query(
                "SELECT id, title
                 FROM entries
                 WHERE project_id = ? AND category_id IS NULL",
            )
            .bind(input.project_id)
            .fetch_all(&mut *tx)
            .await?
        };
        let normalized_title = normalize_entry_compare_text(&input.title);
        let has_duplicate_title = same_category_rows.iter().any(|row| {
            let id = row.try_get::<Uuid, _>("id");
            let title = row.try_get::<String, _>("title");
            matches!(
                (id, title),
                (Ok(id), Ok(title))
                    if id != input.entry_id
                        && normalize_entry_compare_text(&title) == normalized_title
            )
        });
        if has_duplicate_title {
            return Err(WorldflowError::InvalidInput(
                "当前分类下已存在同名词条，请更换标题。".to_string(),
            ));
        }

        let tags_json = input
            .tags
            .map(|tags| serde_json::to_string(&tags))
            .transpose()?;
        let images_json = input
            .images
            .map(|images| serde_json::to_string(&images))
            .transpose()?;
        let cover_path_is_some = input.cover_path.is_some();
        let cover_path = input.cover_path.flatten();
        let entry_row = sqlx::query(
            "UPDATE entries
             SET title       = ?,
                 summary     = ?,
                 content     = ?,
                 category_id = ?,
                 type        = ?,
                 tags        = COALESCE(?, tags),
                 images      = COALESCE(?, images),
                 cover_path  = CASE WHEN ? THEN ? ELSE cover_path END
             WHERE id = ?
             RETURNING id, project_id, category_id, title, summary, content, type, tags, images, cover_path, created_at, updated_at",
        )
        .bind(&normalized_title)
        .bind(input.summary)
        .bind(input.content)
        .bind(input.category_id)
        .bind(input.r#type)
        .bind(tags_json)
        .bind(images_json)
        .bind(cover_path_is_some)
        .bind(cover_path)
        .bind(input.entry_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| super::map_row_not_found(e, format!("entry {}", input.entry_id)))?;
        let entry = row_to_entry(&entry_row)?;

        let title_rows = sqlx::query(
            "SELECT id, title
             FROM entries
             WHERE project_id = ?
             ORDER BY updated_at DESC",
        )
        .bind(input.project_id)
        .fetch_all(&mut *tx)
        .await?;
        let mut title_to_entry_id = HashMap::<String, Uuid>::new();
        for row in title_rows {
            title_to_entry_id.insert(
                normalize_entry_lookup_title(&row.try_get::<String, _>("title")?),
                row.try_get("id")?,
            );
        }

        let mut target_ids = Vec::<Uuid>::new();
        for target in &input.outgoing_link_targets {
            if let Some(id) = target.entry_id {
                target_ids.push(id);
                continue;
            }
            if let Some(id) = title_to_entry_id.get(&normalize_entry_lookup_title(&target.title)) {
                target_ids.push(*id);
            }
        }
        let normalized_target_ids = normalize_linked_entry_ids(&input.entry_id, &target_ids);

        sqlx::query("DELETE FROM entry_links WHERE a_id = ?")
            .bind(input.entry_id)
            .execute(&mut *tx)
            .await?;

        let mut outgoing_links = Vec::with_capacity(normalized_target_ids.len());
        for target_id in normalized_target_ids {
            let row = sqlx::query(
                "INSERT INTO entry_links (id, project_id, a_id, b_id)
                 VALUES (?, ?, ?, ?)
                 RETURNING id, project_id, a_id, b_id",
            )
            .bind(Uuid::now_v7())
            .bind(input.project_id)
            .bind(input.entry_id)
            .bind(target_id)
            .fetch_one(&mut *tx)
            .await?;
            outgoing_links.push(row_to_link(&row)?);
        }

        let existing_relation_rows = sqlx::query(
            "SELECT id, project_id, a_id, b_id, relation, content, created_at, updated_at
             FROM entry_relations
             WHERE a_id = ?
                OR (b_id = ? AND relation = 'two_way')
             ORDER BY created_at ",
        )
        .bind(input.entry_id)
        .bind(input.entry_id)
        .fetch_all(&mut *tx)
        .await?;
        let existing_relations = existing_relation_rows
            .iter()
            .map(row_to_relation)
            .collect::<Result<Vec<_>>>()?;
        let current_relation_map = existing_relations
            .iter()
            .map(|relation| (relation.id, relation.clone()))
            .collect::<HashMap<_, _>>();
        let next_relation_ids = input
            .relation_patches
            .iter()
            .filter_map(|patch| patch.id)
            .collect::<BTreeSet<_>>();

        for patch in &input.relation_patches {
            let content = normalize_entry_compare_text(&patch.content);
            let existing = patch
                .id
                .and_then(|id| current_relation_map.get(&id).cloned());

            let Some(existing) = existing else {
                let (a_id, b_id) =
                    normalize_relation_endpoints(patch.a_id, patch.b_id, &patch.relation);
                sqlx::query(
                    "INSERT INTO entry_relations (id, project_id, a_id, b_id, relation, content)
                     VALUES (?, ?, ?, ?, ?, ?)",
                )
                .bind(Uuid::now_v7())
                .bind(input.project_id)
                .bind(a_id)
                .bind(b_id)
                .bind(patch.relation.as_str())
                .bind(content)
                .execute(&mut *tx)
                .await?;
                continue;
            };

            if existing.a_id != patch.a_id || existing.b_id != patch.b_id {
                sqlx::query("DELETE FROM entry_relations WHERE id = ?")
                    .bind(existing.id)
                    .execute(&mut *tx)
                    .await?;
                let (a_id, b_id) =
                    normalize_relation_endpoints(patch.a_id, patch.b_id, &patch.relation);
                sqlx::query(
                    "INSERT INTO entry_relations (id, project_id, a_id, b_id, relation, content)
                     VALUES (?, ?, ?, ?, ?, ?)",
                )
                .bind(Uuid::now_v7())
                .bind(input.project_id)
                .bind(a_id)
                .bind(b_id)
                .bind(patch.relation.as_str())
                .bind(content)
                .execute(&mut *tx)
                .await?;
                continue;
            }

            if existing.relation != patch.relation
                || normalize_entry_compare_text(&existing.content) != content
            {
                let needs_swap =
                    patch.relation == RelationDirection::TwoWay && existing.a_id > existing.b_id;
                let sql = if needs_swap {
                    "UPDATE entry_relations
                     SET relation = ?,
                         content  = ?,
                         a_id = b_id, b_id = a_id
                     WHERE id = ?"
                } else {
                    "UPDATE entry_relations
                     SET relation = ?,
                         content  = ?
                     WHERE id = ?"
                };
                sqlx::query(sql)
                    .bind(patch.relation.as_str())
                    .bind(content)
                    .bind(existing.id)
                    .execute(&mut *tx)
                    .await?;
            }
        }

        for existing in &existing_relations {
            if next_relation_ids.contains(&existing.id) {
                continue;
            }
            sqlx::query("DELETE FROM entry_relations WHERE id = ?")
                .bind(existing.id)
                .execute(&mut *tx)
                .await?;
        }

        sqlx::query("UPDATE projects SET name = name WHERE id = ?")
            .bind(input.project_id)
            .execute(&mut *tx)
            .await?;

        let incoming_link_rows = sqlx::query(
            "SELECT id, project_id, a_id, b_id
             FROM entry_links
             WHERE b_id = ?
             ORDER BY rowid",
        )
        .bind(input.entry_id)
        .fetch_all(&mut *tx)
        .await?;
        let incoming_links = incoming_link_rows
            .iter()
            .map(row_to_link)
            .collect::<Result<Vec<_>>>()?;

        let relation_rows = sqlx::query(
            "SELECT id, project_id, a_id, b_id, relation, content, created_at, updated_at
             FROM entry_relations
             WHERE a_id = ?
                OR (b_id = ? AND relation = 'two_way')
             ORDER BY created_at ",
        )
        .bind(input.entry_id)
        .bind(input.entry_id)
        .fetch_all(&mut *tx)
        .await?;
        let relations = relation_rows
            .iter()
            .map(row_to_relation)
            .collect::<Result<Vec<_>>>()?;

        tx.commit().await?;

        Ok(SaveEntryBundleResult {
            entry,
            outgoing_links,
            incoming_links,
            relations,
        })
    }
}
