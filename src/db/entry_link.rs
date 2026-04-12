use super::traits::EntryLinkOps;
use crate::{
    db::SqliteDb,
    error::Result,
    models::{CreateEntryLink, EntryLink},
};
use sqlx::Row;
use uuid::Uuid;

fn row_to_link(row: &sqlx::sqlite::SqliteRow) -> Result<EntryLink> {
    Ok(EntryLink {
        id: row.try_get("id")?,
        project_id: row.try_get("project_id")?,
        a_id: row.try_get("a_id")?,
        b_id: row.try_get("b_id")?,
    })
}

fn normalize_linked_entry_ids(entry_id: &Uuid, linked_entry_ids: &[Uuid]) -> Vec<Uuid> {
    let mut unique_ids = Vec::new();
    for linked_id in linked_entry_ids {
        if linked_id == entry_id {
            continue;
        }
        if !unique_ids.contains(linked_id) {
            unique_ids.push(*linked_id);
        }
    }
    unique_ids
}

impl EntryLinkOps for SqliteDb {
    async fn create_link(&self, input: CreateEntryLink) -> Result<EntryLink> {
        let id = Uuid::now_v7();
        let row = sqlx::query(
            "INSERT INTO entry_links (id, project_id, a_id, b_id)
             VALUES (?, ?, ?, ?)
             RETURNING id, project_id, a_id, b_id",
        )
            .bind(id)
            .bind(input.project_id)
            .bind(input.a_id)
            .bind(input.b_id)
            .fetch_one(&self.pool)
            .await?;

        row_to_link(&row)
    }

    async fn list_outgoing_links(&self, entry_id: &Uuid) -> Result<Vec<EntryLink>> {
        let rows = sqlx::query(
            "SELECT id, project_id, a_id, b_id
             FROM entry_links
             WHERE a_id = ?
             ORDER BY rowid",
        )
            .bind(entry_id)
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(row_to_link).collect()
    }

    async fn list_incoming_links(&self, entry_id: &Uuid) -> Result<Vec<EntryLink>> {
        let rows = sqlx::query(
            "SELECT id, project_id, a_id, b_id
             FROM entry_links
             WHERE b_id = ?
             ORDER BY rowid",
        )
            .bind(entry_id)
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(row_to_link).collect()
    }

    async fn delete_links_from_entry(&self, entry_id: &Uuid) -> Result<u64> {
        let result = sqlx::query("DELETE FROM entry_links WHERE a_id = ?")
            .bind(entry_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    async fn replace_outgoing_links(
        &self,
        project_id: &Uuid,
        entry_id: &Uuid,
        linked_entry_ids: &[Uuid],
    ) -> Result<Vec<EntryLink>> {
        let normalized_ids = normalize_linked_entry_ids(entry_id, linked_entry_ids);
        let mut tx = self.pool.begin().await?;

        sqlx::query("DELETE FROM entry_links WHERE a_id = ?")
            .bind(entry_id)
            .execute(&mut *tx)
            .await?;

        let mut links = Vec::with_capacity(normalized_ids.len());
        for linked_id in normalized_ids {
            let row = sqlx::query(
                "INSERT INTO entry_links (id, project_id, a_id, b_id)
                 VALUES (?, ?, ?, ?)
                 RETURNING id, project_id, a_id, b_id",
            )
                .bind(Uuid::now_v7())
                .bind(project_id)
                .bind(entry_id)
                .bind(linked_id)
                .fetch_one(&mut *tx)
                .await?;
            links.push(row_to_link(&row)?);
        }

        tx.commit().await?;
        Ok(links)
    }
}
