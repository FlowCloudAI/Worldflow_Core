use super::traits::ProjectOps;
use crate::{
    db::SqliteDb,
    error::{Result, WorldflowError},
    models::{CreateProject, Project, UpdateProject},
};
use sqlx::Row;
use uuid::Uuid;

fn row_to_project(row: &sqlx::sqlite::SqliteRow) -> Result<Project> {
    Ok(Project {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        description: row.try_get("description")?,
        cover_image: row.try_get("cover_image")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

impl ProjectOps for SqliteDb {
    async fn create_project(&self, input: CreateProject) -> Result<Project> {
        let id = Uuid::now_v7();
        let row = sqlx::query(
            "INSERT INTO projects (id, name, description, cover_image)
             VALUES (?, ?, ?, ?)
             RETURNING id, name, description, cover_image, created_at, updated_at",
        )
        .bind(&id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&input.cover_image)
        .fetch_one(&self.pool)
        .await?;
        let result = row_to_project(&row)?;
        self.trigger_snapshot();
        Ok(result)
    }

    async fn get_project(&self, id: &Uuid) -> Result<Project> {
        let row = sqlx::query(
            "SELECT id, name, description, cover_image, created_at, updated_at
             FROM projects WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| WorldflowError::NotFound(format!("project {id}")))?;
        row_to_project(&row)
    }

    async fn list_projects(&self) -> Result<Vec<Project>> {
        let rows = sqlx::query(
            "SELECT id, name, description, cover_image, created_at, updated_at
             FROM projects ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_project).collect()
    }

    async fn update_project(&self, id: &Uuid, input: UpdateProject) -> Result<Project> {
        self.get_project(id).await?;
        let row = sqlx::query(
            "UPDATE projects
             SET name        = COALESCE(?, name),
                 description = COALESCE(?, description),
                 cover_image = CASE WHEN ? THEN ? ELSE cover_image END
             WHERE id = ?
             RETURNING id, name, description, cover_image, created_at, updated_at",
        )
        .bind(&input.name)
        .bind(&input.description)
        .bind(input.cover_image.is_some())
        .bind(input.cover_image.as_ref().and_then(|v| v.as_deref()))
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        let result = row_to_project(&row)?;
        self.trigger_snapshot();
        Ok(result)
    }

    async fn delete_project(&self, id: &Uuid) -> Result<()> {
        let result = sqlx::query("DELETE FROM projects WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(WorldflowError::NotFound(format!("project {id}")));
        }
        self.trigger_snapshot();
        Ok(())
    }
}
