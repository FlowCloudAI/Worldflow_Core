use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::Row;
use worldflow_core::{ProjectOps, SqliteDb, models::CreateProject};

#[tokio::test]
async fn sqlite_uuid_is_stored_as_blob16() {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let db_path = std::env::temp_dir().join(format!("worldflow_uuid_{millis}.db"));
    let db_url = format!("sqlite:{}?mode=rwc", db_path.to_string_lossy().replace('\\', "/"));

    let db = SqliteDb::new(&db_url).await.unwrap();
    let project = db.create_project(CreateProject {
        name: "UUID 存储测试".to_string(),
        description: None,
        cover_image: None,
    }).await.unwrap();

    let row = sqlx::query("SELECT typeof(id) AS id_type, length(id) AS id_len FROM projects WHERE id = ?")
        .bind(project.id)
        .fetch_one(&db.pool)
        .await
        .unwrap();

    let id_type: String = row.try_get("id_type").unwrap();
    let id_len: i64 = row.try_get("id_len").unwrap();

    assert_eq!(id_type, "blob");
    assert_eq!(id_len, 16);

    let _ = std::fs::remove_file(db_path);
}
