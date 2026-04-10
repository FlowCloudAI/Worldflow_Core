use sqlx::Row;
use uuid::Uuid;
use crate::{
    db::PgDb,
    error::{Result, WorldflowError},
    models::{CreateProject, Project, UpdateProject},
};
use super::traits::ProjectOps;

fn row_to_project(row: &sqlx::postgres::PgRow) -> Result<Project> {
    Ok(Project {
        id:          row.try_get("id")?,
        name:        row.try_get("name")?,
        description: row.try_get("description")?,
        cover_image: row.try_get("cover_image")?,
        created_at:  row.try_get("created_at")?,
        updated_at:  row.try_get("updated_at")?,
    })
}

impl ProjectOps for PgDb {
    async fn create_project(&self, input: CreateProject) -> Result<Project> {
        let id = Uuid::now_v7();
        let row = sqlx::query(
            "INSERT INTO projects (id, name, description, cover_image)
             VALUES ($1, $2, $3, $4)
             RETURNING id, name, description, cover_image, created_at::TEXT, updated_at::TEXT"
        )
            .bind(&id)
            .bind(&input.name)
            .bind(&input.description)
            .bind(&input.cover_image)
            .fetch_one(&self.pool)
            .await?;
        row_to_project(&row)
    }

    async fn get_project(&self, id: &Uuid) -> Result<Project> {
        let row = sqlx::query(
            "SELECT id, name, description, cover_image, created_at::TEXT, updated_at::TEXT
             FROM projects WHERE id = $1"
        )
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| WorldflowError::NotFound(format!("project {id}")))?;
        row_to_project(&row)
    }

    async fn list_projects(&self) -> Result<Vec<Project>> {
        let rows = sqlx::query(
            "SELECT id, name, description, cover_image, created_at::TEXT, updated_at::TEXT
             FROM projects ORDER BY created_at DESC"
        )
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(row_to_project).collect()
    }

    async fn update_project(&self, id: &Uuid, input: UpdateProject) -> Result<Project> {
        self.get_project(id).await?;
        let row = sqlx::query(
            "UPDATE projects
             SET name        = COALESCE($1, name),
                 description = COALESCE($2, description),
                 cover_image = CASE WHEN $3 THEN $4 ELSE cover_image END
             WHERE id = $5
             RETURNING id, name, description, cover_image, created_at::TEXT, updated_at::TEXT"
        )
            .bind(&input.name)
            .bind(&input.description)
            .bind(input.cover_image.is_some())
            .bind(input.cover_image.as_ref().and_then(|v| v.as_deref()))
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        row_to_project(&row)
    }

    async fn delete_project(&self, id: &Uuid) -> Result<()> {
        let result = sqlx::query("DELETE FROM projects WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(WorldflowError::NotFound(format!("project {id}")));
        }
        Ok(())
    }
}
