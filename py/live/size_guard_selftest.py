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

    m.initialize = lambda **kw: True
    m.shutdown = lambda: None
    m.last_error = lambda: (-10004, "no IPC connection")
    m.account_info = lambda: m.account
    m.symbol_select = lambda s, on: True
    m.terminal_info = lambda: Obj(trade_allowed=True)
    m.symbol_info = lambda s: m.info
    m.symbol_info_tick = lambda s: Obj(ask=m.price, bid=m.price - 0.2)
    m.positions_get = lambda **kw: []
    m.history_deals_get = lambda a, b: []
    m.order_calc_margin = lambda t, s, v, p: m.margin

    def order_send(req):
        m.sent.append(req)
        return Obj(retcode=m.TRADE_RETCODE_DONE, comment="ok", order=1, deal=1,
                   price=req["price"])

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


def main() -> int:
    the_ceiling_measures_one_currency()
    both_size_guards_fail_closed()
    the_stamp_knows_whose_directory_it_is()
    print(f"\n{'all checks passed' if not FAIL else str(FAIL) + ' CHECK(S) FAILED'}")
    return 1 if FAIL else 0


if __name__ == "__main__":
    sys.exit(main())
