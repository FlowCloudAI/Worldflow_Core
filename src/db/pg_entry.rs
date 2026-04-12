use super::traits::EntryOps;
use crate::{
    db::PgDb,
    error::{Result, WorldflowError},
    models::{CreateEntry, Entry, EntryBrief, EntryFilter, UpdateEntry},
};
use sqlx::Row;
use std::path::PathBuf;
use uuid::Uuid;

fn row_to_entry(row: &sqlx::postgres::PgRow) -> Result<Entry> {
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

fn row_to_entry_brief(row: &sqlx::postgres::PgRow) -> Result<EntryBrief> {
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

impl EntryOps for PgDb {
    async fn count_entries(&self, project_id: &Uuid, filter: EntryFilter<'_>) -> Result<i64> {
        let mut p = 2usize;
        let mut sql = "SELECT COUNT(*) as cnt FROM entries WHERE project_id = $1".to_string();
        if filter.category_id.is_some() {
            sql.push_str(&format!(" AND category_id = ${p}"));
            p += 1;
        }
        if filter.entry_type.is_some() {
            sql.push_str(&format!(" AND type = ${p}"));
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
        let id = Uuid::now_v7();
        let tags = serde_json::to_string(&input.tags.unwrap_or_default())?;
        let images = serde_json::to_string(&input.images.as_deref().unwrap_or_default())?;
        let cover_path = input
            .images
            .as_ref()
            .and_then(|imgs| imgs.iter().find(|i| i.is_cover))
            .map(|i| i.path.to_string_lossy().to_string());

        let row = sqlx::query(
            "INSERT INTO entries (id, project_id, category_id, title, summary, content, type, tags, images, cover_path)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             RETURNING id, project_id, category_id, title, summary, content, type, tags, images, cover_path, created_at::TEXT, updated_at::TEXT"
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

        row_to_entry(&row)
    }

    async fn get_entry(&self, id: &Uuid) -> Result<Entry> {
        let row = sqlx::query(
            "SELECT id, project_id, category_id, title, summary, content, type, tags, images, cover_path, created_at::TEXT, updated_at::TEXT
             FROM entries WHERE id = $1"
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
        let mut p = 2usize;
        let mut sql =
            "SELECT id, project_id, category_id, title, summary, type, cover_path, updated_at::TEXT
                       FROM entries WHERE project_id = $1"
                .to_string();
        if filter.category_id.is_some() {
            sql.push_str(&format!(" AND category_id = ${p}"));
            p += 1;
        }
        if filter.entry_type.is_some() {
            sql.push_str(&format!(" AND type = ${p}"));
            p += 1;
        }
        sql.push_str(&format!(
            " ORDER BY updated_at DESC LIMIT ${p} OFFSET ${}",
            p + 1
        ));

        let mut q = sqlx::query(&sql).bind(project_id);
        if let Some(cid) = filter.category_id {
            q = q.bind(cid);
        }
        if let Some(t) = filter.entry_type {
            q = q.bind(t);
        }
        let rows = q
            .bind(limit as i64)
            .bind(offset as i64)
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(row_to_entry_brief).collect()
    }

    async fn search_entries(
        &self,
        project_id: &Uuid,
        query: &str,
        filter: EntryFilter<'_>,
        limit: usize,
    ) -> Result<Vec<EntryBrief>> {
        // $3 = fts_limit：内层子查询先限制候选集，防止高频词命中爆炸后排序代价过高
        let fts_limit = (limit * 20).max(500).min(2000) as i64;
        let mut p = 4usize;
        let mut sql = "SELECT id, project_id, category_id, title, summary, type, cover_path, updated_at::TEXT
                       FROM (
                         SELECT id, project_id, category_id, title, summary, type, cover_path, updated_at
                         FROM entries
                         WHERE project_id = $1
                           AND to_tsvector('simple', coalesce(title,'') || ' ' || coalesce(summary,'') || ' ' || coalesce(content,''))
                               @@ plainto_tsquery('simple', $2)
                         LIMIT $3
                       ) _fts
                       WHERE true".to_string();
        if filter.category_id.is_some() {
            sql.push_str(&format!(" AND category_id = ${p}"));
            p += 1;
        }
        if filter.entry_type.is_some() {
            sql.push_str(&format!(" AND type = ${p}"));
            p += 1;
        }
        sql.push_str(&format!(" ORDER BY updated_at DESC LIMIT ${p}"));

        let mut q = sqlx::query(&sql)
            .bind(project_id)
            .bind(query)
            .bind(fts_limit);
        if let Some(cid) = filter.category_id {
            q = q.bind(cid);
        }
        if let Some(t) = filter.entry_type {
            q = q.bind(t);
        }
        let rows = q.bind(limit as i64).fetch_all(&self.pool).await?;

        rows.iter().map(row_to_entry_brief).collect()
    }

    async fn update_entry(&self, id: &Uuid, input: UpdateEntry) -> Result<Entry> {
        self.get_entry(id).await?;

        let tags_json = input.tags.map(|t| serde_json::to_string(&t)).transpose()?;

        let new_cover_path = input.images.as_ref().map(|imgs| {
            imgs.iter()
                .find(|i| i.is_cover)
                .map(|i| i.path.to_string_lossy().to_string())
        });
        let images_is_some = input.images.is_some();
        let images_json = input
            .images
            .map(|i| serde_json::to_string(&i))
            .transpose()?;

        let row = sqlx::query(
            "UPDATE entries
             SET title       = COALESCE($1, title),
                 summary     = COALESCE($2, summary),
                 content     = COALESCE($3, content),
                 category_id = CASE WHEN $4 THEN $5 ELSE category_id END,
                 type        = CASE WHEN $6 THEN $7 ELSE type END,
                 tags        = COALESCE($8, tags),
                 images      = COALESCE($9, images),
                 cover_path  = CASE WHEN $10 THEN $11 ELSE cover_path END
             WHERE id = $12
             RETURNING id, project_id, category_id, title, summary, content, type, tags, images, cover_path, created_at::TEXT, updated_at::TEXT"
        )
            .bind(&input.title)
            .bind(&input.summary)
            .bind(&input.content)
            .bind(input.category_id.is_some())
            .bind(input.category_id.flatten())
            .bind(input.r#type.is_some())
            .bind(input.r#type.flatten())
            .bind(tags_json)
            .bind(images_json)
            .bind(images_is_some)
            .bind(new_cover_path.flatten())
            .bind(id)
            .fetch_one(&self.pool)
            .await?;

        row_to_entry(&row)
    }

    async fn delete_entry(&self, id: &Uuid) -> Result<()> {
        let result = sqlx::query("DELETE FROM entries WHERE id = $1")
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
            let id = Uuid::now_v7();
            let tags = serde_json::to_string(&input.tags.unwrap_or_default())?;
            let images = serde_json::to_string(&input.images.as_deref().unwrap_or_default())?;
            let cover_path = input
                .images
                .as_ref()
                .and_then(|imgs| imgs.iter().find(|i| i.is_cover))
                .map(|i| i.path.to_string_lossy().to_string());

            sqlx::query(
                "INSERT INTO entries (id, project_id, category_id, title, summary, content, type, tags, images, cover_path)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"
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
