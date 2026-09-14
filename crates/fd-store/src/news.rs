//! Reading the scheduled-news calendar.
//!
//! `data/news/events.parquet` is written by the Python side (columns `time`
//! timestamp[ms, UTC], `currency` utf8, `name` utf8, `impact` int8 with 3 =
//! high, `source` utf8, sorted by time). Unlike the tape and bar files, this
//! one is not a Rust-written contract, so columns are found **by name**: the
//! writer is free to add or reorder columns, and a missing or retyped one
//! fails loudly on the first read rather than as a column of nulls.
//!
//! Only what the blackout filter needs is loaded — `name` and `source` stay
//! on disk. The binary that calls this installs the result through
//! `fd_strategy::news::install` once at startup; nothing here is reached from
//! a hot path.

use std::fs::File;
use std::path::Path;

use arrow::array::{Array, Int8Array, RecordBatch, StringArray, TimestampMillisecondArray};
use fd_core::types::NewsEvent;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

use crate::error::StoreError;

const ROW_GROUP: usize = 8192;

/// Read every event in `path`, in stored order.
pub fn read_news(path: &Path) -> Result<Vec<NewsEvent>, StoreError> {
    let reader = ParquetRecordBatchReaderBuilder::try_new(File::open(path)?)?
        .with_batch_size(ROW_GROUP)
        .build()?;
    let mut out = Vec::new();
    for batch in reader {
        append_batch(&batch?, &mut out)?;
    }
    Ok(out)
}

fn column<'a, T: Array + 'static>(batch: &'a RecordBatch, name: &'static str) -> Result<&'a T, StoreError> {
    batch
        .column_by_name(name)
        .and_then(|c| c.as_any().downcast_ref::<T>())
        .ok_or(StoreError::Column(name))
}

fn append_batch(batch: &RecordBatch, out: &mut Vec<NewsEvent>) -> Result<(), StoreError> {
    let time = column::<TimestampMillisecondArray>(batch, "time")?;
    let currency = column::<StringArray>(batch, "currency")?;
    let impact = column::<Int8Array>(batch, "impact")?;

    out.reserve(batch.num_rows());
    for i in 0..batch.num_rows() {
        // A null anywhere means the row is not an event the filter can use.
        if time.is_null(i) || impact.is_null(i) {
            continue;
        }
        // Impact is 1..=3 by contract; a negative value would wrap into a
        // huge one and block everything, so clamp at zero (never matches).
        let impact = u8::try_from(impact.value(i)).unwrap_or(0);
        let currency = if currency.is_valid(i) { currency.value(i).to_string() } else { String::new() };
        out.push(NewsEvent { time: time.value(i), impact, currency });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow::array::ArrayRef;
    use arrow::datatypes::{Field, Schema};
    use parquet::arrow::ArrowWriter;

    use super::*;

    /// Write a file the way the Python exporter does: the documented
    /// schema, in the documented column order, one nullable row.
    fn write_fixture(path: &Path, columns: &[(&str, ArrayRef)]) {
        let fields: Vec<Field> = columns.iter().map(|(name, array)| Field::new(*name, array.data_type().clone(), true)).collect();
        let schema = Arc::new(Schema::new(fields));
        let batch = RecordBatch::try_new(Arc::clone(&schema), columns.iter().map(|(_, a)| Arc::clone(a)).collect()).unwrap();
        let mut writer = ArrowWriter::try_new(File::create(path).unwrap(), schema, None).unwrap();
        writer.write(&batch).unwrap();
        writer.close().unwrap();
    }

    fn ts(values: Vec<Option<i64>>) -> ArrayRef {
        Arc::new(TimestampMillisecondArray::from(values).with_timezone("UTC"))
    }

    fn utf8(values: Vec<Option<&str>>) -> ArrayRef {
        Arc::new(StringArray::from(values))
    }

    fn int8(values: Vec<Option<i8>>) -> ArrayRef {
        Arc::new(Int8Array::from(values))
    }

    #[test]
    fn reads_the_python_schema_by_name_and_skips_null_rows() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.parquet");
        write_fixture(
            &path,
            &[
                ("time", ts(vec![Some(1_789_000_000_000), Some(1_789_003_600_000), None, Some(1_789_007_200_000)])),
                ("currency", utf8(vec![Some("USD"), Some("All"), Some("EUR"), None])),
                ("name", utf8(vec![Some("CPI"), Some("Holiday"), Some("ECB"), Some("Unknown")])),
                ("impact", int8(vec![Some(3), Some(2), Some(3), Some(1)])),
                ("source", utf8(vec![Some("ff"), Some("ff"), Some("ff"), Some("ff")])),
            ],
        );
        let events = read_news(&path).expect("the fixture reads");
        assert_eq!(
            events,
            vec![
                NewsEvent { time: 1_789_000_000_000, impact: 3, currency: "USD".into() },
                NewsEvent { time: 1_789_003_600_000, impact: 2, currency: "All".into() },
                NewsEvent { time: 1_789_007_200_000, impact: 1, currency: String::new() },
            ]
        );
    }

    #[test]
    fn column_order_is_not_a_contract_but_names_and_types_are() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("reordered.parquet");
        write_fixture(
            &path,
            &[
                ("source", utf8(vec![Some("ff")])),
                ("impact", int8(vec![Some(3)])),
                ("time", ts(vec![Some(42)])),
                ("currency", utf8(vec![Some("USD")])),
            ],
        );
        assert_eq!(read_news(&path).unwrap(), vec![NewsEvent { time: 42, impact: 3, currency: "USD".into() }]);

        let path = dir.path().join("no-impact.parquet");
        write_fixture(&path, &[("time", ts(vec![Some(42)])), ("currency", utf8(vec![Some("USD")]))]);
        let err = read_news(&path).unwrap_err();
        assert!(matches!(err, StoreError::Column("impact")), "{err}");

        // The wrong type for `impact` is refused, not coerced.
        let path = dir.path().join("wide-impact.parquet");
        let wide: ArrayRef = Arc::new(arrow::array::Int64Array::from(vec![Some(3)]));
        write_fixture(&path, &[("time", ts(vec![Some(42)])), ("currency", utf8(vec![Some("USD")])), ("impact", wide)]);
        assert!(matches!(read_news(&path).unwrap_err(), StoreError::Column("impact")));
    }
}
