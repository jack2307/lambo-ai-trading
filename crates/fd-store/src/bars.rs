//! Reading and writing bar series as Parquet.
//!
//! Unlike a tape, a bar series is a single coherent thing: one file per symbol
//! and timeframe, rewritten when it grows. The prototype's JSON store did the
//! same, and a bar series is small enough that the simplicity is worth more
//! than avoiding the rewrite.

use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use arrow::array::{Array, ArrayRef, Float64Array, RecordBatch, TimestampMillisecondArray};
use fd_core::types::Bar;
use parquet::arrow::ArrowWriter;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::properties::WriterProperties;

use crate::error::StoreError;
use crate::schema::bars_schema;

const ROW_GROUP: usize = 32_768;

/// Write `bars` to `path`, replacing whatever was there.
pub fn write_bars(path: &Path, bars: &[Bar]) -> Result<(), StoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let schema = bars_schema();
    let props = WriterProperties::builder()
        .set_compression(Compression::ZSTD(ZstdLevel::try_new(3)?))
        .set_max_row_group_size(ROW_GROUP)
        .build();
    let mut writer = ArrowWriter::try_new(File::create(path)?, Arc::clone(&schema), Some(props))?;

    for chunk in bars.chunks(ROW_GROUP) {
        let columns: Vec<ArrayRef> = vec![
            Arc::new(
                TimestampMillisecondArray::from_iter_values(chunk.iter().map(|b| b.time))
                    .with_timezone("UTC"),
            ),
            Arc::new(Float64Array::from_iter_values(chunk.iter().map(|b| b.open))),
            Arc::new(Float64Array::from_iter_values(chunk.iter().map(|b| b.high))),
            Arc::new(Float64Array::from_iter_values(chunk.iter().map(|b| b.low))),
            Arc::new(Float64Array::from_iter_values(chunk.iter().map(|b| b.close))),
            Arc::new(Float64Array::from_iter(chunk.iter().map(|b| b.volume))),
        ];
        writer.write(&RecordBatch::try_new(Arc::clone(&schema), columns)?)?;
    }
    writer.close()?;
    Ok(())
}

/// Read every bar in `path`, in stored order.
pub fn read_bars(path: &Path) -> Result<Vec<Bar>, StoreError> {
    let reader = ParquetRecordBatchReaderBuilder::try_new(File::open(path)?)?
        .with_batch_size(ROW_GROUP)
        .build()?;
    let mut out = Vec::new();
    for batch in reader {
        let batch = batch?;
        let time = batch
            .column(0)
            .as_any()
            .downcast_ref::<TimestampMillisecondArray>()
            .ok_or(StoreError::Column("time"))?;
        let float = |index: usize, name: &'static str| {
            batch.column(index).as_any().downcast_ref::<Float64Array>().ok_or(StoreError::Column(name))
        };
        let (open, high, low, close, volume) =
            (float(1, "open")?, float(2, "high")?, float(3, "low")?, float(4, "close")?, float(5, "volume")?);

        out.reserve(batch.num_rows());
        for i in 0..batch.num_rows() {
            out.push(Bar {
                time: time.value(i),
                open: open.value(i),
                high: high.value(i),
                low: low.value(i),
                close: close.value(i),
                volume: volume.is_valid(i).then(|| volume.value(i)),
            });
        }
    }
    Ok(out)
}
