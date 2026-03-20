use sqlx::Row;
use uuid::Uuid;
use crate::{
    db::SqliteDb,
    error::{Result, WorldflowError},
    models::{CreateProject, Project, UpdateProject},
};

fn row_to_project(row: &sqlx::sqlite::SqliteRow) -> Result<Project> {
    Ok(Project {
        id:          row.try_get("id")?,
        name:        row.try_get("name")?,
        description: row.try_get("description")?,
        created_at:  row.try_get("created_at")?,
        updated_at:  row.try_get("updated_at")?,
    })
}

impl SqliteDb {
    pub async fn create_project(&self, input: CreateProject) -> Result<Project> {
        let id = Uuid::new_v4().to_string();
        let row = sqlx::query(
            "INSERT INTO projects (id, name, description)
             VALUES (?, ?, ?)
             RETURNING id, name, description, created_at, updated_at"
        )
            .bind(&id)
            .bind(&input.name)
            .bind(&input.description)
            .fetch_one(&self.pool)
            .await?;
        row_to_project(&row)
    }

    pub async fn get_project(&self, id: &str) -> Result<Project> {
        let row = sqlx::query(
            "SELECT id, name, description, created_at, updated_at
             FROM projects WHERE id = ?"
        )
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| WorldflowError::NotFound(format!("project {id}")))?;
        row_to_project(&row)
    }

    pub async fn list_projects(&self) -> Result<Vec<Project>> {
        let rows = sqlx::query(
            "SELECT id, name, description, created_at, updated_at
             FROM projects ORDER BY created_at DESC"
        )
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(row_to_project).collect()
    }

    pub async fn update_project(&self, id: &str, input: UpdateProject) -> Result<Project> {
        self.get_project(id).await?;
        let row = sqlx::query(
            "UPDATE projects
             SET name        = COALESCE(?, name),
                 description = COALESCE(?, description)
             WHERE id = ?
             RETURNING id, name, description, created_at, updated_at"
        )
            .bind(&input.name)
            .bind(&input.description)
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        row_to_project(&row)
    }

    pub async fn delete_project(&self, id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM projects WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(WorldflowError::NotFound(format!("project {id}")));
        }
        Ok(())
    }
}