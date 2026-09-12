//! Reading and writing option tapes as Parquet.

use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use arrow::array::{
    Array, ArrayRef, BooleanArray, Float64Array, RecordBatch, StringArray, TimestampMillisecondArray,
};
use fd_core::classify::FlowClass;
use fd_core::types::{AggressorSide, OptionTrade, OptionType, TradeFlags};
use parquet::arrow::ArrowWriter;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::properties::WriterProperties;

use crate::error::StoreError;
use crate::schema::tape_schema;

/// Rows per Parquet row group.
///
/// A row group is the unit a reader can skip, so this is the granularity of a
/// time-range scan. Small groups mean finer skipping and more metadata; this
/// size keeps a day of a busy tape to a handful of groups.
const ROW_GROUP: usize = 32_768;

fn ts_array(values: impl Iterator<Item = i64>) -> ArrayRef {
    Arc::new(TimestampMillisecondArray::from_iter_values(values).with_timezone("UTC"))
}

/// Write `trades` to `path`, replacing whatever was there.
///
/// Parquet has no append: a file is a footer over immutable row groups. Growing
/// a dataset means adding files, not extending one — see [`TapeStore`].
pub fn write_tape(path: &Path, trades: &[OptionTrade]) -> Result<(), StoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let schema = tape_schema();
    let props = WriterProperties::builder()
        // zstd over snappy: these files are written once and read many times,
        // and the tape's repeated symbols and strikes compress well.
        .set_compression(Compression::ZSTD(ZstdLevel::try_new(3)?))
        .set_max_row_group_size(ROW_GROUP)
        .build();
    let mut writer = ArrowWriter::try_new(File::create(path)?, Arc::clone(&schema), Some(props))?;

    for chunk in trades.chunks(ROW_GROUP) {
        writer.write(&batch_of(chunk)?)?;
    }
    writer.close()?;
    Ok(())
}

fn batch_of(trades: &[OptionTrade]) -> Result<RecordBatch, StoreError> {
    let columns: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from_iter_values(trades.iter().map(|t| t.id.as_str()))),
        ts_array(trades.iter().map(|t| t.timestamp)),
        Arc::new(StringArray::from_iter_values(trades.iter().map(|t| t.symbol.as_str()))),
        Arc::new(StringArray::from_iter(trades.iter().map(|t| t.instrument.as_deref()))),
        Arc::new(StringArray::from_iter_values(trades.iter().map(|t| t.underlying.as_str()))),
        ts_array(trades.iter().map(|t| t.expiration)),
        Arc::new(Float64Array::from_iter_values(trades.iter().map(|t| t.dte))),
        Arc::new(Float64Array::from_iter_values(trades.iter().map(|t| t.strike))),
        Arc::new(StringArray::from_iter_values(trades.iter().map(|t| match t.option_type {
            OptionType::Call => "CALL",
            OptionType::Put => "PUT",
        }))),
        Arc::new(Float64Array::from_iter_values(trades.iter().map(|t| t.trade_price))),
        Arc::new(Float64Array::from_iter_values(trades.iter().map(|t| t.contracts))),
        Arc::new(Float64Array::from_iter(trades.iter().map(|t| t.bid))),
        Arc::new(Float64Array::from_iter(trades.iter().map(|t| t.ask))),
        Arc::new(StringArray::from_iter_values(trades.iter().map(|t| match t.aggressor_side {
            AggressorSide::Buy => "BUY",
            AggressorSide::Sell => "SELL",
            AggressorSide::Unknown => "UNKNOWN",
        }))),
        Arc::new(StringArray::from_iter_values(trades.iter().map(|t| t.flow_class.as_str()))),
        Arc::new(Float64Array::from_iter_values(trades.iter().map(|t| t.premium_usd))),
        Arc::new(Float64Array::from_iter_values(trades.iter().map(|t| t.underlying_price))),
        Arc::new(StringArray::from_iter(trades.iter().map(|t| t.exchange.as_deref()))),
        Arc::new(StringArray::from_iter(trades.iter().map(|t| t.sequence_id.as_deref()))),
        Arc::new(Float64Array::from_iter(trades.iter().map(|t| t.implied_volatility))),
        Arc::new(BooleanArray::from_iter(trades.iter().map(|t| Some(t.flags.block)))),
        Arc::new(BooleanArray::from_iter(trades.iter().map(|t| Some(t.flags.sweep)))),
        Arc::new(BooleanArray::from_iter(trades.iter().map(|t| Some(t.flags.multi_leg)))),
        Arc::new(BooleanArray::from_iter(trades.iter().map(|t| Some(t.flags.spread)))),
        Arc::new(StringArray::from_iter_values(trades.iter().map(|t| t.source.as_str()))),
    ];
    Ok(RecordBatch::try_new(tape_schema(), columns)?)
}

/// Read every print in `path`, in stored order.
pub fn read_tape(path: &Path) -> Result<Vec<OptionTrade>, StoreError> {
    let reader = ParquetRecordBatchReaderBuilder::try_new(File::open(path)?)?
        .with_batch_size(ROW_GROUP)
        .build()?;
    let mut out = Vec::new();
    for batch in reader {
        append_batch(&batch?, &mut out)?;
    }
    Ok(out)
}

/// Column accessors, by position.
///
/// Positional rather than by name so that a schema drift shows up as a type
/// error on the very first read instead of as a column of nulls.
macro_rules! column {
    ($batch:expr, $index:expr, $ty:ty, $name:literal) => {
        $batch
            .column($index)
            .as_any()
            .downcast_ref::<$ty>()
            .ok_or_else(|| StoreError::Column($name))?
    };
}

fn append_batch(batch: &RecordBatch, out: &mut Vec<OptionTrade>) -> Result<(), StoreError> {
    let id = column!(batch, 0, StringArray, "id");
    let timestamp = column!(batch, 1, TimestampMillisecondArray, "timestamp");
    let symbol = column!(batch, 2, StringArray, "symbol");
    let instrument = column!(batch, 3, StringArray, "instrument");
    let underlying = column!(batch, 4, StringArray, "underlying");
    let expiration = column!(batch, 5, TimestampMillisecondArray, "expiration");
    let dte = column!(batch, 6, Float64Array, "dte");
    let strike = column!(batch, 7, Float64Array, "strike");
    let option_type = column!(batch, 8, StringArray, "option_type");
    let trade_price = column!(batch, 9, Float64Array, "trade_price");
    let contracts = column!(batch, 10, Float64Array, "contracts");
    let bid = column!(batch, 11, Float64Array, "bid");
    let ask = column!(batch, 12, Float64Array, "ask");
    let aggressor = column!(batch, 13, StringArray, "aggressor_side");
    let flow_class = column!(batch, 14, StringArray, "flow_class");
    let premium = column!(batch, 15, Float64Array, "premium_usd");
    let underlying_price = column!(batch, 16, Float64Array, "underlying_price");
    let exchange = column!(batch, 17, StringArray, "exchange");
    let sequence_id = column!(batch, 18, StringArray, "sequence_id");
    let iv = column!(batch, 19, Float64Array, "implied_volatility");
    let block = column!(batch, 20, BooleanArray, "flag_block");
    let sweep = column!(batch, 21, BooleanArray, "flag_sweep");
    let multi_leg = column!(batch, 22, BooleanArray, "flag_multi_leg");
    let spread = column!(batch, 23, BooleanArray, "flag_spread");
    let source = column!(batch, 24, StringArray, "source");

    let optional = |array: &StringArray, i: usize| array.is_valid(i).then(|| array.value(i).to_string());
    let optional_f64 = |array: &Float64Array, i: usize| array.is_valid(i).then(|| array.value(i));

    out.reserve(batch.num_rows());
    for i in 0..batch.num_rows() {
        out.push(OptionTrade {
            id: id.value(i).to_string(),
            timestamp: timestamp.value(i),
            symbol: symbol.value(i).to_string(),
            instrument: optional(instrument, i),
            underlying: underlying.value(i).to_string(),
            expiration: expiration.value(i),
            dte: dte.value(i),
            strike: strike.value(i),
            option_type: OptionType::from_letter(option_type.value(i)).unwrap_or(OptionType::Call),
            trade_price: trade_price.value(i),
            contracts: contracts.value(i),
            bid: optional_f64(bid, i),
            ask: optional_f64(ask, i),
            aggressor_side: AggressorSide::parse(aggressor.value(i)),
            flow_class: parse_flow_class(flow_class.value(i)),
            premium_usd: premium.value(i),
            underlying_price: underlying_price.value(i),
            exchange: optional(exchange, i),
            sequence_id: optional(sequence_id, i),
            implied_volatility: optional_f64(iv, i),
            flags: TradeFlags {
                block: block.value(i),
                sweep: sweep.value(i),
                multi_leg: multi_leg.value(i),
                spread: spread.value(i),
            },
            source: source.value(i).to_string(),
        });
    }
    Ok(())
}

/// Inverse of [`FlowClass::as_str`].
///
/// An unreadable label becomes `Unknown` rather than an error: a single odd row
/// should drop out of the bull/bear counts, not stop a day's tape from loading.
fn parse_flow_class(label: &str) -> FlowClass {
    match label {
        "LC" => FlowClass::Lc,
        "LP" => FlowClass::Lp,
        "SC" => FlowClass::Sc,
        "SP" => FlowClass::Sp,
        _ => FlowClass::Unknown,
    }
}
