use super::traits::IdeaNoteOps;
use crate::{
    db::SqliteDb,
    error::{Result, WorldflowError},
    models::{CreateIdeaNote, IdeaNote, IdeaNoteFilter, IdeaNoteStatus, UpdateIdeaNote},
};
use sqlx::Row;
use uuid::Uuid;

fn row_to_idea_note(row: &sqlx::sqlite::SqliteRow) -> Result<IdeaNote> {
    let status_str: String = row.try_get("status")?;
    let status = status_str.parse::<IdeaNoteStatus>()?;
    let pinned: i64 = row.try_get("pinned")?;
    Ok(IdeaNote {
        id: row.try_get("id")?,
        project_id: row.try_get("project_id")?,
        content: row.try_get("content")?,
        title: row.try_get("title")?,
        status,
        pinned: pinned != 0,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        last_reviewed_at: row.try_get("last_reviewed_at")?,
        converted_entry_id: row.try_get("converted_entry_id")?,
    })
}

impl IdeaNoteOps for SqliteDb {
    async fn create_idea_note(&self, input: CreateIdeaNote) -> Result<IdeaNote> {
        let id = Uuid::now_v7();
        let pinned = if input.pinned.unwrap_or(false) {
            1i64
        } else {
            0i64
        };

        let row = sqlx::query(
            "INSERT INTO idea_notes (id, project_id, content, title, pinned)
             VALUES (?, ?, ?, ?, ?)
             RETURNING id, project_id, content, title, status, pinned,
                       created_at, updated_at, last_reviewed_at, converted_entry_id",
        )
        .bind(&id)
        .bind(&input.project_id)
        .bind(&input.content)
        .bind(&input.title)
        .bind(pinned)
        .fetch_one(&self.pool)
        .await?;

        row_to_idea_note(&row)
    }

    async fn get_idea_note(&self, id: &Uuid) -> Result<IdeaNote> {
        let row = sqlx::query(
            "SELECT id, project_id, content, title, status, pinned,
                    created_at, updated_at, last_reviewed_at, converted_entry_id
             FROM idea_notes WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| WorldflowError::NotFound(format!("idea_note {id}")))?;

        row_to_idea_note(&row)
    }

    async fn list_idea_notes(
        &self,
        filter: IdeaNoteFilter<'_>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<IdeaNote>> {
        if filter.only_global && filter.project_id.is_some() {
            return Err(WorldflowError::InvalidInput(
                "only_global 与 project_id 不能同时使用".to_string(),
            ));
        }

        let mut sql = "SELECT id, project_id, content, title, status, pinned, \
                    created_at, updated_at, last_reviewed_at, converted_entry_id \
             FROM idea_notes WHERE 1=1"
            .to_string();

        if filter.only_global {
            sql.push_str(" AND project_id IS NULL");
        } else if filter.project_id.is_some() {
            sql.push_str(" AND project_id = ?");
        }
        if filter.status.is_some() {
            sql.push_str(" AND status = ?");
        }
        if filter.pinned.is_some() {
            sql.push_str(" AND pinned = ?");
        }
        sql.push_str(" ORDER BY pinned DESC, updated_at DESC LIMIT ? OFFSET ?");

        let mut q = sqlx::query(&sql);
        if let Some(pid) = filter.project_id {
            q = q.bind(pid);
        }
        if let Some(s) = filter.status {
            q = q.bind(s.as_str());
        }
        if let Some(p) = filter.pinned {
            q = q.bind(if p { 1i64 } else { 0i64 });
        }
        let rows = q
            .bind(limit as i64)
            .bind(offset as i64)
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(row_to_idea_note).collect()
    }

    async fn update_idea_note(&self, id: &Uuid, input: UpdateIdeaNote) -> Result<IdeaNote> {
        self.get_idea_note(id).await?;

        let pinned_val = input.pinned.map(|b| if b { 1i64 } else { 0i64 });

        let row = sqlx::query(
            "UPDATE idea_notes
             SET title              = CASE WHEN ? THEN ? ELSE title END,
                 content            = COALESCE(?, content),
                 status             = COALESCE(?, status),
                 pinned             = COALESCE(?, pinned),
                 last_reviewed_at   = CASE WHEN ? THEN ? ELSE last_reviewed_at END,
                 converted_entry_id = CASE WHEN ? THEN ? ELSE converted_entry_id END
             WHERE id = ?
             RETURNING id, project_id, content, title, status, pinned,
                       created_at, updated_at, last_reviewed_at, converted_entry_id",
        )
        // title: CASE WHEN title.is_some() THEN title.flatten() ELSE 原值 END
        .bind(input.title.is_some())
        .bind(input.title.flatten())
        // content: COALESCE(新值, 原值)
        .bind(&input.content)
        // status: COALESCE(新值字符串, 原值)
        .bind(input.status.as_ref().map(|s| s.as_str()))
        // pinned: COALESCE(新值, 原值)
        .bind(pinned_val)
        // last_reviewed_at
        .bind(input.last_reviewed_at.is_some())
        .bind(input.last_reviewed_at.flatten())
        // converted_entry_id
        .bind(input.converted_entry_id.is_some())
        .bind(input.converted_entry_id.flatten())
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        row_to_idea_note(&row)
    }

    async fn delete_idea_note(&self, id: &Uuid) -> Result<()> {
        let result = sqlx::query("DELETE FROM idea_notes WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(WorldflowError::NotFound(format!("idea_note {id}")));
        }
        Ok(())
    }
}
