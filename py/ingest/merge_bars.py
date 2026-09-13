"""Merge two fd-store bar files of the same symbol and timeframe into one.

    python py/ingest/merge_bars.py data/bars/XAUDUKA-15m.parquet out/XAUDUKA-early-15m.parquet

The first argument is the file to extend, in place, after a `.bak` copy is
written next to it. Rows are keyed by time; where both files carry a bar the
existing file's row wins (the extension is for history the store did not
have, not for corrections). The schema and metadata of the existing file are
kept, with `merged_from` and `merged_at` added.
"""

from __future__ import annotations

import datetime as dt
import os
import shutil
import sys

import pyarrow as pa
import pyarrow.parquet as pq


def main() -> int:
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    target, extra = sys.argv[1], sys.argv[2]
    base = pq.read_table(target)
    more = pq.read_table(extra).cast(base.schema.remove_metadata())
    have = set(base.column("time").to_pylist())
    keep = [i for i, t in enumerate(more.column("time").to_pylist()) if t not in have]
    added = more.take(keep)
    merged = pa.concat_tables([base.cast(base.schema.remove_metadata()), added]).sort_by("time")
    meta = dict(base.schema.metadata or {})
    meta[b"merged_from"] = os.path.basename(extra).encode()
    meta[b"merged_at"] = dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ").encode()
    merged = merged.replace_schema_metadata(meta)
    shutil.copyfile(target, target + ".bak")
    pq.write_table(merged, target, compression="zstd", compression_level=3, row_group_size=32_768)
    times = merged.column("time").to_pylist()
    print(f"{target}: {base.num_rows} + {added.num_rows} new ({more.num_rows - added.num_rows} already present) = {merged.num_rows} bars, {times[0]} .. {times[-1]}; backup at {target}.bak")
    return 0


if __name__ == "__main__":
    sys.exit(main())
