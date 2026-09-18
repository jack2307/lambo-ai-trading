# -*- coding: utf-8 -*-
"""The properties a bar fingerprint must have, or it is not worth registering.

    py -3.9 py/research/bars_fingerprint_selftest.py

Builds small parquets in a temp directory and checks the digest behaves the
way a registration needs it to. Nothing outside %TEMP% is touched.

THE ONE THAT MATTERS is "appending later bars does NOT change a windowed
digest". The VPS export adds bars every five minutes; if a study's
fingerprint of ITS window moved every time the file grew, every registration
would report a mismatch within the hour and the whole idea would be
abandoned in a week. The other checks are there so that this one cannot be
satisfied trivially - a digest that ignored everything would pass it too, so
a moved close and a differing volume must both still change the number.
"""
import datetime as dt, os, sys, tempfile
import pyarrow as pa, pyarrow.parquet as pq

sys.path.insert(0, os.path.join("py", "research"))
import bars_fingerprint as F

UTC = dt.timezone.utc
SCHEMA = pa.schema([
    pa.field("time", pa.timestamp("ms", tz="UTC"), nullable=False),
    pa.field("open", pa.float64(), nullable=False),
    pa.field("high", pa.float64(), nullable=False),
    pa.field("low", pa.float64(), nullable=False),
    pa.field("close", pa.float64(), nullable=False),
    pa.field("volume", pa.float64(), nullable=True),
])
fails = []


def check(name, ok, extra=""):
    print(("ok   " if ok else "FAIL ") + name + (("\n       " + extra) if extra and not ok else ""))
    if not ok:
        fails.append(name)


def write(path, n, start_day=1, volume=1.0, meta=None):
    t0 = dt.datetime(2026, 1, start_day, tzinfo=UTC)
    rows = [(t0 + dt.timedelta(minutes=15 * i), 100.0 + i, 101.0 + i, 99.0 + i, 100.5 + i,
             volume) for i in range(n)]
    tbl = pa.Table.from_pydict(
        {"time": [r[0] for r in rows], "open": [r[1] for r in rows],
         "high": [r[2] for r in rows], "low": [r[3] for r in rows],
         "close": [r[4] for r in rows], "volume": [r[5] for r in rows]},
        schema=SCHEMA.with_metadata(meta or {"exported_at": "2026-01-01T00:00:00Z"}))
    pq.write_table(tbl, path, compression="snappy")


d = tempfile.mkdtemp(prefix="fp-")
try:
    a = os.path.join(d, "A.parquet")
    write(a, 200)
    h1 = F.fingerprint(a, None, None)["sha256"]
    h2 = F.fingerprint(a, None, None)["sha256"]
    check("deterministic across runs", h1 == h2)

    # THE PROPERTY THE WHOLE THING EXISTS FOR: the VPS appends bars every five
    # minutes. A study that fingerprinted ITS window must not be invalidated
    # by bars arriving after it.
    win_from = int(dt.datetime(2026, 1, 1, tzinfo=UTC).timestamp() * 1000)
    win_to = int(dt.datetime(2026, 1, 1, 12, 0, tzinfo=UTC).timestamp() * 1000)
    before = F.fingerprint(a, win_from, win_to)
    b = os.path.join(d, "B.parquet")
    write(b, 400)                      # the same bars, plus 200 more after them
    after = F.fingerprint(b, win_from, win_to)
    check("appending later bars does NOT change a windowed digest",
          before["sha256"] == after["sha256"],
          f"{before['sha256'][:16]} vs {after['sha256'][:16]} rows {before['rows']}/{after['rows']}")
    check("  and the window really did bound it", before["rows"] < 200,
          f"{before['rows']} rows hashed of 200")
    check("  while the WHOLE-file digests differ, as they must",
          F.fingerprint(a, None, None)["sha256"] != F.fingerprint(b, None, None)["sha256"])

    # Metadata churn must not move it: every export rewrites `exported_at`,
    # and two machines holding identical bars must agree.
    c = os.path.join(d, "C.parquet")
    write(c, 200, meta={"exported_at": "2026-09-18T11:59:59Z", "server": "somewhere else"})
    check("a different exported_at does NOT change the digest",
          F.fingerprint(a, None, None)["sha256"] == F.fingerprint(c, None, None)["sha256"])

    # An absent volume is not a zero volume.
    z = os.path.join(d, "Z.parquet")
    n = os.path.join(d, "N.parquet")
    write(z, 50, volume=0.0)
    write(n, 50, volume=None)
    check("absent volume and zero volume hash differently",
          F.fingerprint(z, None, None)["sha256"] != F.fingerprint(n, None, None)["sha256"])

    # A changed price must move it, or it is not a fingerprint.
    e = os.path.join(d, "E.parquet")
    write(e, 200)
    tbl = pq.read_table(e)
    cl = tbl.column("close").to_pylist()
    cl[100] += 0.01
    tbl2 = pa.Table.from_pydict(
        {"time": tbl.column("time").to_pylist(), "open": tbl.column("open").to_pylist(),
         "high": tbl.column("high").to_pylist(), "low": tbl.column("low").to_pylist(),
         "close": cl, "volume": tbl.column("volume").to_pylist()},
        schema=SCHEMA.with_metadata({"exported_at": "2026-01-01T00:00:00Z"}))
    e2 = os.path.join(d, "E2.parquet")
    pq.write_table(tbl2, e2, compression="snappy")
    check("one bar's close moving by a cent changes the digest",
          F.fingerprint(e, None, None)["sha256"] != F.fingerprint(e2, None, None)["sha256"])

    # And the window is inclusive at both ends, as the help says.
    one = F.fingerprint(a, win_from, win_from)
    check("a single-millisecond window hashes exactly the bar at that stamp",
          one["rows"] == 1 and one["first"] == win_from, f"rows={one['rows']}")
finally:
    import shutil
    shutil.rmtree(d, ignore_errors=True)

print()
print("all checks passed" if not fails else f"{len(fails)} FAILED")
sys.exit(1 if fails else 0)
