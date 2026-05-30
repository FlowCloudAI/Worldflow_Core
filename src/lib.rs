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
pub use db::api_usage::{insert_api_usage, query_usage_by_model, query_usage_summary};

#[cfg(feature = "sqlite")]
pub use db::csv_bundle::{
    CsvExportItem, CsvExportScope, CsvImportBundle, CsvImportItem, CsvImportMode,
    CsvImportProgress, CsvImportProgressPhase, CsvImportResult, ProjectCsvExport,
    WorldflowCsvTable,
};

#[cfg(all(feature = "sqlite", feature = "snapshot"))]
pub use db::snapshot::{
    AppendResult, RestoreMode, SnapshotBranchInfo, SnapshotConfig, SnapshotInfo,
};
