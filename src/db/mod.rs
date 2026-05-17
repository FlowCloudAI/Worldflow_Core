#[cfg(feature = "sqlite")]
pub mod api_usage;
pub mod traits;

#[cfg(feature = "sqlite")]
pub mod sqlite;

#[cfg(feature = "postgres")]
pub mod postgres;

// ── SQLite 实现 ──────────────────────────────────────────
#[cfg(feature = "sqlite")]
mod category;
#[cfg(feature = "sqlite")]
mod entry;
#[cfg(feature = "sqlite")]
mod entry_link;
#[cfg(feature = "sqlite")]
mod entry_relation;
#[cfg(feature = "sqlite")]
mod entry_type;
#[cfg(feature = "sqlite")]
mod idea_note;
#[cfg(feature = "sqlite")]
mod project;
#[cfg(feature = "sqlite")]
#[cfg(feature = "snapshot")]
pub(crate) mod snapshot;
#[cfg(feature = "sqlite")]
mod tag_schema;

use crate::error::{Result, WorldflowError};
#[cfg(feature = "sqlite")]
use std::sync::OnceLock;
#[cfg(feature = "sqlite")]
use sysinfo::System;

#[cfg(feature = "sqlite")]
use sha2::{Digest, Sha384};
#[cfg(feature = "sqlite")]
use sqlx::{SqlitePool, migrate::Migrator, sqlite::SqlitePoolOptions};

#[cfg(all(feature = "sqlite", feature = "snapshot"))]
use std::sync::Arc;

#[cfg(all(feature = "sqlite", feature = "snapshot"))]
use snapshot::{SnapshotConfig, SnapshotState};

/// 进程启动时探测一次可用内存，后续所有连接复用同一档位。
/// 避免测试或高负载场景下重复调用时因内存波动导致每次连接拿到不同的 cache 配置。
#[cfg(feature = "sqlite")]
static AVAILABLE_MEMORY_MB: OnceLock<u64> = OnceLock::new();

#[cfg(feature = "sqlite")]
static SQLITE_MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub(in crate::db) fn checked_limit(limit: usize) -> Result<i64> {
    i64::try_from(limit).map_err(|_| WorldflowError::InvalidInput("limit 过大".to_owned()))
}

pub(in crate::db) fn checked_pagination(limit: usize, offset: usize) -> Result<(i64, i64)> {
    let limit = checked_limit(limit)?;
    let offset = i64::try_from(offset)
        .map_err(|_| WorldflowError::InvalidInput("offset 过大".to_owned()))?;
    Ok((limit, offset))
}

pub(in crate::db) fn checked_scaled_limit(
    limit: usize,
    multiplier: usize,
    min: i64,
    max: i64,
) -> Result<i64> {
    let scaled = limit
        .checked_mul(multiplier)
        .ok_or_else(|| WorldflowError::InvalidInput("limit 过大".to_owned()))?;
    Ok(checked_limit(scaled)?.max(min).min(max))
}

pub(in crate::db) fn map_row_not_found(
    error: sqlx::Error,
    resource: impl Into<String>,
) -> WorldflowError {
    match error {
        sqlx::Error::RowNotFound => WorldflowError::NotFound(resource.into()),
        other => WorldflowError::Database(other),
    }
}

#[cfg(feature = "sqlite")]
#[derive(Clone, Debug)]
pub struct SqliteDb {
    pub pool: SqlitePool,
    #[cfg(feature = "snapshot")]
    pub(in crate::db) snapshot: Option<Arc<SnapshotState>>,
}

#[cfg(feature = "sqlite")]
impl SqliteDb {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;

        sqlx::query("PRAGMA foreign_keys = ON;")
            .execute(&pool)
            .await?;
        sqlx::query("PRAGMA journal_mode = WAL;")
            .execute(&pool)
            .await?;
        sqlx::query("PRAGMA synchronous = NORMAL;")
            .execute(&pool)
            .await?;

        Self::reconcile_legacy_migration_checksums(&pool).await?;
        SQLITE_MIGRATOR.run(&pool).await?;

        let memory_mb = Self::get_available_memory();
        Self::apply_memory_pragmas(&pool, memory_mb).await?;

        Ok(Self {
            pool,
            #[cfg(feature = "snapshot")]
            snapshot: None,
        })
    }

    #[cfg(feature = "snapshot")]
    pub async fn new_with_snapshot(database_url: &str, config: SnapshotConfig) -> Result<Self> {
        let db = Self::new(database_url).await?;
        Ok(Self {
            pool: db.pool,
            #[cfg(feature = "snapshot")]
            snapshot: Some(Arc::new(SnapshotState::new(config)?)),
        })
    }

    pub async fn optimize_fts(&self) -> Result<()> {
        sqlx::query("INSERT INTO entries_fts(entries_fts) VALUES('optimize');")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    fn get_available_memory() -> u64 {
        *AVAILABLE_MEMORY_MB.get_or_init(|| {
            let mut sys = System::new_all();
            sys.refresh_memory();
            sys.available_memory() / 1024 / 1024
        })
    }

    async fn apply_memory_pragmas(pool: &SqlitePool, available_mb: u64) -> Result<()> {
        let (cache_kb, mmap_bytes, temp_store) = if available_mb > 16_000 {
            (-131072i64, 1073741824i64, "MEMORY")
        } else if available_mb > 8_000 {
            (-65536, 536870912, "MEMORY")
        } else if available_mb > 4_000 {
            (-32768, 268435456, "MEMORY")
        } else {
            (-16384, 134217728, "DEFAULT")
        };

        sqlx::query(&format!("PRAGMA cache_size = {cache_kb};"))
            .execute(pool)
            .await?;
        sqlx::query(&format!("PRAGMA mmap_size = {mmap_bytes};"))
            .execute(pool)
            .await?;
        sqlx::query(&format!("PRAGMA temp_store = {temp_store};"))
            .execute(pool)
            .await?;

        Ok(())
    }

    async fn reconcile_legacy_migration_checksums(pool: &SqlitePool) -> Result<()> {
        let has_migration_table: i64 = sqlx::query_scalar(
            "SELECT COUNT(1) FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
        )
        .fetch_one(pool)
        .await?;
        if has_migration_table == 0 {
            return Ok(());
        }

        let rows = sqlx::query_as::<_, (i64, Vec<u8>)>(
            "SELECT version, checksum FROM _sqlx_migrations WHERE success = 1",
        )
        .fetch_all(pool)
        .await?;

        for (version, applied_checksum) in rows {
            let Some(migration) = SQLITE_MIGRATOR
                .migrations
                .iter()
                .find(|migration| migration.version == version)
            else {
                continue;
            };

            let current_checksum = migration.checksum.as_ref();
            if applied_checksum.as_slice() == current_checksum {
                continue;
            }

            if !Self::matches_line_ending_variant(&applied_checksum, migration.sql.as_bytes()) {
                continue;
            }

            sqlx::query(
                "UPDATE _sqlx_migrations SET checksum = ? WHERE version = ? AND success = 1",
            )
            .bind(current_checksum)
            .bind(version)
            .execute(pool)
            .await?;
        }

        Ok(())
    }

    fn matches_line_ending_variant(applied_checksum: &[u8], sql: &[u8]) -> bool {
        let lf = Self::normalize_crlf_to_lf(sql);
        let crlf = Self::normalize_lf_to_crlf(&lf);

        Self::sha384(&lf).as_slice() == applied_checksum
            || Self::sha384(&crlf).as_slice() == applied_checksum
    }

    fn sha384(bytes: &[u8]) -> Vec<u8> {
        Sha384::digest(bytes).to_vec()
    }

    fn normalize_crlf_to_lf(bytes: &[u8]) -> Vec<u8> {
        let mut normalized = Vec::with_capacity(bytes.len());
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] == b'\r' && index + 1 < bytes.len() && bytes[index + 1] == b'\n' {
                normalized.push(b'\n');
                index += 2;
            } else {
                normalized.push(bytes[index]);
                index += 1;
            }
        }
        normalized
    }

    fn normalize_lf_to_crlf(bytes: &[u8]) -> Vec<u8> {
        let mut normalized = Vec::with_capacity(bytes.len() + bytes.len() / 32);
        for byte in bytes {
            if *byte == b'\n' {
                normalized.push(b'\r');
                normalized.push(b'\n');
            } else {
                normalized.push(*byte);
            }
        }
        normalized
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::{SQLITE_MIGRATOR, SqliteDb};
    use sqlx::Row;
    use tempfile::TempDir;

    fn temp_database_url(name: &str) -> (TempDir, String) {
        let temp = tempfile::tempdir().expect("创建临时目录失败");
        let path = temp.path().join(format!("{name}.db"));
        let url = format!(
            "sqlite:{}?mode=rwc",
            path.to_string_lossy().replace('\\', "/")
        );
        (temp, url)
    }

    async fn set_migration_checksum(db: &SqliteDb, version: i64, checksum: &[u8]) {
        sqlx::query("UPDATE _sqlx_migrations SET checksum = ? WHERE version = ?")
            .bind(checksum)
            .bind(version)
            .execute(&db.pool)
            .await
            .expect("写入迁移 checksum 失败");
    }

    async fn migration_checksum(db: &SqliteDb, version: i64) -> Vec<u8> {
        sqlx::query("SELECT checksum FROM _sqlx_migrations WHERE version = ?")
            .bind(version)
            .fetch_one(&db.pool)
            .await
            .expect("读取迁移 checksum 失败")
            .try_get("checksum")
            .expect("解析迁移 checksum 失败")
    }

    fn alternate_line_ending_checksum(version: i64) -> Vec<u8> {
        let migration = SQLITE_MIGRATOR
            .migrations
            .iter()
            .find(|migration| migration.version == version)
            .expect("迁移不存在");
        let current = migration.checksum.as_ref();
        let lf = SqliteDb::normalize_crlf_to_lf(migration.sql.as_bytes());
        let crlf = SqliteDb::normalize_lf_to_crlf(&lf);
        [SqliteDb::sha384(&lf), SqliteDb::sha384(&crlf)]
            .into_iter()
            .find(|checksum| checksum.as_slice() != current)
            .expect("没有可用的行尾变体 checksum")
    }

    #[tokio::test]
    async fn sqlite_open_repairs_line_ending_only_migration_checksum() {
        let (_temp, url) = temp_database_url("repair_line_ending_checksum");
        let db = SqliteDb::new(&url).await.expect("初始化数据库失败");
        let alternate = alternate_line_ending_checksum(1);
        set_migration_checksum(&db, 1, &alternate).await;
        drop(db);

        let reopened = SqliteDb::new(&url).await.expect("重新打开数据库失败");
        let repaired = migration_checksum(&reopened, 1).await;
        let current = SQLITE_MIGRATOR
            .migrations
            .iter()
            .find(|migration| migration.version == 1)
            .expect("迁移不存在")
            .checksum
            .as_ref()
            .to_vec();
        assert_eq!(repaired, current);
    }

    #[tokio::test]
    async fn sqlite_open_keeps_failing_for_real_migration_checksum_mismatch() {
        let (_temp, url) = temp_database_url("reject_real_checksum_mismatch");
        let db = SqliteDb::new(&url).await.expect("初始化数据库失败");
        let invalid_checksum = vec![0; 48];
        set_migration_checksum(&db, 1, &invalid_checksum).await;
        drop(db);

        let error = SqliteDb::new(&url)
            .await
            .expect_err("真实迁移差异不应被自动修复");
        assert!(error.to_string().contains("migration 1"));
    }
}

// ── PostgreSQL 实现 ──────────────────────────────────────
#[cfg(feature = "postgres")]
mod pg_category;
#[cfg(feature = "postgres")]
mod pg_entry;
#[cfg(feature = "postgres")]
mod pg_entry_link;
#[cfg(feature = "postgres")]
mod pg_entry_relation;
#[cfg(feature = "postgres")]
mod pg_entry_type;
#[cfg(feature = "postgres")]
mod pg_idea_note;
#[cfg(feature = "postgres")]
mod pg_project;
#[cfg(feature = "postgres")]
mod pg_tag_schema;

#[cfg(feature = "postgres")]
use sqlx::{PgPool, postgres::PgPoolOptions};

#[cfg(feature = "postgres")]
#[derive(Clone, Debug)]
pub struct PgDb {
    pub pool: PgPool,
}

#[cfg(feature = "postgres")]
impl PgDb {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;

        sqlx::migrate!("./migrations_pg").run(&pool).await?;

        Ok(Self { pool })
    }
}
