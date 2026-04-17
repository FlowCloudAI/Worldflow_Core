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

        let mut p = 1usize;
        let mut sql = "SELECT id, project_id, content, title, status, pinned, \
                    created_at, updated_at, last_reviewed_at, converted_entry_id \
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
        let rows = q
            .bind(limit as i64)
            .bind(offset as i64)
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(row_to_idea_note).collect()
    }

    async fn update_idea_note(&self, id: &Uuid, input: UpdateIdeaNote) -> Result<IdeaNote> {
        self.get_idea_note(id).await?;

        // 动态构建 SET 子句，只更新有值的字段
        let mut sets: Vec<String> = Vec::new();
        let mut p = 1usize;

        if input.title.is_some() {
            sets.push(format!("title = ${p}"));
            p += 1;
        }
        if input.content.is_some() {
            sets.push(format!("content = ${p}"));
            p += 1;
        }
        if input.status.is_some() {
            sets.push(format!("status = ${p}"));
            p += 1;
        }
        if input.pinned.is_some() {
            sets.push(format!("pinned = ${p}"));
            p += 1;
        }
        if input.last_reviewed_at.is_some() {
            sets.push(format!("last_reviewed_at = ${p}"));
            p += 1;
        }
        if input.converted_entry_id.is_some() {
            sets.push(format!("converted_entry_id = ${p}"));
            p += 1;
        }

        if sets.is_empty() {
            return self.get_idea_note(id).await;
        }

        let sql = format!(
            "UPDATE idea_notes SET {} WHERE id = ${p} \
             RETURNING id, project_id, content, title, status, pinned, \
                       created_at, updated_at, last_reviewed_at, converted_entry_id",
            sets.join(", ")
        );

        let mut q = sqlx::query(&sql);
        if let Some(t) = input.title {
            q = q.bind(t);
        }
        if let Some(c) = input.content {
            q = q.bind(c);
        }
        if let Some(s) = input.status {
            q = q.bind(s.as_str().to_owned());
        }
        if let Some(pv) = input.pinned {
            q = q.bind(pv);
        }
        if let Some(lr) = input.last_reviewed_at {
            q = q.bind(lr);
        }
        if let Some(ce) = input.converted_entry_id {
            q = q.bind(ce);
        }
        let row = q.bind(id).fetch_one(&self.pool).await?;

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
