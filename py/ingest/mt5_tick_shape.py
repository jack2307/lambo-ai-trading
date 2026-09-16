"""The shape of the tick stream: when quotes arrive, and how long a side is walked.

    python py/ingest/mt5_tick_shape.py --symbol=XAUUSD.sc --days=2026-02-10,2026-04-15

Read-only: `initialize`, `symbol_info`, `copy_ticks_range`, `shutdown`.

This is INVENTORY, not a measurement. It counts how often things happen and
never looks at what price did afterwards, so it can be run before a
registration without spending the question — the same footing as
`mt5_tick_depth.py`, which asked how far the history reached.

Two dimensions of this feed that nothing has used:

  GAPS      the milliseconds between consecutive ticks. A dealer that stops
            quoting has stopped knowing, and the first quote after the silence
            is a guess. `2026-09-15-quote-asymmetry` counted ticks per minute
            and never asked when they arrived.

  RUNS      consecutive one-sided revisions moving the SAME way with no
            revision of the other side in between. That study summed signed
            point moves and 91.2% of them cancelled inside the minute; a run is
            what survives cancelling, and it is a different object: one
            revision is noise, eight in a row is a decision being repeated.

Session boundaries are excluded from the gap counts by construction — a gap
longer than `--session-gap` minutes is the daily stop or a weekend, and is
reported separately rather than mixed in.
"""

from __future__ import annotations

import argparse
import sys
from datetime import datetime, timedelta, timezone

import numpy as np

GAP_BUCKETS_S = (0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0)
RUN_BUCKETS = (2, 3, 4, 5, 6, 8, 10, 15)


def shape_of(ticks, point: float, session_gap_min: float) -> dict:
    bid = ticks["bid"].astype(np.float64)
    ask = ticks["ask"].astype(np.float64)
    tmsc = ticks["time_msc"].astype(np.int64)

    dt_s = np.diff(tmsc) / 1000.0
    session = dt_s > session_gap_min * 60.0
    intra = dt_s[~session]

    db, da = np.diff(bid), np.diff(ask)
    eps = point / 2.0
    mv_b, mv_a = np.abs(db) > eps, np.abs(da) > eps
    one = mv_a ^ mv_b
    # +1 / -1 for a one-sided revision, 0 for anything else. A run is a maximal
    # stretch of the same non-zero value: same side, same direction, nothing of
    # the other side in between.
    step = np.where(mv_a, np.sign(da), np.where(mv_b, np.sign(db), 0.0))
    step = np.where(one, step, 0.0)

    runs = []
    cur = 0.0
    length = 0
    for s in step:
        if s != 0.0 and s == cur:
            length += 1
        else:
            if cur != 0.0 and length >= 2:
                runs.append(length)
            cur, length = s, (1 if s != 0.0 else 0)
    if cur != 0.0 and length >= 2:
        runs.append(length)
    runs = np.array(runs) if runs else np.zeros(0)

    return {
        "ticks": len(ticks),
        "intra": intra,
        "sessions": int(session.sum()),
        "one": int(one.sum()),
        "runs": runs,
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--symbol", default="XAUUSD.sc")
    ap.add_argument("--days", default="2026-02-10,2026-03-12,2026-04-15,2026-05-13,2026-06-09")
    ap.add_argument("--session-gap", type=float, default=20.0,
                    help="minutes above which a gap is the daily stop, not a freeze")
    a = ap.parse_args()

    import MetaTrader5 as mt5

    if not mt5.initialize():
        print(f"initialize failed: {mt5.last_error()}")
        return 2
    try:
        info = mt5.symbol_info(a.symbol)
        if info is None:
            print(f"{a.symbol}: not in Market Watch")
            return 3
        point = float(info.point)
        days = [d.strip() for d in a.days.split(",") if d.strip()]
        print(f"{a.symbol}: point {point}. Inventory only - no forward return is read.")
        print(f"sampling {len(days)} days: {', '.join(days)}\n")

        all_intra, all_runs = [], []
        total_ticks = total_one = 0
        for d in days:
            start = datetime.fromisoformat(d).replace(tzinfo=timezone.utc)
            ticks = mt5.copy_ticks_range(a.symbol, start, start + timedelta(days=1), mt5.COPY_TICKS_ALL)
            if ticks is None or len(ticks) < 3:
                print(f"  {d}  no ticks")
                continue
            s = shape_of(ticks, point, a.session_gap)
            all_intra.append(s["intra"])
            if len(s["runs"]):
                all_runs.append(s["runs"])
            total_ticks += s["ticks"]
            total_one += s["one"]
            print(f"  {d}  {s['ticks']:>8,} ticks  one-sided {s['one']:>7,}  "
                  f"session breaks {s['sessions']}  runs>=2 {len(s['runs']):>6,}")

        intra = np.concatenate(all_intra) if all_intra else np.zeros(0)
        runs = np.concatenate(all_runs) if all_runs else np.zeros(0)
        n_days = len(all_intra)
        if n_days == 0:
            print("nothing sampled")
            return 1

        print(f"\nGAPS between consecutive ticks, {len(intra):,} intervals over {n_days} days")
        print(f"   median {np.median(intra):.3f}s   mean {intra.mean():.3f}s   "
              f"p99 {np.percentile(intra, 99):.3f}s   max {intra.max():.1f}s")
        print(f"   {'at least':>9s} {'count':>8s} {'per day':>8s} {'per month':>10s}")
        for g in GAP_BUCKETS_S:
            c = int((intra >= g).sum())
            print(f"   {g:8.1f}s {c:8,d} {c / n_days:8.1f} {21 * c / n_days:10.0f}")

        print(f"\nRUNS of same-side same-direction one-sided revisions, {len(runs):,} runs")
        if len(runs):
            print(f"   median {np.median(runs):.0f}   mean {runs.mean():.2f}   max {int(runs.max())}")
            print(f"   {'length >=':>9s} {'count':>8s} {'per day':>8s} {'per month':>10s}")
            for r in RUN_BUCKETS:
                c = int((runs >= r).sum())
                print(f"   {r:9d} {c:8,d} {c / n_days:8.1f} {21 * c / n_days:10.0f}")
        print(f"\none-sided share of all revisions: {100 * total_one / max(total_ticks, 1):.1f}%")
        print("a month is 21 trading days; five months of history is about five of those.")
    finally:
        mt5.shutdown()
    return 0


if __name__ == "__main__":
    sys.exit(main())
