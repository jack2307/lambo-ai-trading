"""Push closed MT5 bars to the paper loop. Read-only towards the terminal.

    python py/live/mt5_bars.py --symbol=XAUUSD.sc --market=xauusd --tf=M15 --api=http://127.0.0.1:8138

Every `--poll` seconds this asks the terminal for its last few bars
(`copy_rates_from_pos`) and, when the newest CLOSED bar is one it has not
sent, POSTs it to `<api>/api/paper/bar` as UTC milliseconds. MT5 calls used:
`initialize`, `symbol_info`, `copy_rates_from_pos`, `shutdown` — nothing
that places, modifies or closes an order; the terminal on this machine is a
live account and this script must stay a reader (see `mt5_export.py`).

Why the reader lives in Python: the MetaTrader5 package is the only feed for
the broker's own gold bars, and it exists only for Python. The Rust side
never links to the terminal; it receives bars over HTTP and would accept them
from any source that speaks the same shape.

The bar's `time` from the terminal is the server clock (New York + 7 hours,
i.e. UTC+3 in US summer, UTC+2 in winter); it is converted the way the export
does, through `server_offset_seconds`. A bar is "closed" when the terminal
reports a newer bar after it, so the last element of `copy_rates_from_pos`
(the forming bar) is never sent.

On startup the script also sends the last `--warm` closed bars so the loop can
rebuild its indicators; the loop ignores bars it already has.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import sys
import time
import urllib.request
from zoneinfo import ZoneInfo

UTC = dt.timezone.utc
NEW_YORK = ZoneInfo("America/New_York")


def server_offset_seconds(server_epoch: int) -> int:
    guess = dt.datetime.fromtimestamp(server_epoch - 3 * 3600, tz=UTC)
    dst = guess.astimezone(NEW_YORK).dst() or dt.timedelta(0)
    return 3 * 3600 if dst else 2 * 3600


def to_utc_ms(server_epoch: int) -> int:
    return (int(server_epoch) - server_offset_seconds(int(server_epoch))) * 1000


def post(api: str, payload: dict) -> tuple[int, str]:
    body = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(f"{api}/api/paper/bar", data=body, headers={"Content-Type": "application/json"}, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=10) as r:
            return r.status, r.read().decode("utf-8", "replace")[:200]
    except urllib.error.HTTPError as e:  # type: ignore[attr-defined]
        return e.code, e.read().decode("utf-8", "replace")[:200]
    except Exception as e:  # noqa: BLE001
        return 0, str(e)[:200]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--symbol", default="XAUUSD.sc")
    ap.add_argument("--market", default="xauusd", help="the market id fd-api knows the symbol as")
    ap.add_argument("--tf", default="M15")
    ap.add_argument("--api", default="http://127.0.0.1:8138")
    ap.add_argument("--poll", type=float, default=5.0, help="seconds between reads")
    ap.add_argument("--warm", type=int, default=400, help="closed bars to send on startup")
    ap.add_argument("--once", action="store_true", help="send the warm-up bars and exit")
    args = ap.parse_args()

    try:
        import MetaTrader5 as mt5  # type: ignore
    except ImportError:
        sys.exit("MetaTrader5 package not installed")
    if not mt5.initialize():
        sys.exit(f"initialize failed: {mt5.last_error()}")
    tf = getattr(mt5, f"TIMEFRAME_{args.tf}")
    try:
        info = mt5.symbol_info(args.symbol)
        if info is None:
            sys.exit(f"no symbol {args.symbol}")
        print(f"{args.symbol} {args.tf}: digits {info.digits}, spread now {info.spread * info.point:.2f}; posting to {args.api} as market {args.market}", flush=True)
        sent_upto: int | None = None

        def closed_bars(n: int):
            rates = mt5.copy_rates_from_pos(args.symbol, tf, 0, n + 1)
            if rates is None or len(rates) < 2:
                return []
            rows = list(rates)[:-1]  # the last one is forming
            return rows

        def send(row) -> bool:
            nonlocal sent_upto
            t = to_utc_ms(row["time"])
            if sent_upto is not None and t <= sent_upto:
                return False
            payload = {
                "market": args.market,
                "tf": args.tf.lower().replace("m", "m") if args.tf.startswith("M") else args.tf.lower(),
                "bar": {
                    "time": t,
                    "open": float(row["open"]),
                    "high": float(row["high"]),
                    "low": float(row["low"]),
                    "close": float(row["close"]),
                    "volume": float(row["tick_volume"]),
                },
            }
            # "15m" is the store's spelling; the terminal's is "M15".
            payload["tf"] = args.tf[1:] + "m" if args.tf.startswith("M") and args.tf[1:].isdigit() else args.tf.lower()
            status, text = post(args.api, payload)
            stamp = dt.datetime.fromtimestamp(t / 1000, tz=UTC).strftime("%Y-%m-%d %H:%MZ")
            print(f"{stamp} close {payload['bar']['close']:.2f} -> {status} {text}", flush=True)
            if status == 200:
                sent_upto = t
                return True
            return False

        for row in closed_bars(args.warm):
            send(row)
        if args.once:
            return 0
        while True:
            time.sleep(args.poll)
            for row in closed_bars(3):
                send(row)
    except KeyboardInterrupt:
        return 0
    finally:
        mt5.shutdown()


if __name__ == "__main__":
    sys.exit(main())
