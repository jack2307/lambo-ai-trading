"""Pin the size and identity guards in mt5_executor against synthetic accounts.

    C:/Python39/python.exe py/live/size_guard_selftest.py

The functions under test are the ones the executor calls before it spends
money. A fake `MetaTrader5` module stands in for the terminal, `read_status`
for the API, and `mt5_executor.ROOT` points at a temporary directory, so this
touches no account, no terminal, no network and no file under `data/`.

Written 2026-09-17, the day the desk began mirroring five books into a funded
Vantage cent account (login 33705331, 10,000 USC = USD 100). An audit that
morning found three guards on that path that did not guard; this is what stops
them coming back. Each section names the failure it pins.

  1  THE CEILING MUST MEASURE ONE CURRENCY. `lots x contract_size x price` is
     the SYMBOL's profit currency - USD on every symbol this desk sends, the
     probe below confirms it - and `account_info().equity` is the ACCOUNT's,
     USC on a cent account. The executor compared one against the other, so
     `--max-notional-ratio 10.0` admitted 1000x: 20 lots of XAUUSD.sc at 4350
     is USD 87,000 against USC 10,000, which scored 8.7x and PASSED.

     This is the section that matters most, because the bug was invisible for
     as long as it existed. It was written and reviewed against `vantage-demo`
     (login 26108386), a STANDARD account in USD, where the two units agree by
     coincidence - so the demo could not have caught it and cannot catch the
     next one. That is why the numbers below are the CENT account's, measured,
     and why the demo's row is here only to pin that the fix changed nothing
     there.

  2  BOTH SIZE GUARDS MUST FAIL CLOSED. `getattr(account_info(), "equity",
     0.0)` yields 0.0 when the terminal is not answering, `if equity > 0` was
     then false, and both the notional ceiling and the margin floor were
     skipped while the order went out - logging nothing, because a skipped
     check writes no line. Measured 2026-09-17: a terminal restarted underneath
     five polling executors put every one of them in exactly that state. The
     checks here drive a whole poll of `main()` and assert on what reached
     `order_send`, not on what a helper returned, because the failure was in
     the wiring and not in any single function.

  3  THE STAMP MUST KNOW WHOSE DIRECTORY IT IS. `check_identity` wrote four
     fields and compared two, defaulted a stamp with no login to "matches",
     and permitted an unreadable one. The owner's standing requirement is that
     the trading history reads as one unbroken record; this function is the
     only thing enforcing it.

  4  THE KILL SWITCH MUST LEAVE A TRUE RECORD. The STOP path returned before
     writing a snapshot, so `broker.json` went on claiming a closed position
     was open for as long as the directory existed - and nothing downstream
     could tell a clean stop from a close the broker refused.

  5  THE TWO CLOCKS MUST BE RECONCILED. The book's times are UTC and the deal
     history's are the terminal's server time. `already_taken` compared one
     against the other inside a fifteen-minute window, across a three-hour
     offset, so it could not match - and on this desk it never once has. These
     checks are the only place it has ever fired.

  6  THE ACCOUNT HAS A SHAPE, NOT JUST A SIDE. The reconciler corrected side
     and nothing else, so two positions on one book, or a size that no longer
     matched, stood for as long as the book stayed open. These pin the
     DECISION as much as the detection: a drift is reported and deliberately
     not corrected, because two auto-correcting executors on one run id would
     close and re-open each other's positions at the poll interval.

  7  A PARTIAL FILL IS NOT A FILL. `TRADE_RETCODE_DONE_PARTIAL` sat in the
     same tuple as DONE and was called success, which cleared the refusal
     channel and left the account permanently smaller than the book.

The provenance of every symbol number is marked. MEASURED means read from a
terminal on the date given; DERIVED means computed from a measured value and
said so. Nothing here is a guess presented as a measurement.
"""

from __future__ import annotations

import json
import os
import shutil
import sys
import tempfile
import time
import types
from pathlib import Path

FAIL = 0


def check(name: str, ok: bool, detail: str = "") -> None:
    global FAIL
    if not ok:
        FAIL += 1
    print(f"  {'ok  ' if ok else 'FAIL'} {name}" + (f"   -- {detail}" if detail and not ok else ""))


def section(title: str) -> None:
    print(title)


# ---------------------------------------------------------------------------
# a terminal that answers whatever a scenario needs it to
# ---------------------------------------------------------------------------

class Obj:
    """Anything MetaTrader5 returns as a named tuple."""

    def __init__(self, **kw):
        self.__dict__.update(kw)


def make_mt5():
    m = types.ModuleType("MetaTrader5")
    m.ACCOUNT_TRADE_MODE_DEMO = 0
    m.TRADE_ACTION_DEAL = 1
    m.ORDER_TYPE_BUY = 0
    m.ORDER_TYPE_SELL = 1
    m.POSITION_TYPE_BUY = 0
    m.ORDER_TIME_GTC = 0
    m.ORDER_FILLING_IOC = 1
    m.TRADE_RETCODE_DONE = 10009
    m.TRADE_RETCODE_DONE_PARTIAL = 10010
    m.TRADE_RETCODE_PLACED = 10008
    m.DEAL_ENTRY_IN = 0
    m.DEAL_TYPE_BUY = 0

    m.sent = []          # every request that reached order_send
    m.account = None     # the scenario's account; None means a silent terminal
    m.info = None
    m.margin = 5.0       # order_calc_margin's answer; None means it declines
    m.price = 4311.85
    # How far the fake terminal's clock runs ahead of UTC. +3h is what the
    # real Vantage server was measured at on 2026-09-16 and 2026-09-17.
    m.server_offset_s = 3 * 3600
    m.deals = []

    m.initialize = lambda **kw: True
    m.shutdown = lambda: None
    m.last_error = lambda: (-10004, "no IPC connection")
    m.account_info = lambda: m.account
    m.symbol_select = lambda s, on: True
    m.terminal_info = lambda: Obj(trade_allowed=True)
    m.symbol_info = lambda s: m.info
    # `time` is SERVER epoch seconds, which is what a real tick carries and
    # what the executor measures the clock offset from.
    m.symbol_info_tick = lambda s: Obj(ask=m.price, bid=m.price - 0.2,
                                       time=int(time.time()) + m.server_offset_s)
    m.positions_get = lambda **kw: []
    m.history_deals_get = lambda a, b: m.deals
    m.order_calc_margin = lambda t, s, v, p: m.margin

    def order_send(req):
        m.sent.append(req)
        return Obj(retcode=m.TRADE_RETCODE_DONE, comment="ok", order=1, deal=1,
                   price=req["price"], volume=req["volume"])

    m.order_send = order_send
    return m


MT5 = make_mt5()
# Installed before the import so that `import MetaTrader5 as mt5` inside
# main() finds this one. The executor imports nothing from MT5 at module
# level, so this is the only place it can be intercepted.
sys.modules["MetaTrader5"] = MT5

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import mt5_executor as X  # noqa: E402


# ---------------------------------------------------------------------------
# the symbols, and where each number came from
# ---------------------------------------------------------------------------
#
# MEASURED 2026-09-17, read-only `symbol_info` against the funded cent account
# (login 33705331, currency USC, balance 10,000) on C:/MT5-cent. No order was
# sent to obtain these.
#
#   symbol       contract  tick_size  tick_value  currency_profit
#   XAUUSD.sc    1         0.01       1           USD
#   BTCUSD.sc    0.01      0.01       0.01        USD
#   EURUSD.sc    1000      0.00001    1           USD
#
# `currency_profit` reading USD on all three WHILE the account is USC is the
# whole bug in one line, and it is why the conversion reads no currency name:
# a name table would have had to know about both sides.
#
# XAUUSD on `vantage-demo` (login 26108386): contract 100 is MEASURED
# 2026-09-16 (it is in that account's broker.json). Its tick_size/tick_value
# are DERIVED - a standard USD account where one cent of gold on 100 oz is one
# dollar - and the row exists only to pin that the fix is a no-op there.
#
# XAGUSD.sc: contract 50 is MEASURED 2026-09-16 (config/default.toml records
# the measurement and why it mattered). Its tick fields are DERIVED from the
# 100:1 cent relation the three probed symbols all show. It is here as a
# fourth shape for the property check, not as an account fact.

# name -> (contract, tick_size, tick_value, account units per USD, price)
SYMBOLS = {
    "XAUUSD.sc": (1.0, 0.01, 1.0, 100.0, 4311.85),
    "BTCUSD.sc": (0.01, 0.01, 0.01, 100.0, 76477.41),
    "EURUSD.sc": (1000.0, 0.00001, 1.0, 100.0, 1.14716),
    "XAGUSD.sc": (50.0, 0.001, 5.0, 100.0, 52.0),
    "XAUUSD": (100.0, 0.01, 1.0, 1.0, 4311.85),
}


def info_for(sym: str) -> Obj:
    contract, tick_size, tick_value, _, _ = SYMBOLS[sym]
    return Obj(trade_contract_size=contract, trade_tick_size=tick_size,
               trade_tick_value=tick_value,
               volume_min=0.01, volume_step=0.01, volume_max=100.0)


def old_notional(info, vol: float, price: float) -> float:
    """What `notional_of` computed before 2026-09-17, kept to pin the delta."""
    return vol * info.trade_contract_size * price


# ---------------------------------------------------------------------------
# 1 - the ceiling measures one currency
# ---------------------------------------------------------------------------

def the_ceiling_measures_one_currency() -> None:
    section("the notional ceiling's units")

    # The demo first. The old arithmetic was RIGHT on a standard USD account,
    # and a fix that moved a number there would be a regression on the only
    # account with a year of history behind it.
    info = info_for("XAUUSD")
    new, old = X.notional_of(info, 1.0, 4311.85), old_notional(info, 1.0, 4311.85)
    check("standard USD demo: the fix is a no-op (431,185 either way)",
          abs(new - old) < 1e-6 and abs(new - 431185.0) < 1e-6, f"new={new} old={old}")

    # The audit's worked example, in the units the funded account holds.
    info = info_for("XAUUSD.sc")
    equity_usc, ceiling = 10000.0, 10.0
    new = X.notional_of(info, 20.0, 4350.0)
    old = old_notional(info, 20.0, 4350.0)
    check("cent account: 20 lots XAUUSD.sc at 4350 is 8,700,000 USC",
          new == 8700000.0, f"got {new}")
    check("the bug reproduces: the old number is 87,000 and PASSES 10x at 8.7x",
          old == 87000.0 and old <= ceiling * equity_usc, f"old {old}")
    check("the fix bites: the same order is 870x and is refused",
          new > ceiling * equity_usc and abs(new / equity_usc - 870.0) < 1e-9,
          f"new ratio {new / equity_usc}")

    # The property, stated independently of how notional_of computes it: the
    # answer must be the profit-currency notional times the account's units per
    # dollar. Checked on every shape the desk trades, including a 0.01 contract
    # (BTC) and a 1e-05 tick (euro), which are where a formula with the factors
    # the wrong way round would show up.
    for sym, (contract, _, _, units_per_usd, price) in SYMBOLS.items():
        info = info_for(sym)
        got = X.notional_of(info, 1.0, price)
        want = 1.0 * contract * price * units_per_usd
        check(f"{sym}: lots x contract x price x {units_per_usd:g} units/USD",
              abs(got - want) < 1e-6, f"got {got} want {want}")

    # And the relation that made the bug a FACTOR error rather than a rounding
    # one: on this cent account every .sc symbol is exactly 100x the old
    # number, and the demo's is exactly 1x. If a future account breaks this,
    # the row above will say so first.
    for sym in ("XAUUSD.sc", "BTCUSD.sc", "EURUSD.sc"):
        info = info_for(sym)
        price = SYMBOLS[sym][4]
        ratio = X.notional_of(info, 1.0, price) / old_notional(info, 1.0, price)
        check(f"{sym}: the old code was wrong by exactly 100x", abs(ratio - 100.0) < 1e-9,
              f"ratio {ratio}")

    # A symbol that will not price a tick cannot be sized against equity at
    # all. None means "cannot check" and the caller refuses on it; the old code
    # returned a number in an unknown currency, which is worse than no number.
    check("tick_size 0 -> None",
          X.notional_of(Obj(trade_contract_size=1.0, trade_tick_size=0.0,
                            trade_tick_value=1.0), 1, 1) is None)
    check("tick_value 0 -> None",
          X.notional_of(Obj(trade_contract_size=1.0, trade_tick_size=0.01,
                            trade_tick_value=0.0), 1, 1) is None)
    check("the fields missing entirely -> None",
          X.notional_of(Obj(trade_contract_size=1.0), 1, 1) is None)
    check("the fields present but None -> None",
          X.notional_of(Obj(trade_tick_size=None, trade_tick_value=None), 1, 1) is None)


# ---------------------------------------------------------------------------
# 2 - both size guards fail closed
# ---------------------------------------------------------------------------

BOOK = {"side": "LONG", "lots": 0.05, "entry_price": 4311.85, "stop": 4301.85,
        "target": 4331.85, "entry_time": 1_000_000}
RUN = {"id": "t", "tf": "15m", "last_bar_time": 1_000_000, "open": BOOK}

# A cent account as the terminal reports it. 0.05 lots of XAUUSD.sc on 10,000
# USC is 2.2x equity - inside the 10x ceiling and inside the desk's own 300%
# rule, so this is an order the mirror is supposed to send.
#
# `trade_mode` is DEMO deliberately. What these checks vary is the account's
# CURRENCY, which is what the size guards were wrong about; the real-money
# permission wall is a different guard with its own test in docs/paper/
# DESIGN.md, and driving main() through it here would make this file depend on
# an `[[account]]` block in config/accounts.toml that grants real trading - a
# fixture no test should need and nobody should be tempted to add.
CENT = Obj(login=33705331, server="VantageMarkets-Live 21", currency="USC",
           trade_mode=0, balance=10000.0, equity=10000.0, margin=0.0)


class Flaky:
    """Answers the startup reads, then goes silent.

    Not `always None`: the executor reads the account once at startup to check
    the login and the permission, so a terminal that was never answering would
    exit long before any order. The state that was dangerous is the one that
    was MEASURED - a terminal restarted underneath a process that had already
    started and was polling.
    """

    def __init__(self, acc, answers: int):
        self.acc, self.answers, self.n = acc, answers, 0

    def __call__(self):
        self.n += 1
        return self.acc if self.n <= self.answers else None


def drive(account, symbol: str = "XAUUSD.sc", margin=5.0, lots: float = 0.05) -> tuple:
    """One poll of `main()` against a temporary ROOT.

    Returns `(rc, requests that reached order_send, rows written to
    executor.jsonl)`. `time.sleep` raises KeyboardInterrupt so the loop runs
    exactly once and main() returns through its own handler.
    """
    tmp = Path(tempfile.mkdtemp(prefix="sgst-"))
    real_root, real_status, real_sleep = X.ROOT, X.read_status, X.time.sleep
    argv = sys.argv
    try:
        MT5.sent = []
        MT5.account = account
        MT5.info = info_for(symbol)
        MT5.margin = margin
        MT5.price = SYMBOLS[symbol][4]

        book = dict(BOOK, lots=lots)
        X.ROOT = tmp
        X.read_status = lambda api, run: dict(RUN, open=book)

        def stop_after_one_poll(_):
            raise KeyboardInterrupt

        X.time.sleep = stop_after_one_poll
        sys.argv = ["mt5_executor.py", "--run=t", "--terminal=x", "--login=33705331",
                    f"--symbol={symbol}", "--account=acct"]
        rc = X.main()

        out = tmp / "data" / "live" / "acct" / "t" / "executor.jsonl"
        rows = []
        if out.exists():
            rows = [json.loads(line) for line in
                    out.read_text(encoding="utf-8").splitlines() if line.strip()]
        return rc, list(MT5.sent), rows
    finally:
        X.ROOT, X.read_status, X.time.sleep = real_root, real_status, real_sleep
        sys.argv = argv
        shutil.rmtree(tmp, ignore_errors=True)


def both_size_guards_fail_closed() -> None:
    section("what happens when the terminal stops answering")

    # A guard that refuses everything is not a guard, it is an outage. This
    # control is why the checks below mean something.
    _, sent, rows = drive(CENT)
    check("control: a healthy account still sends the order", len(sent) == 1,
          f"sent {len(sent)}; rows {[r['kind'] for r in rows]}")

    # The measured state: the terminal answers the startup reads and then goes.
    MT5.account_info = Flaky(CENT, answers=1)
    try:
        _, sent, rows = drive(CENT)
    finally:
        MT5.account_info = lambda: MT5.account
    check("terminal silent mid-poll: nothing is sent", len(sent) == 0, f"sent {sent}")
    check("terminal silent mid-poll: the refusal is written down, not skipped",
          any(r["kind"] == "refused-size" and "account_info" in r.get("reason", "")
              for r in rows),
          f"rows {[r['kind'] for r in rows]}")

    # Equity that reads zero is not a licence to send either. It is also not
    # hypothetical: the first cent account this desk opened reported balance
    # 0.0 before it was funded (data/live/_pretest-27488225).
    _, sent, rows = drive(Obj(login=33705331, server="s", currency="USC", trade_mode=0,
                              balance=0.0, equity=0.0, margin=0.0))
    check("equity 0: nothing is sent", len(sent) == 0, f"sent {sent}")
    check("equity 0: logged as a refusal",
          any(r["kind"] == "refused-size" for r in rows),
          f"rows {[r['kind'] for r in rows]}")

    # order_calc_margin declining to answer is the same shape as account_info
    # declining, and is the likelier half to survive an outage because it needs
    # the symbol loaded as well as the connection up.
    _, sent, rows = drive(CENT, margin=None)
    check("margin unpriceable: nothing is sent", len(sent) == 0, f"sent {sent}")
    check("margin unpriceable: logged as a refusal",
          any(r["kind"] == "refused-margin" for r in rows),
          f"rows {[r['kind'] for r in rows]}")

    # The ceiling end to end, in the account's own units. Under the old
    # arithmetic this order went out.
    _, sent, rows = drive(CENT, lots=100.0)
    refusals = [r for r in rows if r["kind"] == "refused-size"]
    check("an order far over the ceiling: nothing is sent", len(sent) == 0, f"sent {sent}")
    check("...and the record names the currency beside the numbers",
          bool(refusals) and refusals[0].get("currency") == "USC", f"{refusals[:1]}")
    check("...and the ratio is the account-unit one, not the 100x-flattering one",
          bool(refusals) and refusals[0].get("ratio", 0) > 100, f"{refusals[:1]}")

    # The `started` line carries what the ceiling is computed from, so the
    # question "is this measuring the right currency?" is answerable from the
    # log alone. On 2026-09-17 it was not, and that is what made the bug survive.
    _, _, rows = drive(CENT)
    started = next((r for r in rows if r["kind"] == "started"), {})
    check("started names the account's currency", started.get("currency") == "USC", f"{started}")
    check("started carries the tick fields the conversion uses",
          started.get("tick_size") == 0.01 and started.get("tick_value") == 1.0, f"{started}")
    check("started says what one lot is worth, in the same units as balance",
          started.get("one_lot_at") == 431185.0,
          f"one_lot_at {started.get('one_lot_at')} vs balance {started.get('balance')}")


# ---------------------------------------------------------------------------
# 3 - the stamp knows whose directory it is
# ---------------------------------------------------------------------------

REAL = Obj(login=33705331, server="VantageMarkets-Live 21", currency="USC", trade_mode=1)
GOOD = {"account": "acct", "login": 33705331, "server": "VantageMarkets-Live 21",
        "currency": "USC", "demo": False}


def stamped(stamp) -> str:
    """`check_identity` against a directory holding `stamp`, or none at all."""
    d = Path(tempfile.mkdtemp(prefix="idst-"))
    try:
        path = d / "identity.json"
        if stamp is not None:
            path.write_text(stamp if isinstance(stamp, str) else json.dumps(stamp),
                            encoding="utf-8")
        return X.check_identity(path, REAL, "acct")
    finally:
        shutil.rmtree(d, ignore_errors=True)


def without(key: str) -> dict:
    return {k: v for k, v in GOOD.items() if k != key}


def the_stamp_knows_whose_directory_it_is() -> None:
    section("check_identity")

    check("no stamp yet: permitted, and written", stamped(None) == "")
    check("the same account: permitted", stamped(GOOD) == "")
    check("a different login: refused",
          "already holds the record of login" in stamped(dict(GOOD, login=26108386)))

    # The finding. A login number is unique to a SERVER, not to a broker, and
    # Vantage runs enough of them that a launcher typo reaches this.
    check("the same login number on a different server: refused",
          "different account" in stamped(dict(GOOD, server="VantageMarkets-Demo")))

    # `was.get("login", now["login"])` used to stand here: a stamp with no
    # login defaulted to the live one and was then compared against itself.
    check("a stamp naming no login: refused (it used to default to 'matches')",
          "records no usable login" in stamped(without("login")))
    check("a stamp whose login is not a number: refused",
          "records no usable login" in stamped(dict(GOOD, login="not-a-number")))

    # Permitted, because `login` already identifies the account and a missing
    # FIELD is a stamp-format gap rather than evidence of a second account.
    check("a stamp with no server: permitted, login still pins it",
          stamped(without("server")) == "")
    check("a stamp with no demo flag: permitted", stamped(without("demo")) == "")
    check("demo where the terminal is real: refused",
          "was started as" in stamped(dict(GOOD, demo=True)))

    # The decision, pinned so that a later reader does not "fix" it: currency
    # is a property of an account, not its identity, and a redenomination must
    # not produce a refusal indistinguishable from a real mismatch.
    check("a different currency on the same login and server: permitted, by decision",
          stamped(dict(GOOD, currency="USD")) == "")

    # The direction chosen for a stamp that cannot be read. The question is not
    # "is there evidence of a mismatch" but "do we know whose directory this
    # is", and permitting made damaging the file the way to defeat the guard.
    check("a corrupt stamp: REFUSED (it used to be permitted)",
          "cannot be read" in stamped("{not json at all"))
    check("a stamp that is a list, not a record: refused",
          "does not contain an account record" in stamped("[1,2,3]"))
    check("a stamp that is a bare string: refused",
          "does not contain an account record" in stamped('"just a string"'))


# ---------------------------------------------------------------------------
# 4 - the kill switch leaves a true record behind it
# ---------------------------------------------------------------------------

def stop_leaves_a_true_record() -> None:
    """The STOP path used to return before writing a snapshot.

    So `broker.json` kept whatever it said on the poll before the file
    appeared - a closed position claimed as open, for as long as the directory
    exists. The desk and the Telegram watch read that file, not executor.jsonl,
    so nothing downstream could tell a clean stop from a refused one.
    """
    section("the kill switch's last snapshot")

    held = Obj(ticket=1, type=MT5.POSITION_TYPE_BUY, volume=0.05, price_open=4311.85,
               price_current=4311.85, sl=4301.85, tp=4331.85, profit=0.0, swap=0.0,
               time=1_000_000, magic=X.magic_for("t"), symbol="XAUUSD.sc")

    def with_stop(close_succeeds: bool) -> dict:
        """Drive one poll with a STOP file present and one position open."""
        tmp = Path(tempfile.mkdtemp(prefix="sgst-stop-"))
        real_root, real_status = X.ROOT, X.read_status
        real_send, argv = MT5.order_send, sys.argv
        try:
            MT5.sent = []
            MT5.account = CENT
            MT5.info = info_for("XAUUSD.sc")
            MT5.margin = 5.0
            MT5.price = SYMBOLS["XAUUSD.sc"][4]
            # The position is held until a close succeeds, exactly as the
            # terminal would report it.
            state = {"open": True}
            MT5.positions_get = lambda **kw: ([held] if state["open"] else [])

            def send_or_refuse(req):
                MT5.sent.append(req)
                if close_succeeds:
                    state["open"] = False
                    return Obj(retcode=MT5.TRADE_RETCODE_DONE, comment="ok", order=1,
                               deal=1, price=req["price"])
                return Obj(retcode=10027, comment="AutoTrading disabled by client",
                           order=0, deal=0, price=0.0)

            MT5.order_send = send_or_refuse
            X.ROOT = tmp
            X.read_status = lambda api, run: dict(RUN)
            here = tmp / "data" / "live" / "acct" / "t"
            here.mkdir(parents=True, exist_ok=True)
            (here / "STOP").write_text("", encoding="utf-8")
            sys.argv = ["mt5_executor.py", "--run=t", "--terminal=x", "--login=33705331",
                        "--symbol=XAUUSD.sc", "--account=acct"]
            rc = X.main()
            snap = here / "broker.json"
            return {"rc": rc, "sent": list(MT5.sent),
                    "snapshot": json.loads(snap.read_text(encoding="utf-8"))
                    if snap.exists() else None}
        finally:
            X.ROOT, X.read_status = real_root, real_status
            MT5.order_send, sys.argv = real_send, argv
            MT5.positions_get = lambda **kw: []
            shutil.rmtree(tmp, ignore_errors=True)

    ok = with_stop(close_succeeds=True)
    check("a STOP file closes the open position", len(ok["sent"]) == 1, f"{ok['sent']}")
    check("a snapshot is written on the way out", ok["snapshot"] is not None)
    check("and it says the position is gone, not still open",
          (ok["snapshot"] or {}).get("position") is None,
          f"{(ok['snapshot'] or {}).get('position')}")
    check("a clean stop leaves nothing in `blocked`",
          (ok["snapshot"] or {}).get("blocked") is None,
          f"{(ok['snapshot'] or {}).get('blocked')}")

    # The case worth having a record of: the kill switch was thrown and the
    # broker would not take the close. The old code returned with the screen
    # showing a healthy mirror.
    bad = with_stop(close_succeeds=False)
    check("a refused close still writes a snapshot", bad["snapshot"] is not None)
    check("and it still shows the position, because it is still held",
          (bad["snapshot"] or {}).get("position") is not None)
    check("and `blocked` carries the broker's refusal",
          "10027" in str((bad["snapshot"] or {}).get("blocked")),
          f"{(bad['snapshot'] or {}).get('blocked')}")


# ---------------------------------------------------------------------------
# 5 - the two clocks, and the trade that was already taken
# ---------------------------------------------------------------------------

MAGIC = X.magic_for("t")


def deal(pos_id: int, entry_in: bool, server_ms: int, price: float, lots: float,
         is_buy: bool, profit: float = 0.0, comment: str = "") -> Obj:
    """One MT5 deal. `time_msc` is SERVER time, as the terminal reports it."""
    return Obj(magic=MAGIC, position_id=pos_id, entry=0 if entry_in else 1,
               type=0 if is_buy else 1, time_msc=server_ms, price=price,
               volume=lots, profit=profit, swap=0.0, commission=0.0, comment=comment)


def closed_trade(entry_utc_ms: int, held_ms: int, is_buy: bool = True,
                 price: float = 4311.85, lots: float = 0.05,
                 comment: str = "sl") -> list:
    """A round trip, given when it was entered in UTC.

    The deals carry SERVER time, because that is what the terminal hands over
    and what the executor now has to correct for.
    """
    off = MT5.server_offset_s * 1000
    return [deal(77, True, entry_utc_ms + off, price, lots, is_buy),
            deal(77, False, entry_utc_ms + held_ms + off, price - 5, lots, is_buy,
                 profit=-5.0, comment=comment)]


def the_two_clocks() -> None:
    """`already_taken` compared a UTC stamp against a server stamp.

    The book's `entry_time` is UTC - `mt5_bars.py` converts every bar stamp
    with `to_utc_ms` before posting it - while `history_of` reports `d.time_msc`
    raw, which is SERVER time. A fifteen-minute window across a three-hour
    offset cannot match, and the auditor's note is the evidence: "already-taken"
    has never appeared in any log on this desk. That reads as never happened
    and means never worked.

    MEASURED 2026-09-16, the one trade that appears on both sides of this
    repo's own records: executor.jsonl logs the fill for SHORT 0.06 at 4344.46
    at 13:46:29 UTC, and the same trade's `entryTime` in broker.json is
    16:46:27. Three hours, to the second.
    """
    section("the book's clock against the terminal's")

    book_entry = BOOK["entry_time"]

    # Nothing in the history: the mirror opens, as it always did.
    MT5.deals = []
    try:
        _, sent, rows = drive(CENT)
        check("no history for this book: the order goes out", len(sent) == 1,
              f"sent {len(sent)}; {[r['kind'] for r in rows]}")

        # The state this guard exists for: the broker stopped the trade out
        # before the book's bar closed, so the book still says LONG and the
        # account is flat. Re-opening would take the same trade twice.
        MT5.deals = closed_trade(book_entry, held_ms=5 * 60_000)
        _, sent, rows = drive(CENT)
        check("the broker already closed this book position: NOTHING is re-opened",
              len(sent) == 0, f"sent {sent}")
        check("...and it is logged as already-taken, which had never once fired",
              any(r["kind"] == "already-taken" for r in rows),
              f"{[r['kind'] for r in rows]}")
        taken = next((r for r in rows if r["kind"] == "already-taken"), {})
        check("...and the log carries the measured offset, so the clocks are answerable",
              taken.get("server_offset_ms") == MT5.server_offset_s * 1000,
              f"{taken.get('server_offset_ms')}")

        # The regression that pins the bug itself. Under the OLD test -
        # `entry <= got < entry + step` against a RAW server stamp - this fill
        # sits three hours past the window and is missed, so the mirror
        # re-enters. Asserted here so that reverting the correction fails.
        raw = book_entry + MT5.server_offset_s * 1000
        step = 900_000
        check("the old test would have missed this fill (this is the bug)",
              not (book_entry <= raw < book_entry + step),
              f"raw {raw} vs window [{book_entry}, {book_entry + step})")

        # The retry storm, measured 2026-09-16: the terminal answered 10027 for
        # sixteen minutes, so the fill landed TWO bars after the book's entry
        # stamp. Any window keyed to one bar would miss it.
        MT5.deals = closed_trade(book_entry + 2 * step, held_ms=60_000)
        _, sent, _ = drive(CENT)
        check("a fill two bars late is still recognised, not re-opened",
              len(sent) == 0, f"sent {sent}")

        # A trade from BEFORE the book entered this position belongs to the
        # previous one and must not stand the mirror down.
        MT5.deals = closed_trade(book_entry - 4 * step, held_ms=60_000)
        _, sent, _ = drive(CENT)
        check("an older trade of this book does not block the current one",
              len(sent) == 1, f"sent {len(sent)}")

        # A closed trade on the other side, entered after the book's stamp.
        # Not treated as this position's fill, and said out loud rather than
        # silently skipped.
        MT5.deals = closed_trade(book_entry, held_ms=60_000, is_buy=False)
        _, sent, rows = drive(CENT)
        check("a closed trade on the WRONG side is not read as this fill",
              len(sent) == 1, f"sent {len(sent)}")
        check("...and the anomaly is recorded",
              any(r["kind"] == "history-anomaly" for r in rows),
              f"{[r['kind'] for r in rows]}")

        # Fail closed, both ways the clock can go unreadable.
        MT5.deals = []
        real_tick = MT5.symbol_info_tick
        try:
            MT5.symbol_info_tick = lambda s: Obj(ask=MT5.price, bid=MT5.price - 0.2)
            _, sent, rows = drive(CENT)
            check("no server clock on the tick: nothing is sent",
                  len(sent) == 0, f"sent {sent}")
            check("...and the refusal says the history is unknown",
                  any(r["kind"] == "refused-unknown-history" for r in rows),
                  f"{[r['kind'] for r in rows]}")

            # A tick two days stale - a Friday close read on a Monday - would
            # otherwise be measured as a two-day clock offset.
            MT5.symbol_info_tick = lambda s: Obj(
                ask=MT5.price, bid=MT5.price - 0.2, time=int(time.time()) - 2 * 86400)
            _, sent, rows = drive(CENT)
            check("a stale tick is rejected, not read as a huge offset",
                  len(sent) == 0 and any(r["kind"] == "refused-unknown-history" for r in rows),
                  f"sent {sent}; {[r['kind'] for r in rows]}")
        finally:
            MT5.symbol_info_tick = real_tick

        # The terminal declining to hand over deal history is "cannot tell",
        # not "nothing has happened".
        real_deals_get = MT5.history_deals_get
        try:
            MT5.history_deals_get = lambda a, b: None
            _, sent, rows = drive(CENT)
            check("no deal history: nothing is sent",
                  len(sent) == 0 and any(r["kind"] == "refused-unknown-history" for r in rows),
                  f"sent {sent}; {[r['kind'] for r in rows]}")
        finally:
            MT5.history_deals_get = real_deals_get
    finally:
        MT5.deals = []


# ---------------------------------------------------------------------------
# 6 - the account's shape, not just its side
# ---------------------------------------------------------------------------

def position(ticket: int, is_buy: bool = True, lots: float = 0.05) -> Obj:
    return Obj(ticket=ticket, type=0 if is_buy else 1, volume=lots,
               price_open=4311.85, price_current=4311.85, sl=4301.85, tp=4331.85,
               profit=0.0, swap=0.0, time=1_000_000, magic=MAGIC, symbol="XAUUSD.sc")


def drive_holding(held: list, lots: float = 0.05) -> dict:
    """One poll with the book LONG and the terminal already holding `held`."""
    tmp = Path(tempfile.mkdtemp(prefix="sgst-drift-"))
    real_root, real_status, real_sleep = X.ROOT, X.read_status, X.time.sleep
    argv = sys.argv
    try:
        MT5.sent = []
        MT5.account = CENT
        MT5.info = info_for("XAUUSD.sc")
        MT5.margin = 5.0
        MT5.price = SYMBOLS["XAUUSD.sc"][4]
        MT5.positions_get = lambda **kw: list(held)
        X.ROOT = tmp
        X.read_status = lambda api, run: dict(RUN, open=dict(BOOK, lots=lots))

        def stop_after_one_poll(_):
            raise KeyboardInterrupt

        X.time.sleep = stop_after_one_poll
        sys.argv = ["mt5_executor.py", "--run=t", "--terminal=x", "--login=33705331",
                    "--symbol=XAUUSD.sc", "--account=acct"]
        X.main()
        here = tmp / "data" / "live" / "acct" / "t"
        snap = here / "broker.json"
        out = here / "executor.jsonl"
        rows = [json.loads(line) for line in out.read_text(encoding="utf-8").splitlines()
                if line.strip()] if out.exists() else []
        return {"sent": list(MT5.sent), "rows": rows,
                "snapshot": json.loads(snap.read_text(encoding="utf-8"))
                if snap.exists() else None}
    finally:
        X.ROOT, X.read_status, X.time.sleep = real_root, real_status, real_sleep
        sys.argv = argv
        MT5.positions_get = lambda **kw: []
        shutil.rmtree(tmp, ignore_errors=True)


def the_account_has_a_shape() -> None:
    """The reconciler corrected SIDE and nothing else.

    Two positions on one book, or a volume that no longer matches, were left
    exactly as they were for as long as the book stayed open. A mirror that
    doubled never healed and the desk went on showing a healthy account.

    What these pin is the DECISION as much as the detection: the drift is
    reported and deliberately not corrected. See `drift_from_book` - two
    auto-correcting executors on one run id would close each other's positions
    and re-open them at the poll interval, paying the spread twice a cycle,
    which is a worse failure than the one being fixed.
    """
    section("the account's shape against the book's")

    clean = drive_holding([position(1)])
    check("one position of the right side and size: no drift",
          (clean["snapshot"] or {}).get("drift") is None,
          f"{(clean['snapshot'] or {}).get('drift')}")
    check("...and nothing is sent", len(clean["sent"]) == 0, f"{clean['sent']}")

    # The doubled mirror.
    two = drive_holding([position(1), position(2)])
    check("two positions on one book: reported as drift",
          "2 positions" in str((two["snapshot"] or {}).get("drift")),
          f"{(two['snapshot'] or {}).get('drift')}")
    check("...and written to the log once",
          any(r["kind"] == "drift" for r in two["rows"]),
          f"{[r['kind'] for r in two['rows']]}")
    check("...and NOTHING is closed or opened to correct it",
          len(two["sent"]) == 0, f"{two['sent']}")

    # A size that no longer matches the book.
    small = drive_holding([position(1, lots=0.01)])
    check("a volume that does not match the book: reported as drift",
          "lots" in str((small["snapshot"] or {}).get("drift")),
          f"{(small['snapshot'] or {}).get('drift')}")
    check("...and still nothing is traded to correct it",
          len(small["sent"]) == 0, f"{small['sent']}")

    # The side correction, which DOES act - and the bug in how it acted. With
    # two wrong-side positions the old code closed the first and immediately
    # opened, leaving one wrong and one right.
    both_wrong = drive_holding([position(1, is_buy=False), position(2, is_buy=False)])
    closes = [r for r in both_wrong["rows"]
              if r["kind"] == "order" and "close" in str(r.get("action"))]
    opens = [r for r in both_wrong["rows"]
             if r["kind"] == "order" and "open" in str(r.get("action"))]
    check("two wrong-side positions: BOTH are closed, not just the first",
          len(closes) == 2, f"closes {len(closes)}")
    check("...and no new position is opened while any is still held",
          len(opens) == 0, f"opens {len(opens)}")


# ---------------------------------------------------------------------------
# 7 - a partial fill is not a fill
# ---------------------------------------------------------------------------

def a_partial_fill_is_not_a_fill() -> None:
    """`TRADE_RETCODE_DONE_PARTIAL` sat in the same tuple as DONE.

    So a broker that filled part of the volume was recorded as a clean
    success, the refusal channel was CLEARED, and the account was left
    permanently smaller than the book with the desk showing a healthy mirror.
    Every trade measured afterwards is measured against a position the book
    never took.
    """
    section("a partial fill")

    real_send = MT5.order_send

    def partial_send(req):
        MT5.sent.append(req)
        # The broker takes a fifth of what was asked for.
        return Obj(retcode=MT5.TRADE_RETCODE_DONE_PARTIAL, comment="partial",
                   order=9, deal=9, price=req["price"], volume=round(req["volume"] / 5, 8))

    try:
        MT5.order_send = partial_send
        _, sent, rows = drive(CENT, lots=0.05)
        orders = [r for r in rows if r["kind"].startswith("order")]
        check("one order was sent", len(sent) == 1, f"{len(sent)}")
        check("a partial fill is NOT logged as a clean order",
              all(r["kind"] != "order" for r in orders), f"{[r['kind'] for r in orders]}")
        check("it has its own kind, and is not called a failure either",
              any(r["kind"] == "order-partial" for r in orders),
              f"{[r['kind'] for r in orders]}")
        rec = next((r for r in orders if r["kind"] == "order-partial"), {})
        check("the record carries what was FILLED, not what was asked",
              rec.get("volume") == 0.01, f"volume {rec.get('volume')}")
        check("...and what was asked, beside it",
              rec.get("asked") == 0.05, f"asked {rec.get('asked')}")
        check("nothing is re-sent to make up the difference", len(sent) == 1, f"{sent}")
    finally:
        MT5.order_send = real_send

    # A full fill records the filled volume too, so both lines read the same way.
    _, _, rows = drive(CENT, lots=0.05)
    done = next((r for r in rows if r["kind"] == "order"), {})
    check("a full fill records volume and asked as the same number",
          done.get("volume") == 0.05 and done.get("asked") == 0.05, f"{done}")

    # The poll after a partial fill: the account holds the wrong size, and that
    # is reported as drift rather than re-opened into a double position.
    after = drive_holding([position(1, lots=0.01)], lots=0.05)
    check("the poll after a partial fill reports drift, and does not re-open",
          len(after["sent"]) == 0 and "lots" in str((after["snapshot"] or {}).get("drift")),
          f"sent {after['sent']}; drift {(after['snapshot'] or {}).get('drift')}")


def main() -> int:
    the_ceiling_measures_one_currency()
    both_size_guards_fail_closed()
    the_stamp_knows_whose_directory_it_is()
    stop_leaves_a_true_record()
    the_two_clocks()
    the_account_has_a_shape()
    a_partial_fill_is_not_a_fill()
    print(f"\n{'all checks passed' if not FAIL else str(FAIL) + ' CHECK(S) FAILED'}")
    return 1 if FAIL else 0


if __name__ == "__main__":
    sys.exit(main())
