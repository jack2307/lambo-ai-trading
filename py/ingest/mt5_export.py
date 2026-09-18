"""Export OHLC history from a running MetaTrader 5 terminal into the fd-store bar format.

READ-ONLY. This script calls `initialize`, `symbol_select`, `symbol_info`,
`copy_rates_range` and `shutdown` — nothing else. It never places, modifies or
closes an order, and it does not need AutoTrading. The terminal it talks to is
whichever one is running; on this machine that is a live Vantage account, which
is exactly why the surface is kept this small.

Why this exists
---------------
The engine will eventually trade gold and BTC through MT5, so the bars it is
tested on should be the bars that broker actually quotes — not COMEX futures
(gold) or Binance spot (BTC). Spreads, session gaps and the broker's own price
feed are part of the question a backtest asks.

Two facts about MT5 history that shape the code:

* **Server time, not UTC.** Bar times come back as epoch seconds of the
  broker's wall clock. Vantage's clock follows New York daylight saving:
  UTC+3 while New York is on DST, UTC+2 otherwise. Verified empirically on
  2026-09-12 by correlating BTCUSD.sc 15m returns against Binance BTCUSDT
  at shifts of 0..4h: +3h gave r=0.9996, every other shift ~0.
* **The terminal caps any single request at its "Max bars in chart" setting**
  (100 000 here). A wider window fails with `Invalid params` rather than
  truncating, so history is pulled in windows sized to stay under the cap and
  merged. This also caps M1 depth at ~100k bars (a few months); raise the
  setting in the terminal (Tools > Options > Charts) to go deeper.

Output
------
`<out>/<SYMBOL>-<tf>.parquet` with exactly the columns fd-store's `bars_schema`
reads by position: time (timestamp[ms, UTC]), open/high/low/close (f64),
volume (f64, nullable). `volume` holds MT5 *tick* volume — the number of price
changes in the bar — because CFD real volume is always zero. That is recorded
in the file's metadata so a reader does not mistake it for contracts.

Exit codes
----------

`0` every requested symbol/timeframe pair was written, `1` none of them were,
`2` some were and some were skipped — the skips are named on the last line
either way. Three rather than two so the CALLER decides whether a partial run
is a failure: a task that needs both H4 and D1 can treat `2` as fatal, and one
pulling a best-effort set need not. This script does not know what the files
are for.

Usage
-----
    python py/ingest/mt5_export.py --symbols XAUUSD.sc,BTCUSD.sc --timeframes M1,M15
    python py/ingest/mt5_export.py --symbols XAUUSD.sc --timeframes M15 --days 400
    python py/ingest/mt5_export.py --symbols XAUUSD.sc --timeframes H4,D1 --terminal C:\MT5-demo\terminal64.exe
"""

from __future__ import annotations

import argparse
import datetime as dt
import os
import sys
from zoneinfo import ZoneInfo

import pyarrow as pa
import pyarrow.parquet as pq

try:
    import MetaTrader5 as mt5
except ImportError:  # pragma: no cover
    sys.exit("MetaTrader5 package not installed: pip install MetaTrader5")

UTC = dt.timezone.utc
NEW_YORK = ZoneInfo("America/New_York")

# MT5 timeframe -> (store name, bar length in seconds). Window sizes keep one
# request under the 100k-bar terminal cap even for a 24/7 symbol (1440 M1/day).
TIMEFRAMES = {
    "M1": ("1m", 60),
    "M5": ("5m", 300),
    "M15": ("15m", 900),
    "H1": ("1h", 3600),
    "H4": ("4h", 14400),
    "D1": ("1d", 86400),
}
# Each window is 86,400 bars for a 24/7 symbol, which is under the terminal's
# 100k cap with room for the overlap. The numbers look like the bar lengths
# above and are not: they are `86400 / bars-per-day`.
#
# H4 and D1 are added 2026-09-18 for the higher-timeframe facts. They must be
# EXPORTED and never resampled from M15. The broker's day starts at 21:00 UTC
# in New York summer time and 22:00 outside it, so its H4 candles run
# 21/01/05/09/13/17 UTC while anything bucketed on the Unix epoch runs
# 00/04/08/12/16/20. Those are different candles with different highs and lows,
# and "yesterday's high" on the wrong anchor is a level a trader would
# recognise the name of and not the value. Verified 2026-09-18: hour 21Z holds
# exactly zero M15 bars against 332-348 in every neighbouring hour.
WINDOW_DAYS = {"M1": 60, "M5": 300, "M15": 900, "H1": 3600, "H4": 14400, "D1": 86400}

SCHEMA = pa.schema(
    [
        pa.field("time", pa.timestamp("ms", tz="UTC"), nullable=False),
        pa.field("open", pa.float64(), nullable=False),
        pa.field("high", pa.float64(), nullable=False),
        pa.field("low", pa.float64(), nullable=False),
        pa.field("close", pa.float64(), nullable=False),
        pa.field("volume", pa.float64(), nullable=True),
    ]
)


def server_offset_seconds(server_epoch: int) -> int:
    """Seconds to subtract from a server-clock epoch to get UTC.

    The clock is UTC+3 when New York observes DST and UTC+2 otherwise. To know
    which, the instant is needed in UTC — a circularity that is resolved by
    guessing +3 first: the guess is only wrong inside the hour around a DST
    transition, which falls at 07:00 UTC on a Sunday when neither market has a
    bar that matters.
    """
    guess = dt.datetime.fromtimestamp(server_epoch - 3 * 3600, tz=UTC)
    dst = guess.astimezone(NEW_YORK).dst() or dt.timedelta(0)
    return 3 * 3600 if dst else 2 * 3600


def mt5_timeframe(name: str) -> int:
    return getattr(mt5, f"TIMEFRAME_{name}")


def fetch_windowed(symbol: str, tf_name: str, days: int | None, log,
                   since_utc_ms: int | None = None) -> list[tuple[int, float, float, float, float, float]]:
    """Pull history back to `days` ago or `since_utc_ms`, in cap-sized windows.

    `since_utc_ms` is the newest bar already on disk. Given it, this asks the
    terminal for the minutes since that bar instead of walking the whole of
    history and merging the result over what was already there.

    THE WALK WAS THE WHOLE COST. `read_existing` runs after the pull, for the
    merge only, so nothing consulted the stored file to decide where to
    resume: every run re-fetched everything the terminal had. At one run an
    hour for two timeframes that was invisible. At a five-minute cadence
    across five timeframes it is the same full walk twelve times an hour,
    against the terminal that is also the price feed and also the funded
    account. Found by b5 costing the cadence rather than assuming it.

    Walks back from now one window at a time. The terminal answers an empty
    window with a single bar rather than nothing, so two consecutive windows
    of <=1 bar mark the end of history.
    """
    tf = mt5_timeframe(tf_name)
    window = dt.timedelta(days=WINDOW_DAYS[tf_name])
    # One bar of this timeframe, for the resume overlap below.
    window_step = dt.timedelta(seconds=TIMEFRAMES[tf_name][1])
    # `copy_rates_range` takes its bounds in the SERVER's clock, not UTC, and
    # this server runs UTC+3 (UTC+2 out of New York DST). Asking for a window
    # ending at `now` in UTC therefore asked for a window ending three hours in
    # the terminal's past, and every export silently stopped three hours short
    # of the market — for weeks, because a file that ends "a few hours ago"
    # looks fine until a paper book warms from it and opens with a four-hour
    # hole in its history.
    #
    # The still-forming bar is dropped later by `to_utc_rows` against real UTC,
    # so reaching to the server's now cannot pull in a partial bar.
    now = dt.datetime.now(UTC)
    offset = dt.timedelta(seconds=server_offset_seconds(int(now.timestamp()) + 3 * 3600))
    now_server = now + offset
    floor = now_server - dt.timedelta(days=days) if days else None
    if since_utc_ms is not None:
        # One bar of overlap, so a bar that was still forming when it was last
        # stored is re-pulled complete rather than left half-written.
        resume = dt.datetime.fromtimestamp(since_utc_ms / 1000.0, UTC) + offset - window_step
        floor = max(floor, resume) if floor is not None else resume
    rows: dict[int, tuple] = {}
    empty_windows = 0
    end = now_server
    while True:
        # CLAMPED TO THE FLOOR, which it was not until 2026-09-18. `start` was
        # always `end - window`, so `--days 3` on M5 still asked the terminal
        # for 300 days - about 86,400 bars - on the first call and only then
        # stopped. The knob bounded how far back the walk went and not how much
        # each step asked for, and on a short run the first step is the only
        # one there is. Found by b5.
        start = end - window
        if floor is not None:
            if end <= floor:
                break
            if start < floor:
                start = floor
        rates = mt5.copy_rates_range(symbol, tf, start, end)
        if rates is None:
            code, text = mt5.last_error()
            raise RuntimeError(f"{symbol} {tf_name} copy_rates_range({start:%Y-%m-%d}..{end:%Y-%m-%d}) failed: {code} {text}")
        n = len(rates)
        log(f"  {symbol} {tf_name} {start:%Y-%m-%d}..{end:%Y-%m-%d}: {n} bars")
        if n <= 1:
            empty_windows += 1
            if empty_windows >= 2:
                break
        else:
            empty_windows = 0
            for r in rates:
                server_epoch = int(r["time"])
                # The terminal returns the same bar in overlapping windows; keying
                # by server time makes the merge idempotent.
                rows[server_epoch] = (
                    server_epoch,
                    float(r["open"]),
                    float(r["high"]),
                    float(r["low"]),
                    float(r["close"]),
                    float(r["tick_volume"]),
                )
        end = start
    return [rows[k] for k in sorted(rows)]


def to_utc_rows(rows, step_seconds: int, now_utc_ms: int):
    """Convert server-clock rows to UTC ms and drop the bar still forming."""
    out = []
    for server_epoch, o, h, l, c, v in rows:
        utc_ms = (server_epoch - server_offset_seconds(server_epoch)) * 1000
        if utc_ms + step_seconds * 1000 > now_utc_ms:
            continue  # incomplete: its close is not the close
        out.append((utc_ms, o, h, l, c, v))
    return out


def read_existing(path: str) -> dict[int, tuple]:
    if not os.path.exists(path):
        return {}
    table = pq.read_table(path)
    times = table.column("time").cast(pa.int64()).to_pylist()
    cols = [table.column(name).to_pylist() for name in ("open", "high", "low", "close", "volume")]
    return {t: (t, *vals) for t, *vals in zip(times, *cols)}


def write_bars(path: str, rows: list[tuple], metadata: dict[str, str]) -> None:
    columns = list(zip(*rows)) if rows else [[] for _ in range(6)]
    table = pa.table(
        {
            "time": pa.array(columns[0], pa.timestamp("ms", tz="UTC")),
            "open": pa.array(columns[1], pa.float64()),
            "high": pa.array(columns[2], pa.float64()),
            "low": pa.array(columns[3], pa.float64()),
            "close": pa.array(columns[4], pa.float64()),
            "volume": pa.array(columns[5], pa.float64()),
        },
        schema=SCHEMA.with_metadata({k: v for k, v in metadata.items()}),
    )
    os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
    tmp = path + ".tmp"
    pq.write_table(table, tmp, compression="zstd", compression_level=3, row_group_size=32_768)
    os.replace(tmp, path)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--symbols", default="XAUUSD.sc,BTCUSD.sc", help="broker symbols, comma-separated")
    # The help listed M1,M5,M15,H1 and the table has carried H4 and D1 since
    # 2026-09-18. A help string that omits an option is the same defect as a
    # comment that has stopped being true, and it is read by the person
    # deciding what to type.
    ap.add_argument("--timeframes", default="M1,M15",
                    help="MT5 timeframes, comma-separated: " + ",".join(TIMEFRAMES))
    ap.add_argument("--days", type=int, default=None, help="only this many days back (default: all the terminal has)")
    ap.add_argument("--out", default=os.path.join(os.path.dirname(__file__), "..", "..", "data", "bars"))
    ap.add_argument("--keep-suffix", action="store_true", help="name files by the full broker symbol (XAUUSD.sc) instead of stripping .sc")
    ap.add_argument("--quiet", action="store_true")
    # Resume from the newest bar already stored instead of walking all of
    # history. Off by default so a hand-run export still backfills; the
    # scheduled job passes it, because at a five-minute cadence the difference
    # is minutes of data against every bar the terminal holds.
    #
    # A first run, or a missing file, backfills exactly as before: there is no
    # stored bar to resume from.
    ap.add_argument("--since-stored", action="store_true",
                    help="pull only bars newer than the newest one already in the parquet "
                         "(a first run still backfills everything)")
    # WHICH terminal, on a machine running more than one.
    #
    # The VPS runs two: C:\MT5-cent holds the funded account and sends the
    # orders, C:\MT5-demo is there for prices. Point this at the DEMO one.
    # Both quote the same instrument from the same server, this script is
    # read-only either way, and attaching to the account that trades in order
    # to copy some candles is a risk taken for no gain.
    #
    # Without it MT5 attaches to whichever terminal is already running, which
    # is right on a single-terminal desktop and a coin toss on the VPS. The
    # terminal cannot be launched from SSH at all - it lives in the owner's RDP
    # session - so this attaches to a terminal that is already up, exactly as
    # `mt5_bars.py --terminal` does.
    ap.add_argument("--terminal", default=None,
                    help="path to terminal64.exe when more than one is running "
                         "(on the VPS: the DEMO terminal, not the one holding the funded account)")
    args = ap.parse_args()
    log = (lambda *_: None) if args.quiet else (lambda *a: print(*a, flush=True))

    if not (mt5.initialize(path=args.terminal) if args.terminal else mt5.initialize()):
        sys.exit(f"MT5 initialize failed: {mt5.last_error()}")
    try:
        term = mt5.terminal_info()
        acct = mt5.account_info()
        log(f"terminal: {term.name} build {term.build} | account {acct.login} {acct.server} | trade_allowed={term.trade_allowed}")
        now_utc_ms = int(dt.datetime.now(UTC).timestamp() * 1000)
        exported_at = dt.datetime.now(UTC).strftime("%Y-%m-%dT%H:%M:%SZ")

        timeframes = [t.strip().upper() for t in args.timeframes.split(",") if t.strip()]
        # Every (symbol, timeframe) pair asked for, and what became of it. The
        # exit code is computed from these and not from whether the loop ran.
        written: list[str] = []
        skipped: list[str] = []

        for symbol in [s.strip() for s in args.symbols.split(",") if s.strip()]:
            if not mt5.symbol_select(symbol, True):
                log(f"{symbol}: symbol_select failed {mt5.last_error()} — skipped")
                skipped += [f"{symbol} {tf} (symbol_select failed)" for tf in timeframes]
                continue
            info = mt5.symbol_info(symbol)
            file_symbol = symbol if args.keep_suffix else symbol.split(".")[0]
            for tf_name in timeframes:
                if tf_name not in TIMEFRAMES:
                    log(f"{symbol} {tf_name}: unsupported timeframe — skipped")
                    skipped.append(f"{symbol} {tf_name} (unsupported timeframe)")
                    continue
                store_tf, step_seconds = TIMEFRAMES[tf_name]
                path = os.path.join(args.out, f"{file_symbol}-{store_tf}.parquet")

                # Read BEFORE the pull now, so the newest stored bar can bound
                # it. It was read after, for the merge alone.
                merged = read_existing(path)
                before = len(merged)
                since = max(merged) if (merged and args.since_stored) else None
                if since is not None:
                    log(f"  {symbol} {tf_name}: resuming from {dt.datetime.fromtimestamp(since / 1000, tz=UTC)}")
                fresh = to_utc_rows(fetch_windowed(symbol, tf_name, args.days, log, since), step_seconds, now_utc_ms)
                for row in fresh:
                    merged[row[0]] = row  # a re-pulled bar replaces the stored one
                rows = [merged[k] for k in sorted(merged)]
                if not rows:
                    # Nothing pulled and nothing stored. Writing the empty file
                    # would leave a parquet that every reader then refuses, and
                    # a run that produced it must not report success.
                    log(f"  {symbol} {tf_name}: no bars from the terminal and none on disk — skipped")
                    skipped.append(f"{symbol} {tf_name} (no bars)")
                    continue

                # How far behind the market this file now ends. Printed every
                # run because the three-hour lag above survived three separate
                # sightings: each time the file looked plausible on its own, and
                # only a book warming from it made the hole visible.
                if fresh:
                    behind_min = (now_utc_ms - fresh[-1][0]) / 60000.0
                    log(f"  {symbol} {tf_name}: last bar is {behind_min:.0f} min behind now"
                        + ("  <-- CHECK THIS" if behind_min > 90 else ""))
                # Gaps larger than a weekend are worth knowing about.
                gaps = [(a, b) for a, b in zip(rows, rows[1:]) if b[0] - a[0] > 3 * 86_400_000]
                metadata = {
                    "source": "mt5",
                    "broker_symbol": symbol,
                    "server": acct.server,
                    "timeframe": tf_name,
                    "clock_rule": "server=NY-DST anchored (UTC+3 summer, UTC+2 winter); stored as UTC",
                    "volume": "tick_volume (price changes per bar), not contracts",
                    "digits": str(info.digits),
                    "spread_points_at_export": str(info.spread),
                    "contract_size": str(info.trade_contract_size),
                    "exported_at": exported_at,
                }
                write_bars(path, rows, metadata)
                written.append(f"{symbol} {tf_name}")
                first = dt.datetime.fromtimestamp(rows[0][0] / 1000, tz=UTC) if rows else None
                last = dt.datetime.fromtimestamp(rows[-1][0] / 1000, tz=UTC) if rows else None
                log(
                    f"{symbol} {tf_name} -> {path}: {len(rows)} bars "
                    f"({len(fresh)} pulled, {len(rows) - before:+d} new) | {first} .. {last} | {len(gaps)} gaps > 3d"
                )
                for a, b in gaps[:5]:
                    log(f"    gap {dt.datetime.fromtimestamp(a[0]/1000, tz=UTC)} -> {dt.datetime.fromtimestamp(b[0]/1000, tz=UTC)}")
    finally:
        mt5.shutdown()

    # THE EXIT CODE IS THE OUTCOME, because nothing here is read by a human.
    #
    # Until 2026-09-18 every failure above was a `continue` and the function
    # returned 0 regardless, so a run that exported NOTHING - a bad symbol, a
    # timeframe this script does not know, a terminal that answered with no
    # bars - reported success. On a desktop that is a log line somebody reads.
    # As an hourly scheduled task it is the failure shape this desk has spent a
    # week finding: Last Run Result 0x0, every hour, forever, while `data/bars`
    # stays empty and the route goes on correctly answering "not exported yet".
    # Found by b5 while building that very task.
    #
    # Three codes rather than two, because "some of it worked" is a real state
    # and the caller - not this script - should decide whether it is a failure.
    # A scheduled task that must have every file can treat 2 as fatal; one
    # exporting a best-effort set can accept it. Collapsing them would force
    # that judgement here, where nothing knows what the files are for.
    if skipped:
        log(f"skipped: {', '.join(skipped)}")
    if not written:
        log("nothing was exported")
        return 1
    if skipped:
        log(f"exported {len(written)} of {len(written) + len(skipped)} requested")
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
