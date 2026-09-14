//! Parquet storage for tapes and bar series.
//!
//! Replaces the prototype's JSON blobs. The reason is not disk space, though
//! that improves: it is that a JSON array has to be parsed in full to read any
//! of it, which caps how much history the system can hold at roughly what fits
//! in memory at once. Parquet is columnar and chunked, so a reader touches only
//! the columns and row groups it asked for.
//!
//! The format is also the boundary with the Python research side. Nothing here
//! is specific to this codebase: the files are ordinary Parquet, readable by
//! polars, pandas or DuckDB without any of this crate.
//!
//! Not a database. There is no index, no transaction and no concurrent writer
//! protocol — one process writes, many read. Anything more belongs upstream in
//! the ingest design, not here.

pub mod bars;
pub mod error;
pub mod news;
pub mod resample;
pub mod schema;
pub mod store;
pub mod tape;

pub use bars::{read_bars, write_bars};
pub use error::StoreError;
pub use news::read_news;
pub use resample::{resample, timeframe_ms};
pub use schema::{bars_schema, tape_schema};
pub use store::TapeStore;
pub use tape::{read_tape, write_tape};
