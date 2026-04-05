#![allow(async_fn_in_trait)]

pub mod db;
pub mod error;
pub mod models;

pub use db::traits::{Db, ProjectOps, CategoryOps, EntryOps, TagSchemaOps, EntryRelationOps, EntryTypeOps};
pub use error::{Result, WorldflowError};

#[cfg(feature = "sqlite")]
pub use db::SqliteDb;

#[cfg(feature = "postgres")]
pub use db::PgDb;
