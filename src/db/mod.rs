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
mod entry_relation;
#[cfg(feature = "sqlite")]
mod project;
#[cfg(feature = "sqlite")]
mod tag_schema;

use sysinfo::System;
use crate::error::Result;

#[cfg(feature = "sqlite")]
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

#[cfg(feature = "sqlite")]
#[derive(Clone, Debug)]
pub struct SqliteDb {
    pub pool: SqlitePool,
}

#[cfg(feature = "sqlite")]
impl SqliteDb {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;

        sqlx::query("PRAGMA foreign_keys = ON;").execute(&pool).await?;
        sqlx::query("PRAGMA journal_mode = WAL;").execute(&pool).await?;
        sqlx::query("PRAGMA synchronous = NORMAL;").execute(&pool).await?;

        sqlx::migrate!("./migrations").run(&pool).await?;

        let memory_mb = Self::get_available_memory();
        Self::apply_memory_pragmas(&pool, memory_mb).await?;

        Ok(Self { pool })
    }

    pub async fn optimize_fts(&self) -> Result<()> {
        sqlx::query("INSERT INTO entries_fts(entries_fts) VALUES('optimize');")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    fn get_available_memory() -> u64 {
        let mut sys = System::new_all();
        sys.refresh_memory();
        sys.available_memory() / 1024 / 1024
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

// ── PostgreSQL 实现 ──────────────────────────────────────
#[cfg(feature = "postgres")]
mod pg_category;
#[cfg(feature = "postgres")]
mod pg_entry;
#[cfg(feature = "postgres")]
mod pg_entry_relation;
#[cfg(feature = "postgres")]
mod pg_project;
#[cfg(feature = "postgres")]
mod pg_tag_schema;

#[cfg(feature = "postgres")]
use sqlx::{postgres::PgPoolOptions, PgPool};

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
