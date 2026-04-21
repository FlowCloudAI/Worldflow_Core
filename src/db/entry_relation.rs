use super::traits::EntryRelationOps;
use crate::{
    db::SqliteDb,
    error::{Result, WorldflowError},
    models::{CreateEntryRelation, EntryRelation, RelationDirection, UpdateEntryRelation},
};
use sqlx::Row;
use uuid::Uuid;

fn row_to_relation(row: &sqlx::sqlite::SqliteRow) -> Result<EntryRelation> {
    let relation_str: String = row.try_get("relation")?;
    let relation = RelationDirection::from_str(&relation_str)
        .ok_or_else(|| WorldflowError::InvalidInput(format!("未知的关系类型: {relation_str}")))?;

    Ok(EntryRelation {
        id: row.try_get("id")?,
        project_id: row.try_get("project_id")?,
        a_id: row.try_get("a_id")?,
        b_id: row.try_get("b_id")?,
        relation,
        content: row.try_get("content")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

impl EntryRelationOps for SqliteDb {
    async fn create_relation(&self, input: CreateEntryRelation) -> Result<EntryRelation> {
        let id = Uuid::now_v7();

        // two_way 关系规范化：强制 a_id < b_id，消除重复
        let (a_id, b_id) = if input.relation == RelationDirection::TwoWay && input.a_id > input.b_id
        {
            (&input.b_id, &input.a_id)
        } else {
            (&input.a_id, &input.b_id)
        };

        let row = sqlx::query(
            "INSERT INTO entry_relations (id, project_id, a_id, b_id, relation, content)
             VALUES (?, ?, ?, ?, ?, ?)
             RETURNING id, project_id, a_id, b_id, relation, content, created_at, updated_at",
        )
        .bind(&id)
        .bind(&input.project_id)
        .bind(a_id)
        .bind(b_id)
        .bind(input.relation.as_str())
        .bind(&input.content)
        .fetch_one(&self.pool)
        .await?;

        let result = row_to_relation(&row)?;
        Ok(result)
    }

    async fn create_relations_bulk(
        &self,
        inputs: Vec<CreateEntryRelation>,
    ) -> Result<Vec<EntryRelation>> {
        let mut tx = self.pool.begin().await?;
        let mut relations = Vec::with_capacity(inputs.len());

        for input in inputs {
            let id = Uuid::now_v7();
            let (a_id, b_id) =
                if input.relation == RelationDirection::TwoWay && input.a_id > input.b_id {
                    (input.b_id, input.a_id)
                } else {
                    (input.a_id, input.b_id)
                };

            let row = sqlx::query(
                "INSERT INTO entry_relations (id, project_id, a_id, b_id, relation, content)
                 VALUES (?, ?, ?, ?, ?, ?)
                 RETURNING id, project_id, a_id, b_id, relation, content, created_at, updated_at",
            )
            .bind(id)
            .bind(input.project_id)
            .bind(a_id)
            .bind(b_id)
            .bind(input.relation.as_str())
            .bind(input.content)
            .fetch_one(&mut *tx)
            .await?;
            relations.push(row_to_relation(&row)?);
        }

        tx.commit().await?;
        Ok(relations)
    }

    async fn get_relation(&self, id: &Uuid) -> Result<EntryRelation> {
        let row = sqlx::query(
            "SELECT id, project_id, a_id, b_id, relation, content, created_at, updated_at
             FROM entry_relations WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| WorldflowError::NotFound(format!("relation {id}")))?;

        row_to_relation(&row)
    }

    async fn list_relations_for_entry(&self, entry_id: &Uuid) -> Result<Vec<EntryRelation>> {
        let rows = sqlx::query(
            "SELECT id, project_id, a_id, b_id, relation, content, created_at, updated_at
             FROM entry_relations
             WHERE a_id = ?
                OR (b_id = ? AND relation = 'two_way')
             ORDER BY created_at ",
        )
        .bind(entry_id)
        .bind(entry_id)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_relation).collect()
    }

    async fn list_relations_for_project(&self, project_id: &Uuid) -> Result<Vec<EntryRelation>> {
        let rows = sqlx::query(
            "SELECT id, project_id, a_id, b_id, relation, content, created_at, updated_at
             FROM entry_relations
             WHERE project_id = ?
             ORDER BY created_at ",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_relation).collect()
    }

    async fn update_relation(
        &self,
        id: &Uuid,
        input: UpdateEntryRelation,
    ) -> Result<EntryRelation> {
        let existing = self.get_relation(id).await?;

        // 如果变更为 two_way，需要规范化 a_id < b_id
        let new_relation = input.relation.as_ref().unwrap_or(&existing.relation);
        let needs_swap =
            *new_relation == RelationDirection::TwoWay && existing.a_id > existing.b_id;

        let sql = if needs_swap {
            "UPDATE entry_relations
             SET relation = COALESCE(?, relation),
                 content  = COALESCE(?, content),
                 a_id = b_id, b_id = a_id
             WHERE id = ?
             RETURNING id, project_id, a_id, b_id, relation, content, created_at, updated_at"
        } else {
            "UPDATE entry_relations
             SET relation = COALESCE(?, relation),
                 content  = COALESCE(?, content)
             WHERE id = ?
             RETURNING id, project_id, a_id, b_id, relation, content, created_at, updated_at"
        };

        let row = sqlx::query(sql)
            .bind(input.relation.as_ref().map(|r| r.as_str()))
            .bind(input.content.as_deref())
            .bind(id)
            .fetch_one(&self.pool)
            .await?;

        let result = row_to_relation(&row)?;
        Ok(result)
    }

    async fn delete_relation(&self, id: &Uuid) -> Result<()> {
        let result = sqlx::query("DELETE FROM entry_relations WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(WorldflowError::NotFound(format!("relation {id}")));
        }
        Ok(())
    }

    async fn delete_relations_between(&self, entry_a: &Uuid, entry_b: &Uuid) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM entry_relations
             WHERE (a_id = ? AND b_id = ?)
                OR (a_id = ? AND b_id = ?)",
        )
        .bind(entry_a)
        .bind(entry_b)
        .bind(entry_b)
        .bind(entry_a)
        .execute(&self.pool)
        .await?;

        let affected = result.rows_affected();
        Ok(affected)
    }
}
