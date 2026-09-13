"""Measure the quoted spread around the daily break from MT5 tick history.

Read-only: `initialize`, `symbol_info`, `copy_ticks_range`. Never an order.

    python py/ingest/mt5_spread_probe.py --symbol=XAUUSD.sc --days=10

The server clock runs on New York time plus seven hours (the NY-DST clock
measured in `mt5_export.py`), so 17:00 New York is 00:00 server time. The
probe buckets ticks by minute of the server day around the close (23:30 to
00:00) and the reopen (01:00 to 01:20) and prints spread quantiles in price
units, so a hold that enters or exits inside the break can be priced.
"""

from __future__ import annotations

import argparse
import statistics
import sys
from collections import defaultdict
from datetime import datetime, timedelta, timezone


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--symbol", default="XAUUSD.sc")
    ap.add_argument("--days", type=int, default=10)
    args = ap.parse_args()

    try:
        import MetaTrader5 as mt5  # type: ignore
    except ImportError:
        sys.exit("MetaTrader5 package not installed")
    if not mt5.initialize():
        sys.exit(f"initialize failed: {mt5.last_error()}")
    try:
        info = mt5.symbol_info(args.symbol)
        if info is None:
            sys.exit(f"no symbol {args.symbol}")
        print(f"{args.symbol}: digits {info.digits}, quoted spread now {info.spread} points = {info.spread * info.point:.2f}")
        now = datetime.now(timezone.utc)
        start = now - timedelta(days=args.days)
        ticks = mt5.copy_ticks_range(args.symbol, start, now, mt5.COPY_TICKS_INFO)
        if ticks is None or len(ticks) == 0:
            sys.exit(f"no ticks: {mt5.last_error()}")
        print(f"{len(ticks)} ticks from {start:%Y-%m-%d} to {now:%Y-%m-%d} (server-time stamps)")
        # Bucket by minute of the server day. The stamps MT5 returns are the
        # server's clock labelled as UTC; we only need the minute of day.
        buckets: dict[int, list[float]] = defaultdict(list)
        for t in ticks:
            bid, ask = float(t["bid"]), float(t["ask"])
            if bid <= 0 or ask <= 0:
                continue
            minute = (int(t["time"]) % 86_400) // 60
            buckets[minute].append(ask - bid)
        windows = [
            ("close  23:00-23:30 srv (16:00-16:30 NY)", range(23 * 60, 23 * 60 + 30)),
            ("into   23:30-23:59 srv (16:30-16:59 NY)", range(23 * 60 + 30, 24 * 60)),
            ("reopen 01:00-01:05 srv (18:00-18:05 NY)", range(1 * 60, 1 * 60 + 5)),
            ("reopen 01:05-01:15 srv (18:05-18:15 NY)", range(1 * 60 + 5, 1 * 60 + 15)),
            ("after  01:15-01:30 srv (18:15-18:30 NY)", range(1 * 60 + 15, 1 * 60 + 30)),
            ("asia   02:00-04:00 srv (19:00-21:00 NY)", range(2 * 60, 4 * 60)),
            ("london 10:00-12:00 srv (03:00-05:00 NY)", range(10 * 60, 12 * 60)),
            ("ny     15:00-17:00 srv (08:00-10:00 NY)", range(15 * 60, 17 * 60)),
        ]
        print(f"{'window':44s} {'ticks':>7s} {'p50':>7s} {'p90':>7s} {'max':>7s}")
        for name, minutes in windows:
            values = [v for m in minutes for v in buckets.get(m, [])]
            if not values:
                print(f"{name:44s} {'0':>7s}")
                continue
            values.sort()
            q = lambda p: values[min(len(values) - 1, int(p * len(values)))]
            print(f"{name:44s} {len(values):7d} {statistics.median(values):7.2f} {q(0.9):7.2f} {values[-1]:7.2f}")
        # Per-minute detail across the reopen, for the record.
        print("reopen minute by minute (server 01:00-01:20):")
        for m in range(60, 80):
            values = sorted(buckets.get(m, []))
            if values:
                print(f"  01:{m - 60:02d}  ticks {len(values):5d}  p50 {statistics.median(values):6.2f}  p90 {values[int(0.9 * (len(values) - 1))]:6.2f}")
    finally:
        mt5.shutdown()
    return 0


if __name__ == "__main__":
    sys.exit(main())
