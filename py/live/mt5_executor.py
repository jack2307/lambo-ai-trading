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


def clamp_volume(info, raw: float) -> float:
    step = info.volume_step or 0.01
    vol = round(round(raw / step) * step, 8)
    return max(info.volume_min, min(info.volume_max, vol))


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

        def open_like(book_open: dict) -> bool:
            tick = mt5.symbol_info_tick(args.symbol)
            is_long = book_open["side"] == "LONG"
            vol = clamp_volume(info, float(book_open["lots"]) * args.lot_scale)
            req = {
                "action": mt5.TRADE_ACTION_DEAL, "symbol": args.symbol, "volume": vol,
                "type": mt5.ORDER_TYPE_BUY if is_long else mt5.ORDER_TYPE_SELL,
                "price": tick.ask if is_long else tick.bid, "deviation": args.deviation, "magic": magic,
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
                time.sleep(args.poll)
                continue
            if run is None:
                log(out, "run-missing", run=args.run)
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
            time.sleep(args.poll)
    except KeyboardInterrupt:
        return 0
    finally:
        mt5.shutdown()


if __name__ == "__main__":
    sys.exit(main())
