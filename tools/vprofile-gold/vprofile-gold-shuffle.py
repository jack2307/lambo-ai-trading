"""Control for the regime shift in precheck 1: shuffle the volume column WITHIN
each trading day, keeping its distribution and destroying its association with
price.

If the shuffled column reproduces the same regime A -> regime B jump in
agreement with the time POC, then the jump is a property of the column's
DISTRIBUTION (how uneven the weights are) and not of any information the column
carries about where price traded. For the 1-day windows the shuffle is exactly
within-window, so that is the row to read.
"""
import sys

import numpy as np
import pandas as pd
import pyarrow.parquet as pq

seed = int(sys.argv[2]) if len(sys.argv) > 2 else 20261010
out = sys.argv[1]

df = pq.read_table(r"E:\rust\flowdesk\data\bars\XAUUSD-15m.parquet").to_pandas()
ts = pd.to_datetime(df["time"], utc=True)
df["ms"] = (ts - pd.Timestamp("1970-01-01", tz="UTC")).dt.total_seconds().mul(1000).round().astype("int64")
step = int(np.median(np.diff(df["ms"].to_numpy())))
df["day"] = (df["ms"].diff().fillna(0) >= step * 4).cumsum()

rng = np.random.default_rng(seed)
v = df["volume"].to_numpy(float).copy()
for _, idx in df.groupby("day").indices.items():
    # `v[idx]` is a COPY under fancy indexing, so shuffling it in place would
    # change nothing. Permute explicitly and write back.
    v[idx] = v[idx][rng.permutation(len(idx))]
df["volume_shuffled"] = v

with open(out, "w", newline="\n", encoding="utf-8") as f:
    f.write("time,open,high,low,close,volume\n")
    for r in df.itertuples(index=False):
        f.write(f"{int(r.ms)},{r.open!r},{r.high!r},{r.low!r},{r.close!r},{float(r.volume_shuffled)!r}\n")
print(f"shuffled within {df['day'].nunique()} trading days, seed {seed} -> {out}")
print(f"  sanity: totals equal? {np.isclose(df['volume'].sum(), df['volume_shuffled'].sum())}")
print(f"  sanity: identical order? {bool((df['volume'].to_numpy() == df['volume_shuffled'].to_numpy()).all())}")
