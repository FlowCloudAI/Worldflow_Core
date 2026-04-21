use sqlx::Row;
use uuid::Uuid;

use crate::{
    db::SqliteDb,
    error::{Result, WorldflowError},
    models::{CreateCustomEntryType, CustomEntryType, EntryTypeView, UpdateCustomEntryType},
};

use super::traits::EntryTypeOps;

/// 将数据库行转换为 CustomEntryType
fn row_to_custom_entry_type(row: &sqlx::sqlite::SqliteRow) -> Result<CustomEntryType> {
    Ok(CustomEntryType {
        id: row.try_get("id")?,
        project_id: row.try_get("project_id")?,
        name: row.try_get("name")?,
        description: row.try_get("description")?,
        icon: row.try_get("icon")?,
        color: row.try_get("color")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

impl EntryTypeOps for SqliteDb {
    async fn create_entry_type(&self, input: CreateCustomEntryType) -> Result<CustomEntryType> {
        let id = Uuid::now_v7();

        let row = sqlx::query(
            "INSERT INTO entry_types (id, project_id, name, description, icon, color)
             VALUES (?, ?, ?, ?, ?, ?)
             RETURNING id, project_id, name, description, icon, color, created_at, updated_at",
        )
        .bind(&id)
        .bind(&input.project_id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&input.icon)
        .bind(&input.color)
        .fetch_one(&self.pool)
        .await?;

        let result = row_to_custom_entry_type(&row)?;
        Ok(result)
    }

    async fn create_entry_types_bulk(
        &self,
        inputs: Vec<CreateCustomEntryType>,
    ) -> Result<Vec<CustomEntryType>> {
        let mut tx = self.pool.begin().await?;
        let mut entry_types = Vec::with_capacity(inputs.len());

        for input in inputs {
            let id = Uuid::now_v7();
            let row = sqlx::query(
                "INSERT INTO entry_types (id, project_id, name, description, icon, color)
                 VALUES (?, ?, ?, ?, ?, ?)
                 RETURNING id, project_id, name, description, icon, color, created_at, updated_at",
            )
            .bind(id)
            .bind(input.project_id)
            .bind(input.name)
            .bind(input.description)
            .bind(input.icon)
            .bind(input.color)
            .fetch_one(&mut *tx)
            .await?;
            entry_types.push(row_to_custom_entry_type(&row)?);
        }

        tx.commit().await?;
        Ok(entry_types)
    }

    async fn get_entry_type(&self, id: &Uuid) -> Result<CustomEntryType> {
        let row = sqlx::query(
            "SELECT id, project_id, name, description, icon, color, created_at, updated_at
             FROM entry_types
             WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| WorldflowError::NotFound(format!("entry_type {id}")))?;

        row_to_custom_entry_type(&row)
    }

    async fn list_all_entry_types(&self, project_id: &Uuid) -> Result<Vec<EntryTypeView>> {
        use crate::models::BUILTIN_ENTRY_TYPES;

        // 先添加所有内置类型
        let mut types: Vec<EntryTypeView> = BUILTIN_ENTRY_TYPES
            .iter()
            .map(|bt| EntryTypeView::from(bt))
            .collect();

        // 查询自定义类型
        let custom_rows = sqlx::query(
            "SELECT id, project_id, name, description, icon, color, created_at, updated_at
             FROM entry_types
             WHERE project_id = ?
             ORDER BY name",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;

        // 添加自定义类型
        for row in custom_rows {
            let custom = row_to_custom_entry_type(&row)?;
            types.push(EntryTypeView::from(custom));
        }

        Ok(types)
    }

    async fn list_custom_entry_types(&self, project_id: &Uuid) -> Result<Vec<CustomEntryType>> {
        let rows = sqlx::query(
            "SELECT id, project_id, name, description, icon, color, created_at, updated_at
             FROM entry_types
             WHERE project_id = ?
             ORDER BY name",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_custom_entry_type).collect()
    }

    async fn update_entry_type(
        &self,
        id: &Uuid,
        input: UpdateCustomEntryType,
    ) -> Result<CustomEntryType> {
        // 先验证该类型是否存在
        self.get_entry_type(id).await?;

        let row = sqlx::query(
            "UPDATE entry_types
             SET name        = COALESCE(?, name),
                 description = CASE WHEN ? THEN ? ELSE description END,
                 icon        = CASE WHEN ? THEN ? ELSE icon END,
                 color       = CASE WHEN ? THEN ? ELSE color END
             WHERE id = ?
             RETURNING id, project_id, name, description, icon, color, created_at, updated_at",
        )
        .bind(&input.name)
        .bind(input.description.is_some())
        .bind(input.description.flatten())
        .bind(input.icon.is_some())
        .bind(input.icon.flatten())
        .bind(input.color.is_some())
        .bind(input.color.flatten())
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        let result = row_to_custom_entry_type(&row)?;
        Ok(result)
    }

    async fn delete_entry_type(&self, id: &Uuid) -> Result<()> {
        // 先验证该类型是否存在
        let custom_type = self.get_entry_type(id).await?;

        // 检查是否有 entries 在使用该 type
        if self
            .check_entry_type_in_use(&custom_type.project_id, id)
            .await?
        {
            return Err(WorldflowError::InvalidInput(
                "Cannot delete entry type that is in use".to_string(),
            ));
        }

        let result = sqlx::query("DELETE FROM entry_types WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(WorldflowError::NotFound(format!("entry_type {id}")));
        }

        Ok(())
    }

    async fn check_entry_type_in_use(&self, _project_id: &Uuid, type_id: &Uuid) -> Result<bool> {
        let row = sqlx::query("SELECT COUNT(*) as cnt FROM entries WHERE type = ? LIMIT 1")
            .bind(type_id.to_string())
            .fetch_one(&self.pool)
            .await?;

        let cnt: i64 = row.try_get("cnt")?;
        Ok(cnt > 0)
    }
}
