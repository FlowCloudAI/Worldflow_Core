use std::path::PathBuf;
use crate::{
    db::SqliteDb,
    error::{Result, WorldflowError},
    models::{CreateEntry, Entry, EntryBrief, UpdateEntry},
};
use sqlx::Row;
use uuid::Uuid;

fn row_to_entry(row: &sqlx::sqlite::SqliteRow) -> Result<Entry> {
    let tags_str: String   = row.try_get("tags")?;
    let images_str: String = row.try_get("images")?;
    Ok(Entry {
        id:          row.try_get("id")?,
        project_id:  row.try_get("project_id")?,
        category_id: row.try_get("category_id")?,
        title:       row.try_get("title")?,
        summary:     row.try_get("summary")?,  // 新增
        content:     row.try_get("content")?,
        r#type:      row.try_get("type")?,
        tags:        sqlx::types::Json(serde_json::from_str(&tags_str)?),
        images:      sqlx::types::Json(serde_json::from_str(&images_str)?),
        cover_path:  row.try_get("cover_path")?,
        created_at:  row.try_get("created_at")?,
        updated_at:  row.try_get("updated_at")?,
    })
}

fn row_to_entry_brief(row: &sqlx::sqlite::SqliteRow) -> Result<EntryBrief> {
    let cover_str: Option<String> = row.try_get("cover_path")?;
    Ok(EntryBrief {
        id:          row.try_get("id")?,
        project_id:  row.try_get("project_id")?,
        category_id: row.try_get("category_id")?,
        title:       row.try_get("title")?,
        summary:     row.try_get("summary")?,  // 新增
        r#type:      row.try_get("type")?,
        cover:       cover_str.map(PathBuf::from),
        updated_at:  row.try_get("updated_at")?,
    })
}

impl SqliteDb {
    pub async fn count_entries(&self, project_id: &str, category_id: Option<&str>) -> Result<i64> {
        let row =
            match category_id {
                Some(cid) => sqlx::query(
                    "SELECT COUNT(*) as cnt FROM entries WHERE project_id = ? AND category_id = ?",
                )
                .bind(project_id)
                .bind(cid)
                .fetch_one(&self.pool)
                .await?,

                None => {
                    sqlx::query("SELECT COUNT(*) as cnt FROM entries WHERE project_id = ?")
                        .bind(project_id)
                        .fetch_one(&self.pool)
                        .await?
                }
            };
        Ok(row.try_get("cnt")?)
    }

    pub async fn create_entry(&self, input: CreateEntry) -> Result<Entry> {
        let id     = Uuid::new_v4().to_string();
        let tags   = serde_json::to_string(&input.tags.unwrap_or_default())?;
        let images = serde_json::to_string(&input.images.as_deref().unwrap_or_default())?;
        let cover_path = input.images.as_ref()
            .and_then(|imgs| imgs.iter().find(|i| i.is_cover))
            .map(|i| i.path.to_string_lossy().to_string());

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

        row_to_entry(&row)
    }

    pub async fn get_entry(&self, id: &str) -> Result<Entry> {
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

    pub async fn list_entries(
        &self,
        project_id: &str,
        category_id: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<EntryBrief>> {
        let limit = limit as i64;
        let offset = offset as i64;

        let rows = match category_id {
            Some(cid) => {
                sqlx::query(
                    "SELECT id, project_id, category_id, title, summary, type, cover_path, updated_at
                    FROM entries
                    WHERE project_id = ? AND category_id = ?
                    ORDER BY updated_at DESC
                    LIMIT ? OFFSET ?",
                )
                .bind(project_id)
                .bind(cid)
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool)
                .await?
            }

            None => {
                sqlx::query(
                    "SELECT id, project_id, category_id, title, summary, type, cover_path, updated_at
                    FROM entries
                    WHERE project_id = ?
                    ORDER BY updated_at DESC
                    LIMIT ? OFFSET ?",
                )
                .bind(project_id)
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool)
                .await?
            }
        };

        rows.iter().map(row_to_entry_brief).collect()
    }

    pub async fn search_entries(
        &self,
        project_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<EntryBrief>> {
        let limit = limit as i64;

        let rows = sqlx::query(
            "SELECT id, project_id, category_id, title, summary, type, cover_path, updated_at
                    FROM entries
                    WHERE project_id = ?
                    AND rowid IN (SELECT rowid FROM entries_fts WHERE entries_fts MATCH ?)
                    ORDER BY updated_at DESC
                    LIMIT ?",
        )
        .bind(project_id)
        .bind(query)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_entry_brief).collect()
    }

    pub async fn update_entry(&self, id: &str, input: UpdateEntry) -> Result<Entry> {
        self.get_entry(id).await?;

        let tags_json = input.tags.map(|t| serde_json::to_string(&t)).transpose()?;

        // 必须在 images 被 move 之前提取 cover_path
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
         SET title       = COALESCE(?, title),
             summary     = COALESCE(?, summary),
             content     = COALESCE(?, content),
             category_id = CASE WHEN ? THEN ? ELSE category_id END,
             type        = CASE WHEN ? THEN ? ELSE type END,
             tags        = COALESCE(?, tags),
             images      = COALESCE(?, images),
             cover_path  = CASE WHEN ? THEN ? ELSE cover_path END
         WHERE id = ?
         RETURNING id, project_id, category_id, title, content, type, tags, images, cover_path, created_at, updated_at"
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
            .bind(images_is_some)       // CASE WHEN 的条件
            .bind(new_cover_path.flatten()) // Some(Some(path)) -> Some(path), Some(None) -> None
            .bind(id)
            .fetch_one(&self.pool)
            .await?;

        row_to_entry(&row)
    }

    pub async fn delete_entry(&self, id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM entries WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(WorldflowError::NotFound(format!("entry {id}")));
        }
        Ok(())
    }

    pub async fn create_entries_bulk(&self, inputs: Vec<CreateEntry>) -> Result<usize> {
        let mut tx = self.pool.begin().await?;
        let mut count = 0;

        for input in inputs {
            let id     = Uuid::new_v4().to_string();
            let tags   = serde_json::to_string(&input.tags.unwrap_or_default())?;
            let images = serde_json::to_string(&input.images.as_deref().unwrap_or_default())?;
            let cover_path = input.images.as_ref()
                .and_then(|imgs| imgs.iter().find(|i| i.is_cover))
                .map(|i| i.path.to_string_lossy().to_string());

            sqlx::query(
                "INSERT INTO entries (id, project_id, category_id, title, summary, content, type, tags, images, cover_path)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
                .bind(&id)
                .bind(&input.project_id)
                .bind(&input.category_id)
                .bind(&input.title)
                .bind(&input.summary)  // 新增
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
