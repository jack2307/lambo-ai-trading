//! The bar file format is a contract with Python, not a Rust implementation
//! detail.
//!
//! `py/ingest/mt5_export.py` writes bar files with pyarrow; `read_bars` must
//! read them without knowing who wrote them. `roundtrip.rs` cannot prove that —
//! a writer and reader from the same crate agree by construction. This fixture
//! was written by pyarrow 21 with the schema the exporter uses (timestamp[ms,
//! UTC], nullable volume, zstd, key-value metadata) and is checked value by
//! value.

use std::path::{Path, PathBuf};

use fd_core::parity_eq;
use fd_store::read_bars;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("tests").join("golden").join("pyarrow-bars.parquet")
}

#[test]
fn a_pyarrow_written_bar_file_reads_value_for_value() {
    let bars = read_bars(&fixture()).expect("the pyarrow fixture reads");
    assert_eq!(bars.len(), 3);

    // Times are UTC milliseconds regardless of the timezone label the writer
    // attached; 2022-06-16 10:30:00Z is the first Vantage XAUUSD.sc M15 bar.
    assert_eq!(bars[0].time, 1_655_375_400_000);
    assert_eq!(bars[1].time - bars[0].time, 900_000);
    assert_eq!(bars[2].time - bars[1].time, 900_000);

    for (bar, (o, h, l, c)) in bars.iter().zip([
        (1818.53, 1819.40, 1817.90, 1819.10),
        (1819.10, 1819.62, 1817.80, 1817.95),
        (1817.95, 1818.30, 1816.44, 1816.70),
    ]) {
        assert!(parity_eq(bar.open, o), "open {} vs {o}", bar.open);
        assert!(parity_eq(bar.high, h), "high {} vs {h}", bar.high);
        assert!(parity_eq(bar.low, l), "low {} vs {l}", bar.low);
        assert!(parity_eq(bar.close, c), "close {} vs {c}", bar.close);
    }

    // A null volume must come back as `None`, not as zero: the schema makes
    // volume nullable precisely so an untraded bar and an unknown one differ.
    assert_eq!(bars[0].volume, Some(412.0));
    assert_eq!(bars[1].volume, None);
    assert_eq!(bars[2].volume, Some(388.0));
}
