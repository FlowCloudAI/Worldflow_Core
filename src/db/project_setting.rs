use super::traits::ProjectSettingOps;
use crate::{db::SqliteDb, error::Result};
use sqlx::Row;
use uuid::Uuid;

impl ProjectSettingOps for SqliteDb {
    async fn get_project_setting(&self, project_id: &Uuid, key: &str) -> Result<Option<String>> {
        let value = sqlx::query("SELECT value FROM project_settings WHERE project_id = ? AND key = ?")
            .bind(project_id)
            .bind(key)
            .fetch_optional(&self.pool)
            .await?
            .map(|row| row.try_get::<String, _>("value"))
            .transpose()?;

        Ok(value)
    }

    async fn set_project_setting(&self, project_id: &Uuid, key: &str, value: &str) -> Result<()> {
        // 存在即覆盖：以 (project_id, key) 主键做 upsert。
        sqlx::query(
            "INSERT INTO project_settings (project_id, key, value)
             VALUES (?, ?, ?)
             ON CONFLICT (project_id, key)
             DO UPDATE SET value = excluded.value",
        )
        .bind(project_id)
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;
    use crate::db::traits::ProjectOps;
    use crate::models::CreateProject;
    use tempfile::TempDir;

    async fn new_test_db(prefix: &str) -> Result<(TempDir, SqliteDb)> {
        let temp = tempfile::tempdir().expect("创建临时目录失败");
        let db_path = temp.path().join(format!("{prefix}.db"));
        let database_url = format!(
            "sqlite:{}?mode=rwc",
            db_path.to_string_lossy().replace('\\', "/")
        );
        let db = SqliteDb::new(&database_url).await?;
        Ok((temp, db))
    }

    async fn seed_project(db: &SqliteDb) -> Result<Uuid> {
        Ok(db
            .create_project(CreateProject {
                name: "测试项目".to_owned(),
                description: None,
                cover_image: None,
            })
            .await?
            .id)
    }

    #[tokio::test]
    async fn set_then_get_roundtrips_and_overwrites() {
        let (_temp, db) = new_test_db("project_setting_roundtrip")
            .await
            .expect("初始化数据库失败");
        let project_id = seed_project(&db).await.expect("创建项目失败");

        // 不存在 → None
        assert_eq!(
            db.get_project_setting(&project_id, "relation_graph_layout")
                .await
                .expect("读取失败"),
            None,
        );

        // 写入 → 读回
        db.set_project_setting(&project_id, "relation_graph_layout", "{\"looseness\":1.2}")
            .await
            .expect("写入失败");
        assert_eq!(
            db.get_project_setting(&project_id, "relation_graph_layout")
                .await
                .expect("读取失败"),
            Some("{\"looseness\":1.2}".to_owned()),
        );

        // 覆盖写入
        db.set_project_setting(&project_id, "relation_graph_layout", "{\"looseness\":0.8}")
            .await
            .expect("覆盖失败");
        assert_eq!(
            db.get_project_setting(&project_id, "relation_graph_layout")
                .await
                .expect("读取失败"),
            Some("{\"looseness\":0.8}".to_owned()),
        );
    }
}
