#[cfg(feature = "sqlite")]
pub mod sqlite;

#[cfg(feature = "postgres")]
pub mod postgres;

use sysinfo::System;
use crate::error::Result;
use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};

pub mod app_setting;
pub mod category;
pub mod entry;
pub mod project;
pub mod tag_schema;

#[derive(Clone, Debug)]
pub struct SqliteDb {
    pub pool: SqlitePool,
}

impl SqliteDb {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;

        sqlx::query("PRAGMA foreign_keys = ON;").execute(&pool).await?;
        sqlx::query("PRAGMA journal_mode = WAL;").execute(&pool).await?;
        sqlx::query("PRAGMA synchronous = NORMAL;").execute(&pool).await?;

        // 先跑 migration，app_settings 表才存在
        sqlx::migrate!("./migrations").run(&pool).await?;

        // 读用户配置，fallback 到自动检测
        let memory_mb = Self::resolve_memory_limit(&pool).await;
        Self::apply_memory_pragmas(&pool, memory_mb).await?;

        Ok(Self { pool })
    }

    async fn resolve_memory_limit(pool: &SqlitePool) -> u64 {
        // 读用户设置
        let row = sqlx::query("SELECT value FROM app_settings WHERE key = 'memory_limit_mb'")
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();

        if let Some(row) = row {
            let val: String = row.try_get("value").unwrap_or_default();
            if val != "auto" {
                if let Ok(mb) = val.parse::<u64>() {
                    return mb;
                }
            }
        }

        // 自动检测
        let mut sys = System::new_all();
        sys.refresh_memory();
        sys.available_memory() / 1024 / 1024  // 转换为 MB
    }

    async fn apply_memory_pragmas(pool: &SqlitePool, available_mb: u64) -> Result<()> {
        let (cache_kb, mmap_bytes, temp_store) = if available_mb > 16_000 {
            (-131072i64, 1073741824i64, "MEMORY")
        } else if available_mb > 8_000 {
            (-65536,     536870912,    "MEMORY")
        } else if available_mb > 4_000 {
            (-32768,     268435456,    "MEMORY")
        } else {
            (-16384,     134217728,    "DEFAULT")
        };

        sqlx::query(&format!("PRAGMA cache_size = {cache_kb};")).execute(pool).await?;
        sqlx::query(&format!("PRAGMA mmap_size = {mmap_bytes};")).execute(pool).await?;
        sqlx::query(&format!("PRAGMA temp_store = {temp_store};")).execute(pool).await?;

        Ok(())
    }
}