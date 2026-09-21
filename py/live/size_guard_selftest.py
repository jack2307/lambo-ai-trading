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

  8  EVERY TIME THAT LEAVES THIS PROCESS IS ON THE BOOK'S CLOCK. `history_of`
     published `d.time_msc` raw - server time - into `broker.json` beside an
     `at` field that is true UTC, and `opened_at` did the same. Two clocks,
     adjacent keys, neither labelled. Pinned here per rule 4 of
     `docs/decisions/2026-09-17-unit-carrying.md`.

  9  A BETTER PRICE IS NOT A REASON TO SIT OUT - BUT IT IS NOT A REASON TO
     JOIN EITHER. `--max-join-r` bounded the ABSOLUTE drift, so four of six
     real entries were refused at least once for being too good, and a refusal
     is a retry, so the mirror waited for the price to come back. 728a4a4 made
     favourable joins unconditional; replaying that over the 55 live entries
     showed 7 of the 8 it added were trades the book was STOPPED OUT of. So
     the one-sided rule now lives behind `--allow-favourable-join`, with its
     two structural bounds, and these checks exercise it switched ON.

 10  AND THE DEFAULT IS OFF, which is pinned separately because the default is
     the thing that ships. The equivalence with the pre-728a4a4 symmetric rule
     is SWEPT rather than asserted, and `--print-join-mode` lets a launcher ask
     the mode instead of hardcoding a claim about it.

 12  THE BOOK'S PENDING ORDER IS MIRRORED ONLY BEHIND `--mirror-pending`, and
     with the flag off nothing changes - not a request, not a call to
     `orders_get`, not a field in the snapshot's meaning. With it on: the
     order-type map (LONG limit is BUY_LIMIT, SHORT stop is SELL_STOP, and
     nothing else is guessed), the expiry put on the terminal's clock by the
     measured offset, the same two size guards a market order runs, removal
     when the book's order disappears without a fill and when the book fills,
     one poll of grace when the terminal fills before the book does, and the
     three-key lock still exiting 3 in front of all of it. Written before the
     flag is ever turned on, per the plan that introduced it.

 13  THE ACCOUNT MUST BE FLAT BEFORE A WEEKEND EVEN WHEN NO BAR ARRIVES. The
     engine's `flat_before_weekend_hhmm` guard is bar-driven, and on
     2026-09-18 the bar it needed never came - MetaTrader closes a bar only
     on a tick after its boundary and the weekly close sends none, so the
     newest closed bar the poller saw all weekend was 20:30Z. Fourteen books
     and two real positions went into the weekend and nothing said so. The
     backstop is `--weekend-flat`, judged on this process's own UTC clock so
     that no bar, tick, broker clock or timezone table can make it miss; this
     section owns that clock, which is the only way to ask about Friday
     evening on a Tuesday afternoon. Every driver in sections 1-12 passes
     `--weekend-flat=off` for the same reason in reverse - see WEEKEND_OFF.

The provenance of every symbol number is marked. MEASURED means read from a
terminal on the date given; DERIVED means computed from a measured value and
said so. Nothing here is a guess presented as a measurement.
"""

from __future__ import annotations

import datetime as dt
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
    # The pending-order half of the terminal, for --mirror-pending. The
    # values are MT5's own enum numbers, so a test asserting on `type` reads
    # like a real request would.
    m.TRADE_ACTION_PENDING = 5
    m.TRADE_ACTION_REMOVE = 8
    m.ORDER_TYPE_BUY_LIMIT = 2
    m.ORDER_TYPE_SELL_LIMIT = 3
    m.ORDER_TYPE_BUY_STOP = 4
    m.ORDER_TYPE_SELL_STOP = 5
    m.ORDER_TIME_SPECIFIED = 2
    m.ORDER_FILLING_RETURN = 2

    m.sent = []          # every request that reached order_send
    m.orders = []        # the terminal's resting orders
    m.orders_get_calls = 0  # how often the executor asked for them; zero with the flag off
    m.account = None     # the scenario's account; None means a silent terminal
    m.info = None
    m.margin = 5.0       # order_calc_margin's answer; None means it declines
    m.price = 4311.85
    # How far the fake terminal's clock runs ahead of UTC. +3h is what the
    # real Vantage server was measured at on 2026-09-16 and 2026-09-17.
    m.server_offset_s = 3 * 3600
    m.deals = []
    # The broker's minimum distance between a price and a stop, in points, and
    # the point size. 0 means "no minimum", which is what every other check
    # here wants; the join tests raise it to prove an order that would be
    # rejected is refused before it is sent.
    m.stops_level = 0
    m.point = 0.01

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

    def orders_get(**kw):
        m.orders_get_calls += 1
        return list(m.orders)

    m.orders_get = orders_get

    def order_send(req):
        m.sent.append(req)
        # A removal takes the order off the terminal, as the real one does;
        # the executor re-asks after a removal before placing a replacement.
        if req.get("action") == m.TRADE_ACTION_REMOVE:
            m.orders = [o for o in m.orders if o.ticket != req.get("order")]
        return Obj(retcode=m.TRADE_RETCODE_DONE, comment="ok", order=1, deal=1,
                   price=req.get("price"), volume=req.get("volume"))

    m.order_send = order_send
    return m


MT5 = make_mt5()
# Installed before the import so that `import MetaTrader5 as mt5` inside
# main() finds this one. The executor imports nothing from MT5 at module
# level, so this is the only place it can be intercepted.
sys.modules["MetaTrader5"] = MT5

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import mt5_executor as X  # noqa: E402

# Every driver in sections 1-12 passes this, and section 13 is where the
# backstop is switched back on with a clock it controls.
#
# `--weekend-flat` is ON by default and judges the clock this PROCESS is
# running on, so without it the suite's answer would depend on the day of the
# week it was run: from Friday 20:45Z to Sunday 21:00Z every section that
# drives `main()` finds the window in force, the book refused and nothing
# sent. That is not hypothetical - it is what happened the first time these
# ran after the flag was added, on Saturday 2026-09-19, and section 9 alone
# failed eleven checks and then raised on an empty `sent`.
#
# So the reconciler sections say so explicitly rather than being right five
# days in seven. A test whose result depends on when it is run is a test that
# will be believed on the day it is wrong.
WEEKEND_OFF = "--weekend-flat=off"


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
               trade_stops_level=MT5.stops_level, point=MT5.point,
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


def drive(account, symbol: str = "XAUUSD.sc", margin=5.0, lots: float = 0.05, book=None,
          extra: list | None = None) -> tuple:
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

        held = dict(book or BOOK, lots=lots)
        X.ROOT = tmp
        X.read_status = lambda api, run: dict(RUN, open=held)

        def stop_after_one_poll(_):
            raise KeyboardInterrupt

        X.time.sleep = stop_after_one_poll
        sys.argv = ["mt5_executor.py", "--run=t", "--terminal=x", "--login=33705331",
                    f"--symbol={symbol}", "--account=acct",
                    WEEKEND_OFF] + list(extra or [])
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
                        "--symbol=XAUUSD.sc", "--account=acct", WEEKEND_OFF]
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
         is_buy: bool, profit: float = 0.0, comment: str = "", ticket: int = 0) -> Obj:
    """One MT5 deal. `time_msc` is SERVER time, as the terminal reports it.

    `ticket` is the deal's own id, which the rebate added a reader for on
    2026-09-21: `history_of` looks each deal up in the order-time spread
    memory by it. Every real deal carries one; this fixture derives a
    distinct pair per position so that nothing here accidentally shares a
    key. Rebate behaviour is pinned in `rebate_selftest.py`, not here.
    """
    return Obj(magic=MAGIC, position_id=pos_id, entry=0 if entry_in else 1,
               ticket=ticket or (pos_id * 10 + (0 if entry_in else 1)),
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
                    "--symbol=XAUUSD.sc", "--account=acct", WEEKEND_OFF]
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


# ---------------------------------------------------------------------------
# 8 - every time that leaves this process is on the book's clock
# ---------------------------------------------------------------------------

def the_published_times_are_utc() -> None:
    """Every time this process publishes is on the book's clock, or is null.

    Rule 4 of docs/decisions/2026-09-17-unit-carrying.md, applied to the code
    that prompted the document: a conversion is pinned by a test AT THE
    BOUNDARY. The boundary is `history_of`, which until 2026-09-17 handed out
    `d.time_msc` raw - server time - straight into `broker.json`, beside an
    `at` field that is true UTC. Two clocks, adjacent keys in one object,
    neither labelled.
    """
    section("the times that leave this process")

    book_entry = BOOK["entry_time"]
    offset_ms = MT5.server_offset_s * 1000
    MT5.deals = closed_trade(book_entry, held_ms=5 * 60_000)
    try:
        snap = drive_holding([position(1)])["snapshot"] or {}
        fills = snap.get("fills") or []
        check("the snapshot carries this book's closed trade", len(fills) == 1, f"{fills}")
        got = (fills[0] if fills else {}).get("entryTime")
        check("its entryTime is published in UTC, on the book's clock",
              got == book_entry, f"got {got}, book {book_entry}")
        check("...and not the server stamp MT5 handed over "
              "(remove the conversion and this fails)",
              got != book_entry + offset_ms, f"got {got}")

        # `at` is true UTC from time.time(); `opened_at` was the broker's clock
        # in the same object until today.
        pos = snap.get("position") or {}
        check("opened_at is on the same clock as `at`",
              pos.get("opened_at") == 1_000_000 * 1000 - offset_ms,
              f"opened_at {pos.get('opened_at')}")
        check("the offset used is published beside the times it converted",
              snap.get("server_offset_ms") == offset_ms, f"{snap.get('server_offset_ms')}")

        # No clock, no times - but the money still reports. A time on an
        # unknown clock is worse than no time; a P&L does not need a clock.
        real_tick = MT5.symbol_info_tick
        try:
            MT5.symbol_info_tick = lambda s: Obj(ask=MT5.price, bid=MT5.price - 0.2)
            snap = drive_holding([position(1)])["snapshot"] or {}
            fills = snap.get("fills") or []
            check("offset unmeasurable: fill times go out as null, not as a guess",
                  bool(fills) and fills[0].get("entryTime") is None, f"{fills[:1]}")
            check("...and the realised money is still reported",
                  snap.get("realised") is not None, f"realised {snap.get('realised')}")
            check("...and opened_at is null rather than a server stamp",
                  (snap.get("position") or {}).get("opened_at") is None,
                  f"{(snap.get('position') or {}).get('opened_at')}")
        finally:
            MT5.symbol_info_tick = real_tick
    finally:
        MT5.deals = []


def drive_at(book_open: dict, price: float, favourable: bool = False) -> tuple:
    """One poll with a given book position and a given market price.

    The spread is ZEROED for these - bid and ask both sit at `price` - because
    what is under test is the join rule and a half-point of spread would shift
    every drift by a fraction of an R and make the expected numbers arguments
    about the fake rather than about the rule.
    """
    real_tick = MT5.symbol_info_tick
    real_status = X.read_status
    try:
        MT5.symbol_info_tick = lambda sym: Obj(
            ask=price, bid=price, time=int(time.time()) + MT5.server_offset_s)
        MT5.price = price
        return drive(CENT, lots=book_open.get("lots", 0.05), book=book_open,
                     extra=["--allow-favourable-join"] if favourable else None)
    finally:
        MT5.symbol_info_tick = real_tick
        X.read_status = real_status


# ---------------------------------------------------------------------------
# 9 - a better price is not a reason to sit out
# ---------------------------------------------------------------------------

def a_better_price_is_not_a_reason_to_sit_out() -> None:
    """`--max-join-r` bounded |drift| and now bounds adverse drift only.

    The six real entries of 2026-09-17 are replayed below from their measured
    drifts. Four of six were refused at least once for being too GOOD, and
    because a refusal is a retry rather than a skip, the mirror then waited for
    the price to come back to it: one book sat eighteen minutes and filled 3.5
    points worse, another was never joined at all - the book made +1.32R on it,
    of which a mirror joining at first sighting would have had about +1.00R and
    fifteen seconds later none, the price being through the target already.

    The favourable half of the bound is replaced by two STRUCTURAL limits,
    which is the part worth pinning: a favourable drift of a full R means the
    price has reached the book's own stop, and a join too close to that stop is
    rejected by the broker rather than filled.
    """
    section("joining at a better price than the book got")

    # Reconstructed from the drifts measured on the funded account, 2026-09-17.
    # terra SHORT's stop comes back as 4317.31 here and the fill that later
    # stopped out reported sl 4317.30 — so these reconstructions are the real
    # trades, not invented ones.
    # (name, side, book entry, risk, price seen, drift r, should it join?)
    REPLAY = [
        ('terra SHORT', 'SHORT', 4306.38, 10.93, 4309.33, -0.27, True),
        ('ds LONG', 'LONG', 4332.12, 11.40, 4328.13, -0.35, True),
        ('ds SHORT', 'SHORT', 4360.41, 19.98, 4373.00, -0.63, True),
        ('terra LONG', 'LONG', 4350.05, 10.00, 4353.40, +0.34, False),
    ]

    def book(side: str, entry: float, risk: float) -> dict:
        stop = entry - risk if side == 'LONG' else entry + risk
        target = entry + 2 * risk if side == 'LONG' else entry - 2 * risk
        return dict(BOOK, side=side, entry_price=entry, stop=stop, target=target)

    for name, side, entry, risk, price, drift, should_join in REPLAY:
        held = book(side, entry, risk)
        MT5.deals = []
        _, sent, rows = drive_at(held, price, favourable=True)
        joined = len(sent) == 1
        check(f"{name}: drift {drift:+.2f}R -> {'joins' if should_join else 'sits out'}",
              joined == should_join,
              f"sent {len(sent)}; {[r['kind'] for r in rows]}")
        if should_join:
            # The book's stop and target, verbatim. Identical exits are what
            # make the per-trade difference exactly the entry slippage; a
            # rescaled stop would put the mirror beyond the book's and leave it
            # holding a position the book had closed.
            req = sent[0]
            check(f"{name}: the book's stop and target go out unchanged",
                  req.get('sl') == held['stop'] and req.get('tp') == held['target'],
                  f"sl {req.get('sl')} vs {held['stop']}, tp {req.get('tp')} vs {held['target']}")
            join = next((r for r in rows if r['kind'] == 'joining'), {})
            check(f"{name}: the accepted join records its drift and direction",
                  abs((join.get('drift_r') or 0) - drift) < 0.02 and join.get('favourable') is True,
                  f"{join}")

    # Adverse is unchanged: still bounded at 0.25R, and the one the guard was
    # built for is still refused.
    held = book('LONG', 4350.05, 10.0)
    _, sent, rows = drive_at(held, 4350.05 + 2.45 * 10.0)
    check("adverse +2.45R: still refused", len(sent) == 0, f"{sent}")
    check("...for a reason naming the adverse direction",
          any('against the book' in str(r.get('reason', '')) for r in rows if r['kind'] == 'not-adopted'),
          f"{[r.get('reason') for r in rows if r['kind'] == 'not-adopted']}")

    # THE STRUCTURAL LIMIT. A full R better means the price is AT the book's
    # own stop, so the book is about to exit and the trade is already lost.
    # Reachable, not impossible - which is the half of this that is easy to
    # get backwards.
    held = book('LONG', 4350.05, 10.0)
    _, sent, rows = drive_at(held, held['stop'] - 0.5, favourable=True)
    check("a full R better puts the price past the book's STOP: refused",
          len(sent) == 0, f"{sent}")
    check("...and says the book is about to exit, not that the price is too far",
          any("already lost" in str(r.get('reason', '')) for r in rows if r['kind'] == 'not-adopted'),
          f"{[r.get('reason') for r in rows if r['kind'] == 'not-adopted']}")

    # Just inside it still joins: the bound is the stop, not a round number.
    _, sent, _ = drive_at(held, held['stop'] + 1.5, favourable=True)
    check("just short of the stop still joins", len(sent) == 1, f"sent {len(sent)}")

    # The broker's own minimum stop distance. An order whose sl sits inside it
    # comes back rejected, and a mirror that retried would refuse every poll.
    try:
        MT5.stops_level = 200  # points, x 0.01 = 2.00 in price
        _, sent, rows = drive_at(held, held['stop'] + 1.5, favourable=True)
        check("a join inside the broker's minimum stop distance: refused, not sent",
              len(sent) == 0, f"{sent}")
        check("...and names the broker's limit rather than the desk's",
              any('minimum stop distance' in str(r.get('reason', ''))
                  for r in rows if r['kind'] == 'not-adopted'),
              f"{[r.get('reason') for r in rows if r['kind'] == 'not-adopted']}")
    finally:
        MT5.stops_level = 0

    # A book with no stop cannot be measured in R at all; that path is
    # unchanged and still joins.
    held = dict(BOOK, side='LONG', entry_price=4350.0, stop=None, target=None)
    _, sent, _ = drive_at(held, 4340.0, favourable=True)
    check("a book with no stop still joins, as before", len(sent) == 1, f"sent {len(sent)}")


# ---------------------------------------------------------------------------
# 10 - the favourable half is OFF unless someone asks for it
# ---------------------------------------------------------------------------

def favourable_joins_are_off_by_default() -> None:
    """728a4a4 made favourable joins unconditional; they are now behind a flag.

    Replaying that rule over the same 55 live entries, it took 8 the symmetric
    rule refused and 7 of those 8 were trades the book was later STOPPED OUT
    of, against a 38% base rate (p = 0.006), for -1.20R after the mirror's own
    fill. A better price in the bar after a signal is the first leg of the move
    to the stop.

    The default matters more than the flag, so it is pinned first and the
    equivalence with the pre-728a4a4 rule is proved rather than asserted.
    """
    section("favourable joins, off unless asked for")

    # The four measured entries again, this time with NO flag: the three
    # favourable ones must now sit out, and the adverse one still does.
    for name, side, entry, risk, price, drift in [
        ('terra SHORT', 'SHORT', 4306.38, 10.93, 4309.33, -0.27),
        ('ds LONG', 'LONG', 4332.12, 11.40, 4328.13, -0.35),
        ('ds SHORT', 'SHORT', 4360.41, 19.98, 4373.00, -0.63),
    ]:
        stop = entry - risk if side == 'LONG' else entry + risk
        target = entry + 2 * risk if side == 'LONG' else entry - 2 * risk
        held = dict(BOOK, side=side, entry_price=entry, stop=stop, target=target)
        MT5.deals = []
        _, sent, rows = drive_at(held, price)
        check(f"{name}: {drift:+.2f}R better, default -> sits out",
              len(sent) == 0, f"sent {len(sent)}")
        check(f"{name}: ...and the refusal names the flag",
              any('--allow-favourable-join' in str(r.get('reason', ''))
                  for r in rows if r['kind'] == 'not-adopted'),
              f"{[r.get('reason') for r in rows if r['kind'] == 'not-adopted']}")

    # THE EQUIVALENCE, swept rather than argued. With the flag off the executor
    # must decide exactly what the rule before 728a4a4 decided:
    #
    #     return r if abs(r) > args.max_join_r else None
    #
    # so the old predicate is written out here and the two are compared at
    # every interesting r, including both sides of the boundary and both sides
    # of the 1R structural bound - which the flag-off path must NOT reach,
    # because it returns first.
    entry, risk = 4350.0, 10.0
    held_base = dict(BOOK, side='LONG', entry_price=entry, stop=entry - risk,
                     target=entry + 2 * risk)
    mismatches = []
    for r in (-2.0, -1.5, -1.01, -1.0, -0.99, -0.5, -0.26, -0.25, -0.24,
              0.0, 0.24, 0.25, 0.26, 0.5, 1.0, 2.0):
        MT5.deals = []
        _, sent, _ = drive_at(held_base, entry + r * risk)
        joined = len(sent) == 1
        old_rule_joins = not (abs(r) > 0.25)
        if joined != old_rule_joins:
            mismatches.append((r, joined, old_rule_joins))
    check("flag off is EXACTLY the pre-728a4a4 symmetric rule, swept over 16 drifts",
          not mismatches, f"disagreed at {mismatches}")

    # And the flag does turn it back on, so the default is a choice rather than
    # a path that no longer exists.
    MT5.deals = []
    _, sent, _ = drive_at(held_base, entry - 0.5 * risk, favourable=True)
    check("with the flag, a 0.50R better price joins again", len(sent) == 1, f"sent {len(sent)}")

    # The structural bounds are the flag's, not the default's: with the flag
    # off the 1R case is refused by the symmetric bound long before the stop
    # bound is consulted, so its reason must be the flag's, not the stop's.
    MT5.deals = []
    _, sent, rows = drive_at(held_base, held_base['stop'] - 0.5)
    reasons = [r.get('reason', '') for r in rows if r['kind'] == 'not-adopted']
    check("past the stop with the flag off: refused by the symmetric bound, not the stop bound",
          len(sent) == 0 and any('--allow-favourable-join' in str(x) for x in reasons)
          and not any('already lost' in str(x) for x in reasons),
          f"{reasons}")


def the_launcher_can_ask_what_the_rule_is() -> None:
    """`--print-join-mode` exists so a launcher never has to CLAIM the mode.

    A hardcoded "symmetric" string in a launcher is a record that agrees with
    the code today and disagrees the day someone flips the default - the defect
    this repo spent 2026-09-17 removing. The sentence lives beside the flag
    that decides it and the launcher echoes it verbatim.
    """
    section("the join mode, asked rather than claimed")

    import subprocess
    exe = sys.executable
    script = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'mt5_executor.py')

    def ask(*flags):
        out = subprocess.run([exe, script, '--print-join-mode', *flags],
                             capture_output=True, text=True)
        return out.returncode, out.stdout.strip(), out.stderr.strip()

    rc, line, err = ask()
    check("the query exits 0 without the required trading arguments", rc == 0, f"rc={rc} err={err}")
    check("one line, and it says symmetric by default",
          line.count('\n') == 0 and 'symmetric' in line and '0.25R' in line, repr(line))

    rc, line, _ = ask('--allow-favourable-join')
    check("with the flag it says one-sided", rc == 0 and 'one-sided' in line and 'TAKEN' in line,
          repr(line))

    rc, line, _ = ask('--max-join-r=0.4')
    check("it reports the bound it was actually given, not a hardcoded 0.25",
          rc == 0 and '0.4R' in line, repr(line))


# ---------------------------------------------------------------------------
# 11 - the close request, against the open request that works
# ---------------------------------------------------------------------------

def the_close_request_can_actually_be_sent() -> None:
    """Closing had never once succeeded, and nothing said why.

    Measured 2026-09-18 over every executor.jsonl this desk has written: 1,389
    closes, all `order_send` returning None, ZERO successful closes ever, on
    any account. Opens from the same terminal in the same minute returned
    10009. Every earlier exit was the broker's own stop or target, so the path
    was never exercised until a book went flat while the account still held -
    and then the mirror was stuck through two winners it could not take.

    The one field that varied with the outcome was the comment: MT5 caps it at
    31, and `[:31]` put every close exactly on the boundary while no open ever
    reached it. These checks pin the length, the shape against the open, and
    that a failure can never again be invisible.
    """
    section("the close request")

    # Every run on this desk, every reason, against the length the account is
    # measured to accept. The old code produced exactly 31 for all of them.
    for run in ('ai-xau-ds-ctx', 'ai-xau-terra-ctx', 'ai-xau-opus-ctx', 'xau-macd-asia'):
        for why in ('book flat', 'side changed', 'STOP file', 'desk-wide STOP file'):
            c = X.close_comment(run, why)
            check(f"close comment fits: {c!r} ({len(c)})", len(c) <= X.COMMENT_MAX, f"{len(c)}")
            # Not merely short - not cut mid-word either, because this is the
            # field a person reads when asking why a position closed.
            check(f"...and is not truncated mid-word: {c!r}", not c.endswith(' ') and c == c.strip(), c)

    held = Obj(ticket=578869788, type=1, volume=0.01, price_open=4365.56, price_current=4362.84,
               sl=4381.2, tp=4330.0, profit=9.0, swap=0.0, time=1_000_000, magic=MAGIC,
               symbol='XAUUSD.sc')

    def drive_flat_with_position() -> tuple:
        """One poll: the book is FLAT and the account still holds. The state
        that produced 1,389 failures."""
        tmp = Path(tempfile.mkdtemp(prefix='sgst-close-'))
        real_root, real_status, real_sleep = X.ROOT, X.read_status, X.time.sleep
        argv = sys.argv
        try:
            MT5.sent = []
            MT5.account = CENT
            MT5.info = info_for('XAUUSD.sc')
            MT5.margin = 5.0
            MT5.price = 4362.84
            MT5.positions_get = lambda **kw: [held]
            X.ROOT = tmp
            X.read_status = lambda api, run: dict(RUN, open=None)

            def stop_after_one_poll(_):
                raise KeyboardInterrupt

            X.time.sleep = stop_after_one_poll
            sys.argv = ['mt5_executor.py', '--run=t', '--terminal=x',
                        '--login=33705331', '--symbol=XAUUSD.sc', '--account=acct',
                        WEEKEND_OFF]
            X.main()
            out = tmp / 'data' / 'live' / 'acct' / 't' / 'executor.jsonl'
            rows = [json.loads(l) for l in out.read_text(encoding='utf-8').splitlines()
                    if l.strip()] if out.exists() else []
            return list(MT5.sent), rows
        finally:
            X.ROOT, X.read_status, X.time.sleep = real_root, real_status, real_sleep
            sys.argv = argv
            MT5.positions_get = lambda **kw: []
            shutil.rmtree(tmp, ignore_errors=True)

    CLOSE_ASK = 4362.84
    sent, rows = drive_flat_with_position()
    check("a book that went flat sends a close", len(sent) == 1, f"sent {len(sent)}")
    req = sent[0] if sent else {}

    check("the close comment is within the measured limit",
          len(str(req.get('comment', ''))) <= X.COMMENT_MAX,
          f"{req.get('comment')!r} is {len(str(req.get('comment','')))}")

    # The shape, against the request that is known to work. The close may add
    # `position` and nothing else; anything else it carries alone is a field
    # the open path has never proved.
    MT5.deals = []
    _, opened, _ = drive(CENT, lots=0.05)
    open_req = opened[0] if opened else {}
    # `sl`/`tp` belong to an OPENING order and are rightly absent from a close -
    # you do not attach a stop to the order that removes the position. Every
    # other field the open proves must be present.
    extra = set(req) - set(open_req)
    missing = set(open_req) - set(req) - {'sl', 'tp'}
    check("the close carries every field the open proves, bar sl/tp",
          not missing, f"missing {missing}")
    check("...and adds only `position`", extra == {'position'}, f"extra {extra}")
    check("...and carries no sl/tp, which belong to an opening order",
          'sl' not in req and 'tp' not in req, f"{sorted(set(req) & {'sl', 'tp'})}")

    # Types the terminal expects, not whatever `positions_get` handed back.
    check("position is an int", isinstance(req.get('position'), int), f"{type(req.get('position'))}")
    check("volume is a float", isinstance(req.get('volume'), float), f"{type(req.get('volume'))}")
    check("magic and deviation are ints",
          isinstance(req.get('magic'), int) and isinstance(req.get('deviation'), int),
          f"{type(req.get('magic'))} {type(req.get('deviation'))}")

    # Closing a SHORT buys, and buys at the ask. The wrong side here would be
    # a rejection or, worse, a second position.
    check("closing a short BUYS", req.get('type') == MT5.ORDER_TYPE_BUY, f"{req.get('type')}")
    check("...at the ask", req.get('price') == CLOSE_ASK, f"{req.get('price')} vs {CLOSE_ASK}")

    # A refusal must never be invisible again.
    real_send = MT5.order_send
    try:
        MT5.order_send = lambda r: None
        _, rows = drive_flat_with_position()
        failed = next((r for r in rows if r['kind'] == 'order-failed'), {})
        check("order_send returning None is logged as a failure", bool(failed), f"{[r['kind'] for r in rows]}")
        check("...with last_error, so the reason is never invisible",
              'error' in failed and failed['error'], f"{failed}")
        check("...and with the request that was refused",
              isinstance(failed.get('sent'), dict) and 'position' in failed['sent'],
              f"{failed.get('sent')}")
    finally:
        MT5.order_send = real_send


# ---------------------------------------------------------------------------
# 12 - the book's pending order, mirrored only behind the flag
# ---------------------------------------------------------------------------

OFFSET_MS = 3 * 3600 * 1000


def pending(side: str = "LONG", kind: str = "limit", price: float = 4305.0, lots=0.05,
            valid_ms=None, **over) -> dict:
    """A pending order as `/api/paper/status` publishes it. Its deadline is an
    hour ahead on UTC unless said otherwise, so the terminal-clock check passes."""
    if valid_ms is None:
        valid_ms = int(time.time() * 1000) + 3_600_000
    risk = 10.0
    stop = price - risk if side == "LONG" else price + risk
    target = price + 2 * risk if side == "LONG" else price - 2 * risk
    p = {"type": kind, "price": price, "side": side, "stop": stop, "target": target, "zone": None,
         "valid_until_bar_ms": valid_ms, "decided_at": valid_ms - 3_600_000,
         "invalidate_above": None, "invalidate_below": None, "lots": lots}
    p.update(over)
    return p


def resting(ticket: int, otype: int, price: float, lots: float = 0.05, valid_ms=None) -> Obj:
    """A TradeOrder as `orders_get` returns it: server seconds on `time_expiration`."""
    if valid_ms is None:
        valid_ms = int(time.time() * 1000) + 3_600_000
    return Obj(ticket=ticket, type=otype, price_open=price, sl=price - 10, tp=price + 20,
               volume_current=lots, magic=MAGIC, symbol="XAUUSD.sc",
               time_expiration=(valid_ms + OFFSET_MS) // 1000, comment="t plan")


def drive_status(status: dict, extra=None, orders=None, held=None, polls: int = 1,
                 account=None) -> dict:
    """`polls` polls of main() against one status entry, resting orders and positions."""
    tmp = Path(tempfile.mkdtemp(prefix="sgst-pend-"))
    real_root, real_status, real_sleep = X.ROOT, X.read_status, X.time.sleep
    argv = sys.argv
    try:
        MT5.sent = []
        MT5.account = account or CENT
        MT5.info = info_for("XAUUSD.sc")
        MT5.margin = 5.0
        MT5.price = SYMBOLS["XAUUSD.sc"][4]
        MT5.orders = list(orders or [])
        MT5.orders_get_calls = 0
        MT5.deals = []
        MT5.positions_get = lambda **kw: list(held or [])
        X.ROOT = tmp
        X.read_status = lambda api, run: dict(status)
        n = {"polls": 0}

        def stop_after(_):
            n["polls"] += 1
            if n["polls"] >= polls:
                raise KeyboardInterrupt

        X.time.sleep = stop_after
        sys.argv = ["mt5_executor.py", "--run=t", "--terminal=x", "--login=33705331",
                    "--symbol=XAUUSD.sc", "--account=acct",
                    WEEKEND_OFF] + list(extra or [])
        rc = X.main()
        here = tmp / "data" / "live" / "acct" / "t"
        out, snap = here / "executor.jsonl", here / "broker.json"
        rows = [json.loads(l) for l in out.read_text(encoding="utf-8").splitlines()
                if l.strip()] if out.exists() else []
        return {"rc": rc, "sent": list(MT5.sent), "rows": rows,
                "snapshot": json.loads(snap.read_text(encoding="utf-8")) if snap.exists() else None,
                "orders_get_calls": MT5.orders_get_calls}
    finally:
        X.ROOT, X.read_status, X.time.sleep = real_root, real_status, real_sleep
        sys.argv = argv
        MT5.positions_get = lambda **kw: []
        MT5.orders = []
        shutil.rmtree(tmp, ignore_errors=True)


def the_pending_order_is_mirrored_only_behind_the_flag() -> None:
    """`--mirror-pending`, default OFF, and everything it does when on.

    The plan books answer with a limit or a stop that waits on the desk
    (docs/hypotheses/2026-09-18-plan-entry.md). Off, the executor sees the
    book flat until the order fills and mirrors the position as it always
    has. On, the terminal holds the same order at the same price with the
    same expiry. Pending orders on a funded account add states this
    executor has never held, so these are written BEFORE the flag is ever
    turned on, and the default is pinned first because the default ships.
    """
    section("the pending order, behind --mirror-pending")

    # The map, and that it guesses nothing.
    for side, kind, want in (("LONG", "limit", MT5.ORDER_TYPE_BUY_LIMIT),
                             ("SHORT", "limit", MT5.ORDER_TYPE_SELL_LIMIT),
                             ("LONG", "stop", MT5.ORDER_TYPE_BUY_STOP),
                             ("SHORT", "stop", MT5.ORDER_TYPE_SELL_STOP)):
        check(f"{side} {kind} -> {want}", X.pending_order_type(MT5, side, kind) == want)
    check("case does not matter", X.pending_order_type(MT5, "long", "Limit") == MT5.ORDER_TYPE_BUY_LIMIT)
    for side, kind in (("LONG", "market"), ("NONE", "limit"), (None, None), ("LONG", "stop-limit")):
        check(f"{side} {kind} -> None, not a guess", X.pending_order_type(MT5, side, kind) is None)

    # The expiry, converted once at the boundary onto the terminal's clock.
    check("UTC ms + offset -> server seconds",
          X.expiry_on_server(1_700_000_000_000, OFFSET_MS) == 1_700_000_000 + 10_800)
    check("a float ms stamp converts the same", X.expiry_on_server(1_700_000_000_000.0, OFFSET_MS) == 1_700_010_800)
    check("no offset -> None", X.expiry_on_server(1_700_000_000_000, None) is None)
    check("no deadline -> None", X.expiry_on_server(None, OFFSET_MS) is None)

    # The comment, within the measured limit and never cut mid-word.
    check("a short run id gets the tag", X.pending_comment("ai-xau-ds-plan") == "ai-xau-ds-plan plan")
    check("ai-xau-ds-plan-trigger (22 chars) has no room for it and is sent alone",
          X.pending_comment("ai-xau-ds-plan-trigger") == "ai-xau-ds-plan-trigger")
    for run in ("t", "ai-xau-ds-plan", "ai-xau-ds-plan-trigger", "xau-macd-asia"):
        c = X.pending_comment(run)
        check(f"pending comment fits and is whole: {c!r}", len(c) <= X.COMMENT_MAX and c == c.strip())

    flat_with_pending = dict(RUN, open=None, pending_order=pending())

    # THE DEFAULT. A pending order on the book and the flag off: nothing.
    off = drive_status(flat_with_pending)
    check("flag off: nothing is sent", off["sent"] == [], f"{off['sent']}")
    check("flag off: the terminal is never asked for resting orders", off["orders_get_calls"] == 0,
          f"{off['orders_get_calls']}")
    check("flag off: no pending line of any kind is written",
          not any("pending" in r["kind"] for r in off["rows"]), f"{[r['kind'] for r in off['rows']]}")
    check("flag off: the snapshot says so and holds no order",
          (off["snapshot"] or {}).get("mirror_pending") is False and (off["snapshot"] or {}).get("pending") is None,
          f"{(off['snapshot'] or {}).get('mirror_pending')} {(off['snapshot'] or {}).get('pending')}")
    check("flag off: started says mirror_pending false",
          next((r for r in off["rows"] if r["kind"] == "started"), {}).get("mirror_pending") is False)

    # ON. A LONG limit becomes a BUY_LIMIT at the book's price, sized by the
    # lot rule, expiring on the terminal's clock when the book's does.
    on = drive_status(flat_with_pending, extra=["--mirror-pending"])
    check("flag on: one request", len(on["sent"]) == 1, f"{[r['kind'] for r in on['rows']]}")
    req = on["sent"][0] if on["sent"] else {}
    p = flat_with_pending["pending_order"]
    check("...a pending-order action", req.get("action") == MT5.TRADE_ACTION_PENDING, f"{req}")
    check("...BUY_LIMIT", req.get("type") == MT5.ORDER_TYPE_BUY_LIMIT, f"{req.get('type')}")
    check("...at the book's price", req.get("price") == 4305.0, f"{req.get('price')}")
    check("...sized by the lot rule (0.05 x scale 1.0)", req.get("volume") == 0.05, f"{req.get('volume')}")
    check("...with the book's stop and target verbatim",
          req.get("sl") == p["stop"] and req.get("tp") == p["target"], f"sl {req.get('sl')} tp {req.get('tp')}")
    check("...ORDER_TIME_SPECIFIED", req.get("type_time") == MT5.ORDER_TIME_SPECIFIED, f"{req.get('type_time')}")
    check("...expiring at the book's deadline ON THE TERMINAL'S CLOCK (+3h, whole seconds)",
          req.get("expiration") == (p["valid_until_bar_ms"] + OFFSET_MS) // 1000
          and isinstance(req.get("expiration"), int),
          f"{req.get('expiration')} vs {(p['valid_until_bar_ms'] + OFFSET_MS) // 1000}")
    check("...and NOT on UTC (remove the conversion and this fails)",
          req.get("expiration") != p["valid_until_bar_ms"] // 1000)
    check("...comment within the measured limit", len(str(req.get("comment", ""))) <= X.COMMENT_MAX,
          f"{req.get('comment')!r}")
    check("...magic and expiration are ints",
          isinstance(req.get("magic"), int) and isinstance(req.get("expiration"), int))
    placing = next((r for r in on["rows"] if r["kind"] == "pending-placing"), {})
    check("the placing line carries both clocks and the offset between them",
          placing.get("valid_until_utc_ms") == p["valid_until_bar_ms"]
          and placing.get("expiration_server_s") == req.get("expiration")
          and placing.get("server_offset_ms") == OFFSET_MS, f"{placing}")
    check("the snapshot says the flag is on", (on["snapshot"] or {}).get("mirror_pending") is True)

    # The other three types.
    for side, kind, want in (("SHORT", "limit", MT5.ORDER_TYPE_SELL_LIMIT),
                             ("LONG", "stop", MT5.ORDER_TYPE_BUY_STOP),
                             ("SHORT", "stop", MT5.ORDER_TYPE_SELL_STOP)):
        price = 4318.0 if (side, kind) in (("SHORT", "limit"), ("LONG", "stop")) else 4305.0
        r = drive_status(dict(RUN, open=None, pending_order=pending(side, kind, price)),
                         extra=["--mirror-pending"])
        check(f"{side} {kind} is sent as {want} at {price}",
              len(r["sent"]) == 1 and r["sent"][0].get("type") == want and r["sent"][0].get("price") == price,
              f"{r['sent']}")

    # The same two size guards a market order runs.
    big = drive_status(dict(RUN, open=None, pending_order=pending(lots=100.0)), extra=["--mirror-pending"])
    check("a pending order over the notional ceiling: nothing is sent", big["sent"] == [], f"{big['sent']}")
    check("...and it is the size guard that says so",
          any(r["kind"] == "refused-size" for r in big["rows"]), f"{[r['kind'] for r in big['rows']]}")

    # What it refuses on, each written down.
    nolots = drive_status(dict(RUN, open=None, pending_order=pending(lots=None)), extra=["--mirror-pending"])
    check("no lots on the book's order: nothing is sent", nolots["sent"] == [], f"{nolots['sent']}")
    check("...refused-pending names the lots",
          any(r["kind"] == "refused-pending" and "lots" in r.get("reason", "") for r in nolots["rows"]),
          f"{[(r['kind'], r.get('reason')) for r in nolots['rows']]}")
    check("...and the snapshot says it is standing out, not blocked",
          (nolots["snapshot"] or {}).get("standing_out") and not (nolots["snapshot"] or {}).get("blocked"),
          f"{(nolots['snapshot'] or {}).get('standing_out')} / {(nolots['snapshot'] or {}).get('blocked')}")

    real_tick = MT5.symbol_info_tick
    try:
        MT5.symbol_info_tick = lambda s: Obj(ask=MT5.price, bid=MT5.price - 0.2)
        noclock = drive_status(flat_with_pending, extra=["--mirror-pending"])
        check("no server clock: nothing is sent", noclock["sent"] == [], f"{noclock['sent']}")
        check("...refused-pending names the clock",
              any(r["kind"] == "refused-pending" and "clock" in r.get("reason", "") for r in noclock["rows"]),
              f"{[(r['kind'], r.get('reason')) for r in noclock['rows']]}")
    finally:
        MT5.symbol_info_tick = real_tick

    gone = drive_status(dict(RUN, open=None, pending_order=pending(valid_ms=int(time.time() * 1000) - 60_000)),
                        extra=["--mirror-pending"])
    check("a deadline already past: nothing is sent", gone["sent"] == [], f"{gone['sent']}")
    check("...and the line says expired",
          any(r["kind"] == "pending-expired" for r in gone["rows"]), f"{[r['kind'] for r in gone['rows']]}")

    # Cancel on disappear: the book's order went (expired, invalidated,
    # cancelled) without a fill, and the terminal still holds its copy.
    held_order = resting(555, MT5.ORDER_TYPE_BUY_LIMIT, 4305.0)
    cancel = drive_status(dict(RUN, open=None, pending_order=None), extra=["--mirror-pending"],
                          orders=[held_order])
    check("book pending gone, no position: the terminal's order is removed",
          len(cancel["sent"]) == 1 and cancel["sent"][0].get("action") == MT5.TRADE_ACTION_REMOVE
          and cancel["sent"][0].get("order") == 555, f"{cancel['sent']}")
    check("...and the removal carries only action and order",
          set(cancel["sent"][0]) == {"action", "order"} if cancel["sent"] else False,
          f"{cancel['sent']}")
    check("...with the flag off the same state removes nothing",
          drive_status(dict(RUN, open=None, pending_order=None), orders=[held_order])["sent"] == [])

    # A matching order already rests: nothing is duplicated.
    same = drive_status(flat_with_pending, extra=["--mirror-pending"], orders=[held_order])
    check("the book's order already rests on the terminal: nothing is sent", same["sent"] == [], f"{same['sent']}")
    check("...and the snapshot shows it, with its expiry on the book's clock",
          (same["snapshot"] or {}).get("pending", {}).get("ticket") == 555
          and (same["snapshot"] or {}).get("pending", {}).get("expires_at")
          == held_order.time_expiration * 1000 - OFFSET_MS,
          f"{(same['snapshot'] or {}).get('pending')}")

    # The book replaced its order (a new decision): the old copy goes, the new one is placed.
    moved = drive_status(dict(RUN, open=None, pending_order=pending(price=4300.0)),
                         extra=["--mirror-pending"], orders=[resting(555, MT5.ORDER_TYPE_BUY_LIMIT, 4305.0)])
    check("book pending replaced: remove the old, then place the new, in that order",
          [r.get("action") for r in moved["sent"]] == [MT5.TRADE_ACTION_REMOVE, MT5.TRADE_ACTION_PENDING]
          and moved["sent"][1].get("price") == 4300.0, f"{moved['sent']}")

    # The book filled (or was triggered at market) while a copy still rests:
    # the copy goes and the position is mirrored as it always has been.
    filled = drive_status(dict(RUN, open=BOOK, pending_order=None), extra=["--mirror-pending"],
                          orders=[resting(555, MT5.ORDER_TYPE_BUY_LIMIT, 4305.0)])
    check("book holds, terminal order rests: remove it, then open at market as today",
          [r.get("action") for r in filled["sent"]] == [MT5.TRADE_ACTION_REMOVE, MT5.TRADE_ACTION_DEAL],
          f"{filled['sent']}")

    # The terminal filled before the book did: one poll of grace, then the
    # book's word wins.
    pos = position(1)
    ahead = drive_status(flat_with_pending, extra=["--mirror-pending"], held=[pos], polls=1)
    check("terminal filled first, poll 1: nothing is closed", ahead["sent"] == [], f"{ahead['sent']}")
    check("...and the grace is written down",
          any(r["kind"] == "pending-filled-ahead" for r in ahead["rows"]), f"{[r['kind'] for r in ahead['rows']]}")
    ahead2 = drive_status(flat_with_pending, extra=["--mirror-pending"], held=[pos], polls=2)
    closes = [r for r in ahead2["sent"] if r.get("action") == MT5.TRADE_ACTION_DEAL and "position" in r]
    check("...poll 2, book still flat: the position is closed - the account must not hold what the book does not",
          len(closes) == 1, f"{ahead2['sent']}")

    # THE LOCK. A real account with the flag on and no --allow-real exits 3
    # having sent nothing, exactly as it does for a market order.
    real = Obj(login=33705331, server="VantageMarkets-Live 21", currency="USC",
               trade_mode=1, balance=10000.0, equity=10000.0, margin=0.0)
    locked = drive_status(flat_with_pending, extra=["--mirror-pending"], account=real)
    check("REAL account, flag on, no --allow-real: exit 3", locked["rc"] == 3, f"rc {locked['rc']}")
    check("...nothing was sent", locked["sent"] == [] and locked["orders_get_calls"] == 0, f"{locked['sent']}")
    check("...and the refusal names the missing key",
          any(r["kind"] == "refused" and "--allow-real" in r.get("reason", "") for r in locked["rows"]),
          f"{[(r['kind'], r.get('reason')) for r in locked['rows']]}")

    # Dry run: the request is logged and nothing reaches the terminal.
    dry = drive_status(flat_with_pending, extra=["--mirror-pending", "--dry-run"])
    check("dry run: nothing is sent", dry["sent"] == [], f"{dry['sent']}")
    check("...and the pending request is written as a dry-run line",
          any(r["kind"] == "dry-run" and "pending" in str(r.get("action")) for r in dry["rows"]),
          f"{[(r['kind'], r.get('action')) for r in dry['rows']]}")


# ---------------------------------------------------------------------------
# 13 - the weekend backstop, on a clock this test controls
# ---------------------------------------------------------------------------

# Friday 2026-09-18, the day the first layer failed to fire, and the three
# days after it. Real dates rather than invented ones so the weekday
# arithmetic below can be checked against a calendar by anyone who doubts it:
# 18/09/2026 is a Friday, and the desk's own record of that evening is
# docs/decisions/2026-09-19-weekend-flat-never-fires.md.
FRI = (2026, 9, 18)
SAT = (2026, 9, 19)
SUN = (2026, 9, 20)
MON = (2026, 9, 21)
THU = (2026, 9, 17)
NEXT_FRI = (2026, 9, 25)


def at(day, hh: int, mm: int, ss: int = 0):
    """A UTC instant on one of the days above."""
    return dt.datetime(day[0], day[1], day[2], hh, mm, ss, tzinfo=dt.timezone.utc)


class FrozenClock:
    """`datetime` stopped at a given instant, advanced one step per poll.

    The executor reads its own wall clock to decide the window, which is the
    whole point of the backstop - it must not need a bar, a tick or a broker
    to know it is Friday evening. That makes the SUITE's answer depend on when
    it runs unless the clock is handed to it, so here it is handed to it.

    Advanced by the `time.sleep` hook rather than per call, because one poll
    makes several calls to `now()` - the window test, every `log()` stamp, the
    history bounds - and they must all see the same instant, exactly as they
    do in a real poll.
    """

    def __init__(self, instants):
        self.instants = list(instants)
        self.i = 0

    def now(self, tz=None):
        when = self.instants[min(self.i, len(self.instants) - 1)]
        return when if tz is None else when.astimezone(tz)

    def tick(self) -> None:
        self.i += 1


def drive_weekend(instants, held=None, book=BOOK, extra=None, orders=None,
                  polls: int = None) -> dict:
    """Poll `main()` once per instant, with a clock frozen at each in turn.

    `held` is the account's positions, and a close REMOVES one - which the
    other drivers in this file do not need and this section cannot do
    without: the question "is it closed exactly once and not re-opened" is
    only askable of a terminal whose positions go away when they are closed.
    """
    instants = list(instants)
    polls = polls if polls is not None else len(instants)
    tmp = Path(tempfile.mkdtemp(prefix="sgst-wknd-"))
    real_root, real_status, real_sleep = X.ROOT, X.read_status, X.time.sleep
    real_dt, real_send, real_positions = X.dt, MT5.order_send, MT5.positions_get
    argv = sys.argv
    live = list(held or [])
    try:
        MT5.sent = []
        MT5.account = CENT
        MT5.info = info_for("XAUUSD.sc")
        MT5.margin = 5.0
        MT5.price = SYMBOLS["XAUUSD.sc"][4]
        MT5.orders = list(orders or [])
        MT5.orders_get_calls = 0
        MT5.deals = []
        MT5.positions_get = lambda **kw: list(live)

        def order_send(req):
            MT5.sent.append(req)
            if req.get("action") == MT5.TRADE_ACTION_REMOVE:
                MT5.orders = [o for o in MT5.orders if o.ticket != req.get("order")]
            if req.get("position") is not None:
                live[:] = [p for p in live if p.ticket != req.get("position")]
            return Obj(retcode=MT5.TRADE_RETCODE_DONE, comment="ok", order=1, deal=1,
                       price=req.get("price"), volume=req.get("volume"))

        MT5.order_send = order_send
        clock = FrozenClock(instants)
        # Only `datetime` is replaced; `timedelta` and `timezone` are the real
        # ones, because `history_of` does arithmetic with them and a fake
        # would be testing the fake.
        X.dt = types.SimpleNamespace(datetime=clock, timedelta=dt.timedelta,
                                     timezone=dt.timezone)
        X.ROOT = tmp
        X.read_status = lambda api, run: dict(RUN, open=book) if book is not None \
            else dict(RUN, open=None)
        n = {"polls": 0}

        def stop_after(_):
            n["polls"] += 1
            clock.tick()
            if n["polls"] >= polls:
                raise KeyboardInterrupt

        X.time.sleep = stop_after
        sys.argv = ["mt5_executor.py", "--run=t", "--terminal=x", "--login=33705331",
                    "--symbol=XAUUSD.sc", "--account=acct"] + list(extra or [])
        rc = X.main()
        here = tmp / "data" / "live" / "acct" / "t"
        out, snap = here / "executor.jsonl", here / "broker.json"
        rows = [json.loads(l) for l in out.read_text(encoding="utf-8").splitlines()
                if l.strip()] if out.exists() else []
        return {"rc": rc, "sent": list(MT5.sent), "rows": rows, "held": list(live),
                "snapshot": json.loads(snap.read_text(encoding="utf-8")) if snap.exists() else None,
                "orders_get_calls": MT5.orders_get_calls}
    finally:
        X.ROOT, X.read_status, X.time.sleep = real_root, real_status, real_sleep
        X.dt, MT5.order_send, MT5.positions_get = real_dt, real_send, real_positions
        sys.argv = argv
        MT5.orders = []
        shutil.rmtree(tmp, ignore_errors=True)


def the_weekend_backstop_holds_without_a_bar() -> None:
    """The account is flat before the weekend even when no bar arrives.

    Layer one - `flat_before_weekend_hhmm` in the engine - is bar-driven, and
    on 2026-09-18 the bar it needed never came: MetaTrader closes a bar only
    on a tick after its boundary, the weekly close sends none, so the newest
    closed bar the poller saw all weekend was 20:30Z. Fourteen books and two
    real positions went into the weekend with nothing said
    (docs/decisions/2026-09-19-weekend-flat-never-fires.md).

    This layer runs on the executor's own UTC wall clock, so it is asked and
    answered when the feed is dead, the API is down and the broker clock is
    two days stale. These checks own that clock, which is the only way to
    test a rule about Friday evening on a Tuesday afternoon.
    """
    section("the weekend backstop")

    CUT = X.weekend_cut_minute("20:45")
    check("--weekend-flat 20:45 is minute 1245 UTC", CUT == 1245, f"{CUT}")

    # ---- the flag, and that it refuses rather than defaults ----
    check("'off' disables it", X.weekend_cut_minute("off") is None)
    check("'OFF' too, whatever the case", X.weekend_cut_minute("OFF") is None)
    check("an empty value disables it", X.weekend_cut_minute("") is None)
    check("00:00 is a time and not a falsy off", X.weekend_cut_minute("00:00") == 0)
    for bad in ("2045", "20:45:00", "24:00", "20:60", "banana", "-1:00"):
        raised = False
        try:
            X.weekend_cut_minute(bad)
        except SystemExit:
            raised = True
        check(f"{bad!r} exits rather than quietly becoming the default", raised)

    # ---- the window, minute by minute, in UTC ----
    #
    # Friday at or after the cut, all of Saturday, Sunday until the reopen.
    # The two boundaries face opposite ways on purpose: Friday is `>=` and
    # Sunday is `<`, so an equality lets the week start rather than lets a
    # position ride.
    for when, want, why in (
            (at(THU, 20, 45), False, "Thursday at the same minute is a trading night"),
            (at(THU, 23, 59), False, "Thursday midnight is not the weekend"),
            (at(FRI, 0, 0), False, "Friday morning"),
            (at(FRI, 20, 44), False, "Friday one minute before the cut"),
            (at(FRI, 20, 44, 59), False, "...and 59 seconds, still before it"),
            (at(FRI, 20, 45), True, "Friday exactly on the cut"),
            (at(FRI, 20, 46), True, "Friday after the cut"),
            (at(FRI, 23, 59), True, "Friday midnight"),
            (at(SAT, 0, 0), True, "Saturday opens"),
            (at(SAT, 12, 0), True, "Saturday midday"),
            (at(SAT, 20, 44), True, "Saturday at a minute that is inside Friday's cut"),
            (at(SAT, 23, 59), True, "Saturday closes"),
            (at(SUN, 0, 0), True, "Sunday morning"),
            (at(SUN, 20, 59), True, "Sunday one minute before the reopen"),
            (at(SUN, 21, 0), False, "Sunday exactly at the reopen"),
            (at(SUN, 21, 0, 30), False, "...and half a minute past it"),
            (at(SUN, 23, 59), False, "Sunday night, the week is running"),
            (at(MON, 0, 0), False, "Monday"),
            (at(MON, 20, 45), False, "Monday at the same minute as the cut")):
        got = X.in_weekend_window(when, CUT)
        check(f"{when:%a %H:%M}Z {'in' if want else 'out'}: {why}", got == want, f"got {got}")

    # Off is off on every one of them, which is the property the flag sells.
    check("with the flag off no instant is in the window",
          not any(X.in_weekend_window(at(d, h, 0), None)
                  for d in (FRI, SAT, SUN, MON) for h in range(24)))

    # A different cut moves the Friday boundary and nothing else.
    early = X.weekend_cut_minute("16:00")
    check("a 16:00 cut takes Friday from 16:00Z", X.in_weekend_window(at(FRI, 16, 0), early)
          and not X.in_weekend_window(at(FRI, 15, 59), early))
    check("...and leaves Saturday and the Sunday reopen exactly where they were",
          X.in_weekend_window(at(SAT, 3, 0), early)
          and not X.in_weekend_window(at(SUN, 21, 0), early))

    # ---- a position held into the window is closed, once ----
    pos = position(555, is_buy=True, lots=0.05)
    r = drive_weekend([at(FRI, 20, 45), at(FRI, 20, 45, 15)], held=[pos])
    closes = [q for q in r["sent"] if q.get("position") is not None]
    opens = [q for q in r["sent"] if q.get("action") == MT5.TRADE_ACTION_DEAL
             and q.get("position") is None]
    check("a position held at the cut is closed", len(closes) == 1, f"{len(closes)} closes")
    check("...exactly once, and not again on the next poll",
          len(r["sent"]) == 1, f"sent {len(r['sent'])}")
    check("...and NOT re-opened, though the book is still LONG",
          not opens, f"{opens}")
    check("...leaving the account flat", r["held"] == [], f"{r['held']}")

    # ---- and it goes through the close path that already exists ----
    #
    # Not "a close was sent" but "the SAME close was sent". The field set is
    # compared against the `book flat` close the reconciler has always sent,
    # because a backstop with its own order-sending routine is a routine no
    # broker has ever accepted - and this file's section 11 exists because
    # the close path's one difference from the open path cost 1,389 failures.
    flat = drive_weekend([at(MON, 12, 0)], held=[position(556)], book=None)
    ordinary = [q for q in flat["sent"] if q.get("position") is not None]
    check("the reconciler's own close still goes out on a weekday", len(ordinary) == 1,
          f"{flat['sent']}")
    if closes and ordinary:
        check("the backstop's close carries exactly the reconciler's fields",
              set(closes[0]) == set(ordinary[0]),
              f"{sorted(set(closes[0]) ^ set(ordinary[0]))}")
        check("...the same action, and a position ticket as an int",
              closes[0].get("action") == MT5.TRADE_ACTION_DEAL
              and isinstance(closes[0].get("position"), int))
        check("...and closing a LONG sells at the bid",
              closes[0].get("type") == MT5.ORDER_TYPE_SELL
              and closes[0].get("price") == MT5.price - 0.2,
              f"{closes[0].get('type')} at {closes[0].get('price')}")

    # The comment is the field that decides whether a close leaves the
    # terminal at all, so the weekend reason is measured against the same
    # limit as every other reason.
    for run in ("ai-xau-ds-ctx", "ai-xau-terra-ctx", "ai-xau-opus-ctx-b", "xau-ema"):
        c = X.close_comment(run, "weekend backstop")
        check(f"weekend close comment fits: {c!r} ({len(c)})", len(c) <= X.COMMENT_MAX)
        check(f"...and is not cut mid-word: {c!r}", c == c.strip() and c.endswith("wknd"))
    check("the run under test closes with 't wknd'",
          X.close_comment("t", "weekend backstop") == "t wknd")

    # ---- the rows ----
    flat_rows = [q for q in r["rows"] if q["kind"] == "weekend-flat"]
    check("one weekend-flat row for the one position closed", len(flat_rows) == 1,
          f"{[q['kind'] for q in r['rows']]}")
    if flat_rows:
        row = flat_rows[0]
        want = {"kind", "time", "book", "ticket", "side", "lots", "price", "profit",
                "currency", "sent", "dry_run", "reason"}
        check("weekend-flat carries exactly the fields the desk needs",
              set(row) == want, f"{sorted(set(row) ^ want)}")
        check("...the ticket, the side and the lots", row["ticket"] == 555
              and row["side"] == "LONG" and row["lots"] == 0.05, f"{row}")
        check("...the book it belongs to", row["book"] == "t", f"{row.get('book')}")
        check("...the quote it was sent at", row["price"] == MT5.price - 0.2, f"{row['price']}")
        # The unit-carrying rule, rule 1 and rule 5: money never travels
        # without the name of what it is in. `profit` here is USC on this
        # account, and a reader taking it for USD is out by a factor of 100.
        check("...the profit the terminal reports", row["profit"] == 0.0, f"{row['profit']}")
        check("...and the ACCOUNT's currency beside it, named", row["currency"] == "USC",
              f"{row['currency']}")
        check("...and it says the order actually went", row["sent"] is True
              and row["dry_run"] is False, f"{row}")

    win = [q for q in r["rows"] if q["kind"] == "weekend-window"]
    check("the window opening is recorded", len(win) == 1 and win[0]["state"] == "open",
          f"{win}")
    check("...once, not once per poll", len(win) == 1, f"{len(win)} rows")
    if win:
        check("...naming the instant, the cut and the reopen",
              win[0]["at_utc"] == "2026-09-18T20:45:00Z" and win[0]["cut_utc"] == "20:45"
              and win[0]["reopen_utc"] == "21:00", f"{win[0]}")
        check("...and how much it found to close", win[0]["positions_held"] == 1,
              f"{win[0].get('positions_held')}")

    # A window that finds nothing still says it ran. "The backstop fired and
    # the account was already flat" is the evidence that layer one worked,
    # and it only exists if it is written down at the time.
    quiet = drive_weekend([at(SAT, 3, 0)], held=[], book=None)
    qwin = [q for q in quiet["rows"] if q["kind"] == "weekend-window"]
    check("a window with nothing to close still writes its row", len(qwin) == 1, f"{qwin}")
    check("...saying the account was already flat",
          bool(qwin) and qwin[0]["positions_held"] == 0, f"{qwin}")
    check("...and nothing was sent", quiet["sent"] == [], f"{quiet['sent']}")

    # ---- the refusal: once per book per window, not once per poll ----
    many = drive_weekend([at(SAT, 1, 0), at(SAT, 1, 0, 15), at(SAT, 1, 0, 30),
                          at(SAT, 1, 0, 45)], held=[])
    refused = [q for q in many["rows"] if q["kind"] == "weekend-refused"]
    check("four polls with the book open send nothing", many["sent"] == [], f"{many['sent']}")
    check("...and record the refusal once, not four times", len(refused) == 1,
          f"{len(refused)} rows")
    if refused:
        row = refused[0]
        want = {"kind", "time", "book", "side", "lots", "book_entry", "at_utc",
                "cut_utc", "reason"}
        check("weekend-refused carries exactly its fields", set(row) == want,
              f"{sorted(set(row) ^ want)}")
        check("...naming the book and what it wanted",
              row["book"] == "t" and row["side"] == "LONG" and row["lots"] == 0.05,
              f"{row}")
    check("the desk can see why the book is flat", bool(many["snapshot"])
          and "weekend backstop" in str(many["snapshot"].get("standing_out")),
          f"{(many['snapshot'] or {}).get('standing_out')}")
    check("...and the snapshot keeps being written every poll",
          bool(many["snapshot"]) and many["snapshot"].get("login") == 33705331)

    # ---- the window closes, and the mirror goes back to work ----
    over = drive_weekend([at(SUN, 20, 59), at(SUN, 21, 0)], held=[])
    kinds = [q["kind"] for q in over["rows"]]
    states = [q["state"] for q in over["rows"] if q["kind"] == "weekend-window"]
    check("the window opens and then closes", states == ["open", "closed"], f"{states}")
    check("...and the book is mirrored again at the reopen",
          any(q.get("action") == MT5.TRADE_ACTION_DEAL and q.get("position") is None
              for q in over["sent"]), f"{kinds}")

    # A book refused in one window is refused again in the NEXT one. The
    # bookkeeping that makes a refusal once-per-window must not make it
    # once-ever.
    twice = drive_weekend([at(SAT, 1, 0), at(SUN, 21, 0), at(NEXT_FRI, 20, 50)], held=[])
    check("a new window refuses the same book again",
          len([q for q in twice["rows"] if q["kind"] == "weekend-refused"]) == 2,
          f"{[q['kind'] for q in twice['rows']]}")

    # ---- a dry run closes nothing and still says what it would have done ----
    dry = drive_weekend([at(SAT, 2, 0)], held=[position(557)], extra=["--dry-run"])
    check("a dry run sends nothing in the window", dry["sent"] == [], f"{dry['sent']}")
    dry_flat = [q for q in dry["rows"] if q["kind"] == "weekend-flat"]
    check("...and still writes the weekend-flat row", len(dry_flat) == 1, f"{dry_flat}")
    check("...marked as a dry run", bool(dry_flat) and dry_flat[0]["dry_run"] is True)

    # ---- the pending order, which only exists behind --mirror-pending ----
    #
    # A resting order left over the weekend fills on Sunday's gap, and the
    # gap is the whole argument for the guard: the median weekend gap over
    # the last twelve months is 13.55 points against a 12-point stop.
    rest = resting(9001, MT5.ORDER_TYPE_BUY_LIMIT, 4305.0)
    with_flag = drive_weekend([at(SAT, 4, 0)], held=[], orders=[rest],
                              extra=["--mirror-pending"])
    check("with --mirror-pending the resting order is withdrawn",
          [q.get("action") for q in with_flag["sent"]] == [MT5.TRADE_ACTION_REMOVE],
          f"{with_flag['sent']}")
    without = drive_weekend([at(SAT, 4, 0)], held=[], orders=[rest])
    check("...and with the flag off the terminal is not even asked for orders",
          without["orders_get_calls"] == 0 and without["sent"] == [],
          f"{without['orders_get_calls']} calls, {without['sent']}")

    # ---- the flag off changes nothing at all ----
    #
    # The string below is what the commit that added this would remove. With
    # it passed, a Saturday poll must be indistinguishable from a Monday one:
    # same requests, same row kinds, and not one row of the three new kinds.
    weekday = drive_weekend([at(MON, 12, 0)], held=[])
    saturday_off = drive_weekend([at(SAT, 12, 0)], held=[], extra=[WEEKEND_OFF])
    check("off: the Saturday poll sends exactly what the Monday poll sends",
          [dict(q) for q in saturday_off["sent"]] == [dict(q) for q in weekday["sent"]],
          f"{saturday_off['sent']} vs {weekday['sent']}")
    check("...including actually opening the book's position",
          len(saturday_off["sent"]) == 1
          and saturday_off["sent"][0].get("action") == MT5.TRADE_ACTION_DEAL,
          f"{saturday_off['sent']}")
    check("...and the same rows, kind for kind",
          [q["kind"] for q in saturday_off["rows"]] == [q["kind"] for q in weekday["rows"]],
          f"{[q['kind'] for q in saturday_off['rows']]}")
    check("...with no row of any weekend kind",
          not any(q["kind"].startswith("weekend") for q in saturday_off["rows"]),
          f"{[q['kind'] for q in saturday_off['rows']]}")
    check("...and nothing about the weekend in the snapshot",
          "weekend" not in str((saturday_off["snapshot"] or {}).get("standing_out")),
          f"{(saturday_off['snapshot'] or {}).get('standing_out')}")


def main() -> int:
    the_ceiling_measures_one_currency()
    both_size_guards_fail_closed()
    the_stamp_knows_whose_directory_it_is()
    stop_leaves_a_true_record()
    the_two_clocks()
    the_account_has_a_shape()
    a_partial_fill_is_not_a_fill()
    the_published_times_are_utc()
    a_better_price_is_not_a_reason_to_sit_out()
    favourable_joins_are_off_by_default()
    the_launcher_can_ask_what_the_rule_is()
    the_close_request_can_actually_be_sent()
    the_pending_order_is_mirrored_only_behind_the_flag()
    the_weekend_backstop_holds_without_a_bar()
    print(f"\n{'all checks passed' if not FAIL else str(FAIL) + ' CHECK(S) FAILED'}")
    return 1 if FAIL else 0


if __name__ == "__main__":
    sys.exit(main())
