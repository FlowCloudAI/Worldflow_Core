#![allow(async_fn_in_trait)]

pub mod db;
pub mod error;
pub mod models;

pub use db::traits::{
    CategoryOps, Db, EntryLinkOps, EntryOps, EntryRelationOps, EntryTypeOps, IdeaNoteOps,
    ProjectOps, TagSchemaOps,
};
pub use error::{Result, WorldflowError};

#[cfg(feature = "sqlite")]
pub use db::SqliteDb;

#[cfg(feature = "sqlite")]
pub use db::snapshot::{AppendResult, RestoreMode, SnapshotConfig, SnapshotInfo};

#[cfg(feature = "postgres")]
pub use db::PgDb;
