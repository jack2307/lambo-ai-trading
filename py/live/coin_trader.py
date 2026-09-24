"""A seeded coin trading its own paper book, with no model above it.

WHY THIS IS NOT `ai_trader.py --model=coin`. The coin control inside
`ai_trader.py` posts a mirrored side only on the bars its MODEL chose, and sits
out every bar the model sat out — 242 of them on `ai-xau-ds-ctx` at the time of
writing. That makes it a shadow of a model, not a strategy, and it is why "run
the good coin book" is not a thing that can be done: a coin book has no entry
rule of its own to run.

This does have one. It draws its own bars from a seeded stream at a fixed rate,
so it is a standalone random-entry book on the live feed.

WHAT IT IS FOR, which is not what it looks like. Every percentile this desk has
ever published is read against `RandomEntry`, the backtest's random-entry null,
and that null has only ever been measured inside the backtest against stored
bars at a charged spread of 0.28. Nobody has checked it against the live feed,
where the logger's measured median spread is 0.220. The prediction, written into
`docs/hypotheses/2026-09-24-live-coin-parity.md` before this file existed, is a
pooled profit factor near 0.881 against the backtest null's 0.858 — and NOT near
1.00. If it comes out near 1.00, the null is wrong and so is a great deal of the
record.

THE RULE IS MATCHED TO THE NULL, NOT CHOSEN. Entry rate 0.04 per closed bar is
the top of `RandomEntry`'s own grid (`crates/fd-backtest/src/control.rs:73`);
the stop is 1.5 x ATR14, the value `RandomEntry::default_params()` carries; the
target comes from the config's `reward_risk`, as it does for every other book.
Changing any of them to make the number nicer would break the comparison this
exists to make.

It calls no model, so it costs nothing to run.

Usage:
    python py/live/coin_trader.py --run=coin-live-101 --seed=101 \
        --api=http://127.0.0.1:8138 --market=xauusd --tf=15m
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import random
import sys
import time
import urllib.error
import urllib.request

# The null's own values. Not tuning knobs - see the module docstring.
ENTRY_RATE = 0.04
STOP_ATR = 1.5
ATR_PERIOD = 14


def get_json(url: str, timeout: float = 20.0) -> dict:
    with urllib.request.urlopen(url, timeout=timeout) as r:
        return json.loads(r.read().decode("utf-8"))


def post_json(url: str, body: dict, timeout: float = 20.0) -> dict:
    data = json.dumps(body).encode("utf-8")
    req = urllib.request.Request(url, data=data, headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return json.loads(r.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        detail = e.read().decode("utf-8", "replace")[:300]
        return {"accepted": False, "error": f"HTTP {e.code}: {detail}"}


def atr(bars: list[dict], period: int) -> float | None:
    """Wilder's ATR over the last `period` true ranges, or None.

    `None`, not zero: a book that cannot size a trade must not post one sized
    on a fabricated stop. The caller skips the bar and says so.
    """
    if len(bars) < period + 1:
        return None
    trs = []
    for i in range(len(bars) - period, len(bars)):
        prev_close = float(bars[i - 1]["close"])
        high, low = float(bars[i]["high"]), float(bars[i]["low"])
        trs.append(max(high - low, abs(high - prev_close), abs(low - prev_close)))
    if not trs:
        return None
    value = sum(trs) / len(trs)
    return value if value > 0 else None


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", required=True, help="the paper book to post into; must be strategy=external")
    ap.add_argument("--seed", type=int, required=True, help="the coin's seed, so the book replays exactly")
    ap.add_argument("--api", default="http://127.0.0.1:8138")
    ap.add_argument("--market", default="xauusd")
    ap.add_argument("--tf", default="15m")
    ap.add_argument("--poll", type=float, default=20.0, help="seconds between status polls")
    ap.add_argument("--dry-run", action="store_true", help="decide and print, post nothing")
    args = ap.parse_args()

    # Two independent streams from one seed: whether this bar trades, and which
    # side. Splitting them means the side sequence does not shift when the rate
    # changes, so a rate change stays a rate change rather than silently
    # becoming a different coin as well.
    take = random.Random(args.seed)
    coin = random.Random(args.seed + 1_000_000)

    print(
        f"coin_trader run={args.run} seed={args.seed} rate={ENTRY_RATE} "
        f"stop={STOP_ATR}xATR{ATR_PERIOD} market={args.market}:{args.tf} "
        f"dry_run={args.dry_run}",
        flush=True,
    )

    last_seen: int | None = None
    posted = skipped = no_atr = 0

    while True:
        try:
            status = get_json(f"{args.api}/api/paper/status")
            run = next((r for r in status.get("runs", []) if r.get("id") == args.run), None)
            if run is None:
                print(f"{dt.datetime.now():%H:%M:%S}  run `{args.run}` is not on the desk", flush=True)
                time.sleep(args.poll)
                continue

            bar_time = run.get("last_bar_time")
            if bar_time is None or bar_time == last_seen:
                time.sleep(args.poll)
                continue

            # A new closed bar. Draw for THIS bar before anything can fail, so
            # a network error later does not consume a draw twice or skip one:
            # the streams must advance exactly once per bar or the seed stops
            # being a replay.
            last_seen = bar_time
            trades_this_bar = take.random() < ENTRY_RATE
            side = "LONG" if coin.random() < 0.5 else "SHORT"
            stamp = dt.datetime.utcfromtimestamp(bar_time / 1000).strftime("%m-%d %H:%MZ")

            if not trades_this_bar:
                skipped += 1
                continue

            # `tf` and `n`, NOT `interval` and `limit`. Checked against
            # `MarketQuery` in routes.rs rather than guessed: the first version
            # of this file sent `interval=15m&limit=200`, and because axum
            # ignores unknown query fields it got the CONFIG's default
            # timeframe and the whole 77,000-bar file, silently, with no error
            # anywhere. `n` returns the newest bars, never the oldest.
            bars = get_json(f"{args.api}/api/chart/bars?market={args.market}&tf={args.tf}&n=200")
            series = bars.get("bars") or []
            a = atr(series, ATR_PERIOD)
            close = float(series[-1]["close"]) if series else None
            if a is None or close is None:
                no_atr += 1
                print(f"{stamp}  {side}  SKIPPED: no ATR{ATR_PERIOD} (bars={len(series)})", flush=True)
                continue

            distance = STOP_ATR * a
            stop = close - distance if side == "LONG" else close + distance

            if args.dry_run:
                print(f"{stamp}  {side}  close {close:.2f} stop {stop:.2f} (ATR {a:.3f})  [dry run]", flush=True)
                continue

            # No target: the engine derives one from `reward_risk`, the same way
            # it does for every other book. Posting one here would make this
            # book's geometry a choice of mine rather than the config's.
            reply = post_json(
                f"{args.api}/api/paper/intent",
                dict(run=args.run, side=side, bar_time=bar_time, stop=stop,
                     reason=f"coin seed {args.seed}: {side}", decider="coin"),
            )
            if reply.get("accepted"):
                posted += 1
                print(f"{stamp}  {side}  close {close:.2f} stop {stop:.2f}  posted "
                      f"(posted {posted}, sat out {skipped})", flush=True)
            else:
                print(f"{stamp}  {side}  REFUSED: {reply.get('error') or reply}", flush=True)

        except KeyboardInterrupt:
            print(f"stopping: posted {posted}, sat out {skipped}, no ATR {no_atr}", flush=True)
            return 0
        except Exception as e:  # noqa: BLE001 - a poller must outlive any one failure
            print(f"{dt.datetime.now():%H:%M:%S}  error: {type(e).__name__}: {e}", flush=True)
            time.sleep(args.poll)


if __name__ == "__main__":
    sys.exit(main())
