use super::traits::EntryOps;
use crate::{
    db::SqliteDb,
    error::{Result, WorldflowError},
    models::{
        CreateEntry, Entry, EntryBrief, EntryFilter, FCImage, UpdateEntry,
        validate_builtin_type_key,
    },
};
use sqlx::Row;
use std::path::PathBuf;
use uuid::Uuid;

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
        return Err(WorldflowError::InvalidInput(
            "自定义词条类型不存在或不属于当前项目".to_string(),
        ));
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
        let mut tx = self.pool.begin().await?;
        let mut count = 0;

        for input in inputs {
            validate_entry_type(self, &input.project_id, input.r#type.as_deref()).await?;

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
}
