use sqlx::Row;
use uuid::Uuid;
use crate::{
    db::SqliteDb,
    error::{Result, WorldflowError},
    models::{CreateEntryRelation, EntryRelation, RelationDirection, UpdateEntryRelation},
};
use super::traits::EntryRelationOps;

fn row_to_relation(row: &sqlx::sqlite::SqliteRow) -> Result<EntryRelation> {
    let relation_str: String = row.try_get("relation")?;
    let relation = RelationDirection::from_str(&relation_str)
        .ok_or_else(|| WorldflowError::InvalidInput(
            format!("未知的关系类型: {relation_str}")
        ))?;

    Ok(EntryRelation {
        id:         row.try_get("id")?,
        project_id: row.try_get("project_id")?,
        a_id:       row.try_get("a_id")?,
        b_id:       row.try_get("b_id")?,
        relation,
        content:    row.try_get("content")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

impl EntryRelationOps for SqliteDb {
    async fn create_relation(&self, input: CreateEntryRelation) -> Result<EntryRelation> {
        let id = Uuid::new_v4().to_string();

        let row = sqlx::query(
            "INSERT INTO entry_relations (id, project_id, a_id, b_id, relation, content)
             VALUES (?, ?, ?, ?, ?, ?)
             RETURNING id, project_id, a_id, b_id, relation, content, created_at, updated_at"
        )
            .bind(&id)
            .bind(&input.project_id)
            .bind(&input.a_id)
            .bind(&input.b_id)
            .bind(input.relation.as_str())
            .bind(&input.content)
            .fetch_one(&self.pool)
            .await?;

        row_to_relation(&row)
    }

    async fn get_relation(&self, id: &str) -> Result<EntryRelation> {
        let row = sqlx::query(
            "SELECT id, project_id, a_id, b_id, relation, content, created_at, updated_at
             FROM entry_relations WHERE id = ?"
        )
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| WorldflowError::NotFound(format!("relation {id}")))?;

        row_to_relation(&row)
    }

    async fn list_relations_for_entry(
        &self,
        entry_id: &str,
    ) -> Result<Vec<EntryRelation>> {
        let rows = sqlx::query(
            "SELECT id, project_id, a_id, b_id, relation, content, created_at, updated_at
             FROM entry_relations
             WHERE a_id = ?
                OR (b_id = ? AND relation = 'two_way')
             ORDER BY created_at "
        )
            .bind(entry_id)
            .bind(entry_id)
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(row_to_relation).collect()
    }

    async fn list_relations_for_project(
        &self,
        project_id: &str,
    ) -> Result<Vec<EntryRelation>> {
        let rows = sqlx::query(
            "SELECT id, project_id, a_id, b_id, relation, content, created_at, updated_at
             FROM entry_relations
             WHERE project_id = ?
             ORDER BY created_at "
        )
            .bind(project_id)
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(row_to_relation).collect()
    }

    async fn update_relation(
        &self,
        id: &str,
        input: UpdateEntryRelation,
    ) -> Result<EntryRelation> {
        self.get_relation(id).await?;

        let row = sqlx::query(
            "UPDATE entry_relations
             SET relation = COALESCE(?, relation),
                 content  = COALESCE(?, content)
             WHERE id = ?
             RETURNING id, project_id, a_id, b_id, relation, content, created_at, updated_at"
        )
            .bind(input.relation.as_ref().map(|r| r.as_str()))
            .bind(input.content.as_deref())
            .bind(id)
            .fetch_one(&self.pool)
            .await?;

        row_to_relation(&row)
    }

    async fn delete_relation(&self, id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM entry_relations WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(WorldflowError::NotFound(format!("relation {id}")));
        }
        Ok(())
    }

    async fn delete_relations_between(
        &self,
        entry_a: &str,
        entry_b: &str,
    ) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM entry_relations
             WHERE (a_id = ? AND b_id = ?)
                OR (a_id = ? AND b_id = ?)"
        )
            .bind(entry_a)
            .bind(entry_b)
            .bind(entry_b)
            .bind(entry_a)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }
}
