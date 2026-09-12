//! Store errors.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("parquet: {0}")]
    Parquet(#[from] parquet::errors::ParquetError),

    #[error("arrow: {0}")]
    Arrow(#[from] arrow::error::ArrowError),

    /// A stored column was missing or had an unexpected type. Means the file
    /// was written by a different schema version, not that the data is bad.
    #[error("column `{0}` is missing or has the wrong type — file written by a different schema?")]
    Column(&'static str),

    #[error("not a store directory: {0}")]
    NotAStore(PathBuf),
}
