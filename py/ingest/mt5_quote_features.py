"""Per-minute quote-revision features from the broker's own tick history.

    python py/ingest/mt5_quote_features.py --symbol=XAUUSD.sc \
        --from=2026-01-20 --to=2026-06-16

Read-only: `initialize`, `symbol_info`, `copy_ticks_range`, `shutdown`. Never an
order; `symbol_select` is not called.

Registered as `docs/hypotheses/2026-09-15-quote-asymmetry.md` at commit 13178b5.
The window is passed explicitly and nothing defaults to "everything", because
the out-of-sample dates named in that file must stay unread until the in-sample
record exists.

Raw ticks are not kept — about 0.4M a day, 65M over the window. What is kept is
one row a minute:

  counts     n_ticks, n_one (one side moved), n_two (both), n_none
  the signal os_sum   = signed points summed over ONE-SIDED revisions
  the control ts_sum  = signed mid points summed over TWO-SIDED revisions
  diagnostics ask_up/ask_dn/bid_up/bid_dn, each the signed point sum of that
             one-sided event type, so widening (ask_up + bid_dn) and narrowing
             (ask_dn + bid_up) can be read apart after the gate
  fills      bid/ask of the first tick at least 0, 1, 5 and 30 seconds after
             THIS minute's end — the quotes an entry or an exit would have been
             given, so no spread is ever assumed
  context    mid at the close, the minute's mid range (the volatility variable),
             and flag_disagree, where the price-derived classification and the
             tick's own TICK_FLAG_BID/ASK disagree

A minute with no tick is not written; the gap is what marks the daily stop and
the weekend, and the measurement refuses to hold a position across one.
"""

from __future__ import annotations

import argparse
import sys
from datetime import datetime, timedelta, timezone
from pathlib import Path

import numpy as np
import pyarrow as pa
import pyarrow.parquet as pq

ROOT = Path(__file__).resolve().parents[2]
OFFSETS_S = (0, 1, 5, 30)

SCHEMA = pa.schema(
    [pa.field("minute", pa.timestamp("ms", tz="UTC"), nullable=False)]
    + [pa.field(n, pa.int32()) for n in ("n_ticks", "n_one", "n_two", "n_none", "flag_disagree")]
    + [pa.field(n, pa.float64()) for n in
       ("os_sum", "ts_sum", "ask_up", "ask_dn", "bid_up", "bid_dn", "mid_close", "mid_range")]
    + [pa.field(f"{side}_{o}s", pa.float64()) for o in OFFSETS_S for side in ("bid", "ask")]
)


def day_features(ticks, point: float) -> dict:
    """Classify every revision in one day and fold it into minutes."""
    bid, ask = ticks["bid"].astype(np.float64), ticks["ask"].astype(np.float64)
    tmsc = ticks["time_msc"].astype(np.int64)
    flags = ticks["flags"].astype(np.int64) if "flags" in ticks.dtype.names else None

    db, da = np.diff(bid), np.diff(ask)
    eps = point / 2.0
    mv_b, mv_a = np.abs(db) > eps, np.abs(da) > eps
    one, two = mv_a ^ mv_b, mv_a & mv_b
    none = ~(mv_a | mv_b)

    # Signed size of the revision, in points. For a one-sided event it is the
    # side that moved; for a two-sided one it is the mid.
    s_one = np.where(mv_a, da, db) / point
    s_two = ((da + db) / 2.0) / point

    ask_up = one & mv_a & (da > 0)
    ask_dn = one & mv_a & (da < 0)
    bid_up = one & mv_b & (db > 0)
    bid_dn = one & mv_b & (db < 0)

    # The tick's own flags say which side it reports as changed. Derived from
    # price is what the hypothesis declared; the disagreement rate is reported.
    if flags is not None:
        FLAG_BID, FLAG_ASK = 2, 4
        f_b = (flags[1:] & FLAG_BID) != 0
        f_a = (flags[1:] & FLAG_ASK) != 0
        disagree = (f_b != mv_b) | (f_a != mv_a)
    else:
        disagree = np.zeros(len(one), dtype=bool)

    # Minute buckets, on the tick that ENDS each revision.
    minute = (tmsc[1:] // 60_000).astype(np.int64)
    keys, inv = np.unique(minute, return_inverse=True)
    nm = len(keys)

    def fold(mask, values=None):
        out = np.zeros(nm)
        np.add.at(out, inv, mask.astype(np.float64) if values is None else np.where(mask, values, 0.0))
        return out

    n_one, n_two, n_none = fold(one), fold(two), fold(none)
    out = {
        "minute": (keys * 60_000).astype("datetime64[ms]"),
        "n_ticks": (n_one + n_two + n_none).astype(np.int32),
        "n_one": n_one.astype(np.int32),
        "n_two": n_two.astype(np.int32),
        "n_none": n_none.astype(np.int32),
        "flag_disagree": fold(disagree).astype(np.int32),
        "os_sum": fold(one, s_one),
        "ts_sum": fold(two, s_two),
        "ask_up": fold(ask_up, da / point),
        "ask_dn": fold(ask_dn, da / point),
        "bid_up": fold(bid_up, db / point),
        "bid_dn": fold(bid_dn, db / point),
    }

    mid = (bid + ask) / 2.0
    mid_end = mid[1:]
    close = np.zeros(nm)
    close[inv] = mid_end              # last write per bucket == the minute's close
    hi = np.full(nm, -np.inf)
    lo = np.full(nm, np.inf)
    np.maximum.at(hi, inv, mid_end)
    np.minimum.at(lo, inv, mid_end)
    out["mid_close"] = close
    out["mid_range"] = hi - lo

    # The quotes a fill would have been given: first tick at least N seconds
    # after THIS minute ends.
    for off in OFFSETS_S:
        want = (keys + 1) * 60_000 + off * 1000
        idx = np.searchsorted(tmsc, want, side="left")
        ok = idx < len(tmsc)
        b = np.full(nm, np.nan)
        a = np.full(nm, np.nan)
        b[ok] = bid[idx[ok]]
        a[ok] = ask[idx[ok]]
        out[f"bid_{off}s"], out[f"ask_{off}s"] = b, a
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--symbol", default="XAUUSD.sc")
    ap.add_argument("--from", dest="start", required=True)
    ap.add_argument("--to", dest="end", required=True, help="exclusive")
    a = ap.parse_args()

    import MetaTrader5 as mt5

    if not mt5.initialize():
        print(f"initialize failed: {mt5.last_error()}")
        return 2
    try:
        info = mt5.symbol_info(a.symbol)
        if info is None:
            print(f"{a.symbol}: not in Market Watch; symbol_select is not called here")
            return 3
        point = float(info.point)
        outdir = ROOT / "data" / "ticks" / a.symbol.replace(".", "_")
        outdir.mkdir(parents=True, exist_ok=True)
        print(f"{a.symbol}: point {point}, window {a.start} -> {a.end} (exclusive)")
        print(f"fetched {datetime.now(timezone.utc):%Y-%m-%dT%H:%M:%SZ}, writing to {outdir}")

        day = datetime.fromisoformat(a.start).replace(tzinfo=timezone.utc)
        stop = datetime.fromisoformat(a.end).replace(tzinfo=timezone.utc)
        total_minutes = total_ticks = 0
        while day < stop:
            path = outdir / f"date={day:%Y-%m-%d}.parquet"
            if path.exists():
                day += timedelta(days=1)
                continue
            if day.weekday() == 5:                     # Saturday: nothing quotes
                day += timedelta(days=1)
                continue
            ticks = mt5.copy_ticks_range(a.symbol, day, day + timedelta(days=1), mt5.COPY_TICKS_ALL)
            if ticks is None or len(ticks) < 2:
                print(f"  {day:%Y-%m-%d}  no ticks")
                day += timedelta(days=1)
                continue
            feats = day_features(ticks, point)
            table = pa.table({k: pa.array(v) for k, v in feats.items()}, schema=SCHEMA)
            pq.write_table(table, path, compression="zstd")
            one, two = int(feats["n_one"].sum()), int(feats["n_two"].sum())
            share = 100.0 * one / max(one + two, 1)
            print(f"  {day:%Y-%m-%d}  {len(ticks):>8,} ticks  {len(feats['minute']):>5,} minutes  "
                  f"one-sided {share:5.1f}%  disagree {int(feats['flag_disagree'].sum()):>7,}")
            total_minutes += len(feats["minute"])
            total_ticks += len(ticks)
            day += timedelta(days=1)
        print(f"done: {total_ticks:,} ticks folded into {total_minutes:,} minutes")
    finally:
        mt5.shutdown()
    return 0


if __name__ == "__main__":
    sys.exit(main())
