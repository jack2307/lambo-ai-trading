"""Decide whether a bar store that GREW is the same instrument it was, or a seam.

    python py/ingest/bar_seam_check.py --old data/bars/XAUUSD-15m.parquet
    python py/ingest/bar_seam_check.py --old BEFORE.parquet --new AFTER.parquet

Why this exists
---------------
`AGENT-BRIEF-ADDENDUM-11` section VII: 25 of 34 published rows stopped
reproducing because a store GREW under a receipt that had pinned neither end of
its window (`XAUDUKA-5m` 198,716 -> 1,135,389 bars, x5.31, and the trade counts
moved by exactly that factor). Extending a store is therefore not a neutral act.
The question an extension has to answer first is not "is it longer" but "is the
new part the SAME SERIES" -- because if it is not, every two-window result
computed across the join is an artefact of the join and not of the market.

Three checks, and the first is where a false result is cheapest to manufacture:

A. SHAPE. `XAUUSD` volume is MT5 *tick* volume (`mt5_export.py:33-34`: "CFD real
   volume is always zero"), an instrument-side count that has already changed
   shape once in this store's life. So: per YEAR, the share of zero and null
   volume, the level, and the Gini of the distribution; plus a two-sample KS `D`
   between adjacent years. `BTCUSD-15m` is the precedent that forces the yearly
   breakdown rather than a single number -- its volume column is 100% zero in
   2023-24, 42.1% in 2025 and 0% in 2026, so ONE store can hold TWO columns.

   And the provider's price signature: `open == low` on the bar that reopens
   after the 17:00 New York CME halt. Two things make this easy to get wrong.
   `fd_core::clock::new_york_local` returns `(weekday, minute-of-day)` and NOT
   `(hour, minute)`, so a histogram keyed on its second element is a histogram
   of the day of the week. And the CLASS WIDTH decides the answer: the whole
   18:00 New York hour dilutes the lift about fourfold against the single 15m
   bar at 18:00. Both widths are printed here so neither can be quoted alone.

B. SEAM. Not in frames, in BARS. `OptionsTimeline` stamped 6,010 of 9,491 frames
   with a duplicate timestamp (63%) because it stamped them with
   `snapshot.as_of`, so this dossier has a precedent for duplicate stamps and a
   rule drawn from it: read by bar. Counts duplicate stamps, off-grid stamps,
   holes, and -- where old and new overlap -- how many shared timestamps carry
   DIFFERENT prices, which is the one thing a merge silently resolves.

C. REACH. How much of the options tape sits inside the candles, by print; and the
   tape's own time-separated blocks, which are what a two-window gate can
   actually be built on.

It writes nothing. It only prints.
"""

from __future__ import annotations

import argparse
import collections
import datetime as dt
import glob
import os
from zoneinfo import ZoneInfo

import pyarrow.parquet as pq

UTC = dt.timezone.utc
NEW_YORK = ZoneInfo("America/New_York")
COLS = ("time", "open", "high", "low", "close", "volume")
META_KEYS = ("source", "broker_symbol", "server", "timeframe", "volume",
             "spread_points_at_export", "exported_at")


def load(path):
    t = pq.read_table(path, columns=list(COLS))
    times = t.column("time").cast("int64").to_pylist()
    c = {name: t.column(name).to_pylist() for name in COLS[1:]}
    rows = sorted(zip(times, c["open"], c["high"], c["low"], c["close"], c["volume"]))
    md = pq.read_schema(path).metadata or {}
    md = {k.decode(): v.decode() for k, v in md.items() if not k.decode().startswith("pandas")}
    return rows, md


def gini(xs):
    """Gini of a non-negative sample. `None` -- not 0 -- when it is undefined."""
    xs = sorted(x for x in xs if x is not None)
    n = len(xs)
    total = sum(xs)
    if n == 0 or total <= 0:
        return None
    cum = 0.0
    for i, x in enumerate(xs, 1):
        cum += i * x
    return (2.0 * cum) / (n * total) - (n + 1.0) / n


def ks_d(a, b):
    """Two-sample Kolmogorov-Smirnov D. No scipy: the statistic is a max gap."""
    a = sorted(x for x in a if x is not None)
    b = sorted(x for x in b if x is not None)
    if not a or not b:
        return None
    i = j = 0
    d = 0.0
    while i < len(a) and j < len(b):
        x = min(a[i], b[j])
        while i < len(a) and a[i] <= x:
            i += 1
        while j < len(b) and b[j] <= x:
            j += 1
        gap = abs(i / len(a) - j / len(b))
        if gap > d:
            d = gap
    return d


def pct(num, den):
    return "n/a" if den == 0 else "{:.2f}%".format(100.0 * num / den)


def fmt(x, spec="{:.0f}"):
    return "null" if x is None else spec.format(x)


def utcs(ms):
    return dt.datetime.fromtimestamp(ms / 1000, UTC)


def check_a(rows, label, step_ms):
    print("\n--- A. SHAPE: {} ---".format(label))
    by_year = collections.defaultdict(list)
    for r in rows:
        by_year[utcs(r[0]).year].append(r)
    print("  volume column, by year (tick volume: a COUNT of price changes, not contracts)")
    print("    {:<6}{:>8}{:>9}{:>9}{:>10}{:>10}{:>9}".format(
        "year", "bars", "null", "zero", "median", "mean", "gini"))
    samples = {}
    for y in sorted(by_year):
        v = [r[5] for r in by_year[y]]
        n_null = sum(1 for x in v if x is None)
        n_zero = sum(1 for x in v if x == 0)
        ok = sorted(x for x in v if x is not None)
        samples[y] = ok
        med = ok[len(ok) // 2] if ok else None
        mean = (sum(ok) / len(ok)) if ok else None
        print("    {:<6}{:>8}{:>9}{:>9}{:>10}{:>10}{:>9}".format(
            y, len(v), pct(n_null, len(v)), pct(n_zero, len(v)),
            fmt(med), fmt(mean), fmt(gini(ok), "{:.4f}")))
    years = sorted(samples)
    print("  KS D between ADJACENT years (a level shift shows as D near 1)")
    for a, b in zip(years, years[1:]):
        d = ks_d(samples[a], samples[b])
        ma = (sum(samples[a]) / len(samples[a])) if samples[a] else None
        mb = (sum(samples[b]) / len(samples[b])) if samples[b] else None
        ratio = (mb / ma) if (ma and mb and ma > 0) else None
        print("    {}->{}:  D = {}   mean ratio = {}".format(
            a, b, fmt(d, "{:.4f}"), "null" if ratio is None else "x{:.3f}".format(ratio)))

    # Provider price signature at the 17:00 NY CME halt. The bar that REOPENS is
    # the 18:00 one. Printed at two class widths because the width moves it.
    narrow_hit = narrow_n = wide_hit = wide_n = base_hit = base_n = 0
    for t, o, h, l, c, v in rows:
        ny = utcs(t).astimezone(NEW_YORK)
        hit = 1 if o == l else 0
        if ny.hour == 18:
            wide_n += 1
            wide_hit += hit
            if ny.minute < step_ms // 60000:
                narrow_n += 1
                narrow_hit += hit
        else:
            base_n += 1
            base_hit += hit
    print("  open == low, by class (NEW YORK clock; the reopen bar after the 17:00 halt)")
    print("    exactly 18:00 NY (one {}m bar): {}/{} = {}".format(
        step_ms // 60000, narrow_hit, narrow_n, pct(narrow_hit, narrow_n)))
    print("    the whole 18:00 NY hour       : {}/{} = {}".format(
        wide_hit, wide_n, pct(wide_hit, wide_n)))
    print("    every other bar (baseline)    : {}/{} = {}".format(
        base_hit, base_n, pct(base_hit, base_n)))
    if base_n and base_hit and narrow_n and wide_n:
        lift_n = (narrow_hit / narrow_n) / (base_hit / base_n)
        lift_w = (wide_hit / wide_n) / (base_hit / base_n)
        print("    lift: narrow x{:.2f}   wide x{:.2f}   (taking the whole hour dilutes it {:.2f}x)".format(
            lift_n, lift_w, (lift_n / lift_w) if lift_w else float("nan")))


def grid_report(rows, label, step_ms):
    ts = [r[0] for r in rows]
    dup = len(ts) - len(set(ts))
    off = sum(1 for t in ts if t % step_ms != 0)
    holes = [(a, b) for a, b in zip(ts, ts[1:]) if b - a > step_ms]
    weekendish = [g for g in holes if g[1] - g[0] >= 2 * 86_400_000]
    missing = sum((b - a) // step_ms - 1 for a, b in holes)
    print("  {}: {} bars | duplicate stamps {} | off-grid stamps {}".format(
        label, len(ts), dup, off))
    print("    holes {} (of which >= 2d, i.e. weekend-shaped: {}) | bars missing inside them {}".format(
        len(holes), len(weekendish), missing))
    print("    => grid occupancy {} of its own span".format(pct(len(ts), len(ts) + missing)))
    for a, b in sorted(holes, key=lambda g: g[0] - g[1])[:5]:
        print("      biggest hole {} -> {}  ({:.1f} h, {} bars)".format(
            utcs(a), utcs(b), (b - a) / 3_600_000, (b - a) // step_ms - 1))


def check_b(old, new, step_ms):
    print("\n--- B. SEAM (by BAR, never by frame) ---")
    grid_report(old, "old", step_ms)
    if new is None:
        print("  (no --new: nothing is joined, so there is no seam to measure -- this is null, not 0)")
        return
    grid_report(new, "new", step_ms)
    om = {r[0]: r for r in old}
    nm = {r[0]: r for r in new}
    shared = sorted(set(om) & set(nm))
    print("  overlap: {} shared stamps | only in old {} | only in new {}".format(
        len(shared), len(set(om) - set(nm)), len(set(nm) - set(om))))
    if not shared:
        print("    NO SHARED STAMP AT ALL. The two files do not touch, so the join is a"
              " butt joint and nothing cross-checks it.")
        return
    diff = [t for t in shared if om[t][1:5] != nm[t][1:5]]
    vdiff = [t for t in shared if om[t][5] != nm[t][5]]
    print("    shared stamps whose OHLC differs  : {} ({})".format(len(diff), pct(len(diff), len(shared))))
    print("    shared stamps whose volume differs: {} ({})".format(len(vdiff), pct(len(vdiff), len(shared))))
    for t in diff[:3]:
        print("      {}  old {}  new {}".format(utcs(t), om[t][1:5], nm[t][1:5]))
    print("    A non-zero OHLC count means the merge REWROTE bars that a published receipt"
          " may already have been read on. 0 is the only safe value.")
    last_old = max(om)
    after = sorted(t for t in nm if t > last_old)
    print("  the new tail: {} bars after the old last bar ({})".format(len(after), utcs(last_old)))
    if after:
        print("    first new bar {} | step across the join {:.2f} h = {} bar-widths".format(
            utcs(after[0]), (after[0] - last_old) / 3_600_000, (after[0] - last_old) // step_ms))


def check_c(rows, tape_glob, step_ms):
    print("\n--- C. REACH: options tape against the candles ---")
    files = sorted(glob.glob(tape_glob))
    if not files:
        print("  no tape parts matched {} -- NOT MEASURED (null, not 0)".format(tape_glob))
        return
    ts = []
    for f in files:
        ts += pq.read_table(f, columns=["timestamp"]).column("timestamp").cast("int64").to_pylist()
    ts.sort()
    cut = max(r[0] for r in rows) + step_ms  # a print inside the LAST bar is covered
    inside = sum(1 for x in ts if x < cut)
    print("  tape: {} prints, {} .. {}".format(len(ts), utcs(ts[0]), utcs(ts[-1])))
    print("  candles end {} (last bar + one bar)".format(utcs(cut)))
    print("  prints INSIDE candle coverage : {} ({})".format(inside, pct(inside, len(ts))))
    print("  prints AFTER the last candle  : {} ({})".format(len(ts) - inside, pct(len(ts) - inside, len(ts))))
    blocks = [[ts[0]]]
    for a, b in zip(ts, ts[1:]):
        if b - a >= 2 * 86_400_000:
            blocks.append([])
        blocks[-1].append(b)
    print("  the tape's own time-separated blocks (split on a >= 2d print gap): {}".format(len(blocks)))
    for i, bl in enumerate(blocks, 1):
        covered = sum(1 for x in bl if x < cut)
        print("    block {}: {:>6} prints  {:%Y-%m-%d %H:%M} .. {:%Y-%m-%d %H:%M}  | inside candles {}".format(
            i, len(bl), utcs(bl[0]), utcs(bl[-1]), pct(covered, len(bl))))
    print("  NOTE: a print is not a trade. These counts bound how many signals a window")
    print("  COULD hold; they are not the gate's n >= 40 and must not be read as it.")


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--old", required=True, help="the store BEFORE the pull")
    ap.add_argument("--new", default=None, help="the store AFTER the pull (omit for a baseline-only run)")
    ap.add_argument("--step-min", type=int, default=15, help="bar length in minutes")
    ap.add_argument("--tape", default=os.path.join("data", "gold", "tape", "date=*", "*.parquet"))
    args = ap.parse_args()
    step_ms = args.step_min * 60_000

    old, omd = load(args.old)
    print("OLD {}".format(args.old))
    print("    {} bars | {} .. {}".format(len(old), utcs(old[0][0]), utcs(old[-1][0])))
    for k in META_KEYS:
        if k in omd:
            print("    {} = {}".format(k, omd[k]))
    new = None
    if args.new:
        new, nmd = load(args.new)
        print("NEW {}".format(args.new))
        print("    {} bars | {} .. {}".format(len(new), utcs(new[0][0]), utcs(new[-1][0])))
        for k in META_KEYS:
            if k in nmd:
                print("    {} = {}".format(k, nmd[k]))
        # The one comparison that costs nothing and settles the most.
        if omd.get("server") and nmd.get("server") and omd["server"] != nmd["server"]:
            print("\n  *** SERVER CHANGED: '{}' -> '{}'. The two halves are quoted by"
                  " DIFFERENT servers. That is a seam by construction, before a single"
                  " number is compared. ***".format(omd["server"], nmd["server"]))

    check_a(old, "OLD ({} bars)".format(len(old)), step_ms)
    if new:
        check_a(new, "NEW ({} bars)".format(len(new)), step_ms)
    check_b(old, new, step_ms)
    check_c(new if new else old, args.tape, step_ms)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
