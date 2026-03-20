use sqlx::Row;
use crate::{
    db::SqliteDb,
    error::Result,
    models::AppSetting,
};

fn row_to_app_setting(row: &sqlx::sqlite::SqliteRow) -> Result<AppSetting> {
    Ok(AppSetting {
        key:        row.try_get("key")?,
        value:      row.try_get("value")?,
        updated_at: row.try_get("updated_at")?,
    })
}

impl SqliteDb {
    pub async fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let row = sqlx::query("SELECT value FROM app_settings WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| r.try_get("value")).transpose()?)
    }

    pub async fn set_setting(&self, key: &str, value: &str) -> Result<AppSetting> {
        let row = sqlx::query(
            "INSERT INTO app_settings (key, value)
             VALUES (?, ?)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value
             RETURNING key, value, updated_at"
        )
            .bind(key)
            .bind(value)
            .fetch_one(&self.pool)
            .await?;

        row_to_app_setting(&row)
    }

    pub async fn delete_setting(&self, key: &str) -> Result<()> {
        sqlx::query("DELETE FROM app_settings WHERE key = ?")
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}