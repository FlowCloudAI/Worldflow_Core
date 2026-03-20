use sqlx::Row;
use uuid::Uuid;
use crate::{
    db::SqliteDb,
    error::{Result, WorldflowError},
    models::{CreateTagSchema, TagSchema},
};

fn row_to_tag_schema(row: &sqlx::sqlite::SqliteRow) -> Result<TagSchema> {
    Ok(TagSchema {
        id:          row.try_get("id")?,
        project_id:  row.try_get("project_id")?,
        name:        row.try_get("name")?,
        description: row.try_get("description")?,
        r#type:      row.try_get("type")?,
        target:      row.try_get("target")?,
        default_val: row.try_get("default_val")?,
        range_min:   row.try_get("range_min")?,
        range_max:   row.try_get("range_max")?,
        sort_order:  row.try_get("sort_order")?,
        created_at:  row.try_get("created_at")?,
        updated_at:  row.try_get("updated_at")?,
    })
}

impl SqliteDb {
    pub async fn create_tag_schema(&self, input: CreateTagSchema) -> Result<TagSchema> {
        if let Some(ref val) = input.default_val {
            if input.r#type == "number" && val.parse::<f64>().is_err() {
                return Err(WorldflowError::InvalidInput(
                    format!("default_val '{}' 不是合法数字", val)
                ));
            }
            if input.r#type == "boolean" && val != "true" && val != "false" {
                return Err(WorldflowError::InvalidInput(
                    format!("default_val '{}' 不是合法布尔值", val)
                ));
            }
        }

        let id = Uuid::new_v4().to_string();
        let sort_order = input.sort_order.unwrap_or(0);
        let target = serde_json::to_string(&input.target)?;

        let row = sqlx::query(
            "INSERT INTO tag_schemas
             (id, project_id, name, description, type, target, default_val, range_min, range_max, sort_order)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             RETURNING id, project_id, name, description, type, target,
                       default_val, range_min, range_max, sort_order, created_at, updated_at"
        )
            .bind(&id)
            .bind(&input.project_id)
            .bind(&input.name)
            .bind(&input.description)
            .bind(&input.r#type)
            .bind(&target)
            .bind(&input.default_val)
            .bind(input.range_min)
            .bind(input.range_max)
            .bind(sort_order)
            .fetch_one(&self.pool)
            .await?;

        row_to_tag_schema(&row)
    }

    pub async fn get_tag_schema(&self, id: &str) -> Result<TagSchema> {
        let row = sqlx::query(
            "SELECT id, project_id, name, description, type, target,
                    default_val, range_min, range_max, sort_order, created_at, updated_at
             FROM tag_schemas WHERE id = ?"
        )
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| WorldflowError::NotFound(format!("tag_schema {id}")))?;

        row_to_tag_schema(&row)
    }

    pub async fn list_tag_schemas(&self, project_id: &str) -> Result<Vec<TagSchema>> {
        let rows = sqlx::query(
            "SELECT id, project_id, name, description, type, target,
                    default_val, range_min, range_max, sort_order, created_at, updated_at
             FROM tag_schemas
             WHERE project_id = ?
             ORDER BY sort_order , name "
        )
            .bind(project_id)
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(row_to_tag_schema).collect()
    }

    pub async fn update_tag_schema(&self, id: &str, input: CreateTagSchema) -> Result<TagSchema> {
        self.get_tag_schema(id).await?;
        let target = serde_json::to_string(&input.target)?;

        let row = sqlx::query(
            "UPDATE tag_schemas
             SET name        = ?,
                 description = ?,
                 type        = ?,
                 target      = ?,
                 default_val = ?,
                 range_min   = ?,
                 range_max   = ?,
                 sort_order  = COALESCE(?, sort_order)
             WHERE id = ?
             RETURNING id, project_id, name, description, type, target,
                       default_val, range_min, range_max, sort_order, created_at, updated_at"
        )
            .bind(&input.name)
            .bind(&input.description)
            .bind(&input.r#type)
            .bind(&target)
            .bind(&input.default_val)
            .bind(input.range_min)
            .bind(input.range_max)
            .bind(input.sort_order)
            .bind(id)
            .fetch_one(&self.pool)
            .await?;

        row_to_tag_schema(&row)
    }

    pub async fn delete_tag_schema(&self, id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM tag_schemas WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(WorldflowError::NotFound(format!("tag_schema {id}")));
        }
        Ok(())
    }
}