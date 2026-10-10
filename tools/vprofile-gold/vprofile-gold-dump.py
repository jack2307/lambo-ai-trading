"""Dump data/bars/<SYM>.parquet to CSV for the vprofile-gold Rust harness.

Columns are the parquet file's own, unchanged. An EMPTY volume field means the
feed published none (NULL), which the harness parses to `None` and never to 0
or 1.
"""
import sys
import pyarrow.parquet as pq

sym, out = sys.argv[1], sys.argv[2]
t = pq.read_table(rf"E:\rust\flowdesk\data\bars\{sym}.parquet")
df = t.to_pandas()
# `time` is a timestamp[ms, UTC] logical type; the harness wants epoch ms.
import pandas as pd

df["time"] = (
    pd.to_datetime(df["time"], utc=True) - pd.Timestamp("1970-01-01", tz="UTC")
).dt.total_seconds().mul(1000).round().astype("int64")
n_null = int(df["volume"].isna().sum())
with open(out, "w", newline="\n", encoding="utf-8") as f:
    f.write("time,open,high,low,close,volume\n")
    for r in df.itertuples(index=False):
        v = "" if r.volume != r.volume else repr(float(r.volume))
        f.write(f"{int(r.time)},{r.open!r},{r.high!r},{r.low!r},{r.close!r},{v}\n")
print(f"{sym}: {len(df)} bars -> {out}  (null volume: {n_null})")
