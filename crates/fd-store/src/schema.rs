//! Arrow schemas for the stored types.
//!
//! Written out by hand rather than derived. The Parquet files are the interface
//! between this workspace and the Python research side, so their column names
//! and types are a contract: a derive macro would let a rename in a Rust struct
//! silently rewrite that contract.
//!
//! Times are stored as Arrow timestamps rather than raw integers so that a
//! reader which knows nothing about this codebase — a notebook, DuckDB, a
//! spreadsheet — sees instants rather than large numbers. Prices and sizes stay
//! `f64`: they are already the engine's working type, and rounding them on the
//! way to disk would make a stored tape disagree with a live one.

use std::sync::Arc;

use arrow::datatypes::{DataType, Field, Schema, TimeUnit};

/// UTC millisecond timestamp, the only time type this store writes.
fn ts() -> DataType {
    DataType::Timestamp(TimeUnit::Millisecond, Some("UTC".into()))
}

fn required(name: &str, data_type: DataType) -> Field {
    Field::new(name, data_type, false)
}

fn optional(name: &str, data_type: DataType) -> Field {
    Field::new(name, data_type, true)
}

/// One option print per row.
///
/// The column order matches [`crate::tape`]'s writer and reader; changing one
/// without the other is caught by the round-trip test rather than by a reader
/// silently getting a shifted field.
#[must_use]
pub fn tape_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        required("id", DataType::Utf8),
        required("timestamp", ts()),
        // The expiry contract (`BTC-18SEP26`), not the full instrument.
        required("symbol", DataType::Utf8),
        optional("instrument", DataType::Utf8),
        required("underlying", DataType::Utf8),
        required("expiration", ts()),
        required("dte", DataType::Float64),
        required("strike", DataType::Float64),
        required("option_type", DataType::Utf8),
        required("trade_price", DataType::Float64),
        required("contracts", DataType::Float64),
        optional("bid", DataType::Float64),
        optional("ask", DataType::Float64),
        required("aggressor_side", DataType::Utf8),
        required("flow_class", DataType::Utf8),
        required("premium_usd", DataType::Float64),
        required("underlying_price", DataType::Float64),
        optional("exchange", DataType::Utf8),
        optional("sequence_id", DataType::Utf8),
        optional("implied_volatility", DataType::Float64),
        required("flag_block", DataType::Boolean),
        required("flag_sweep", DataType::Boolean),
        required("flag_multi_leg", DataType::Boolean),
        required("flag_spread", DataType::Boolean),
        required("source", DataType::Utf8),
    ]))
}

/// One price bar per row.
///
/// `volume` is nullable because the gold feed has none, and a stored zero would
/// be indistinguishable from a genuinely untraded bar.
#[must_use]
pub fn bars_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        required("time", ts()),
        required("open", DataType::Float64),
        required("high", DataType::Float64),
        required("low", DataType::Float64),
        required("close", DataType::Float64),
        optional("volume", DataType::Float64),
    ]))
}
