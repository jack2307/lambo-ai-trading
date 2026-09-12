"""Convert a dukascopy-node CSV (m1, bid) into an fd-store bar file.

    npx dukascopy-node -i xauusd -from 2022-06-16 -to 2026-06-01 -t m1 -f csv -dir out
    python py/ingest/dukascopy_to_parquet.py out/xauusd-m1-bid-*.csv data/bars/XAUDUKA-1m.parquet

Why a second gold feed: the broker terminal caps one-minute history at
100k bars (three months). Dukascopy serves years of one-minute bars for free,
on its own bid feed. Structure-based strategies read the shape of bars, not
the exact quote, so this is a legitimate out-of-sample sample — with the
caveat, recorded in the file's metadata, that it is a different venue's price.

Timestamps are UTC milliseconds already. Volume is Dukascopy's traded-volume
estimate, kept as-is.

dukascopy-node fills the CME daily break (17:00-18:00 New York) with flat,
zero-volume bars — open = high = low = close, volume 0. Those are not
trades; they are dropped (data-integrity, 2026-09-13: 9,732 of them in four
years, enough to depress an ATR). Pass `--also 5` to write a five-minute
resample next to the minutes.
"""

from __future__ import annotations

import csv
import datetime as dt
import os
import sys

import pyarrow as pa
import pyarrow.parquet as pq

SCHEMA = pa.schema(
    [
        pa.field("time", pa.timestamp("ms", tz="UTC"), nullable=False),
        pa.field("open", pa.float64(), nullable=False),
        pa.field("high", pa.float64(), nullable=False),
        pa.field("low", pa.float64(), nullable=False),
        pa.field("close", pa.float64(), nullable=False),
        pa.field("volume", pa.float64(), nullable=True),
    ]
)


def main() -> int:
    argv = [a for a in sys.argv[1:] if not a.startswith("--also")]
    also = [int(a.split("=", 1)[1]) for a in sys.argv[1:] if a.startswith("--also=")]
    if len(argv) < 2:
        sys.exit(__doc__)
    sources, out = argv[:-1], argv[-1]
    rows: dict[int, tuple] = {}
    flat_dropped = 0
    for path in sources:
        with open(path, newline="", encoding="utf-8") as f:
            for r in csv.DictReader(f):
                t = int(r["timestamp"])
                o, h, l, c = (float(r[k]) for k in ("open", "high", "low", "close"))
                if not (l <= min(o, c) and max(o, c) <= h):
                    continue  # a malformed row is dropped, not repaired
                v = float(r["volume"]) if r.get("volume") not in (None, "") else None
                if o == h == l == c and not v:
                    flat_dropped += 1  # the feed's fill for a closed market
                    continue
                rows[t] = (t, o, h, l, c, v)
    ordered = [rows[k] for k in sorted(rows)]
    if not ordered:
        sys.exit("no rows")
    cols = list(zip(*ordered))
    table = pa.table(
        {
            "time": pa.array(cols[0], pa.timestamp("ms", tz="UTC")),
            "open": pa.array(cols[1], pa.float64()),
            "high": pa.array(cols[2], pa.float64()),
            "low": pa.array(cols[3], pa.float64()),
            "close": pa.array(cols[4], pa.float64()),
            "volume": pa.array(cols[5], pa.float64()),
        },
        schema=SCHEMA.with_metadata(
            {
                "source": "dukascopy",
                "feed": "bid",
                "note": "different venue from the trading account; structure, not quotes",
                "converted_at": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            }
        ),
    )
    os.makedirs(os.path.dirname(out) or ".", exist_ok=True)
    pq.write_table(table, out, compression="zstd", compression_level=3, row_group_size=32_768)
    first = dt.datetime.fromtimestamp(ordered[0][0] / 1000, tz=dt.timezone.utc)
    last = dt.datetime.fromtimestamp(ordered[-1][0] / 1000, tz=dt.timezone.utc)
    print(f"{out}: {len(ordered)} bars, {first} .. {last} ({flat_dropped} flat zero-volume bars dropped)")

    for minutes in also:
        step = minutes * 60_000
        buckets: dict[int, list] = {}
        for t, o, h, l, c, v in ordered:
            b = t - t % step
            r = buckets.get(b)
            if r is None:
                buckets[b] = [b, o, h, l, c, v or 0.0]
            else:
                r[2] = max(r[2], h)
                r[3] = min(r[3], l)
                r[4] = c
                r[5] += v or 0.0
        res = [buckets[k] for k in sorted(buckets)]
        rcols = list(zip(*res))
        rtable = pa.table(
            {
                "time": pa.array(rcols[0], pa.timestamp("ms", tz="UTC")),
                "open": pa.array(rcols[1], pa.float64()),
                "high": pa.array(rcols[2], pa.float64()),
                "low": pa.array(rcols[3], pa.float64()),
                "close": pa.array(rcols[4], pa.float64()),
                "volume": pa.array(rcols[5], pa.float64()),
            },
            schema=table.schema.with_metadata({**table.schema.metadata, b"resampled_from": os.path.basename(out).encode()}),
        )
        rout = out.replace("-1m.parquet", f"-{minutes}m.parquet")
        pq.write_table(rtable, rout, compression="zstd", compression_level=3, row_group_size=32_768)
        print(f"{rout}: {len(res)} bars")
    return 0


if __name__ == "__main__":
    sys.exit(main())
