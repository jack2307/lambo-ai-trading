# -*- coding: utf-8 -*-
"""Fingerprint the bars a study actually read, so a registration can name them.

    py -3.9 py/research/bars_fingerprint.py data/bars/XAUUSD-15m.parquet
    py -3.9 py/research/bars_fingerprint.py data/bars/XAUUSD-4h.parquet --from 2022-06-16 --to 2026-09-17
    py -3.9 py/research/bars_fingerprint.py data/bars/XAUUSD-*.parquet --brief

Prints a block meant to be pasted into a registration under
`docs/hypotheses/` or `docs/decisions/`.

THE PROBLEM IT SOLVES. A registration that says "measured on
data/bars/XAUUSD-15m.parquet" names a PATH, and a path is not a thing. Since
2026-09-18 that same path holds different bytes on the desktop and on the
VPS - the desktop's corpus is frozen, the VPS copy is rewritten every five
minutes by the export task - and nothing in the filename says which. The two
diverge further every day, silently, with the same name and the same schema.
`docs/research/WHICH-BARS.md` states a convention about it, and a convention
survives only as long as everyone remembers it, which on this desk is the
thing that has failed most often.

A hash does not need remembering. A later reader recomputes it and either
gets the same number or knows the ground moved.

WHAT IS HASHED, AND WHY NOT THE FILE.

  Not the file. Every export rewrites the parquet - `exported_at` alone
  changes on every run - so a file hash would differ between two machines
  holding IDENTICAL bars, and would change hourly on the VPS while nothing
  about the data moved. It would be a hash of when, not of what.

  The BARS. Each row is packed as its five or six numbers, big-endian, and
  fed to sha256 in time order: the timestamp as int64 milliseconds, then
  open/high/low/close as IEEE-754 float64 bit patterns, then volume, with an
  absent volume written as a NaN-distinct sentinel so "no volume" and
  "volume 0" cannot collide. Bit patterns rather than decimal text, because
  repr() of a float has changed between Python versions and a fingerprint
  that moves with the interpreter is worse than none.

  THE WINDOW, not the whole file, when one is given. A study that read four
  years out of a sixteen-year file and hashed all sixteen has pinned bytes
  it never looked at - and will report a mismatch the day someone extends
  the file backwards, over a window that did not change. `--from`/`--to`
  bound what is hashed, and the block records the bounds beside the digest
  so the claim is exactly as wide as the reading was.

WHAT THIS IS NOT. It says two readings saw the same bars. It says nothing
about whether the bars are RIGHT - a wrong-anchor resample fingerprints just
as cleanly as a correct export. For that, the metadata this also prints
(`broker_symbol`, `server`, `clock_rule`, `timeframe`) is the thing to read,
and `docs/research/WHICH-BARS.md` says what the absence of that metadata
means.
"""
from __future__ import annotations

import argparse
import datetime as dt
import glob
import hashlib
import os
import struct
import sys

try:
    import pyarrow.parquet as pq
except ImportError:  # pragma: no cover - environment, not logic
    sys.exit("pyarrow is not installed: py -3.9 -m pip install pyarrow")

for _s in (sys.stdout, sys.stderr):
    try:
        _s.reconfigure(encoding="utf-8", errors="replace")
    except (AttributeError, ValueError):
        pass

# An absent volume is not a zero volume. Parquet stores `volume` nullable and
# the Dukascopy imports leave it empty, so the two have to hash differently or
# a corpus with volumes and one without could collide.
NO_VOLUME = b"\xff" * 8


def _ms(value) -> int:
    """A pyarrow timestamp cell as UTC epoch milliseconds."""
    if isinstance(value, dt.datetime):
        if value.tzinfo is None:
            value = value.replace(tzinfo=dt.timezone.utc)
        return int(value.timestamp() * 1000)
    return int(value)


def fingerprint(path: str, since_ms: int | None, until_ms: int | None) -> dict:
    table = pq.read_table(path)
    cols = {name: table.column(name).to_pylist()
            for name in ("time", "open", "high", "low", "close")}
    volume = (table.column("volume").to_pylist()
              if "volume" in table.schema.names else [None] * len(cols["time"]))

    digest = hashlib.sha256()
    rows = 0
    first = last = None
    for i, raw_t in enumerate(cols["time"]):
        t = _ms(raw_t)
        if since_ms is not None and t < since_ms:
            continue
        if until_ms is not None and t > until_ms:
            continue
        digest.update(struct.pack(">q", t))
        for name in ("open", "high", "low", "close"):
            digest.update(struct.pack(">d", float(cols[name][i])))
        v = volume[i]
        digest.update(NO_VOLUME if v is None else struct.pack(">d", float(v)))
        rows += 1
        if first is None:
            first = t
        last = t

    meta = {k.decode(): v.decode(errors="replace")
            for k, v in (pq.read_schema(path).metadata or {}).items()}
    return {"path": path.replace("\\", "/"), "rows": rows, "first": first,
            "last": last, "sha256": digest.hexdigest(), "meta": meta}


def _u(ms) -> str:
    if ms is None:
        return "-"
    return dt.datetime.utcfromtimestamp(ms / 1000).strftime("%Y-%m-%dT%H:%M:%SZ")


def _parse_day(text: str, end: bool) -> int:
    d = dt.datetime.strptime(text, "%Y-%m-%d").replace(tzinfo=dt.timezone.utc)
    if end:
        d = d + dt.timedelta(days=1) - dt.timedelta(milliseconds=1)
    return int(d.timestamp() * 1000)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("files", nargs="+", help="parquet paths; globs are expanded")
    ap.add_argument("--from", dest="since", default=None, metavar="YYYY-MM-DD",
                    help="hash only bars at or after this UTC day")
    ap.add_argument("--to", dest="until", default=None, metavar="YYYY-MM-DD",
                    help="hash only bars at or before the end of this UTC day")
    ap.add_argument("--brief", action="store_true",
                    help="one line per file instead of a pasteable block")
    args = ap.parse_args()

    since = _parse_day(args.since, False) if args.since else None
    until = _parse_day(args.until, True) if args.until else None

    paths: list[str] = []
    for pattern in args.files:
        hits = sorted(glob.glob(pattern))
        if not hits and os.path.exists(pattern):
            hits = [pattern]
        if not hits:
            print(f"no such file: {pattern}", file=sys.stderr)
            return 1
        paths.extend(hits)

    if not args.brief:
        print("| file | bars | first | last | sha256 (first 16) |")
        print("|---|---|---|---|---|")
    for p in paths:
        fp = fingerprint(p, since, until)
        if args.brief:
            print(f"{fp['path']}  {fp['rows']:>9,}  {_u(fp['first'])}  {_u(fp['last'])}  "
                  f"{fp['sha256'][:16]}")
            continue
        print(f"| `{fp['path']}` | {fp['rows']:,} | {_u(fp['first'])} | {_u(fp['last'])} "
              f"| `{fp['sha256'][:16]}` |")

    if args.brief:
        return 0

    print()
    window = "the whole file" if not (since or until) else \
        f"bars from {args.since or 'the start'} to {args.until or 'the end'} inclusive, UTC"
    print(f"Window hashed: {window}.")
    print("Full digests, and the provenance the files carry:")
    print()
    for p in paths:
        fp = fingerprint(p, since, until)
        print(f"- `{fp['path']}`")
        print(f"  - `sha256:{fp['sha256']}`")
        m = fp["meta"]
        if m:
            bits = [f"{k}={m[k]}" for k in ("broker_symbol", "server", "timeframe",
                                            "exported_at", "contract_size") if k in m]
            print(f"  - {'; '.join(bits)}")
        else:
            # Absence is informative: the MT5 exporter always stamps these, so
            # a file without them came through another importer. See
            # docs/research/WHICH-BARS.md.
            print("  - no provenance metadata: not an MT5 export "
                  "(Dukascopy, Binance or the GC futures file)")
    print()
    print("Recompute with: `py -3.9 py/research/bars_fingerprint.py <path>"
          + (f" --from {args.since}" if args.since else "")
          + (f" --to {args.until}" if args.until else "") + "`")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
