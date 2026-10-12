r"""Export OHLC history from a running MetaTrader 5 terminal into the fd-store bar format.

(The `r` matters: the Usage block below contains Windows paths. Without it
`--terminal C:\MT5-demo\terminal64.exe` renders as `C:\MT5-demo<TAB>erminal64.exe`
because `\t` is a valid escape, and `\M` raises a DeprecationWarning on every
import. The one line in this file a person copies by hand was the one line that
was wrong.)

READ-ONLY. This script calls `initialize`, `symbol_info`, `copy_rates_range`,
`shutdown` and — only when `--allow-symbol-select` is passed AND the symbol
reads back `visible == False` — `symbol_select`. Nothing else. It never places,
modifies or closes an order, and it does not need AutoTrading. The terminal it
talks to is whichever one is running; on this machine that is a live Vantage
account, which is exactly why the surface is kept this small.

`symbol_select` sits behind a flag because `py/live/spread_log.py` states the
standing rule for this account in its own doc comment: *"`symbol_select` is
deliberately NOT called. Adding a symbol to Market Watch would make an
unreadable symbol readable, and it is not on the approved read-only list for
this account."* Until 2026-10-11 this script called it UNCONDITIONALLY on every
symbol, so two scripts in one tree contradicted each other about one MT5 call.
The rule wins, and the default is now not to call it: a symbol already in Market
Watch never needs it, and one that is not is REPORTED and SKIPPED rather than
quietly added. The decision is `symbol_select_plan`, which is a pure function so
that it can be tested without a terminal.

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

# Output is UTF-8, and undisplayable characters are replaced rather than fatal.
# Lifted verbatim in intent from `py/live/spread_log.py`, which carries the
# incident: Windows hands a redirected stdout the cp1252 codec, and on
# 2026-09-17 a single arrow in a model's text raised UnicodeEncodeError out of a
# print, out of main(), and killed the trader for five bars. Several log lines
# in this file hold an em dash, so without this an export piped to a file dies
# on its own progress message. `errors="replace"` is the important half:
# encoding can then never be the thing that stops a run, whatever arrives.
for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")
    except (AttributeError, ValueError):  # not a TextIOWrapper; nothing to do
        pass

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


def symbol_select_plan(info, allow_select: bool):
    """What to do about Market Watch for one symbol. Pure; no MT5 call in here.

    Returns `(action, reason)` with `action` one of:

    * `"proceed"` — the symbol is already in Market Watch. **`symbol_select` is
      not called at all**, which is the whole point: the hazard the standing
      rule guards against is turning an UNREADABLE symbol readable, and a
      symbol that already reads back `visible == True` cannot be turned into
      anything. This is the path a logged-in Live 21 terminal is expected to
      take for `XAUUSD.sc` — `data/spreads/XAUUSD_sc.csv` holds 8,968 rows
      written by `spread_log.py`, which never calls `symbol_select`, so the
      symbol was in Market Watch on that account. That is evidence, not a
      licence: `visible` is checked here every run rather than inferred.
    * `"select"` — the symbol is not visible (or not known at all) and
      `--allow-symbol-select` was passed, so the caller has taken the decision.
    * `"skip"` — the symbol is not visible and the flag was not passed. The pair
      goes on the `skipped` list, which moves the exit code off 0. **Refusing is
      reported, never silent**: a run that exported nothing must not look like a
      run that worked, which is the failure this script's exit codes exist for.

    `info is None` means the terminal does not know the symbol at all, which is
    a different state from "knows it, not shown" — and it is the state where
    `symbol_select` does the thing the rule forbids. It is therefore treated the
    same as not-visible, and named differently in the reason so a log reader can
    tell the two apart.
    """
    if info is None:
        if allow_select:
            return "select", ("symbol_info returned None (terminal does not know this symbol); "
                              "--allow-symbol-select was passed, so calling symbol_select")
        return "skip", ("symbol_info returned None (terminal does not know this symbol, or no "
                        "account is logged in). symbol_select NOT called: pass "
                        "--allow-symbol-select to permit adding it to Market Watch")
    if getattr(info, "visible", False):
        return "proceed", "already visible in Market Watch — symbol_select NOT called"
    if allow_select:
        return "select", ("visible == False; --allow-symbol-select was passed, so calling "
                          "symbol_select to add it to Market Watch")
    return "skip", ("visible == False and --allow-symbol-select was NOT passed. symbol_select "
                    "NOT called (see py/live/spread_log.py for the rule). Add the symbol by hand "
                    "in the terminal, or pass the flag")


def rewrite_plan(stored_count: int, since_stored: bool, rewrite_existing: bool):
    """Whether this run may overwrite bars that are ALREADY on disk.

    Returns `(ok, message)`.

    `write_bars` replaces the whole file, and its content is a read-modify-write
    merge: `read_existing` loads every stored bar and a freshly pulled bar
    REPLACES the stored one at the same timestamp. No bar is lost — but every
    stored bar the pull covers has its values rewritten, and the file's bytes
    and metadata block are replaced wholesale.

    That is the exact shape that cost this dossier 25 of 34 published rows
    (`AGENT-BRIEF-ADDENDUM-11` section VII): a store GREW under receipts that
    had pinned neither end of their window, and the trade counts moved by the
    growth factor. So the dangerous default is not `--out` pointing somewhere in
    particular — it is `--days` being `None`, which walks ALL of history the
    terminal holds, while `--since-stored` is off. One mistyped command then
    rewrites every bar back to the start of the store.

    The latch is keyed on **an existing store being present**, not on a
    hardcoded path. A path list would protect one directory and leave a copy of
    the same store unprotected, and the hazard belongs to the file, not the
    folder. Three ways through, and nothing else:

    * nothing stored yet — there is no value to rewrite, so a backfill is free;
    * `--since-stored` — the pull is bounded to newer than the newest stored
      bar, less one bar of deliberate, documented overlap;
    * `--rewrite-existing` — the caller said so in as many words.

    `--days` is deliberately NOT a way through. A bounded walk still rewrites
    every stored bar inside its bound; `--days 400` on this store would rewrite
    400 days of it. Bounding how far back the walk goes is not the same as
    leaving stored values alone, and conflating the two is what makes the trap
    cheap to fall into.
    """
    if stored_count == 0:
        return True, "nothing stored yet — a backfill rewrites nothing"
    if since_stored:
        return True, (f"{stored_count} bars stored; --since-stored bounds the pull to the tail "
                      f"(one bar of overlap by design)")
    if rewrite_existing:
        return True, (f"{stored_count} bars stored and --rewrite-existing was passed: this run "
                      f"MAY rewrite the value of every one of them")
    return False, (f"REFUSED: {stored_count} bars are already stored and this run would re-pull "
                   f"and rewrite them (--days is None, so the walk covers all of history). Pass "
                   f"--since-stored to pull only the tail, or --rewrite-existing to say you mean "
                   f"it. Note that --days does NOT make this safe: a bounded walk still rewrites "
                   f"every stored bar inside its bound")


def ensure_market_watch(symbol: str, allow_select: bool, mt5_mod, log):
    """Resolve `symbol` against Market Watch. **The only `symbol_select` call site.**

    Returns `(ok, info, action)`. `mt5_mod` is injected rather than taken from
    the module global so that a test can hand in a stub whose `symbol_select`
    trips an assertion — which is the only way to show that the "already
    visible" path does not call it. A test that only checks what
    `symbol_select_plan` RETURNS would pass just as happily if the call site
    ignored the plan, and this dossier has already paid for the difference
    between a flag's declaration and a flag's behaviour (`--null-sides=` was
    printed by every mode and read by one).

    Keeping the call in exactly one function is also what makes the source scan
    in `test_mt5_export_guards.py` meaningful: one occurrence, in here, and the
    test fails if a second one appears anywhere in the file.
    """
    info = mt5_mod.symbol_info(symbol)
    action, reason = symbol_select_plan(info, allow_select)
    log(f"{symbol}: {reason}")
    if action == "skip":
        return False, None, action
    if action == "select":
        if not mt5_mod.symbol_select(symbol, True):
            log(f"{symbol}: symbol_select failed {mt5_mod.last_error()} — skipped")
            return False, None, action
        info = mt5_mod.symbol_info(symbol)  # re-read: the metadata below comes from it
        if info is None:
            log(f"{symbol}: still unreadable after symbol_select — skipped")
            return False, None, action
    return True, info, action


def flag_table(parser, args, argv) -> list:
    """Every flag, its EFFECTIVE value, and whether the command line set it.

    Printed every run. A flag that is read but ignored, or spelled in a way the
    parser quietly turned into something else, has been this dossier's most
    expensive class of defect: `--null-sides=` was printed in the header of
    every mode and consumed by exactly one, and `max_hold_ms` under `[run]` in a
    batch file was accepted without complaint while the cap stayed at four
    hours. There is no way for a caller to tell either of those from the outside
    except by changing the value and watching the numbers, so the least this can
    do is state what it actually ended up with, next to where it came from.

    Abbreviation is off at the parser (`allow_abbrev=False`), so `--allow-sym`
    is an error rather than a silent match for `--allow-symbol-select`, and an
    unknown flag is an error rather than a flag that does nothing.
    """
    rows = []
    for action in parser._actions:  # noqa: SLF001 - argparse exposes no public view of this
        if not action.option_strings or isinstance(action, argparse._HelpAction):  # noqa: SLF001
            continue
        name = max(action.option_strings, key=len)
        value = getattr(args, action.dest, None)
        on_cli = any(tok == name or tok.startswith(name + "=") or tok in action.option_strings
                     for tok in argv)
        rows.append((name, value, "command line" if on_cli else "default"))
    return rows


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


def build_parser() -> argparse.ArgumentParser:
    """The parser, built outside `main` so a test can exercise the REAL one.

    A test that rebuilt an equivalent parser would be testing its own copy, and
    the defect class it is here to catch — a flag that is accepted and does
    nothing — lives in the difference between the two.
    """
    # allow_abbrev=False: an abbreviation is the one way argparse will silently
    # turn a flag the caller did not write into one it did not mean. With it off,
    # `--allow-sym` is an error. Unknown flags are already an error, and
    # `parse_args` (not `parse_known_args`) is what keeps them one.
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0], allow_abbrev=False)
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
    # Permission to touch Market Watch. OFF by default — see `symbol_select_plan`
    # and the rule quoted in this module's doc comment.
    ap.add_argument("--allow-symbol-select", action="store_true",
                    help="permit symbol_select() for a symbol that reads visible == False "
                         "(default: report it and skip the symbol, never call symbol_select)")
    # Permission to rewrite bars already on disk. OFF by default — see
    # `rewrite_plan` for why the latch is keyed on the file and not on a path.
    ap.add_argument("--rewrite-existing", action="store_true",
                    help="permit this run to re-pull and overwrite bars already stored "
                         "(default: refuse, because --days is None means ALL of history)")
    return ap


def parse_args(argv=None):
    """Parse and VALIDATE. Garbage is a hard error, not a value that travels."""
    ap = build_parser()
    args = ap.parse_args(argv)
    # `--days 0` would set `floor = now_server`, so the walk breaks on its first
    # test and the run exports nothing — a silent no-op wearing the clothes of a
    # bounded pull. A negative value is the same thing with a worse log line.
    if args.days is not None and args.days < 1:
        ap.error(f"--days must be >= 1 (got {args.days}); omit it to pull all of history")
    return ap, args


def main() -> int:
    ap, args = parse_args()
    log = (lambda *_: None) if args.quiet else (lambda *a: print(*a, flush=True))

    log("flags in effect (value, and whether the command line set it):")
    for name, value, origin in flag_table(ap, args, sys.argv[1:]):
        log(f"  {name:<24} = {value!r:<30} [{origin}]")

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
            # READ `visible` FIRST, decide SECOND, and only then maybe call.
            # The old order was `symbol_select` then `symbol_info`, which meant
            # the call the standing rule forbids happened before anything had
            # looked at whether it was needed.
            ok, info, action = ensure_market_watch(symbol, args.allow_symbol_select, mt5, log)
            if not ok:
                skipped += [f"{symbol} {tf} (Market Watch: {action})" for tf in timeframes]
                continue
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
                # The latch. Checked HERE, after the store is read and before
                # one bar is asked of the terminal, so a refusal costs nothing
                # and a permitted run has already said how much it may rewrite.
                may_write, why = rewrite_plan(before, args.since_stored, args.rewrite_existing)
                log(f"  {symbol} {tf_name}: {why}")
                if not may_write:
                    skipped.append(f"{symbol} {tf_name} (refused: would rewrite {before} stored bars)")
                    continue
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
