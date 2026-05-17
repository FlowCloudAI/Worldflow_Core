use super::traits::IdeaNoteOps;
use crate::{
    db::PgDb,
    error::{Result, WorldflowError},
    models::{CreateIdeaNote, IdeaNote, IdeaNoteFilter, IdeaNoteStatus, UpdateIdeaNote},
};
use sqlx::Row;
use uuid::Uuid;

fn row_to_idea_note(row: &sqlx::postgres::PgRow) -> Result<IdeaNote> {
    let status_str: String = row.try_get("status")?;
    let status = status_str.parse::<IdeaNoteStatus>()?;
    Ok(IdeaNote {
        id: row.try_get("id")?,
        project_id: row.try_get("project_id")?,
        content: row.try_get("content")?,
        title: row.try_get("title")?,
        status,
        pinned: row.try_get("pinned")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        last_reviewed_at: row.try_get("last_reviewed_at")?,
        converted_entry_id: row.try_get("converted_entry_id")?,
    })
}

impl IdeaNoteOps for PgDb {
    async fn create_idea_note(&self, input: CreateIdeaNote) -> Result<IdeaNote> {
        let id = Uuid::now_v7();
        let pinned = input.pinned.unwrap_or(false);

        let row = sqlx::query(
            "INSERT INTO idea_notes (id, project_id, content, title, pinned)
             VALUES ($1, $2, $3, $4, $5)
             RETURNING id, project_id, content, title, status, pinned,
                       created_at::TEXT AS created_at,
                       updated_at::TEXT AS updated_at,
                       last_reviewed_at::TEXT AS last_reviewed_at,
                       converted_entry_id",
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
                    created_at::TEXT AS created_at,
                    updated_at::TEXT AS updated_at,
                    last_reviewed_at::TEXT AS last_reviewed_at,
                    converted_entry_id
             FROM idea_notes WHERE id = $1",
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
        let (limit, offset) = super::checked_pagination(limit, offset)?;

        let mut p = 1usize;
        let mut sql = "SELECT id, project_id, content, title, status, pinned, \
                    created_at::TEXT AS created_at, \
                    updated_at::TEXT AS updated_at, \
                    last_reviewed_at::TEXT AS last_reviewed_at, \
                    converted_entry_id \
             FROM idea_notes WHERE 1=1"
            .to_string();

        if filter.only_global {
            sql.push_str(" AND project_id IS NULL");
        } else if filter.project_id.is_some() {
            sql.push_str(&format!(" AND project_id = ${p}"));
            p += 1;
        }
        if filter.status.is_some() {
            sql.push_str(&format!(" AND status = ${p}"));
            p += 1;
        }
        if filter.pinned.is_some() {
            sql.push_str(&format!(" AND pinned = ${p}"));
            p += 1;
        }
        sql.push_str(&format!(
            " ORDER BY pinned DESC, updated_at DESC LIMIT ${p} OFFSET ${}",
            p + 1
        ));

        let mut q = sqlx::query(&sql);
        if let Some(pid) = filter.project_id {
            q = q.bind(pid);
        }
        if let Some(s) = filter.status {
            q = q.bind(s.as_str());
        }
        if let Some(pv) = filter.pinned {
            q = q.bind(pv);
        }
        let rows = q.bind(limit).bind(offset).fetch_all(&self.pool).await?;

        rows.iter().map(row_to_idea_note).collect()
    }

    async fn update_idea_note(&self, id: &Uuid, input: UpdateIdeaNote) -> Result<IdeaNote> {
        self.get_idea_note(id).await?;

        let row = sqlx::query(
            "UPDATE idea_notes
             SET project_id         = CASE WHEN $1 THEN $2 ELSE project_id END,
                 title              = CASE WHEN $3 THEN $4 ELSE title END,
                 content            = COALESCE($5, content),
                 status             = COALESCE($6, status),
                 pinned             = COALESCE($7, pinned),
                 last_reviewed_at   = CASE WHEN $8 THEN $9 ELSE last_reviewed_at END,
                 converted_entry_id = CASE WHEN $10 THEN $11 ELSE converted_entry_id END
             WHERE id = $12
             RETURNING id, project_id, content, title, status, pinned,
                       created_at::TEXT AS created_at,
                       updated_at::TEXT AS updated_at,
                       last_reviewed_at::TEXT AS last_reviewed_at,
                       converted_entry_id",
        )
        .bind(input.project_id.is_some())
        .bind(input.project_id.flatten())
        .bind(input.title.is_some())
        .bind(input.title.flatten())
        .bind(&input.content)
        .bind(input.status.as_ref().map(|s| s.as_str()))
        .bind(input.pinned)
        .bind(input.last_reviewed_at.is_some())
        .bind(input.last_reviewed_at.flatten())
        .bind(input.converted_entry_id.is_some())
        .bind(input.converted_entry_id.flatten())
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| super::map_row_not_found(e, format!("idea_note {id}")))?;

        row_to_idea_note(&row)
    }

    async fn delete_idea_note(&self, id: &Uuid) -> Result<()> {
        let result = sqlx::query("DELETE FROM idea_notes WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(WorldflowError::NotFound(format!("idea_note {id}")));
        }
        Ok(())
    }
}
