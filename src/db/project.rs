use super::traits::ProjectOps;
use crate::{
    db::SqliteDb,
    error::{Result, WorldflowError},
    models::{CreateProject, Project, UpdateProject},
};
use sqlx::Row;
use uuid::Uuid;

struct DefaultTimelineTagDefinition {
    name: &'static str,
    description: &'static str,
    value_type: &'static str,
    default_value: Option<&'static str>,
    range_min: Option<f64>,
    range_max: Option<f64>,
    sort_order: i64,
}

const DEFAULT_TIMELINE_TAG_TARGETS: &[&str] = &["event"];

const DEFAULT_TIMELINE_TAG_DEFINITIONS: &[DefaultTimelineTagDefinition] = &[
    DefaultTimelineTagDefinition {
        name: "开始年份",
        description: "事件在时间线上的起始年份。公元前年份请填写负数，例如 -221。",
        value_type: "number",
        default_value: None,
        range_min: None,
        range_max: None,
        sort_order: 0,
    },
    DefaultTimelineTagDefinition {
        name: "结束年份",
        description: "事件结束年份；若留空则视为单点事件。",
        value_type: "number",
        default_value: None,
        range_min: None,
        range_max: None,
        sort_order: 1,
    },
    DefaultTimelineTagDefinition {
        name: "父事件ID",
        description: "用于把事件挂到上层事件下，可填写父事件词条 ID 或标题。",
        value_type: "string",
        default_value: None,
        range_min: None,
        range_max: None,
        sort_order: 2,
    },
    DefaultTimelineTagDefinition {
        name: "时间线",
        description: "是否在项目时间线中显示该事件。",
        value_type: "boolean",
        default_value: Some("true"),
        range_min: None,
        range_max: None,
        sort_order: 3,
    },
];

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
        Ok(result)
    }

    async fn create_project_with_default_timeline_tags(
        &self,
        input: CreateProject,
    ) -> Result<Project> {
        let mut tx = self.pool.begin().await?;
        let id = Uuid::now_v7();
        let row = sqlx::query(
            "INSERT INTO projects (id, name, description, cover_image)
             VALUES (?, ?, ?, ?)
             RETURNING id, name, description, cover_image, created_at, updated_at",
        )
        .bind(id)
        .bind(input.name)
        .bind(input.description)
        .bind(input.cover_image)
        .fetch_one(&mut *tx)
        .await?;
        let project = row_to_project(&row)?;
        let target = serde_json::to_string(&DEFAULT_TIMELINE_TAG_TARGETS)?;

        for definition in DEFAULT_TIMELINE_TAG_DEFINITIONS {
            sqlx::query(
                "INSERT INTO tag_schemas
                 (id, project_id, name, description, type, target, default_val, range_min, range_max, sort_order)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(Uuid::now_v7())
            .bind(project.id)
            .bind(definition.name)
            .bind(definition.description)
            .bind(definition.value_type)
            .bind(&target)
            .bind(definition.default_value)
            .bind(definition.range_min)
            .bind(definition.range_max)
            .bind(definition.sort_order)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(project)
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
                 description = CASE WHEN ? THEN ? ELSE description END,
                 cover_image = CASE WHEN ? THEN ? ELSE cover_image END
             WHERE id = ?
             RETURNING id, name, description, cover_image, created_at, updated_at",
        )
        .bind(&input.name)
        .bind(input.description.is_some())
        .bind(input.description.as_ref().and_then(|v| v.as_deref()))
        .bind(input.cover_image.is_some())
        .bind(input.cover_image.as_ref().and_then(|v| v.as_deref()))
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| super::map_row_not_found(e, format!("project {id}")))?;
        let result = row_to_project(&row)?;
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
        Ok(())
    }
}
