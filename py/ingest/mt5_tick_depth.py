"""How far back does the broker's tick history actually go?

    python py/ingest/mt5_tick_depth.py --symbol=XAUUSD.sc

Read-only: `initialize`, `symbol_info`, `copy_ticks_range`, `shutdown`. Never an
order, and `symbol_select` is not called — a symbol that answers nothing is
reported and skipped, because adding one to Market Watch is the operator's
decision on this machine's live account.

Asked before any hypothesis is written, because the last registration nearly
went to a market where the two feeds did not overlap by a single minute. One
hour is sampled at each probe date rather than a whole day: the question is
whether ticks exist at that depth and whether they carry both sides, not what
they say.
"""

from __future__ import annotations

import argparse
import sys
from datetime import datetime, timedelta, timezone


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--symbol", default="XAUUSD.sc")
    ap.add_argument("--probes", type=int, nargs="+",
                    default=[7, 30, 60, 90, 180, 270, 365, 540, 730])
    a = ap.parse_args()

    import MetaTrader5 as mt5

    if not mt5.initialize():
        print(f"initialize failed: {mt5.last_error()}")
        return 2
    try:
        info = mt5.symbol_info(a.symbol)
        if info is None:
            print(f"{a.symbol}: symbol_info returned nothing; not in Market Watch")
            return 3
        print(f"{a.symbol}: digits {info.digits}, point {info.point}, "
              f"spread now {info.spread} points")
        print(f"probing one hour of ticks at each depth (13:00-14:00 UTC, "
              f"stepped back to the nearest weekday)\n")
        print(f"   {'days back':>9s} {'date':>12s} {'ticks':>9s} {'both sides':>10s} "
              f"{'ask-only':>9s} {'bid-only':>9s} {'ms stamps':>10s}")

        for days in a.probes:
            day = datetime.now(timezone.utc) - timedelta(days=days)
            while day.weekday() >= 5:
                day -= timedelta(days=1)
            lo = day.replace(hour=13, minute=0, second=0, microsecond=0)
            hi = lo + timedelta(hours=1)
            ticks = mt5.copy_ticks_range(a.symbol, lo, hi, mt5.COPY_TICKS_ALL)
            if ticks is None or len(ticks) == 0:
                print(f"   {days:9d} {lo:%Y-%m-%d} {0:9d}   none")
                continue
            n = len(ticks)
            bid, ask = ticks["bid"], ticks["ask"]
            db, da = bid[1:] - bid[:-1], ask[1:] - ask[:-1]
            eps = info.point / 2
            moved_b, moved_a = abs(db) > eps, abs(da) > eps
            both = int((moved_b & moved_a).sum())
            ask_only = int((moved_a & ~moved_b).sum())
            bid_only = int((moved_b & ~moved_a).sum())
            ms = "yes" if "time_msc" in ticks.dtype.names else "no"
            print(f"   {days:9d} {lo:%Y-%m-%d} {n:9,d} {both:10,d} {ask_only:9,d} "
                  f"{bid_only:9,d} {ms:>10s}")
    finally:
        mt5.shutdown()
    return 0


if __name__ == "__main__":
    sys.exit(main())
