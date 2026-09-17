"""Log the Vantage spread by the hour, read-only, so a thin hour can be priced.

    python py/live/spread_log.py --symbols XAUUSD.sc XAGUSD.sc --interval=20

`2026-09-15-pair-residual` closed with one question it could not answer. The
whole of its surviving effect sits between 17:00 and 02:00 UTC, where the
15-minute range is a third of its 13:00 value, and the only spread number this
repository owns is a three-day median blended across every hour of the day.
Pricing the thinnest hours in the tape at an all-hours cost is the difference
between a real overnight dislocation and an artefact, and nothing in the data
can settle it — only a week of quotes can.

So this writes one line per sample to `data/spreads/<symbol>.csv`:

    time_utc,bid,ask,spread,mid,spread_bp

and nothing else. It never aggregates, never decides, and never trades. The
buckets are computed afterwards from the file, by whoever asks the question.

MT5 calls used: `initialize`, `symbol_info`, `symbol_info_tick`, `shutdown`.
**Nothing here places, modifies or closes an order, and nothing here may ever
be made to.** The terminal on this machine is a REAL Vantage live account.

`symbol_select` is deliberately NOT called. Adding a symbol to Market Watch
would make an unreadable symbol readable, and it is not on the approved
read-only list for this account. A symbol that returns nothing is reported as
missing, once, and then skipped — the operator can add it by hand in the
terminal if they want it logged.

The log is quiet on purpose: a line at startup, a line when a symbol's
readability changes, and one summary line an hour. A week of samples at twenty
seconds is about 30,000 rows a symbol, which is a few megabytes.
"""

from __future__ import annotations

import argparse
import csv
import datetime as dt
import os
import sys
import time

# Output is UTF-8, and undisplayable characters are replaced rather than fatal.
#
# Windows hands a redirected stdout the cp1252 codec, and the text below is not
# ours - it is whatever a model wrote in its reasoning, or whatever a broker put
# in a comment. On 2026-09-17 a single arrow in a DeepSeek stand-aside raised
# UnicodeEncodeError out of the print, out of main(), and killed the trader for
# five bars. `errors="replace"` is the important half: encoding can then never
# be the thing that stops a process, whatever arrives.
for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")
    except (AttributeError, ValueError):  # not a TextIOWrapper; nothing to do
        pass

UTC = dt.timezone.utc
ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT = os.path.join(ROOT, "data", "spreads")
HEADER = ["time_utc", "bid", "ask", "spread", "mid", "spread_bp"]


def writer_for(symbol: str):
    """An open append handle and its csv writer, with the header written once."""
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, f"{symbol.replace('.', '_')}.csv")
    fresh = not os.path.exists(path) or os.path.getsize(path) == 0
    fh = open(path, "a", encoding="utf-8", newline="")
    w = csv.writer(fh, lineterminator="\n")
    if fresh:
        w.writerow(HEADER)
        fh.flush()
    return path, fh, w


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--symbols", nargs="+", default=["XAUUSD.sc", "BTCUSD.sc", "XAGUSD.sc"])
    ap.add_argument("--interval", type=float, default=20.0, help="seconds between samples")
    ap.add_argument("--hours", type=float, default=0.0, help="stop after this many hours; 0 runs until killed")
    args = ap.parse_args()

    try:
        import MetaTrader5 as mt5  # type: ignore
    except ImportError:
        sys.exit("MetaTrader5 package not installed")
    if not mt5.initialize():
        sys.exit(f"initialize failed: {mt5.last_error()}")

    sinks, missing = {}, set()
    try:
        for s in args.symbols:
            if mt5.symbol_info(s) is None:
                # Not in Market Watch, or not offered. Reported and skipped;
                # see the module docstring on why symbol_select is not called.
                print(f"{s}: no symbol_info — not in Market Watch. Add it by hand in the "
                      f"terminal to have it logged; this process will not select it.", flush=True)
                missing.add(s)
                continue
            path, fh, w = writer_for(s)
            sinks[s] = (fh, w)
            print(f"{s}: logging to {os.path.relpath(path, ROOT)}", flush=True)
        if not sinks:
            sys.exit("no readable symbols; nothing to log")

        deadline = time.monotonic() + args.hours * 3600 if args.hours else None
        counts = {s: 0 for s in sinks}
        last_report = time.monotonic()
        while True:
            now = dt.datetime.now(UTC)
            for s, (fh, w) in sinks.items():
                t = mt5.symbol_info_tick(s)
                if t is None or not t.bid or not t.ask:
                    continue
                spread = t.ask - t.bid
                mid = (t.ask + t.bid) / 2.0
                w.writerow([now.strftime("%Y-%m-%dT%H:%M:%SZ"), f"{t.bid:.5f}", f"{t.ask:.5f}",
                            f"{spread:.5f}", f"{mid:.5f}", f"{1e4 * spread / mid:.4f}"])
                counts[s] += 1
            for fh, _ in sinks.values():
                fh.flush()
            if time.monotonic() - last_report >= 3600:
                part = "  ".join(f"{s} {counts[s]}" for s in sinks)
                print(f"{now.strftime('%Y-%m-%d %H:%MZ')}  samples so far: {part}", flush=True)
                last_report = time.monotonic()
            if deadline and time.monotonic() >= deadline:
                return 0
            time.sleep(args.interval)
    except KeyboardInterrupt:
        return 0
    finally:
        for fh, _ in sinks.values():
            fh.close()
        mt5.shutdown()


if __name__ == "__main__":
    sys.exit(main())
