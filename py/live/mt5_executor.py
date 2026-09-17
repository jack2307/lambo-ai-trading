"""Mirror one paper run into one MT5 DEMO account. The only order-sending
code in this repository, and it refuses to run against anything but a demo.

    python py/live/mt5_executor.py --run=xau-ema --terminal="C:/MT5-demo/terminal64.exe" \
        --login=12345678 --symbol=XAUUSD.sc --api=http://127.0.0.1:8138 [--dry-run]

What it does, every `--poll` seconds:

1. Reads `<api>/api/paper/status` and finds the run `--run`. The paper
   book is the truth: it says whether the run is long, short or flat, at
   what size, with what stop and target.
2. Reads the terminal's open positions carrying this run's magic number.
3. Reconciles: book open and terminal flat → send a market order with the
   book's stop and target; book flat and terminal open → close the
   position at market; book's side differs from the terminal's → close,
   then open. Size = the book's lots × `--lot-scale`, clamped to the
   symbol's volume min/step/max. Nothing else: no averaging, no partial
   exits, no trailing.
4. Appends every action and every refusal to
   `data/paper/<run>/executor.jsonl` with the terminal's ticket, the
   fill price the terminal reports, and the book's price — the slippage
   the record needs.

What it refuses, in code, before any order:

* A REAL account, unless THREE independent things agree: `--allow-real` is
  on the command line, `config/accounts.toml` says `real_money = true` for
  the account named by `--account`, and `--login` matches the account the
  terminal is actually holding. Any one missing and the process exits 3
  having sent nothing.

  Until 2026-09-17 this was simply "demo only, and no flag disables it".
  The owner funded a real cent account and asked for it, so the wall became
  a permission that has to be spent three times over. The shape is the one
  `-Live` already uses: a file can only ever make a run SAFER by itself,
  and making it riskier costs a word on the command line too. A forgotten
  flag, a stale config, or the wrong terminal — each alone still stops it.
* `account_info().login != --login` — exits, demo or real. A terminal that
  is not the one you named is not the one you meant.
* An account directory whose recorded login is not this one — exits. See
  `check_identity`. This is what keeps a real account's trades from being
  written into the demo's history.
* A `STOP` file at `data/paper/<run>/STOP` — closes any open position and
  exits. The kill switch.
* The terminal path is required (`--terminal`), so the executor never
  attaches to whichever terminal happens to be running.

The permission is tested by pointing it at the real account WITHOUT
`--allow-real`, and again with the flag but with `real_money` absent from
the registry: both must print the refusal and exit 3 without touching
anything. That test is in `docs/paper/DESIGN.md` and is repeated at every
review.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import sys
import time
import urllib.request
from pathlib import Path

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

ROOT = Path(__file__).resolve().parents[2]
UTC = dt.timezone.utc


def log(path: Path, kind: str, **fields) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    row = {"kind": kind, "time": int(time.time() * 1000), **fields}
    with path.open("a", encoding="utf-8") as f:
        f.write(json.dumps(row, ensure_ascii=False) + "\n")
    stamp = dt.datetime.now(tz=UTC).strftime("%H:%M:%SZ")
    print(f"{stamp} {kind} {json.dumps(fields, ensure_ascii=False)[:160]}", flush=True)


def magic_for(run: str) -> int:
    # A stable 31-bit magic per run id, so ten runs on one terminal never
    # touch each other's positions.
    return int(hashlib.sha1(run.encode("utf-8")).hexdigest()[:7], 16) & 0x7FFF_FFFF


def read_status(api: str, run: str) -> dict | None:
    with urllib.request.urlopen(f"{api}/api/paper/status", timeout=10) as r:
        runs = json.load(r)["runs"]
    for entry in runs:
        if entry["id"] == run:
            return entry
    return None


def clamp_volume(info, raw: float) -> tuple:
    """The broker's own volume grid, and whether it changed the answer.

    Returns `(volume, clipped)`. `clipped` says the symbol's min/max moved the
    size rather than merely rounding it to the step, and that is worth saying
    out loud: a mirror that cannot send the book's size is no longer mirroring
    the book, and the difference must appear in the record instead of being
    absorbed quietly. On 2026-09-16 a book asking for 256.3 lots arrived at the
    broker's 100.0 ceiling with nothing said about it.
    """
    step = info.volume_step or 0.01
    vol = round(round(raw / step) * step, 8)
    held = max(info.volume_min, min(info.volume_max, vol))
    return held, abs(held - vol) > step / 2


def notional_of(info, vol: float, price: float) -> float:
    """What the order is actually worth, in the account's currency."""
    return vol * info.trade_contract_size * price


MAX_FILLS = 40


def history_of(mt5, magic: int) -> tuple:
    """This book's closed trades ON THE ACCOUNT, and what they came to.

    Returns `(realised, closed, fills)`. The paper book's fills are the rule
    executed perfectly at the bar's price; these are what the broker actually
    did, and the desk shows one or the other rather than mixing them - the
    difference between the two entry prices IS the slippage, and it is only
    visible if neither number is quietly standing in for the other.

    Deals are matched by the book's magic number, which is how one account
    carries several books without their results running together, and grouped
    by `position_id` - one position is one trade however many deals closed it.

    The window is deliberately far wider than it needs to be. MT5 takes history
    bounds in SERVER time, not UTC, and this desk has been caught by that
    before; 400 days back and two days forward makes a three-hour offset
    irrelevant instead of making it a bug to remember.

    Commission is counted on every deal and profit and swap only on the closing
    ones: an entry deal carries a charge but no result, and counting its zero
    profit as a trade would double the count.
    """
    now = dt.datetime.now()
    deals = mt5.history_deals_get(now - dt.timedelta(days=400), now + dt.timedelta(days=2))
    if deals is None:
        return None, 0, []

    total = 0.0
    trades = {}
    for d in deals:
        if d.magic != magic:
            continue
        total += d.commission
        t = trades.setdefault(d.position_id, {
            "direction": None, "entryTime": None, "entryPrice": None,
            "exitTime": None, "exitPrice": None, "lots": None,
            "exitReason": "", "pnl": 0.0,
        })
        t["pnl"] += d.commission
        if d.entry == mt5.DEAL_ENTRY_IN:
            # DEAL_TYPE_BUY on the way IN is a long position.
            t["direction"] = "LONG" if d.type == mt5.DEAL_TYPE_BUY else "SHORT"
            t["entryTime"] = int(d.time_msc)
            t["entryPrice"] = d.price
            t["lots"] = d.volume
        else:
            total += d.profit + d.swap
            t["pnl"] += d.profit + d.swap
            # The LAST closing deal wins the exit: a position closed in parts
            # ends when its final part does.
            if t["exitTime"] is None or int(d.time_msc) >= t["exitTime"]:
                t["exitTime"] = int(d.time_msc)
                t["exitPrice"] = d.price
                # The broker's own word for why it ended. "sl"/"tp" come from
                # MT5 itself; anything else is the comment the executor wrote,
                # and an empty one stays empty rather than being guessed at.
                t["exitReason"] = (d.comment or "").strip()[:24]

    fills = [t for t in trades.values()
             if t["entryTime"] is not None and t["exitTime"] is not None]
    fills.sort(key=lambda t: t["entryTime"])
    for t in fills:
        t["pnl"] = round(t["pnl"], 2)
    closed = len(fills)
    return round(total, 2), closed, fills[-MAX_FILLS:]


def write_snapshot(path, payload: dict) -> None:
    """The broker's side of this book, for anything that cannot reach MT5.

    `fd-api` is Rust and has no way to ask a terminal anything, so the only
    process that can see the account is this one. It writes what it sees each
    poll and the API serves the file. Written whole then renamed, because a
    reader that catches a half-written file would show a half-true account.

    The timestamp is the point of it: a file that has stopped moving means this
    executor has stopped, and a reader that cannot tell a live account from a
    remembered one is worse than a reader with no account at all.
    """
    tmp = path.with_suffix(".tmp")
    try:
        with open(tmp, "w", encoding="utf-8") as fh:
            json.dump(payload, fh)
        os.replace(tmp, path)
    except (OSError, TypeError, ValueError):
        # A snapshot is a convenience and must never stop the mirror. Caught
        # narrowly on purpose: these three are the ways WRITING can fail, and
        # swallowing everything here would have hidden the NameError that made
        # this function crash every executor the first time it ran.
        pass


def real_money_refusal(args, acc) -> str:
    """Why this REAL account may not be traded — empty string if it may.

    A reason and not a boolean, because the log line is the only place anyone
    will read about a refusal after the fact, and "refused" without a which-one
    sends the reader to the source to guess.

    Three keys, checked independently. Two of them live in different places on
    purpose: a command line is typed by a person now, a config file was edited
    by a person once. Requiring both means neither a stale file nor a typed
    mistake is enough on its own.
    """
    if not args.allow_real:
        return ("account is REAL and --allow-real was not given")
    if not args.account:
        # Without a named account there is no registry entry to consult, and
        # the fallback id (`login-<n>`) is generated here rather than written
        # by anyone - so nothing has actually granted permission.
        return "account is REAL and --account was not given, so no registry entry grants it"
    try:
        import tomli
    except ImportError:  # pragma: no cover - the desk installs it
        return "account is REAL and tomli is missing, so the registry cannot be read"
    path = ROOT / "config" / "accounts.toml"
    try:
        with open(path, "rb") as f:
            registry = tomli.load(f)
    except (OSError, ValueError) as e:
        return f"account is REAL and {path.name} could not be read: {type(e).__name__}"
    for entry in registry.get("account", []):
        if entry.get("id") != args.account:
            continue
        if not entry.get("real_money", False):
            return f"account is REAL and {path.name} does not set real_money for '{args.account}'"
        # The registry names a login too. If it disagrees with the terminal,
        # the permission that was granted was granted for a different account.
        named = entry.get("login")
        if named is not None and int(named) != int(acc.login):
            return (f"account is REAL and {path.name} grants '{args.account}' to login "
                    f"{named}, not {acc.login}")
        return ""
    return f"account is REAL and '{args.account}' is not in {path.name}"


def check_identity(path: Path, acc, account: str) -> str:
    """Bind an account directory to one login, for good.

    Returns a refusal reason, or an empty string. Writes the record the first
    time and compares every time after.

    This exists because the owner's history has to read as one unbroken thing.
    A directory that quietly changes which account it describes does not
    destroy any data - it destroys the meaning of all of it, and leaves every
    file looking perfectly fine.
    """
    now = {"account": account, "login": int(acc.login), "server": str(acc.server),
           "currency": str(acc.currency),
           "demo": int(acc.trade_mode) == 0}
    try:
        with open(path, encoding="utf-8") as f:
            was = json.load(f)
    except FileNotFoundError:
        try:
            path.parent.mkdir(parents=True, exist_ok=True)
            tmp = str(path) + ".tmp"
            with open(tmp, "w", encoding="utf-8") as f:
                json.dump({**now, "first_seen": int(time.time() * 1000)}, f, indent=2)
            os.replace(tmp, str(path))
        except OSError:
            # Not fatal: the guard is a safety net, not the trade path. A
            # directory that cannot be stamped is still the right directory.
            pass
        return ""
    except (OSError, ValueError):
        return ""  # unreadable stamp is not evidence of a mismatch
    if int(was.get("login", now["login"])) != now["login"]:
        return (f"data/live/{account}/ already holds the record of login "
                f"{was.get('login')}; this terminal is {now['login']}. Use a new account id "
                f"rather than writing two accounts into one history")
    if bool(was.get("demo")) != now["demo"]:
        kind = "a demo" if was.get("demo") else "a real"
        return (f"data/live/{account}/ was started as {kind} account and this one is not; "
                f"use a new account id")
    return ""


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", required=True)
    ap.add_argument("--terminal", required=True, help="path to the DEMO terminal64.exe")
    ap.add_argument("--login", type=int, required=True, help="the demo account number the terminal must be logged into")
    ap.add_argument("--symbol", required=True)
    # Which account's record this is. Everything this program writes lives
    # under it, because two accounts may mirror the SAME book - a demo and a
    # small real one, running one strategy side by side, is the comparison most
    # worth making - and a single file per book would have the second executor
    # silently overwrite the first's fills, slippage and account snapshot.
    #
    # Defaulted from the login rather than required, so an executor started by
    # hand still lands somewhere sane and distinct.
    ap.add_argument("--account", default="", help="account id from config/accounts.toml")
    ap.add_argument("--api", default="http://127.0.0.1:8138")
    ap.add_argument("--poll", type=float, default=15.0)
    ap.add_argument("--lot-scale", type=float, default=1.0, help="terminal volume = book lots x this")
    ap.add_argument("--deviation", type=int, default=30, help="max slippage in points for a market order")
    # A ceiling on SIZE, which this program had none of.
    #
    # Not the desk's rule restated - the desk caps notional at 300% of equity
    # and owns that decision. This is a sanity bound far above it, there to
    # catch an order that is wrong by a FACTOR rather than by a judgement: a
    # unit error, a mis-declared contract size, an ATR that came back zero.
    # The one that prompted it asked for 100 lots of EURUSD - $11.5m against a
    # $10k account - because the book's contract size was declared 1,000 times
    # too small. Every risk number in the book was right; only the lots were
    # wrong, and lots are the one thing that crosses over to here.
    ap.add_argument("--max-notional-ratio", type=float, default=10.0,
                    help="refuse to open if order notional exceeds this multiple of account equity")
    # A floor under the whole ACCOUNT, which the per-order ceiling above does
    # not provide.
    #
    # Each paper book sizes as though it owns its own 10,000 USC. Mirrored, they
    # share one account, so eight books each taking 300% of "their" equity put
    # 24x the account's equity in notional on it at once. At 1:500 that is a
    # margin level around 2000% and perfectly safe - but safe by today's
    # arithmetic, not by any rule. This is the rule: no executor opens a
    # position that would take the ACCOUNT's margin level below this.
    #
    # 500% is far above the broker's 30% stop-out and far below where these
    # sizes land, so it never binds in normal running and catches the case
    # where someone adds books, raises lot_scale, or the broker cuts leverage.
    # Each executor checks it independently and sees the same account, so they
    # arrive at the same answer without having to talk to each other.
    ap.add_argument("--min-margin-level", type=float, default=500.0,
                    help="refuse to open if the ACCOUNT's margin level would fall below this percent")
    # How stale a book's position may be before this refuses to join it.
    #
    # The reconciler's job is to make the account match the book, and taken
    # literally that means opening a position the book started hours ago - at
    # today's price. On 2026-09-16, the first minute of live running did exactly
    # that: three books were already long, the terminal's Algo Trading gate had
    # been shut, and the moment it opened the mirror bought 6 to 11 points above
    # where the books had entered.
    #
    # Nothing failed. The account simply held the right direction at the wrong
    # price, and the trade it will eventually report is not the trade the book
    # took - which destroys the one measurement the mirror exists for. Worse, it
    # is biased: the longer a book has been RIGHT, the worse the mirror's entry.
    #
    # So a position older than this is not adopted. The mirror sits the trade
    # out and joins on the next one, where it can enter within a bar of the
    # book. One bar of lag is allowed because the book's `entry_time` is a bar
    # stamp and the executor polls every 15 seconds inside it.
    ap.add_argument("--max-adopt-bars", type=float, default=1.0,
                    help="do not open a mirror for a book position older than this many bars")
    # How far from the book's own entry this may fill, as a fraction of the
    # trade's RISK.
    #
    # The bar limit above bounds time, and time is not what ruins the
    # comparison - price is. On 2026-09-16 a book went short at 4356.33 and
    # gold fell nineteen points inside two bars while every order came back
    # 10027; each retry was priced afresh, all of them inside the bar limit, and
    # the fill that eventually landed was at 4344.46. The book took 18.25 points
    # and the account took 6.52 out of the same trade.
    #
    # Expressed in R rather than points because R is the unit the whole desk
    # measures in, and because it scales itself: a quarter of the stop distance
    # is the same distortion on gold as on EURUSD, and on a wide-stop trade as
    # on a tight one.
    #
    # Symmetric on purpose. Refusing only the joins that went AGAINST the mirror
    # would leave a sample of only the favourable ones, and the mirror would
    # then beat the book by construction - the same bias as late joins, pointed
    # the other way.
    ap.add_argument("--max-join-r", type=float, default=0.25,
                    help="do not open if the price has moved this far from the book's entry, in R")
    ap.add_argument("--dry-run", action="store_true", help="reconcile and log, send nothing")
    # One of the three keys to a real account. On its own it does nothing:
    # the registry must also say `real_money = true` for --account, and the
    # terminal must hold exactly --login. See the module docstring.
    ap.add_argument("--allow-real", action="store_true",
                    help="permit a REAL account (registry must also allow it)")
    args = ap.parse_args()

    # data/live/<account>/<run>/ - the live side, kept out of data/paper
    # entirely. The paper book is one thing that happened; what each account
    # did with it is a separate record per account, and the directory layout
    # says so rather than a naming convention inside one folder.
    account = args.account or f"login-{args.login}"
    here = ROOT / "data" / "live" / account / args.run
    here.mkdir(parents=True, exist_ok=True)
    out = here / "executor.jsonl"

    # The record of WHOSE account this directory is, written once and checked
    # every time after.
    #
    # The owner's standing requirement is that the trading history runs
    # unbroken from the first day and survives any update. Deletion was never
    # the real danger - `data/` is gitignored and no deploy script touches it.
    # The danger is MIXING: point a real account at an id that a demo has been
    # writing under, and both accounts' fills land in one executor.jsonl. The
    # history is all still there and it no longer means anything, which is
    # worse than losing it, because nothing looks wrong.
    #
    # So the directory remembers its login, and refuses to become a different
    # one. Moving an account to a new login is then a deliberate act: use a
    # new account id, which is a new directory, and the old record stays
    # exactly as it was.
    identity = ROOT / "data" / "live" / account / "identity.json"


    # Two kill switches, and both are honoured.
    #
    #   data/live/<account>/<run>/STOP  stops this book on THIS account
    #   data/paper/<run>/STOP           stops this book on EVERY account
    #
    # The second is the one to reach for when a book itself is wrong; the first
    # when one account should sit out. Neither is cleared by the launcher: a
    # book stopped by hand stays stopped until someone deletes the file.
    stop_file = here / "STOP"
    stop_all = ROOT / "data" / "paper" / args.run / "STOP"

    try:
        import MetaTrader5 as mt5  # type: ignore
    except ImportError:
        sys.exit("MetaTrader5 package not installed")

    if not mt5.initialize(path=args.terminal, portable=True):
        # A fresh terminal may need a non-portable init; try once more plainly.
        if not mt5.initialize(path=args.terminal):
            sys.exit(f"initialize({args.terminal}) failed: {mt5.last_error()}")
    try:
        acc = mt5.account_info()
        if acc is None:
            log(out, "refused", reason="no account_info", error=str(mt5.last_error()))
            return 3
        # ---- the lock: the account you named, and a real one only on
        #      three separate permissions ----
        #
        # The login check comes FIRST on purpose. Everything below reasons
        # about "this account"; until the terminal is confirmed to be holding
        # the account that was named, there is no such thing.
        if acc.login != args.login:
            log(out, "refused", reason="account login differs from --login", login=acc.login, expected=args.login, server=acc.server)
            print(f"REFUSED: terminal login {acc.login} is not {args.login}. Nothing was sent.", flush=True)
            return 3
        if acc.trade_mode != mt5.ACCOUNT_TRADE_MODE_DEMO:
            why = real_money_refusal(args, acc)
            if why:
                log(out, "refused", reason=why, login=acc.login, server=acc.server,
                    trade_mode=int(acc.trade_mode), account=account)
                print(f"REFUSED: {why}. Nothing was sent.", flush=True)
                return 3
            # Said out loud, every start, because a line that scrolls past is
            # the only moment anyone is told this is not practice.
            print(f"REAL MONEY: account {acc.login} on {acc.server}, "
                  f"{acc.balance:.2f} {acc.currency}, lot scale {args.lot_scale}.", flush=True)
            log(out, "real-money", login=acc.login, server=acc.server,
                balance=acc.balance, currency=acc.currency, lot_scale=args.lot_scale)
        why = check_identity(identity, acc, account)
        if why:
            log(out, "refused", reason=why, login=acc.login, account=account)
            print(f"REFUSED: {why}. Nothing was sent.", flush=True)
            return 3
        if not mt5.symbol_select(args.symbol, True):
            log(out, "refused", reason="symbol not available", symbol=args.symbol)
            return 3
        info = mt5.symbol_info(args.symbol)
        magic = magic_for(args.run)
        log(out, "started", login=acc.login, server=acc.server, balance=acc.balance, symbol=args.symbol, magic=magic,
            dry_run=args.dry_run, lot_scale=args.lot_scale, contract=info.trade_contract_size)

        def positions():
            return [p for p in (mt5.positions_get(symbol=args.symbol) or []) if p.magic == magic]

        def send(request: dict, what: str) -> bool:
            if args.dry_run:
                log(out, "dry-run", action=what, request={k: v for k, v in request.items() if k != "type_filling"})
                return True
            result = mt5.order_send(request)
            ok = result is not None and result.retcode in (mt5.TRADE_RETCODE_DONE, mt5.TRADE_RETCODE_DONE_PARTIAL, mt5.TRADE_RETCODE_PLACED)
            log(out, "order" if ok else "order-failed", action=what, retcode=getattr(result, "retcode", None),
                comment=getattr(result, "comment", None), ticket=getattr(result, "order", None), deal=getattr(result, "deal", None),
                price=getattr(result, "price", None), volume=request.get("volume"), requested=request.get("price"))
            # Carried into the snapshot so it leaves this machine. A refusal
            # that only ever reaches a log file is a book that quietly stopped
            # trading: the desk goes on deciding, the paper P&L goes on moving,
            # and the account does nothing. The one that prompted this was
            # 10027 "AutoTrading disabled by client" - a button in the terminal,
            # off by default, that refused thirty-six orders in a row while
            # every other part of the system reported itself healthy.
            if ok:
                nonlocal_blocked(None)
                nonlocal_standing_out(None)
            else:
                nonlocal_blocked(
                    f"broker refused: {getattr(result, 'retcode', '?')} "
                    f"{(getattr(result, 'comment', '') or mt5.last_error())}"[:160])
            return ok

        def close(pos, why: str) -> bool:
            tick = mt5.symbol_info_tick(args.symbol)
            is_long = pos.type == mt5.POSITION_TYPE_BUY
            req = {
                "action": mt5.TRADE_ACTION_DEAL, "symbol": args.symbol, "volume": pos.volume, "position": pos.ticket,
                "type": mt5.ORDER_TYPE_SELL if is_long else mt5.ORDER_TYPE_BUY,
                "price": tick.bid if is_long else tick.ask, "deviation": args.deviation, "magic": magic,
                "comment": f"flowdesk {args.run} {why}"[:31], "type_time": mt5.ORDER_TIME_GTC, "type_filling": mt5.ORDER_FILLING_IOC,
            }
            return send(req, f"close {why}")

        def nonlocal_blocked(why) -> None:
            nonlocal blocked
            blocked = why

        def nonlocal_standing_out(why) -> None:
            # Kept apart from `blocked` on purpose. The mirror sitting out a
            # trade the book opened too long ago is the guard working, it
            # happens for the whole life of that position, and an alert channel
            # that fires every fifteen seconds on correct behaviour is one
            # nobody reads by morning.
            nonlocal standing_out
            standing_out = why

        TF_MS = {"1m": 60_000, "5m": 300_000, "15m": 900_000, "1h": 3_600_000}

        def too_old(book_open: dict, run: dict) -> float | None:
            """Bars between the book's entry and its latest bar, or None if fresh.

            Measured against the book's own clock (`last_bar_time`) rather than
            the wall clock: the book only moves when a bar closes, and over a
            weekend or a feed outage the wall clock would call every position
            stale while the book has not advanced a single bar.
            """
            step = TF_MS.get(run.get("tf") or "", 900_000)
            entry, last = book_open.get("entry_time"), run.get("last_bar_time")
            if not entry or not last:
                return None
            bars = (last - entry) / step
            return bars if bars > args.max_adopt_bars else None

        def drifted(book_open: dict, price: float) -> float | None:
            """How far this fill would be from the book's entry, in R, or None.

            Signed so the record says WHICH WAY it drifted: positive is worse
            for the mirror than the book got, negative is better. Both are
            refused - see `--max-join-r` - but only one of them is the failure
            people expect, and a log that collapsed them would hide the other.
            """
            entry, stop = book_open.get("entry_price"), book_open.get("stop")
            if not entry or not stop:
                return None
            risk = abs(entry - stop)
            if risk <= 0:
                return None
            adverse = (price - entry) if book_open.get("side") == "LONG" else (entry - price)
            r = adverse / risk
            return r if abs(r) > args.max_join_r else None

        def already_taken(book_open: dict, run: dict) -> dict | None:
            """The account's own fill for the book position it is holding, if any.

            This closes a hole that only opens on a LIVE account, and only
            between two bars.

            The paper book learns about a stop or a target when a bar CLOSES -
            it reads the bar's high and low, so it never misses one and never
            prices one wrongly, but it finds out at the close. The broker holds
            the real orders and exits the instant price touches. So for up to a
            whole bar the account is flat while the book still says it is
            holding, and that is exactly the state the reconciler reads as
            "open it" - which would re-enter the trade that just stopped out,
            at a worse price, on the same bar.

            Asked of the ACCOUNT rather than remembered in this process, so it
            survives a restart of the executor: a mirror that forgot its own
            history across a restart would make the same mistake at the worst
            possible moment.

            Matched by the bar the book entered on. The broker's fill is a few
            seconds after the book's bar stamp, never before it and never into
            the next bar, so one bar's width identifies it without needing the
            two clocks to agree exactly.
            """
            entry = book_open.get("entry_time")
            if not entry:
                return None
            step = TF_MS.get(run.get("tf") or "", 900_000)
            _, _, fills = history_of(mt5, magic)
            for f in fills:
                got = f.get("entryTime")
                if got is not None and entry <= got < entry + step:
                    return f
            return None

        def open_like(book_open: dict, run: dict) -> bool:
            taken = already_taken(book_open, run)
            if taken is not None:
                nonlocal_standing_out(
                    f"this account already traded the book's current position "
                    f"({taken.get('direction')} from {taken.get('entryPrice')}, "
                    f"closed {taken.get('exitReason') or 'out'} for {taken.get('pnl')})")
                log(out, "already-taken", side=book_open.get("side"),
                    book_entry=book_open.get("entry_price"), fill=taken,
                    reason="the broker closed this trade before the book's bar did; "
                           "re-opening would take the same trade twice")
                return False

            late = too_old(book_open, run)
            if late is not None:
                nonlocal_standing_out(f"the book opened this {late:.0f} bars ago")
                log(out, "not-adopted", side=book_open.get("side"), lots=book_open.get("lots"),
                    book_entry=book_open.get("entry_price"), bars_old=round(late, 1),
                    limit=args.max_adopt_bars,
                    reason="the book opened this too long ago to mirror at a comparable price")
                return False
            tick = mt5.symbol_info_tick(args.symbol)
            is_long = book_open["side"] == "LONG"
            vol, clipped = clamp_volume(info, float(book_open["lots"]) * args.lot_scale)
            if clipped:
                log(out, "clipped", asked=float(book_open["lots"]) * args.lot_scale, sending=vol,
                    volume_min=info.volume_min, volume_max=info.volume_max)
            price = tick.ask if is_long else tick.bid

            off = drifted(book_open, price)
            if off is not None:
                nonlocal_standing_out(
                    f"{price} is {off:+.2f}R from the book's entry {book_open.get('entry_price')}")
                log(out, "not-adopted", side=book_open.get("side"), lots=book_open.get("lots"),
                    book_entry=book_open.get("entry_price"), would_fill=price,
                    drift_r=round(off, 2), limit=args.max_join_r,
                    reason="the price has moved too far from the book's entry to mirror the same trade")
                return False

            equity = getattr(mt5.account_info(), "equity", 0.0) or 0.0
            notional = notional_of(info, vol, price)
            if equity > 0 and notional > args.max_notional_ratio * equity:
                log(out, "refused-size", asked_lots=float(book_open["lots"]), sending_lots=vol,
                    notional=round(notional, 2), equity=round(equity, 2),
                    ratio=round(notional / equity, 2), limit=args.max_notional_ratio,
                    reason="order notional exceeds the sanity ceiling; nothing sent")
                print(f"REFUSED: {vol} lots = {notional:,.0f} on {equity:,.0f} equity "
                      f"({notional / equity:.1f}x, ceiling {args.max_notional_ratio}x). Nothing sent.", flush=True)
                nonlocal_blocked(f"{vol} lots is {notional / equity:.0f}x equity, over the {args.max_notional_ratio}x ceiling")
                return False
            # What this order would tie up, and what the account would look
            # like holding it. `order_calc_margin` is the broker's own answer
            # rather than notional/leverage, which is wrong for any symbol with
            # a margin rate of its own.
            need = mt5.order_calc_margin(
                mt5.ORDER_TYPE_BUY if is_long else mt5.ORDER_TYPE_SELL, args.symbol, vol, price)
            acc_now = mt5.account_info()
            used = getattr(acc_now, "margin", 0.0) or 0.0
            eq = getattr(acc_now, "equity", 0.0) or 0.0
            if need is not None and eq > 0:
                after = 100.0 * eq / (used + need) if (used + need) > 0 else float("inf")
                if after < args.min_margin_level:
                    log(out, "refused-margin", lots=vol, margin_needed=round(need, 2),
                        margin_used=round(used, 2), equity=round(eq, 2),
                        level_after=round(after, 1), floor=args.min_margin_level,
                        reason="opening this would take the account below its margin floor")
                    print(f"REFUSED: margin level would be {after:.0f}% "
                          f"(floor {args.min_margin_level:.0f}%). Nothing sent.", flush=True)
                    nonlocal_blocked(f"margin level would be {after:.0f}%, under the "
                                     f"{args.min_margin_level:.0f}% floor")
                    return False

            req = {
                "action": mt5.TRADE_ACTION_DEAL, "symbol": args.symbol, "volume": vol,
                "type": mt5.ORDER_TYPE_BUY if is_long else mt5.ORDER_TYPE_SELL,
                "price": price, "deviation": args.deviation, "magic": magic,
                "comment": f"flowdesk {args.run}"[:31], "type_time": mt5.ORDER_TIME_GTC, "type_filling": mt5.ORDER_FILLING_IOC,
            }
            # The book's stop is a real order only for engine-managed strategies;
            # a sizing-only stop (self-managed) is still sent as a hard stop —
            # on a demo the guard is the terminal's, and the record says so.
            if book_open.get("stop") is not None:
                req["sl"] = float(book_open["stop"])
            if book_open.get("target") is not None:
                req["tp"] = float(book_open["target"])
            return send(req, f"open {book_open['side'].lower()} {vol}")

        snap_path = here / "broker.json"
        blocked = None   # something is wrong and a person has to act
        standing_out = None  # working as designed: this trade is being sat out

        def snapshot(held, book_open) -> None:
            acc = mt5.account_info()
            realised, closed, fills = history_of(mt5, magic)
            tick = mt5.symbol_info_tick(args.symbol)
            pos = held[0] if held else None
            payload = {
                "at": int(time.time() * 1000),
                "account": account,
                "login": getattr(acc, "login", None),
                "server": getattr(acc, "server", None),
                "demo": getattr(acc, "trade_mode", None) == mt5.ACCOUNT_TRADE_MODE_DEMO,
                "currency": getattr(acc, "currency", None),
                "balance": getattr(acc, "balance", None),
                "equity": getattr(acc, "equity", None),
                "margin": getattr(acc, "margin", None),
                "margin_free": getattr(acc, "margin_free", None),
                # MT5 reports 0 for a flat account; null says "no ratio" rather
                # than claiming a level of zero, which would read as a stop-out.
                "margin_level": (getattr(acc, "margin_level", 0.0) or None),
                "symbol": args.symbol,
                "contract_size": info.trade_contract_size,
                "magic": magic,
                "lot_scale": args.lot_scale,
                "dry_run": bool(args.dry_run),
                "bid": getattr(tick, "bid", None),
                "ask": getattr(tick, "ask", None),
                # What the book wants versus what the account holds. Kept as two
                # fields rather than one "in sync" flag: the interesting state is
                # WHICH of them is ahead, and a boolean throws that away.
                "book_side": (book_open or {}).get("side"),
                "book_lots": (book_open or {}).get("lots"),
                "realised": realised, "closed": closed, "fills": fills,
                # Cleared on every successful open, so `blocked` describes the
                # state now rather than the last thing that ever went wrong.
                "margin_floor": args.min_margin_level,
                "position": None if pos is None else {
                    "ticket": pos.ticket,
                    "side": "LONG" if pos.type == mt5.POSITION_TYPE_BUY else "SHORT",
                    "lots": pos.volume,
                    "entry_price": pos.price_open,
                    "price_now": pos.price_current,
                    "sl": pos.sl or None,
                    "tp": pos.tp or None,
                    "profit": pos.profit,
                    "swap": pos.swap,
                    "opened_at": int(pos.time) * 1000,
                },
                "blocked": blocked,
                "standing_out": standing_out,
            }
            write_snapshot(snap_path, payload)

        while True:
            if stop_file.exists() or stop_all.exists():
                which = "STOP file" if stop_file.exists() else "desk-wide STOP file"
                for p in positions():
                    close(p, which)
                log(out, "stopped", reason=f"{which} present")
                return 0
            try:
                run = read_status(args.api, args.run)
            except Exception as e:  # noqa: BLE001
                log(out, "api-unreachable", error=str(e)[:200])
                snapshot(positions(), None)
                time.sleep(args.poll)
                continue
            if run is None:
                log(out, "run-missing", run=args.run)
                snapshot(positions(), None)
                time.sleep(args.poll)
                continue
            book_open = run.get("open")
            held = positions()
            if book_open is None and held:
                for p in held:
                    close(p, "book flat")
            elif book_open is not None and not held:
                open_like(book_open, run)
            elif book_open is not None and held:
                want_long = book_open["side"] == "LONG"
                for p in held:
                    if (p.type == mt5.POSITION_TYPE_BUY) != want_long:
                        close(p, "side changed")
                        open_like(book_open, run)
                        break
            snapshot(positions(), book_open)
            time.sleep(args.poll)
    except KeyboardInterrupt:
        return 0
    finally:
        mt5.shutdown()


if __name__ == "__main__":
    sys.exit(main())
