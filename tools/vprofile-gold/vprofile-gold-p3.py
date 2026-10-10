"""Precheck 3: is there a two-window split with >= 40 signals on both sides
that does NOT cut inside a regime?

Mechanism as registered: a bar closing OUTSIDE the prior profile window's value
area is a signal toward the POC. Counted on the committed time profile (arm A)
and on the tick-volume profile (arm B1), because the two disagree and the count
is what the gate's `n >= 40` leg reads.

At most one signal per window, taken at the first qualifying bar, so the count
is of opportunities and not of bars.
"""
import sys

import numpy as np
import pandas as pd
import pyarrow.parquet as pq

SEAM = pd.Timestamp("2023-12-01", tz="UTC")

rows = pd.read_csv(sys.argv[1])
df = pq.read_table(r"E:\rust\flowdesk\data\bars\XAUUSD-15m.parquet").to_pandas()
ts = pd.to_datetime(df["time"], utc=True)
df["ms"] = (ts - pd.Timestamp("1970-01-01", tz="UTC")).dt.total_seconds().mul(1000).round().astype("int64")
df["ts"] = ts

for wd in [1, 5]:
    for arm in ["A_TIME_AT_PRICE", "B1_TICK_VOLUME_PER_BUCKET"]:
        p = rows[(rows["window_days"] == wd) & (rows["bucket_scheme"] == "ATR14_OVER_4") & (rows["arm"] == arm)]
        p = p.sort_values("start_ms").reset_index(drop=True)
        for thresh_atr, tlabel in [(0.0, "close outside VA"), (0.25, ">=0.25 ATR outside"), (0.50, ">=0.50 ATR outside")]:
            sigs = []
            # Window i's value area is applied to window i+1's bars: strictly
            # prior, never the window the level came from.
            for i in range(len(p) - 1):
                prev, cur = p.iloc[i], p.iloc[i + 1]
                bars = df[(df["ms"] >= cur["start_ms"]) & (df["ms"] <= cur["end_ms"])]
                if bars.empty:
                    continue
                pad = thresh_atr * prev["atr14"]
                hi, lo = prev["vah"] + pad, prev["val"] - pad
                hit = bars[(bars["close"] > hi) | (bars["close"] < lo)]
                if not hit.empty:
                    sigs.append(hit["ts"].iloc[0])
            s = pd.Series(sigs)
            nA = int((s < SEAM).sum())
            nB = int((s >= SEAM).sum())
            pct_A = 100 * nA / max(len(s), 1)
            fired = nA < 40 or nB < 40
            print(
                f"window_days={wd:<2} arm={arm:<26} {tlabel:<20} signals={len(s):>5} "
                f"| at the 2023-12 seam: A={nA:>4} B={nB:>4} (A is {pct_A:.1f}% of all) "
                f"-> P3 {'FIRES' if fired else 'does NOT fire'}"
            )
