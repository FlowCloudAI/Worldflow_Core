use super::traits::CategoryOps;
use crate::{
    db::SqliteDb,
    error::{Result, WorldflowError},
    models::{
        CascadeDeleteCategoryResult, Category, CreateCategory, DeleteCategoryMoveToParentResult,
        UpdateCategory,
    },
};
use sqlx::{Row, Sqlite, Transaction};
use uuid::Uuid;

fn row_to_category(row: &sqlx::sqlite::SqliteRow) -> Result<Category> {
    Ok(Category {
        id: row.try_get("id")?,
        project_id: row.try_get("project_id")?,
        parent_id: row.try_get("parent_id")?,
        name: row.try_get("name")?,
        sort_order: row.try_get("sort_order")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

async fn collect_subtree_ids_in_tx(
    tx: &mut Transaction<'_, Sqlite>,
    root_id: &Uuid,
) -> Result<Vec<(Uuid, i64)>> {
    let rows = sqlx::query(
        "WITH RECURSIVE subtree(id, depth) AS (
             SELECT id, 0 FROM categories WHERE id = ?
             UNION ALL
             SELECT c.id, subtree.depth + 1
             FROM categories c
             JOIN subtree ON c.parent_id = subtree.id
         )
         SELECT id, depth FROM subtree",
    )
    .bind(root_id)
    .fetch_all(&mut **tx)
    .await?;

    rows.into_iter()
        .map(|row| Ok((row.try_get("id")?, row.try_get("depth")?)))
        .collect()
}

impl CategoryOps for SqliteDb {
    async fn would_create_cycle(&self, id: &Uuid, new_parent_id: &Uuid) -> Result<bool> {
        let row = sqlx::query(
            "WITH RECURSIVE descendants(id) AS (
             SELECT id FROM categories WHERE id = ?
             UNION ALL
             SELECT c.id FROM categories c
             JOIN descendants d ON c.parent_id = d.id
         )
         SELECT COUNT(*) as cnt FROM descendants WHERE id = ?",
        )
        .bind(id)
        .bind(new_parent_id)
        .fetch_one(&self.pool)
        .await?;

        let cnt: i64 = row.try_get("cnt")?;
        Ok(cnt > 0)
    }

    async fn create_category(&self, input: CreateCategory) -> Result<Category> {
        let id = Uuid::now_v7();
        let sort_order = input.sort_order.unwrap_or(0);
        let row = sqlx::query(
            "INSERT INTO categories (id, project_id, parent_id, name, sort_order)
             VALUES (?, ?, ?, ?, ?)
             RETURNING id, project_id, parent_id, name, sort_order, created_at, updated_at",
        )
        .bind(&id)
        .bind(&input.project_id)
        .bind(&input.parent_id)
        .bind(&input.name)
        .bind(sort_order)
        .fetch_one(&self.pool)
        .await?;
        let result = row_to_category(&row)?;
        Ok(result)
    }

    async fn create_categories_bulk(&self, inputs: Vec<CreateCategory>) -> Result<Vec<Category>> {
        let mut tx = self.pool.begin().await?;
        let mut categories = Vec::with_capacity(inputs.len());

        for input in inputs {
            let id = Uuid::now_v7();
            let sort_order = input.sort_order.unwrap_or(0);
            let row = sqlx::query(
                "INSERT INTO categories (id, project_id, parent_id, name, sort_order)
                 VALUES (?, ?, ?, ?, ?)
                 RETURNING id, project_id, parent_id, name, sort_order, created_at, updated_at",
            )
            .bind(id)
            .bind(input.project_id)
            .bind(input.parent_id)
            .bind(input.name)
            .bind(sort_order)
            .fetch_one(&mut *tx)
            .await?;
            categories.push(row_to_category(&row)?);
        }

        tx.commit().await?;
        Ok(categories)
    }

    async fn get_category(&self, id: &Uuid) -> Result<Category> {
        let row = sqlx::query(
            "SELECT id, project_id, parent_id, name, sort_order, created_at, updated_at
             FROM categories WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| WorldflowError::NotFound(format!("category {id}")))?;
        row_to_category(&row)
    }

    async fn list_categories(&self, project_id: &Uuid) -> Result<Vec<Category>> {
        let rows = sqlx::query(
            "SELECT id, project_id, parent_id, name, sort_order, created_at, updated_at
             FROM categories
             WHERE project_id = ?
             ORDER BY parent_id NULLS FIRST, sort_order , name ",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_category).collect()
    }

    async fn update_category(&self, id: &Uuid, input: UpdateCategory) -> Result<Category> {
        if let Some(Some(ref new_parent)) = input.parent_id {
            if self.would_create_cycle(id, new_parent).await? {
                return Err(WorldflowError::InvalidInput(
                    "不能将分类移动到自己的子孙节点下".to_string(),
                ));
            }
        }
        self.get_category(id).await?;
        let row = match input.parent_id {
            None => sqlx::query(
                "UPDATE categories
                 SET name       = COALESCE(?, name),
                     sort_order = COALESCE(?, sort_order)
                 WHERE id = ?
                 RETURNING id, project_id, parent_id, name, sort_order, created_at, updated_at",
            )
            .bind(&input.name)
            .bind(input.sort_order)
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| super::map_row_not_found(e, format!("category {id}")))?,

            Some(new_parent) => sqlx::query(
                "UPDATE categories
                 SET parent_id  = ?,
                     name       = COALESCE(?, name),
                     sort_order = COALESCE(?, sort_order)
                 WHERE id = ?
                 RETURNING id, project_id, parent_id, name, sort_order, created_at, updated_at",
            )
            .bind(new_parent)
            .bind(&input.name)
            .bind(input.sort_order)
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| super::map_row_not_found(e, format!("category {id}")))?,
        };
        let result = row_to_category(&row)?;
        Ok(result)
    }

    async fn delete_category(&self, id: &Uuid) -> Result<()> {
        let result = sqlx::query("DELETE FROM categories WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(WorldflowError::NotFound(format!("category {id}")));
        }
        Ok(())
    }

    async fn cascade_delete_category(&self, id: &Uuid) -> Result<CascadeDeleteCategoryResult> {
        let mut tx = self.pool.begin().await?;

        let root_row = sqlx::query(
            "SELECT id, project_id, parent_id, name, sort_order, created_at, updated_at
             FROM categories WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| WorldflowError::NotFound(format!("category {id}")))?;
        let root_category = row_to_category(&root_row)?;

        let mut subtree_ids = collect_subtree_ids_in_tx(&mut tx, id).await?;
        subtree_ids.sort_by(|left, right| right.1.cmp(&left.1));

        let mut deleted_entries = 0usize;
        for (category_id, _) in &subtree_ids {
            let result =
                sqlx::query("DELETE FROM entries WHERE project_id = ? AND category_id = ?")
                    .bind(root_category.project_id)
                    .bind(category_id)
                    .execute(&mut *tx)
                    .await?;
            deleted_entries += result.rows_affected() as usize;
        }

        let mut deleted_categories = 0usize;
        for (category_id, _) in &subtree_ids {
            let result = sqlx::query("DELETE FROM categories WHERE id = ?")
                .bind(category_id)
                .execute(&mut *tx)
                .await?;
            deleted_categories += result.rows_affected() as usize;
        }

        sqlx::query("UPDATE projects SET name = name WHERE id = ?")
            .bind(root_category.project_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        Ok(CascadeDeleteCategoryResult {
            deleted_entries,
            deleted_categories,
        })
    }

    async fn delete_category_move_to_parent(
        &self,
        id: &Uuid,
    ) -> Result<DeleteCategoryMoveToParentResult> {
        let mut tx = self.pool.begin().await?;

        let category_row = sqlx::query(
            "SELECT id, project_id, parent_id, name, sort_order, created_at, updated_at
             FROM categories WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| WorldflowError::NotFound(format!("category {id}")))?;
        let category = row_to_category(&category_row)?;
        let new_parent = category.parent_id;

        let moved_categories = sqlx::query(
            "UPDATE categories
             SET parent_id = ?
             WHERE project_id = ? AND parent_id = ?",
        )
        .bind(new_parent)
        .bind(category.project_id)
        .bind(id)
        .execute(&mut *tx)
        .await?
        .rows_affected() as usize;

        let moved_entries = sqlx::query(
            "UPDATE entries
             SET category_id = ?
             WHERE project_id = ? AND category_id = ?",
        )
        .bind(new_parent)
        .bind(category.project_id)
        .bind(id)
        .execute(&mut *tx)
        .await?
        .rows_affected() as usize;

        let delete_result = sqlx::query("DELETE FROM categories WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        if delete_result.rows_affected() == 0 {
            return Err(WorldflowError::NotFound(format!("category {id}")));
        }

        sqlx::query("UPDATE projects SET name = name WHERE id = ?")
            .bind(category.project_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        Ok(DeleteCategoryMoveToParentResult {
            moved_categories,
            moved_entries,
        })
    }
}
