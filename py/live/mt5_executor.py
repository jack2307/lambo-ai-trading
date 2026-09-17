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

* A REAL account, unless three things agree: `--allow-real` is on the command
  line, `config/accounts.toml` says `real_money = true` for the account named
  by `--account`, and `--login` matches the account the terminal is actually
  holding. Any one missing and the process exits 3 having sent nothing.

  They are three CHECKS. They are not three independent decisions, and the
  word "independent" was wrong here until 2026-09-17. Started the documented
  way, through `start_executors.ps1`, the launcher reads `real_money` from the
  registry and adds `--allow-real` itself, and `--login` comes from the same
  `[[account]]` block that granted it - so one edit to that block plus one
  typed `-AllowReal` spends the whole permission, and the third check is asked
  a question the same block supplied the answer to. Only a hand-started
  executor puts three separate hands on it.

  What the third check IS worth, on every path: it is the only one of the
  three that asks the TERMINAL rather than a file. A registry that grants
  `vantage-cent` cannot reach an account the terminal is not holding, so a
  terminal logged into the wrong account still stops everything, whatever the
  file says and whatever was typed. That is a real guarantee, and it is the
  one to say out loud instead of a count.

  Until 2026-09-17 this was simply "demo only, and no flag disables it". The
  owner funded a real cent account and asked for it, so the wall became a
  permission that can be granted. The shape is the one `-Live` already uses: a
  file can only ever make a run SAFER by itself, and making it riskier costs a
  word on the command line too.
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


# Roll `executor.jsonl` aside once it passes 32 MiB.
#
# The arithmetic, measured 2026-09-17: the busiest of them,
# `data/live/vantage-demo/ai-xau-ds-ctx/executor.jsonl`, is 73,862 bytes over
# 334 lines — 221 bytes a line — written across 18.4 hours, so 3.9 KB an hour,
# 94 KB a day. `xau-ema` manages 1.0 KB an hour. 32 MiB is therefore about 350
# days of the busiest account's busiest book, and years of a quiet one.
#
# Which is to say: here this is a guard and not housekeeping. It should never
# fire, and the number is deliberately the same 32 MiB as
# `ai_trader.ROTATE_BYTES` rather than scaled down to make it fire — a
# threshold picked so that rotation HAPPENS would be rolling a file nobody has
# any trouble reading. What it is for is the day something starts logging per
# tick instead of per poll, which is a bug that has happened elsewhere in this
# repo and would otherwise fill the VPS disk quietly overnight.
#
# Duplicated from `ai_trader.py` rather than shared. The executor imports
# nothing from the AI side on purpose — it is the process that sends orders to
# a real account, and its import list is short so that it is auditable. Twenty
# lines of duplication is the cheaper of the two prices.
ROTATE_BYTES = 32 * 1024 * 1024


def roll_aside(path: Path) -> None:
    """Move a full log out of the way, under a name that sorts by when.

    Not truncation and not pruning, and there is deliberately NO option to
    keep only the last N files. The owner's standing requirement is that this
    account's record reads as one unbroken thing from the first day — the same
    requirement the identity check further down enforces against mixing — and
    a rotation able to delete is one that eventually will. If disk is ever
    genuinely short, moving old files elsewhere is a decision a person makes
    once; it is not a flag that runs every day without being watched.

    `os.rename`, never `os.replace`. Rename refuses to overwrite on Windows;
    replace overwrites everywhere and would silently destroy an existing
    rolled file. The explicit `exists` check is there because POSIX `rename`
    DOES overwrite, so without it the guarantee would hold on this VPS and not
    on Linux.

    Failing to roll is acceptable; losing a line is not. It runs BEFORE the
    append and never touches content: the new line lands in the fresh file,
    and fd-api holding the old one open keeps reading it under its new name —
    Rust opens with FILE_SHARE_DELETE, so its reading does not block the
    rename, and if some other reader does, the file just keeps growing and the
    next line tries again.
    """
    try:
        if path.stat().st_size < ROTATE_BYTES:
            return
        stamp = dt.datetime.now(tz=UTC).strftime("%Y%m%dT%H%M%SZ")
        rolled = path.with_name(f"{path.stem}-{stamp}{path.suffix}")
        if rolled.exists():
            return
        os.rename(path, rolled)
    except OSError:
        # Narrow on purpose, the same way write_snapshot() below is narrow:
        # these are the ways a rename can fail, and a mirror that stopped
        # sending orders because it could not tidy a log would be a far worse
        # bug than a log that grew too big.
        pass


def log(path: Path, kind: str, **fields) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    roll_aside(path)
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


def notional_of(info, vol: float, price: float) -> float | None:
    """What this order is worth, in the units `account_info().equity` is in.

    Returns None when the broker has not said enough to convert. A caller must
    read that as "cannot check" and refuse, never as "no limit" - see
    `open_like`, where that distinction is the whole guard.

    WHY THIS IS NOT `vol * contract_size * price`, which is what it was until
    2026-09-17:

      * `vol * trade_contract_size * price` is the position's value in the
        SYMBOL's profit currency. For XAUUSD.sc, XAGUSD.sc and EURUSD.sc -
        every symbol this desk sends - that is USD.
      * `account_info().equity` is in the ACCOUNT's currency. On the Vantage
        cent account funded 2026-09-17 (login 33705331) that is USC, and
        10,000 USC is USD 100.

    The caller compared the first against the second. Dollars against cents:
    a hundred times more numerous for the same money, so `--max-notional-ratio
    10.0` admitted 1000x. The audit's worked example - 20 lots of XAUUSD.sc at
    4350 - is USD 87,000 of gold against USC 10,000 of equity, which the old
    arithmetic scored 8.7x and PASSED. In one currency it is USC 8,700,000
    against USC 10,000: 870x, and refused.

    This guard exists to catch an order wrong by a FACTOR, and it was wrong by
    a factor. It was written and signed off against `vantage-demo` (login
    26108386), a STANDARD account in USD, where `trade_tick_value /
    trade_tick_size` equals `trade_contract_size` exactly and the two units
    agree by coincidence. The demo cannot reproduce this bug. The next guard
    proved out on it deserves the same question asked twice.

    The conversion is the broker's own, not a table of currency names.
    `trade_tick_value` is the ACCOUNT-currency value of one `trade_tick_size`
    of price, on one lot, so

        account units per unit of profit currency
            = trade_tick_value / (trade_tick_size * trade_contract_size)

    and `trade_contract_size` cancels out of the product, leaving

        notional (account units) = vol * price * trade_tick_value / trade_tick_size

    There is deliberately no `if currency == "USC": x100` here. That form is
    right for the one account this desk holds today and silently wrong for an
    account denominated in anything else - which is the same class of mistake
    as the one it would be fixing, only harder to see the second time.

    Checked against both accounts, 2026-09-17:
      vantage-demo   XAUUSD    contract 100, tick 0.01, tick_value 1.0
                     -> 1 lot at 4350 = 435,000 USD, identical to the old
                        arithmetic. Nothing changes on the demo.
      vantage cent   XAUUSD.sc contract 1.0, tick 0.01, tick_value 1.0
                     -> 1 lot at 4350 = 435,000 USC = USD 4,350. The old
                        arithmetic said 4,350 and meant USD, against equity
                        that meant USC.
    """
    tick_size = getattr(info, "trade_tick_size", 0.0) or 0.0
    tick_value = getattr(info, "trade_tick_value", 0.0) or 0.0
    if tick_size <= 0 or tick_value <= 0:
        # A symbol that will not say what a tick is worth cannot be sized
        # against equity at all. Refusing here is the point: the value this
        # used to return in that case was a number in an unknown currency,
        # which is worse than no number.
        return None
    return vol * price * tick_value / tick_size


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

    WHAT IS COMPARED, and what is only recorded (tightened 2026-09-17, after an
    audit found this function writing four fields and checking two):

      login   compared. The identity of the account.
      server  compared when the stamp has it. A login number is unique to a
              SERVER, not to the broker: VantageMarkets-Demo 26108386 and
              VantageMarkets-Live 21 26108386 are two different accounts that
              this function used to call the same one. Vantage runs enough
              servers for that collision to be reachable by a typo in a
              launcher argument, which is how account ids get crossed.
      demo    compared when the stamp has it.
      currency  RECORDED AND NOT COMPARED, deliberately. Currency is a property
              of an account, not its identity, and the login+server pair
              already pins the identity. A broker redenominating an account
              would then fail this check for a reason that has nothing to do
              with mixing two accounts' fills, and the refusal would be
              indistinguishable from the real one. It stays in the stamp
              because the record should say what the numbers in it are in -
              see `notional_of` for what that costs when nobody wrote it down.

    A stamp missing `server` or `demo` is compared on what it does have. Those
    two are permitted rather than refused because `login` alone already
    identifies the account and a missing FIELD is a stamp-format gap, not
    evidence of a different account; a missing `login` is refused, because
    without it the stamp guards nothing. (Every stamp this repo has ever
    written carries all four. The permissive branch is for a stamp edited by
    hand, and anyone editing it by hand could have edited `login` too.)
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
    except (OSError, ValueError) as e:
        # REFUSE. This used to return "" - "unreadable stamp is not evidence of
        # a mismatch" - which is true and is the wrong test. The question this
        # guard answers is not "do we have evidence of a mismatch", it is "do
        # we know whose directory this is", and an unreadable stamp is exactly
        # the state where the answer is no. Permitting there means the one way
        # to defeat the guard is to damage the file it reads.
        #
        # The cost, stated because it is not small: this refusal EXITS the
        # executor, and it can exit one that is mirroring an open position on
        # the funded account. What survives that is the broker's own stop and
        # target, which are sent with every order and sit on the server, so the
        # position is not unprotected - what stops is the reconciler, and the
        # position rides to sl/tp instead of closing when the book says to.
        # That is a bounded loss of fidelity. Two accounts' fills written into
        # one executor.jsonl is not bounded and is not repairable afterwards,
        # because nothing about the file looks wrong.
        #
        # It is also cheap to be wrong about: the stamp is read once per start,
        # not per poll, so a transient lock has to land in the same instant as
        # a start; the refusal prints what to look at; and the repair is to
        # read the file and either fix it or move the directory aside. A person
        # decides, once, which is the rule the rest of this module follows.
        return (f"{path} cannot be read ({type(e).__name__}), so there is no way to tell whether "
                f"data/live/{account}/ is this account's history or another's. Read or repair the "
                f"file, or use a new account id; nothing is sent until one of those happens")
    if not isinstance(was, dict):
        return (f"{path} does not contain an account record, so it cannot say whose history "
                f"data/live/{account}/ is. Repair it or use a new account id")
    try:
        was_login = int(was["login"])
    except (KeyError, TypeError, ValueError):
        # `was.get("login", now["login"])` used to stand here, which defaulted a
        # stamp with NO login to "matches" and then compared it to itself. A
        # stamp that does not name an account cannot vouch for one.
        return (f"{path} records no usable login, so it cannot say whose history "
                f"data/live/{account}/ is. Repair it or use a new account id")
    if was_login != now["login"]:
        return (f"data/live/{account}/ already holds the record of login "
                f"{was_login}; this terminal is {now['login']}. Use a new account id "
                f"rather than writing two accounts into one history")
    was_server = was.get("server")
    if was_server is not None and str(was_server) != now["server"]:
        return (f"data/live/{account}/ holds login {was_login} on {was_server}; this terminal is "
                f"the same login number on {now['server']}, which is a different account. Use a "
                f"new account id rather than writing two accounts into one history")
    if "demo" in was and bool(was["demo"]) != now["demo"]:
        kind = "a demo" if was["demo"] else "a real"
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
        # AutoTrading, checked here rather than discovered at the first order.
        #
        # With it off the terminal answers every order with retcode 10027,
        # "AutoTrading disabled by client", and an executor whose every order
        # is refused is indistinguishable on the screen from a strategy that
        # never fires. It cost 36 refused orders on the demo before anyone read
        # the code.
        #
        # And it turns itself off: MetaTrader disables automated trading
        # whenever the ACCOUNT CHANGES, which is exactly what logging a fresh
        # terminal in does. A start config that sets it at launch therefore
        # loses it a minute later - measured 2026-09-17 on the cent account.
        # So this is not a one-time setup question, it is a per-start check.
        term = mt5.terminal_info()
        if term is not None and not term.trade_allowed:
            if args.dry_run:
                # A dry run sends nothing, so this is not fatal - but it IS
                # the reason the live run would fail, and saying it now is
                # worth more than saying it after the switch is thrown.
                log(out, "autotrading-off", note="dry run continues; a live run would be refused")
                print("NOTE: AutoTrading is OFF in this terminal. Nothing is sent in a dry run "
                      "anyway, but a live run would have every order refused (10027).", flush=True)
            else:
                log(out, "refused", reason="AutoTrading is disabled in the terminal",
                    login=acc.login, server=acc.server)
                print("REFUSED: AutoTrading is OFF in this terminal - every order would come back "
                      "10027. Click Algo Trading in its toolbar. Nothing was sent.", flush=True)
                return 3
        info = mt5.symbol_info(args.symbol)
        magic = magic_for(args.run)
        # `currency`, `tick_size` and `tick_value` are on this line because the
        # notional ceiling is computed from them and a number nobody can see is
        # a number nobody checks. With them here, "is the ceiling measuring the
        # right currency?" is answerable from the log for any account, past or
        # present, without a terminal - which it was not on 2026-09-17, when
        # the answer was no. `one_lot_at` is that arithmetic already done:
        # what ONE lot of this symbol is worth, in the same units as `balance`
        # directly above it, so the two can be read against each other at a
        # glance.
        log(out, "started", login=acc.login, server=acc.server, balance=acc.balance,
            currency=acc.currency, symbol=args.symbol, magic=magic,
            dry_run=args.dry_run, lot_scale=args.lot_scale, contract=info.trade_contract_size,
            tick_size=getattr(info, "trade_tick_size", None),
            tick_value=getattr(info, "trade_tick_value", None),
            one_lot_at=(lambda n: None if n is None else round(n, 2))(
                notional_of(info, 1.0, getattr(mt5.symbol_info_tick(args.symbol), "ask", 0.0) or 0.0)))

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

        def server_offset_ms():
            """How far the terminal's clock runs ahead of UTC, in ms, or None.

            MEASURED from the terminal each time it is asked, not assumed from
            a timezone. `mt5_bars.py` derives the same offset by anchoring the
            server to New York DST, and that derivation is right today - but it
            is a belief about which zone this broker keeps, and this function
            is the one standing between a real account and a duplicated trade.
            Asking the terminal what time it thinks it is costs one call and
            needs no belief at all.

            The check against reality, 2026-09-16, from this repo's own files:
            `executor.jsonl` stamps a fill at 13:46:29 UTC and the deal history
            reports the same trade at 16:46:27. +3h, which is what the New York
            derivation gives for that date too. The two agree; this one keeps
            agreeing if the broker moves.

            Returns None rather than a guess in two cases, and the caller
            refuses on it:

            * no tick to read. The terminal is not answering, or the symbol has
              never ticked in this session.
            * a drift too large to be an offset. `tick.time` is the LAST tick,
              which over a weekend or a halt can be days old, and a stale tick
              would otherwise be read as an enormous offset. No broker keeps a
              clock more than fourteen hours from UTC, so anything beyond that
              is a stale tick and not a timezone.

            Snapped to the nearest quarter hour because server offsets are
            whole or half hours everywhere, and because the tick that measures
            it is seconds old rather than simultaneous.
            """
            tick = mt5.symbol_info_tick(args.symbol)
            server_s = getattr(tick, "time", None)
            if not server_s:
                return None
            drift_s = float(server_s) - time.time()
            if abs(drift_s) > 14 * 3600:
                return None
            return int(round(drift_s / 900.0) * 900.0 * 1000)

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

        def already_taken(book_open: dict, run: dict) -> tuple:
            """Has this account already traded the position the book is holding?

            Returns `(verdict, fill)` where verdict is "taken", "not-taken" or
            "unknown". Three states and not two, because the two clocks this
            has to reconcile can fail to be readable, and a mirror that reads
            "cannot tell" as "no" re-enters a trade it already took.

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

            THE TWO CLOCKS, and why this never once fired before 2026-09-17.

            The book's `entry_time` is UTC: `mt5_bars.py` converts every bar
            stamp with `to_utc_ms`, which subtracts the server offset before
            posting. `history_of` above reports `d.time_msc` exactly as MT5
            gives it, which is SERVER time - the same distinction this module's
            own `history_of` docstring warns about for history bounds, and
            which was then ignored twenty lines later.

            So the old test, `entry <= got < entry + step`, compared a UTC
            stamp against a server stamp inside a fifteen-minute window, across
            an offset of two or three HOURS. It could not match, and the
            auditor's note is the part that matters: "already-taken" has never
            appeared in any log on this desk. That reads as "never happened".
            It means "never worked".

            Measured from this repo's own records, 2026-09-16, the one trade
            that appears on both sides: `executor.jsonl` logs the fill for
            SHORT 0.06 at 4344.46 at 13:46:29 UTC (`log()` stamps wall-clock
            UTC), and `broker.json` carries the same trade's `entryTime` as
            16:46:27. Three hours, to the second.

            HOW THE WINDOW IS DRAWN NOW. Not one bar wide. The same trade shows
            why: the terminal answered 10027 for sixteen minutes before it
            filled, so the fill landed TWO bars after the book's entry stamp,
            and any window keyed to a fixed bar would have missed it.

            Instead: the book is holding a position it opened at `entry_time`
            and has not left. `history_of` returns only CLOSED trades. So any
            closed trade of this magic that was ENTERED at or after the book's
            entry stamp was necessarily opened while the book was already in
            this position - there is no other trade it could be. That is the
            whole test, and it needs no assumption about how long a retry storm
            lasts. A previous book position closed before this one opened, so
            its entry stamp is strictly earlier and cannot be caught by it.

            The direction is required to match as a cross-check. It should
            always match by the argument above; if it ever does not, the
            mismatch is logged rather than quietly treated as a match.
            """
            entry = book_open.get("entry_time")
            if not entry:
                return "not-taken", None
            offset = server_offset_ms()
            if offset is None:
                return "unknown", None
            # `history_of` reports a failed `history_deals_get` by returning
            # None for the realised total. That is "the terminal would not say
            # what this book has done", not "it has done nothing", and the two
            # must not collapse here.
            realised, _, fills = history_of(mt5, magic)
            if realised is None:
                return "unknown", None
            side = book_open.get("side")
            # A minute of slack under the book's stamp. The broker fills after
            # the bar the book entered on, never before it, so this is only
            # absorbing the seconds of jitter in snapping the offset - not
            # widening the test in any direction that matters.
            floor_ms = entry - 60_000
            for f in fills:
                got = f.get("entryTime")
                if got is None:
                    continue
                got_utc = got - offset
                if got_utc < floor_ms:
                    continue
                if f.get("direction") != side:
                    log(out, "history-anomaly", book_side=side, fill=f,
                        fill_entry_utc=got_utc, book_entry=entry,
                        reason="a closed trade of this magic was entered after the book's "
                               "current position opened, but on the other side; not treated "
                               "as this position's fill")
                    continue
                return "taken", f
            return "not-taken", None

        def open_like(book_open: dict, run: dict) -> bool:
            verdict, taken = already_taken(book_open, run)
            if verdict == "unknown":
                # Fail closed, for the same reason as the size guards: the
                # question "has this account already taken this trade?" has no
                # safe default. Answering "no" when the answer is unknown is
                # what re-enters a position that has already been stopped out.
                # This clears itself on the next poll as soon as a tick or the
                # deal history comes back, so the cost is a delayed entry and
                # not a dead mirror.
                log(out, "refused-unknown-history", side=book_open.get("side"),
                    book_entry=book_open.get("entry_time"),
                    reason="cannot read the server's clock offset or this book's deal "
                           "history, so whether this account already traded the book's "
                           "current position is unknown; nothing sent")
                nonlocal_blocked("cannot tell whether this trade was already taken "
                                 "(no server clock or no deal history)")
                return False
            if taken is not None:
                nonlocal_standing_out(
                    f"this account already traded the book's current position "
                    f"({taken.get('direction')} from {taken.get('entryPrice')}, "
                    f"closed {taken.get('exitReason') or 'out'} for {taken.get('pnl')})")
                log(out, "already-taken", side=book_open.get("side"),
                    book_entry=book_open.get("entry_price"), fill=taken,
                    book_entry_time=book_open.get("entry_time"),
                    fill_entry_utc=(taken.get("entryTime") or 0) - (server_offset_ms() or 0),
                    server_offset_ms=server_offset_ms(),
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

            # ---- the two size guards, and the reading they both depend on ----
            #
            # One `account_info()` for both, and a refusal if it does not
            # answer. Both guards used to read it with `getattr(..., 0.0)` and
            # then test `> 0`, so a terminal that was not answering scored zero
            # equity, failed the test, and the order was sent WITHOUT EITHER
            # GUARD having run. Nothing in the log said so, because skipping a
            # check writes no line.
            #
            # Measured 2026-09-17: the terminal was restarted underneath five
            # polling executors and `account_info()` returned None to every one
            # of them for as long as it was down - the same outage that filled
            # `broker.json` with `login: null, demo: false`. That is not a
            # hypothetical state, it is the state this desk was in today, and
            # in it the funded account had no notional ceiling and no margin
            # floor while the process went on sending.
            #
            # So the failure direction is inverted: no account reading means no
            # order. `snapshot()` below has always done this - it returns on
            # `acc is None` - and this is the same rule on the path that spends
            # money. The cost is real and is accepted: a terminal that blinks
            # out for a single poll costs a trade the book wanted, and on a
            # mirror that exists to measure slippage a missed entry is a gap in
            # the record. It is still the cheaper of the two errors, and unlike
            # the old behaviour it leaves a line saying which one happened.
            acc_now = mt5.account_info()
            if acc_now is None:
                log(out, "refused-size", sending_lots=vol, error=str(mt5.last_error()),
                    reason="the terminal did not answer account_info(), so neither the notional "
                           "ceiling nor the margin floor could be evaluated; nothing sent")
                print("REFUSED: the terminal is not answering account_info() - neither size "
                      "guard can run, so nothing was sent.", flush=True)
                nonlocal_blocked("terminal not answering account_info(); size guards cannot run")
                return False
            # Equity and notional are both in the ACCOUNT's currency here, and
            # the name says so. A bare `notional` compared against a bare
            # `equity` is exactly how the USD-against-USC error survived
            # review - see `notional_of`.
            equity_acct = getattr(acc_now, "equity", None)
            currency = getattr(acc_now, "currency", "?")
            if equity_acct is None or equity_acct <= 0:
                log(out, "refused-size", sending_lots=vol, equity=equity_acct, currency=currency,
                    reason="the account reports no equity, so no size can be justified against it; "
                           "nothing sent")
                print(f"REFUSED: account equity reads {equity_acct} {currency}. Nothing sent.",
                      flush=True)
                nonlocal_blocked(f"account equity reads {equity_acct} {currency}")
                return False
            notional_acct = notional_of(info, vol, price)
            if notional_acct is None:
                log(out, "refused-size", sending_lots=vol, symbol=args.symbol,
                    tick_size=getattr(info, "trade_tick_size", None),
                    tick_value=getattr(info, "trade_tick_value", None),
                    reason="the symbol did not give a usable tick value, so the order's notional "
                           "cannot be put in the account's currency; nothing sent")
                print(f"REFUSED: {args.symbol} gives no usable tick value, so this order's size "
                      f"cannot be checked against equity. Nothing sent.", flush=True)
                nonlocal_blocked(f"{args.symbol} gives no tick value; notional cannot be checked")
                return False
            if notional_acct > args.max_notional_ratio * equity_acct:
                log(out, "refused-size", asked_lots=float(book_open["lots"]), sending_lots=vol,
                    notional=round(notional_acct, 2), equity=round(equity_acct, 2),
                    currency=currency,
                    ratio=round(notional_acct / equity_acct, 2), limit=args.max_notional_ratio,
                    reason="order notional exceeds the sanity ceiling; nothing sent")
                print(f"REFUSED: {vol} lots = {notional_acct:,.0f} {currency} on "
                      f"{equity_acct:,.0f} {currency} equity "
                      f"({notional_acct / equity_acct:.1f}x, ceiling {args.max_notional_ratio}x). "
                      f"Nothing sent.", flush=True)
                nonlocal_blocked(f"{vol} lots is {notional_acct / equity_acct:.0f}x equity, over "
                                 f"the {args.max_notional_ratio}x ceiling")
                return False
            # What this order would tie up, and what the account would look
            # like holding it. `order_calc_margin` is the broker's own answer
            # rather than notional/leverage, which is wrong for any symbol with
            # a margin rate of its own. It answers in the account's currency,
            # as `margin` and `equity` are, so this arithmetic needs no
            # conversion - which is why the unit error was in the ceiling above
            # and not here.
            need = mt5.order_calc_margin(
                mt5.ORDER_TYPE_BUY if is_long else mt5.ORDER_TYPE_SELL, args.symbol, vol, price)
            used = getattr(acc_now, "margin", None)
            if need is None or used is None:
                # Same inversion as above and for the same reason: `need is
                # None` is the terminal declining to answer, and the old code
                # read that as permission. It is the likelier half of the
                # 2026-09-17 outage to survive, because `order_calc_margin`
                # needs the symbol loaded as well as the connection up.
                log(out, "refused-margin", lots=vol, margin_needed=need, margin_used=used,
                    error=str(mt5.last_error()),
                    reason="the terminal would not say what this order costs in margin, so the "
                           "account's margin floor could not be checked; nothing sent")
                print("REFUSED: the terminal will not price this order's margin, so the margin "
                      "floor cannot be checked. Nothing sent.", flush=True)
                nonlocal_blocked("terminal will not price margin; the margin floor cannot be checked")
                return False
            after = 100.0 * equity_acct / (used + need) if (used + need) > 0 else float("inf")
            if after < args.min_margin_level:
                log(out, "refused-margin", lots=vol, margin_needed=round(need, 2),
                    margin_used=round(used, 2), equity=round(equity_acct, 2), currency=currency,
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
        drift = None     # the account holds the right SIDE but not the right shape

        def drift_from_book(book_open: dict, held: list):
            """The account agrees with the book on side, but not on count or size.

            Returns a description, or None when the account matches. Recomputed
            every poll from what is actually held, so it can never be stale -
            unlike `blocked`, which remembers the last thing that went wrong.

            The reconciler used to correct SIDE and nothing else. Two positions
            on one book, or a volume that no longer matches, were left exactly
            as they were for as long as the book stayed open - so a mirror that
            doubled (two machines, two executors on one run id) never healed,
            and the desk went on showing a healthy account.

            THIS REPORTS AND DOES NOT CORRECT, which is a decision and not an
            oversight. Closing the extra position automatically is right in
            principle and wrong as a first version, for three reasons, and the
            first is the one that settles it:

            * Auto-correction makes the condition it is fixing WORSE in the
              case that causes it. The way a book ends up with two positions is
              two executors on one run id - and two auto-correcting executors
              see the same two positions, both close one, and the account goes
              flat; both then read "book open, terminal flat" and both open, so
              it is two again. That is an oscillation at the poll interval,
              paying the spread twice a cycle, forever. Reporting has no such
              mode: two reporting executors both write the same line and
              neither spends anything.
            * The failure directions are not comparable. Every other guard on
              this path can only stop the mirror from sending; correcting count
              or size is the first thing here that would CLOSE a live position
              or open an extra one on its own. If the detection is wrong, a
              report costs a false line on a screen and a correction costs a
              trade that a person did not authorise.
            * Correcting SIZE cannot be done without damage. A part-close or a
              close-and-reopen crystallises a result the book never took and
              re-enters at a price the book never saw, which destroys the one
              measurement this mirror exists to produce. The likeliest cause of
              a size mismatch is a partial fill, and that is answered where it
              happens rather than by trading the account back into shape.

            It is also not left forever. The drift clears itself the next time
            the book goes flat, because that path closes every position this
            magic holds - so the bound on how long a doubled account can
            persist is one trade, not indefinitely.

            Logged on CHANGE rather than every poll. A line every fifteen
            seconds for a condition that lasts hours is a log nobody reads by
            morning - the same reason `standing_out` is kept apart from
            `blocked`.
            """
            want, _ = clamp_volume(info, float(book_open["lots"]) * args.lot_scale)
            have = round(sum(p.volume for p in held), 8)
            step = info.volume_step or 0.01
            reasons = []
            if len(held) > 1:
                reasons.append(f"{len(held)} positions open on one book "
                               f"(tickets {', '.join(str(p.ticket) for p in held)})")
            if abs(have - want) > step / 2:
                reasons.append(f"holding {have} lots where the book asks for {want}")
            if not reasons:
                return None
            why = "; ".join(reasons)
            if why != drift:
                log(out, "drift", positions=len(held), lots_held=have, lots_wanted=want,
                    tickets=[p.ticket for p in held], detail=why,
                    reason="the account matches the book's side but not its shape; reported "
                           "and deliberately not corrected - a person decides, and it clears "
                           "itself when the book next goes flat")
                print(f"DRIFT: {why}. Nothing was changed; see the comment on "
                      f"drift_from_book.", flush=True)
            return why

        def snapshot(held, book_open) -> None:
            acc = mt5.account_info()
            # None means the terminal is not answering - it was closed, or
            # restarted out from under this process. Everything below reads
            # `acc` with getattr and would quietly write a snapshot full of
            # nulls, which the desk cannot tell from a quiet account.
            #
            # Measured 2026-09-17: the demo terminal was restarted, five
            # executors kept polling, and every one of them wrote
            # `login: null, balance: null, demo: false` every fifteen seconds.
            # `demo: false` is the one that bites - it is not "unknown", it is
            # the assertion that this is NOT a practice account, produced by
            # comparing None to a constant. On a desk that now has a funded
            # account, a disconnected demo reading as real money is the wrong
            # way round for that mistake to go.
            if acc is None:
                log(out, "no-account", error=str(mt5.last_error()))
                return
            realised, closed, fills = history_of(mt5, magic)
            tick = mt5.symbol_info_tick(args.symbol)
            pos = held[0] if held else None
            payload = {
                "at": int(time.time() * 1000),
                "account": account,
                "login": acc.login,
                "server": acc.server,
                # Three states, not two: a demo, a real account, and an
                # answer nobody has. The guard above means `acc` is real here,
                # so this is now only ever the first two.
                "demo": acc.trade_mode == mt5.ACCOUNT_TRADE_MODE_DEMO,
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
                # How far the terminal's clock runs ahead of UTC, measured this
                # poll. Here because the book's times are UTC and the deal
                # history's are the server's, and a reader comparing the two
                # without knowing the offset is making the mistake that kept
                # `already_taken` inert until 2026-09-17. `null` means it could
                # not be measured, which is also when the mirror refuses to
                # open - so this field says why a book is sitting out.
                "server_offset_ms": server_offset_ms(),
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
                # Recomputed from what the account actually holds, every poll,
                # so unlike `blocked` it can never describe a state that has
                # since resolved. `null` means the account matches the book in
                # side, count and size. See `drift_from_book` for why a drift
                # is reported here and not corrected.
                "drift": drift,
            }
            write_snapshot(snap_path, payload)

        while True:
            if stop_file.exists() or stop_all.exists():
                which = "STOP file" if stop_file.exists() else "desk-wide STOP file"
                # A list and not a generator: `all(close(p) for p in ...)`
                # would stop closing at the first refusal and leave the rest of
                # the positions open, which is the opposite of what a kill
                # switch is for.
                [close(p, which) for p in positions()]
                log(out, "stopped", reason=f"{which} present")
                # The last snapshot, and the reason this path has one.
                #
                # Until 2026-09-17 this returned here, so `broker.json` kept
                # whatever it said on the poll before the STOP file appeared -
                # a position that is now closed, claimed as open, for as long
                # as the directory exists. The desk reads that file, not this
                # log, so the screen went on showing a holding that was not
                # there and no reader could tell a clean stop from a failed one.
                #
                # `positions()` is asked AGAIN rather than reusing the list
                # above: what belongs in the record is what the account holds
                # after the close attempt, not what it held before. If a close
                # was refused the position is still in it, `blocked` still
                # carries the broker's reason from `send()`, and the pair says
                # "stopped while still holding, and here is why" - which is the
                # state a person has to act on.
                #
                # `standing_out` is cleared because it describes a trade being
                # sat out by a running mirror, and this one has stopped.
                standing_out = None
                snapshot(positions(), None)
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
                wrong_side = [p for p in held
                              if (p.type == mt5.POSITION_TYPE_BUY) != want_long]
                if wrong_side:
                    # ALL of them, then open once - and only if the close left
                    # the account flat.
                    #
                    # This was `for p in held: if wrong: close; open; break`,
                    # which closed the FIRST wrong-side position and
                    # immediately opened a new one. With two wrong-side
                    # positions held that leaves one wrong and one right, so
                    # the correction made the account less like the book than
                    # it found it and the next poll would do it again. Closing
                    # every position that contradicts the book is what "the
                    # side changed" means; opening is the separate question,
                    # and it is only safe to answer once nothing is left.
                    for p in wrong_side:
                        close(p, "side changed")
                    if not positions():
                        open_like(book_open, run)
                else:
                    drift = drift_from_book(book_open, held)
            snapshot(positions(), book_open)
            time.sleep(args.poll)
    except KeyboardInterrupt:
        return 0
    finally:
        mt5.shutdown()


if __name__ == "__main__":
    sys.exit(main())
