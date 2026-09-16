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

* `account_info().trade_mode != ACCOUNT_TRADE_MODE_DEMO` — the process
  exits. There is no flag that disables this check.
* `account_info().login != --login` — exits. A terminal that is not the
  one you named is not the one you meant.
* A `STOP` file at `data/paper/<run>/STOP` — closes any open position and
  exits. The kill switch.
* The terminal path is required (`--terminal`), so the executor never
  attaches to whichever terminal happens to be running.

The lock is tested by pointing it at the live terminal on this machine: it
must print the refusal and exit 3 without touching anything. That test is
in `docs/paper/DESIGN.md` and is repeated at every review.
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


def realised_of(mt5, magic: int) -> tuple:
    """What this book has actually BANKED on the account, and over how many exits.

    The paper book's `net_usd` is the rule executed perfectly at the bar's
    price. This is the same rule after a spread, a slip and a fill, and the
    difference between the two numbers is the entire reason for running a
    mirror at all - so the account view is not worth much without it.

    Deals are matched by the book's magic number, which is how one account
    carries several books without their results running together.

    The window is deliberately far wider than it needs to be. MT5 takes history
    bounds in SERVER time, not UTC, and this desk has been caught by that
    before; 400 days back and two days forward makes a three-hour offset
    irrelevant instead of making it a bug to remember.

    Commission is summed over every deal and profit and swap only over the
    closing ones: an entry deal carries a charge but no result, and counting
    its zero profit as a trade would inflate the count by exactly double.
    """
    now = dt.datetime.now()
    deals = mt5.history_deals_get(now - dt.timedelta(days=400), now + dt.timedelta(days=2))
    if deals is None:
        return None, 0
    total, closed = 0.0, 0
    for d in deals:
        if d.magic != magic:
            continue
        total += d.commission
        if d.entry != mt5.DEAL_ENTRY_IN:
            total += d.profit + d.swap
            closed += 1
    return round(total, 2), closed


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


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", required=True)
    ap.add_argument("--terminal", required=True, help="path to the DEMO terminal64.exe")
    ap.add_argument("--login", type=int, required=True, help="the demo account number the terminal must be logged into")
    ap.add_argument("--symbol", required=True)
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
    ap.add_argument("--dry-run", action="store_true", help="reconcile and log, send nothing")
    args = ap.parse_args()

    out = ROOT / "data" / "paper" / args.run / "executor.jsonl"
    stop_file = ROOT / "data" / "paper" / args.run / "STOP"

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
        # ---- the lock: demo only, and the demo you named ----
        if acc.trade_mode != mt5.ACCOUNT_TRADE_MODE_DEMO:
            log(out, "refused", reason="account is not a demo account", login=acc.login, server=acc.server, trade_mode=int(acc.trade_mode))
            print("REFUSED: this terminal is not logged into a demo account. Nothing was sent.", flush=True)
            return 3
        if acc.login != args.login:
            log(out, "refused", reason="account login differs from --login", login=acc.login, expected=args.login, server=acc.server)
            print(f"REFUSED: terminal login {acc.login} is not {args.login}. Nothing was sent.", flush=True)
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

        def open_like(book_open: dict) -> bool:
            tick = mt5.symbol_info_tick(args.symbol)
            is_long = book_open["side"] == "LONG"
            vol, clipped = clamp_volume(info, float(book_open["lots"]) * args.lot_scale)
            if clipped:
                log(out, "clipped", asked=float(book_open["lots"]) * args.lot_scale, sending=vol,
                    volume_min=info.volume_min, volume_max=info.volume_max)
            price = tick.ask if is_long else tick.bid
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

        snap_path = ROOT / "data" / "paper" / args.run / "broker.json"
        blocked = None  # why the last open was refused, if it was

        def snapshot(held, book_open) -> None:
            acc = mt5.account_info()
            realised, closed = realised_of(mt5, magic)
            tick = mt5.symbol_info_tick(args.symbol)
            pos = held[0] if held else None
            payload = {
                "at": int(time.time() * 1000),
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
                "realised": realised, "closed": closed,
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
            }
            write_snapshot(snap_path, payload)

        while True:
            if stop_file.exists():
                for p in positions():
                    close(p, "STOP file")
                log(out, "stopped", reason="STOP file present")
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
                open_like(book_open)
            elif book_open is not None and held:
                want_long = book_open["side"] == "LONG"
                for p in held:
                    if (p.type == mt5.POSITION_TYPE_BUY) != want_long:
                        close(p, "side changed")
                        open_like(book_open)
                        break
            snapshot(positions(), book_open)
            time.sleep(args.poll)
    except KeyboardInterrupt:
        return 0
    finally:
        mt5.shutdown()


if __name__ == "__main__":
    sys.exit(main())
