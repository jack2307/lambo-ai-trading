"""Pin the introducing-broker rebate in mt5_executor, on the numbers that define it.

    C:/Python39/python.exe py/live/rebate_selftest.py

The owner is the introducing broker on his own accounts, so part of the spread
his trading pays comes back. He was offered three ways to count it and chose a
SEPARATE CREDIT LINE on live and on paper - not a change to the cost model. So
the first thing this file checks is the thing the choice was about: that
`realised` does not move.

Nothing here touches a terminal, an account, the network, or any file under
`data/`. A fake `MetaTrader5` module hands `history_of` a deal history, and a
temporary directory stands in for the run's own folder.

  1  THE ARITHMETIC, ON THE ACCOUNT THE DESK ACTUALLY TRADES. XAUUSD.sc on
     the funded cent account (login 33705331): contract 1 oz, tick 0.01, tick
     value 1.00 USC, so one unit of price is 100 USC a lot. At the configured
     0.28 spread and 0.07 lots a round turn costs 1.96 USC and 45% of it is
     0.88 USC. Those two numbers are the whole feature and they are checked
     literally.

     0.88 USC is about 0.011 R on a book risking ~0.82 USD a trade. It is
     written here because the temptation with a credit line is to imply it
     rescues something: on the recent-year screen stoch-reversal is -0.044 R
     of expectancy, the rebate moves that to about -0.033 R, and to about
     -0.023 R even if the 45% were paid per side rather than per round turn.
     Still negative.

  2  `realised` IS UNTOUCHED. The same history with and without the terms
     must give the same realised total and the same per-trade `pnl`. This is
     the check that would fail if anyone ever folded the credit into the cost
     model after all, and it is the reason this file exists.

  3  MEASURED AND ESTIMATED ARE NEVER ONE NUMBER. A trade whose spread was
     read at both its orders is EXACT; anything else falls back to the book's
     configured spread and is ESTIMATED; with no configured spread it is
     unpriced, credited nothing, and COUNTED. `docs/decisions/2026-09-17-
     unit-carrying.md` rule 5: the unit - and here, the provenance - appears
     in the record and not only in the code.

  4  NULL IS NOT ZERO. With no `[rebate]` table in config/accounts.toml the
     whole figure is `None`, not 0.0. A rebate of zero is one that was
     calculated and came to nothing; a null one was never calculated.

  5  THE CONVERSION IS READ, NOT ASSUMED. `contract_size` is 1.0 on
     XAUUSD.sc and one unit of price is worth 100 USC a lot. Anything that
     multiplies by the contract size on a cent account is out by a hundred,
     which is the `notional_of` defect of 2026-09-17 written a second time.
"""

from __future__ import annotations

import os
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


class Obj:
    """Anything MetaTrader5 returns as a named tuple."""

    def __init__(self, **kw):
        self.__dict__.update(kw)


sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import mt5_executor as X  # noqa: E402


# --------------------------------------------------------------- the terminal

MAGIC = 4242

# MEASURED from the live cent account, 2026-09-16 (config/default.toml records
# the same three): XAUUSD.sc contract 1 ounce, tick 0.01, tick value 1.00 USC.
SYMBOL = Obj(trade_contract_size=1.0, trade_tick_size=0.01, trade_tick_value=1.0)

# The configured spread the book is charged - config/default.toml
# [markets.xauusd.trading]. Deliberately NOT the measured median of 0.21: the
# desk charges 0.28 and the estimate has to be made from what the book pays.
CONFIGURED = 0.28

SHARE = 0.45   # config/accounts.toml [rebate] share_of_spread


def deals():
    """One closed LONG of 0.07 lots, in and out, as MT5 hands it over.

    The deal TICKETS are what the order-time spread memory is keyed by, so
    they are the only part of this that the rebate join depends on.
    """
    return [
        Obj(ticket=1001, magic=MAGIC, position_id=900, entry=0, type=0,
            time_msc=1_700_000_000_000, price=4378.59, volume=0.07,
            commission=0.0, profit=0.0, swap=0.0, comment=""),
        Obj(ticket=1002, magic=MAGIC, position_id=900, entry=1, type=1,
            time_msc=1_700_000_600_000, price=4380.00, volume=0.07,
            commission=0.0, profit=9.87, swap=0.0, comment="tp"),
    ]


def make_mt5(rows):
    m = types.ModuleType("MetaTrader5")
    m.DEAL_ENTRY_IN = 0
    m.DEAL_TYPE_BUY = 0
    m.history_deals_get = lambda a, b: rows
    return m


def basis(configured=CONFIGURED, share=SHARE, per_unit=None):
    return {
        "share": share,
        "configured_spread": configured,
        "per_price_unit": X.money_per_price_unit(SYMBOL) if per_unit is None else per_unit,
        "currency": "USC",
    }


# ------------------------------------------------------------------ 1. the sum

def the_arithmetic_is_the_owners() -> None:
    section("\n1. the arithmetic, on the account the desk actually trades")

    per_unit = X.money_per_price_unit(SYMBOL)
    check("one unit of price is 100 USC a lot, read from the symbol",
          per_unit == 100.0, f"got {per_unit}")
    check("and it is NOT the contract size, which is 1.0 on this symbol",
          per_unit != SYMBOL.trade_contract_size)

    # A round turn at the configured spread: 0.28 x 100 x 0.07 = 1.96 USC.
    round_turn = CONFIGURED * 0.07 * per_unit
    check("a round turn costs 1.96 USC at 0.07 lots", abs(round_turn - 1.96) < 1e-9,
          f"got {round_turn}")

    mt5 = make_mt5(deals())
    realised, closed, fills, rebate = X.history_of(mt5, MAGIC, 0, {}, basis())
    check("45% of it is 0.88 USC", abs(rebate["amount"] - 0.88) < 0.005,
          f"got {rebate['amount']}")
    check("and the trade carries its own copy of that figure",
          abs(fills[0]["rebate"] - 0.882) < 1e-9, f"got {fills[0]['rebate']}")
    check("the credit is in the account's currency and says so",
          rebate["currency"] == "USC", str(rebate))
    check("one closed trade", closed == 1 and len(fills) == 1)

    # ~0.011 R on a book risking about 0.82 USD (82 USC) a trade. The rebate
    # is small and this line is here so nobody has to take that on trust.
    r = rebate["amount"] / 82.0
    check("which is about 0.011 R of the book's own risk", 0.009 < r < 0.013, f"got {r:.4f} R")


# ------------------------------------------------------- 2. nothing else moves

def realised_is_untouched() -> None:
    section("\n2. `realised` is the number it always was")

    mt5 = make_mt5(deals())
    without = X.history_of(mt5, MAGIC, 0)
    with_terms = X.history_of(mt5, MAGIC, 0, {}, basis())

    check("the realised total is identical with and without the terms",
          without[0] == with_terms[0], f"{without[0]} vs {with_terms[0]}")
    check("and it is the deals' own money, 9.87 USC",
          with_terms[0] == 9.87, str(with_terms[0]))
    check("the per-trade pnl is identical too",
          without[2][0]["pnl"] == with_terms[2][0]["pnl"])
    check("the credit is published beside it, never inside it",
          with_terms[3]["amount"] > 0
          and with_terms[3]["realised_with_rebate"] == round(9.87 + with_terms[3]["amount"], 2),
          str(with_terms[3]))
    check("with no terms there is no rebate object at all", without[3] is None)
    check("and no trade pretends to carry one", without[2][0]["rebate"] is None)


# ------------------------------------------- 3. measured is not estimated

def measured_is_never_mixed_with_estimated() -> None:
    section("\n3. a measured spread and a guessed one are never one number")

    mt5 = make_mt5(deals())

    # Nothing captured: the configured spread, labelled.
    _, _, fills, rebate = X.history_of(mt5, MAGIC, 0, {}, basis())
    check("with no quote captured the trade is ESTIMATED",
          fills[0]["rebateBasis"] == "ESTIMATED", str(fills[0]))
    check("and the counts say so", (rebate["exact"], rebate["estimated"], rebate["unpriced"]) == (0, 1, 0),
          str(rebate))
    check("the spread the estimate used is published with it",
          rebate["configured_spread"] == CONFIGURED)

    # Both orders' quotes captured: exact, and on a tighter spread than the
    # configured one - which is the real case, since the logger's measured
    # median on this symbol is 0.21 against a configured 0.28.
    seen = {"1001": 0.21, "1002": 0.21}
    _, _, fills, rebate = X.history_of(mt5, MAGIC, 0, seen, basis())
    check("with both quotes captured the trade is EXACT",
          fills[0]["rebateBasis"] == "EXACT", str(fills[0]))
    check("and the counts say so", (rebate["exact"], rebate["estimated"], rebate["unpriced"]) == (1, 0, 0),
          str(rebate))
    exact_expected = round(SHARE * 0.21 * 0.07 * 100.0, 4)
    check("the credit is 45% of the spread that was actually paid",
          abs(rebate["amount"] - round(exact_expected, 2)) < 1e-9,
          f"got {rebate['amount']}, wanted {round(exact_expected, 2)}")
    check("which is LESS than the estimate, because the real spread was tighter",
          rebate["amount"] < 0.88)

    # One side only. Half a measurement is not a measurement.
    _, _, fills, rebate = X.history_of(mt5, MAGIC, 0, {"1001": 0.21}, basis())
    check("one side captured is still an ESTIMATE, not half of one",
          fills[0]["rebateBasis"] == "ESTIMATED" and rebate["estimated"] == 1, str(rebate))

    # Nothing to price from at all.
    _, _, fills, rebate = X.history_of(mt5, MAGIC, 0, {}, basis(configured=None))
    check("with no basis at all the trade's rebate is null, not zero",
          fills[0]["rebate"] is None and fills[0]["rebateBasis"] is None, str(fills[0]))
    check("and the total says how many it could not price",
          (rebate["exact"], rebate["estimated"], rebate["unpriced"]) == (0, 0, 1), str(rebate))
    check("the amount is the sum over what COULD be priced, which is nothing",
          rebate["amount"] == 0.0)

    # The terminal would not say what a unit of price is worth.
    _, _, fills, rebate = X.history_of(mt5, MAGIC, 0, {}, basis(per_unit=None) | {"per_price_unit": None})
    check("an unreadable tick value refuses rather than guessing",
          fills[0]["rebate"] is None and rebate["unpriced"] == 1, str(rebate))


# ------------------------------------------------------------ 4. null vs zero

def null_is_not_zero() -> None:
    section("\n4. null is not zero, in the file and on the wire")

    root = Path(tempfile.mkdtemp(prefix="rebate-selftest-"))
    try:
        missing = root / "no-such.toml"
        check("a missing registry records no arrangement", X.rebate_share(missing) is None)

        silent = root / "silent.toml"
        silent.write_text("[prices]\nterminal = 'x'\n", encoding="utf-8")
        check("a registry with no [rebate] table records no arrangement",
              X.rebate_share(silent) is None)

        zero = root / "zero.toml"
        zero.write_text("[rebate]\nshare_of_spread = 0.0\n", encoding="utf-8")
        check("but a share of 0.0 IS an arrangement, and it is zero",
              X.rebate_share(zero) == 0.0, str(X.rebate_share(zero)))

        real = root / "real.toml"
        real.write_text("[rebate]\nshare_of_spread = 0.45\n", encoding="utf-8")
        check("and 0.45 reads as 0.45", X.rebate_share(real) == 0.45)

        # The shipped file is the one that matters.
        shipped = Path(X.ROOT) / "config" / "accounts.toml"
        check("config/accounts.toml grants 0.45 of the round-turn spread",
              X.rebate_share(shipped) == 0.45, str(X.rebate_share(shipped)))

        mt5 = make_mt5(deals())
        _, _, fills, rebate = X.history_of(mt5, MAGIC, 0, {}, basis(share=0.0))
        check("a zero share credits zero and says it priced the trade",
              rebate["amount"] == 0.0 and rebate["estimated"] == 1 and fills[0]["rebate"] == 0.0,
              str(rebate))
    finally:
        for p in sorted(root.rglob("*"), reverse=True):
            p.unlink()
        root.rmdir()


# ------------------------------------------------- 5. the order-time memory

def the_order_time_memory_survives_a_restart() -> None:
    section("\n5. the spread read at the order outlives the process that read it")

    root = Path(tempfile.mkdtemp(prefix="rebate-selftest-"))
    try:
        path = root / "spreads.json"
        store = X.load_spreads(path)
        check("a missing file is an empty memory, not an error", store == {})

        X.remember_spread(path, store, 1001, 0.21)
        X.remember_spread(path, store, 1002, 0.22)
        check("both quotes are on disk", X.load_spreads(path) == {"1001": 0.21, "1002": 0.22},
              str(X.load_spreads(path)))

        X.remember_spread(path, store, None, 0.21)
        X.remember_spread(path, store, 1003, None)
        check("a deal with no spread and a spread with no deal are both dropped",
              X.load_spreads(path) == {"1001": 0.21, "1002": 0.22}, str(X.load_spreads(path)))

        # Reloaded, a restarted executor still prices the same trade exactly.
        mt5 = make_mt5(deals())
        _, _, fills, rebate = X.history_of(mt5, MAGIC, 0, X.load_spreads(path), basis())
        check("and a restarted executor still calls that trade EXACT",
              fills[0]["rebateBasis"] == "EXACT" and rebate["exact"] == 1, str(rebate))

        # The memory is bounded.
        big = {}
        for n in range(X.SPREAD_MEMORY + 20):
            X.remember_spread(path, big, 9000 + n, 0.2)
        check(f"the memory stops at {X.SPREAD_MEMORY} and drops the oldest first",
              len(big) == X.SPREAD_MEMORY and "9000" not in big and str(9000 + X.SPREAD_MEMORY + 19) in big,
              f"{len(big)} kept")
    finally:
        for p in sorted(root.rglob("*"), reverse=True):
            p.unlink()
        root.rmdir()


def the_quote_is_read_or_refused() -> None:
    section("\n6. an unreadable quote is not a spread of nothing")

    check("a crossed book is unreadable", X.quoted_spread(Obj(bid=4378.6, ask=4378.3)) is None)
    check("a missing tick is unreadable", X.quoted_spread(None) is None)
    check("a missing side is unreadable", X.quoted_spread(Obj(bid=4378.3)) is None)
    got = X.quoted_spread(Obj(bid=4378.31, ask=4378.59))
    check("and a real quote is the difference: 4378.59 - 4378.31 = 0.28",
          abs(got - 0.28) < 1e-9, f"got {got}")


def the_book_supplies_the_estimate() -> None:
    section("\n7. the estimate's spread comes from the book, and is remembered")

    check("a status carrying the book's spread supplies it",
          X.spread_of({"rebate": {"spread": 0.28}}, None) == 0.28)
    check("a status with no rebate line leaves the last one standing",
          X.spread_of({}, 0.28) == 0.28)
    check("and an unreachable API does not turn the estimate to null",
          X.spread_of(None, 0.28) == 0.28)
    check("a nonsense spread is refused rather than used",
          X.spread_of({"rebate": {"spread": 0}}, 0.28) == 0.28)
    check("with nothing ever seen it stays None", X.spread_of(None, None) is None)


def main() -> int:
    print(__doc__.strip().splitlines()[0])
    the_arithmetic_is_the_owners()
    realised_is_untouched()
    measured_is_never_mixed_with_estimated()
    null_is_not_zero()
    the_order_time_memory_survives_a_restart()
    the_quote_is_read_or_refused()
    the_book_supplies_the_estimate()
    print(f"\n{'all checks passed' if not FAIL else str(FAIL) + ' CHECK(S) FAILED'}")
    return 1 if FAIL else 0


if __name__ == "__main__":
    sys.exit(main())
