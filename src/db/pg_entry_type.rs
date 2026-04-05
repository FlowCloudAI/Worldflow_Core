use sqlx::Row;
use uuid::Uuid;

use crate::{
    db::PgDb,
    error::{Result, WorldflowError},
    models::{CustomEntryType, CreateCustomEntryType, UpdateCustomEntryType, EntryTypeView},
};

use super::traits::EntryTypeOps;

/// 将数据库行转换为 CustomEntryType
fn row_to_custom_entry_type(row: &sqlx::postgres::PgRow) -> Result<CustomEntryType> {
    Ok(CustomEntryType {
        id: row.try_get("id")?,
        project_id: row.try_get("project_id")?,
        name: row.try_get("name")?,
        description: row.try_get("description")?,
        icon: row.try_get("icon")?,
        color: row.try_get("color")?,
        created_at: row.try_get::<String, _>("created_at")?
            .replace("T", " ").split('+').next().unwrap_or("").to_string(),
        updated_at: row.try_get::<String, _>("updated_at")?
            .replace("T", " ").split('+').next().unwrap_or("").to_string(),
    })
}

impl EntryTypeOps for PgDb {
    async fn create_entry_type(&self, input: CreateCustomEntryType) -> Result<CustomEntryType> {
        let id = Uuid::new_v4().to_string();

        let row = sqlx::query(
            "INSERT INTO entry_types (id, project_id, name, description, icon, color)
             VALUES ($1, $2, $3, $4, $5, $6)
             RETURNING id, project_id, name, description, icon, color, created_at::TEXT, updated_at::TEXT",
        )
        .bind(&id)
        .bind(&input.project_id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&input.icon)
        .bind(&input.color)
        .fetch_one(&self.pool)
        .await?;

        row_to_custom_entry_type(&row)
    }

    async fn get_entry_type(&self, id: &str) -> Result<CustomEntryType> {
        let row = sqlx::query(
            "SELECT id, project_id, name, description, icon, color, created_at::TEXT, updated_at::TEXT
             FROM entry_types
             WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| WorldflowError::NotFound(format!("entry_type {id}")))?;

        row_to_custom_entry_type(&row)
    }

    async fn list_all_entry_types(&self, project_id: &str) -> Result<Vec<EntryTypeView>> {
        use crate::models::BUILTIN_ENTRY_TYPES;

        // 先添加所有内置类型
        let mut types: Vec<EntryTypeView> = BUILTIN_ENTRY_TYPES
            .iter()
            .map(|bt| EntryTypeView::from(bt))
            .collect();

        // 查询自定义类型
        let custom_rows = sqlx::query(
            "SELECT id, project_id, name, description, icon, color, created_at::TEXT, updated_at::TEXT
             FROM entry_types
             WHERE project_id = $1
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

    async fn list_custom_entry_types(&self, project_id: &str) -> Result<Vec<CustomEntryType>> {
        let rows = sqlx::query(
            "SELECT id, project_id, name, description, icon, color, created_at::TEXT, updated_at::TEXT
             FROM entry_types
             WHERE project_id = $1
             ORDER BY name",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_custom_entry_type).collect()
    }

    async fn update_entry_type(&self, id: &str, input: UpdateCustomEntryType) -> Result<CustomEntryType> {
        // 先验证该类型是否存在
        self.get_entry_type(id).await?;

        let mut p = 2usize;
        let mut sql = "UPDATE entry_types SET ".to_string();
        let mut set_clauses = vec![];

        if input.name.is_some() {
            set_clauses.push(format!("name = ${}", p));
            p += 1;
        }

        if input.description.is_some() {
            set_clauses.push(format!("description = ${}", p));
            p += 1;
        }

        if input.icon.is_some() {
            set_clauses.push(format!("icon = ${}", p));
            p += 1;
        }

        if input.color.is_some() {
            set_clauses.push(format!("color = ${}", p));
            p += 1;
        }

        if set_clauses.is_empty() {
            // 如果没有任何更新，直接返回原对象
            return self.get_entry_type(id).await;
        }

        sql.push_str(&set_clauses.join(", "));
        sql.push_str(&format!(" WHERE id = ${} RETURNING id, project_id, name, description, icon, color, created_at::TEXT, updated_at::TEXT", p));

        let mut query = sqlx::query(&sql);
        query = query.bind(&input.name);
        query = query.bind(input.description.flatten());
        query = query.bind(&input.icon);
        query = query.bind(input.color.flatten());
        query = query.bind(id);

        let row = query.fetch_one(&self.pool).await?;

        row_to_custom_entry_type(&row)
    }

    async fn delete_entry_type(&self, id: &str) -> Result<()> {
        // 先验证该类型是否存在
        let custom_type = self.get_entry_type(id).await?;

        // 检查是否有 entries 在使用该 type
        if self.check_entry_type_in_use(&custom_type.project_id, id).await? {
            return Err(WorldflowError::InvalidInput(
                "Cannot delete entry type that is in use".to_string(),
            ));
        }

        let result = sqlx::query("DELETE FROM entry_types WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(WorldflowError::NotFound(format!("entry_type {id}")));
        }

        Ok(())
    }

    async fn check_entry_type_in_use(&self, _project_id: &str, type_id: &str) -> Result<bool> {
        let row = sqlx::query("SELECT COUNT(*) as cnt FROM entries WHERE type = $1 LIMIT 1")
            .bind(type_id)
            .fetch_one(&self.pool)
            .await?;

        let cnt: i64 = row.try_get("cnt")?;
        Ok(cnt > 0)
    }
}
