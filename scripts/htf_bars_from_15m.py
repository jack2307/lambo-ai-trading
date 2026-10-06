"""Aggregate XAUUSD 15m parquet into UTC-aligned 1h and 4h series.

Schema is written to match crates/fd-store/src/bars.rs read_bars, which reads
by COLUMN INDEX: 0 time (timestamp[ms, tz=UTC]), 1 open, 2 high, 3 low,
4 close (all float64 non-null), 5 volume (float64, nullable).

Buckets with no 15m bar are dropped, never emitted as zeros.
Reads only the shared store; writes only into /e/rust/fd-a3/data/bars.
"""
import pandas as pd
import pyarrow as pa
import pyarrow.parquet as pq

SRC = "E:/rust/flowdesk/data/bars/XAUUSD-15m.parquet"
OUT = "E:/rust/fd-a3/data/bars"

src = pq.ParquetFile(SRC)
src_meta = src.schema_arrow.metadata or {}
df = src.read().to_pandas()
df = df.sort_values("time").reset_index(drop=True)
print("source rows:", len(df), df["time"].min(), "->", df["time"].max())

# gap census on the source, so the derived series' bucket sizes can be read
delta = df["time"].diff().dt.total_seconds().div(60).dropna()
print("source 15m step: min %.0f median %.0f max %.0f minutes"
      % (delta.min(), delta.median(), delta.max()))

schema_fields = [
    pa.field("time", pa.timestamp("ms", tz="UTC"), nullable=False),
    pa.field("open", pa.float64(), nullable=False),
    pa.field("high", pa.float64(), nullable=False),
    pa.field("low", pa.float64(), nullable=False),
    pa.field("close", pa.float64(), nullable=False),
    pa.field("volume", pa.float64(), nullable=True),
]

for label, rule, n_src in (("1h", "1h", 4), ("4h", "4h", 16)):
    g = df.set_index("time").resample(rule, label="left", closed="left")
    out = pd.DataFrame({
        "time": g["open"].first().index,
        "open": g["open"].first().values,
        "high": g["high"].max().values,
        "low": g["low"].min().values,
        "close": g["close"].last().values,
        "volume": g["volume"].sum(min_count=1).values,
        "n": g["open"].count().values,
    })
    before = len(out)
    out = out[out["n"] > 0].reset_index(drop=True)
    print("%s: %d buckets kept of %d spanned; full buckets (n==%d) %d = %.1f%%; "
          "partial %d; min n %d"
          % (label, len(out), before, n_src, int((out["n"] == n_src).sum()),
             100.0 * (out["n"] == n_src).mean(), int((out["n"] < n_src).sum()),
             int(out["n"].min())))
    n = out.pop("n")
    meta = {k.decode(): v.decode() for k, v in src_meta.items()}
    meta["timeframe"] = {"1h": "H1", "4h": "H4"}[label]
    meta["derivation"] = (
        "DERIVED by resampling XAUUSD-15m.parquet into UTC-aligned %s buckets "
        "(open=first, high=max, low=min, close=last, volume=sum, empty buckets "
        "dropped). NOT a broker export. Built 2026-10-06 for the bar-interval "
        "axis, docs/decisions/2026-10-06-bar-interval.md." % label)
    tbl = pa.Table.from_pandas(out, schema=pa.schema(schema_fields, metadata=meta),
                               preserve_index=False)
    path = "%s/XAUUSD-%s.parquet" % (OUT, label)
    pq.write_table(tbl, path, compression="ZSTD", version="2.6")
    chk = pq.ParquetFile(path)
    print("   wrote %s rows=%d" % (path, chk.metadata.num_rows))
    print("   cols:", [f.name for f in chk.schema_arrow],
          [str(f.type) for f in chk.schema_arrow])
