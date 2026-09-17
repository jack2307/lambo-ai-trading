"""Push MT5 bars to the paper loop. Read-only towards the terminal.

    python py/live/mt5_bars.py --symbol=XAUUSD.sc --market=xauusd --tf=M15 --api=http://127.0.0.1:8138

This sends **two** things every `--poll` seconds, and only one of them is
ever decided on:

1. The newest CLOSED bar, when it is one this process has not sent yet, to
   `<api>/api/paper/bar`. That is the bot's whole input: a paper run steps
   on closed bars and on nothing else.
2. The bar still FORMING, plus the current bid/ask, to
   `<api>/api/paper/tick`. That is for the screen alone. The API keeps it
   outside every book and drops it after ninety seconds; no strategy, guard
   or fill can read it. It exists because a desk whose newest number is
   fifteen minutes old reads as a feed that has died.

MT5 calls used: `initialize`, `symbol_select`, `symbol_info`,
`symbol_info_tick`, `copy_rates_from_pos`, `shutdown` — nothing that places,
modifies or closes an order; the terminal on this machine is a live account
and this script must stay a reader (see `mt5_export.py`). `symbol_select`
writes, but only to Market Watch: it is what makes the symbol quotable at
all, and it cannot touch a position.

Why the reader lives in Python: the MetaTrader5 package is the only feed for
the broker's own gold bars, and it exists only for Python. The Rust side
never links to the terminal; it receives bars over HTTP and would accept them
from any source that speaks the same shape.

The bar's `time` from the terminal is the server clock (New York + 7 hours,
i.e. UTC+3 in US summer, UTC+2 in winter); both kinds of bar are converted the
same way, through `server_offset_seconds`. A bar is "closed" when the terminal
reports a newer bar after it, so the last element of `copy_rates_from_pos` is
the forming one: it goes to `/tick` and never to `/bar`.

On startup the script also sends the last `--warm` closed bars so the loop can
rebuild its indicators; the loop ignores bars it already has.

The log stays readable: a closed bar prints a line (there are four an hour on
M15), a tick prints at most one line a minute, and a change in the tick's
state — the first one through, a failure, a recovery — prints straight away.
A failed tick never interrupts the closed-bar loop; the price on the screen is
worth less than the bars the book is built from.

The two run on separate clocks. `--tick-poll` (a second) is the forming bar,
which is the only thing on the chart that is supposed to move; `--poll` (five
seconds) is the closed-bar check, which on a 15-minute stream has something new
four times an hour and would otherwise be asked 3,600 times for it.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import sys
import time
import urllib.request
from zoneinfo import ZoneInfo

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
NEW_YORK = ZoneInfo("America/New_York")


def server_offset_seconds(server_epoch: int) -> int:
    guess = dt.datetime.fromtimestamp(server_epoch - 3 * 3600, tz=UTC)
    dst = guess.astimezone(NEW_YORK).dst() or dt.timedelta(0)
    return 3 * 3600 if dst else 2 * 3600


def to_utc_ms(server_epoch: int) -> int:
    return (int(server_epoch) - server_offset_seconds(int(server_epoch))) * 1000


def post(api: str, path: str, payload: dict, timeout: float = 10.0) -> tuple[int, str]:
    body = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(f"{api}{path}", data=body, headers={"Content-Type": "application/json"}, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return r.status, r.read().decode("utf-8", "replace")[:200]
    except urllib.error.HTTPError as e:  # type: ignore[attr-defined]
        return e.code, e.read().decode("utf-8", "replace")[:200]
    except Exception as e:  # noqa: BLE001
        return 0, str(e)[:200]


def api_tf(tf: str) -> str:
    """The store's spelling of a timeframe. The terminal says `M15`; every
    endpoint here says `15m`."""
    return tf[1:] + "m" if tf.startswith("M") and tf[1:].isdigit() else tf.lower()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--symbol", default="XAUUSD.sc")
    ap.add_argument("--market", default="xauusd", help="the market id fd-api knows the symbol as")
    ap.add_argument("--tf", default="M15")
    ap.add_argument("--api", default="http://127.0.0.1:8138")
    ap.add_argument("--poll", type=float, default=5.0, help="seconds between CLOSED-BAR reads")
    ap.add_argument("--tick-poll", type=float, default=1.0,
                    help="seconds between forming-bar reads. Separate from --poll because the two "
                         "have nothing in common: a closed 15-minute bar arrives four times an hour "
                         "and asking for it every second is 3,600 pointless requests, while the "
                         "forming bar is the only thing on the screen that is supposed to move.")
    ap.add_argument("--warm", type=int, default=400, help="closed bars to send on startup")
    ap.add_argument("--once", action="store_true", help="send the warm-up bars and exit")
    ap.add_argument("--no-tick", action="store_true", help="closed bars only: do not send the forming bar")
    # Which terminal the prices come from.
    #
    # Optional, because a machine with one terminal has nothing to choose
    # between and every existing launcher omits it. Naming it is what makes a
    # two-terminal machine deterministic: the cent symbols this desk quotes
    # exist on the live account only, so a poller that attached to the demo
    # terminal would find no symbol and quit.
    ap.add_argument("--terminal", default="", help="path to the terminal64.exe to read prices from")
    args = ap.parse_args()

    try:
        import MetaTrader5 as mt5  # type: ignore
    except ImportError:
        sys.exit("MetaTrader5 package not installed")
    ok = mt5.initialize(path=args.terminal) if args.terminal else mt5.initialize()
    if not ok:
        sys.exit(f"initialize failed: {mt5.last_error()}"
                 + (f" (terminal {args.terminal})" if args.terminal else ""))
    # Said out loud, because attaching to the wrong terminal is the failure
    # this argument exists to prevent and it is otherwise invisible.
    term = mt5.terminal_info()
    acct = mt5.account_info()
    print(f"terminal {getattr(term, 'path', '?')}"
          f" | account {getattr(acct, 'login', '?')} {getattr(acct, 'server', '?')}", flush=True)
    tf = getattr(mt5, f"TIMEFRAME_{args.tf}")
    try:
        info = mt5.symbol_info(args.symbol)
        if info is None:
            sys.exit(f"no symbol {args.symbol}")
        # Into Market Watch before asking for bars. symbol_info answers for
        # every symbol the broker lists, selected or not, but
        # copy_rates_from_pos returns nothing at all for one that is not - so
        # the poller would run, print this line, and then post no bars, which
        # on the screen is indistinguishable from a quiet market.
        if not info.visible and not mt5.symbol_select(args.symbol, True):
            sys.exit(f"could not put {args.symbol} in Market Watch: {mt5.last_error()}")
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
                "tf": api_tf(args.tf),
                "bar": {
                    "time": t,
                    "open": float(row["open"]),
                    "high": float(row["high"]),
                    "low": float(row["low"]),
                    "close": float(row["close"]),
                    "volume": float(row["tick_volume"]),
                },
            }
            status, text = post(args.api, "/api/paper/bar", payload)
            stamp = dt.datetime.fromtimestamp(t / 1000, tz=UTC).strftime("%Y-%m-%d %H:%MZ")
            print(f"{stamp} close {payload['bar']['close']:.2f} -> {status} {text}", flush=True)
            if status == 200:
                sent_upto = t
                return True
            return False

        # The tick log is deliberately thin: one line when the state
        # changes (first tick through, a failure, a recovery) and otherwise
        # one a minute. At a five-second poll the noisy version would bury
        # the four closed-bar lines an hour that actually matter.
        tick_ok: bool | None = None
        tick_logged_at = 0.0

        def note(ok: bool, message: str) -> None:
            """One line on a change of state, else one a minute."""
            nonlocal tick_ok, tick_logged_at
            now = time.monotonic()
            if ok != tick_ok or now - tick_logged_at >= 60.0:
                print(f"tick: {message}", flush=True)
                tick_logged_at = now
            tick_ok = ok

        def send_tick() -> None:
            rates = mt5.copy_rates_from_pos(args.symbol, tf, 0, 1)
            if rates is None or len(rates) == 0:
                note(False, f"no forming bar: {mt5.last_error()}")
                return
            row = rates[-1]  # position 0 is the bar still forming
            quote = mt5.symbol_info_tick(args.symbol)
            payload = {
                "market": args.market,
                "tf": api_tf(args.tf),
                "bar": {
                    "time": to_utc_ms(row["time"]),
                    "open": float(row["open"]),
                    "high": float(row["high"]),
                    "low": float(row["low"]),
                    "close": float(row["close"]),
                    "volume": float(row["tick_volume"]),
                },
            }
            if quote is not None and quote.bid and quote.ask:
                payload["bid"] = float(quote.bid)
                payload["ask"] = float(quote.ask)
            # A short timeout: a tick that arrives late is a stale price, and
            # the API drops it anyway. The closed-bar loop must not wait.
            status, text = post(args.api, "/api/paper/tick", payload, timeout=3.0)
            if status != 200:
                note(False, f"{status} {text}")
                return
            spread = f" spread {payload['ask'] - payload['bid']:.2f}" if "bid" in payload else ""
            note(True, f"live {payload['bar']['close']:.2f}{spread}")

        def tick() -> None:
            """A tick is decoration; a traceback out of it would stop the
            only loop that feeds the book."""
            if args.no_tick:
                return
            try:
                send_tick()
            except Exception as e:  # noqa: BLE001
                note(False, f"tick failed: {type(e).__name__}: {e}"[:200])

        for row in closed_bars(args.warm):
            send(row)
        tick()
        if args.once:
            return 0
        # Two clocks. The fast one is the price on the screen; the slow one is
        # the only thing a book is built from, and it must not be starved by
        # the fast one — so a closed-bar read that runs long simply delays the
        # next closed-bar read rather than the ticks in between.
        next_bars = 0.0
        while True:
            time.sleep(max(0.05, args.tick_poll))
            now = time.monotonic()
            if now >= next_bars:
                next_bars = now + max(args.poll, args.tick_poll)
                for row in closed_bars(3):
                    send(row)
            tick()
    except KeyboardInterrupt:
        return 0
    finally:
        mt5.shutdown()


if __name__ == "__main__":
    sys.exit(main())
