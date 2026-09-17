# -*- coding: utf-8 -*-
"""The prompt variants differ by a removal and by nothing else.

    py -3.9 py/live/prompt_variant_selftest.py

WHY THIS EXISTS. `ai-xau-opus-ctx-b` is an experiment whose entire content is
one deleted sentence; the registration at
`docs/hypotheses/2026-09-17-prompt-coin-penalty.md` says so and says that
nothing was added in its place. If a later edit to PROMPT changes the two
variants unequally - a word added to one, a line reflowed in the other - the
comparison stops measuring the clause and starts measuring the edit, and
nothing anywhere would say so. The books would go on producing numbers.

This does not check that the prompt is GOOD. It checks that the difference
between the two is the difference the registration claims.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ai_trader as A  # noqa: E402

FIELDS = dict(market="xauusd", tf="15m", n=40, bars="<BARS>",
              position="<POS>", desk="<DESK>", context="<CTX>")

# The clause the experiment removes, quoted here independently of the source
# so that editing the source cannot quietly edit the test's idea of it.
PENALTY = ("so a trade you are not\nactually confident in is worse than no "
           "trade: it hands the coin a free sample.")

fails = []


def check(ok: bool, what: str) -> None:
    print(("ok   " if ok else "FAIL ") + what)
    if not ok:
        fails.append(what)


def render(variant: str, otl_block: str = "") -> str:
    return A.PROMPT.format(coin_clause=A.COIN_CLAUSE[A.VARIANTS[variant]["coin"]],
                           otl_block=otl_block, **FIELDS)


base, nocoin = render("base"), render("no-coin-penalty")

check(tuple(A.PROMPT_VARIANTS) == ("base", "no-coin-penalty", "otl-context"),
      "the variants are the three the registrations name")
check(PENALTY in base, "base still carries the penalty clause")
check(PENALTY not in nocoin, "no-coin-penalty does not carry it")
check("measured against a" in nocoin and "random side" in nocoin,
      "the coin is still named as the measurement in the variant")

# The whole of the difference: deleting the clause from base must produce the
# variant exactly. This is what "nothing was added" means, and it is the
# assertion a future edit will trip over.
check(base.replace(PENALTY, "").replace("random side, ", "random side.") == nocoin,
      "the variant IS base minus the clause - nothing added, nothing else changed")
check(len(base) > len(nocoin), "the variant is shorter, so the change was a removal")

# Everything after the opening sentence is shared, and that is where the desk
# state, the bars and the JSON contract live. A change there must land on both.
tail = "You may only answer in one of two ways"
check(base[base.index(tail):] == nocoin[nocoin.index(tail):],
      "every line after the opening sentence is identical in both")

# The default must be the prompt the running books have always had. A default
# that silently became the variant would rewrite four campaigns at once.
import argparse  # noqa: E402
ap = argparse.ArgumentParser()
ap.add_argument("--prompt-variant", default="base", choices=A.PROMPT_VARIANTS)
check(ap.parse_args([]).prompt_variant == "base",
      "a campaign that names no variant gets base")


# ---- otl-context: base PLUS a block, and nothing else ----
#
# The second experiment's whole content is an ADDITION, so the assertion is
# the mirror of the one above: strip the block back out and base must return.
# If a later edit moves a word while adding the block, the -otl book would be
# measuring the block and the word together with nothing saying so.
MARK = "\n\nBLOCK-LINE-ONE\nBLOCK-LINE-TWO"
otl = render("otl-context", MARK)
check(A.VARIANTS["otl-context"]["coin"] == "base",
      "otl-context runs the BASE coin clause, so it changes one thing and not two")
check(otl.replace(MARK, "") == base,
      "otl-context IS base plus the block - remove the block and base returns exactly")
check(A.VARIANTS["otl-context"]["otl"] and not A.VARIANTS["base"]["otl"]
      and not A.VARIANTS["no-coin-penalty"]["otl"],
      "only otl-context fetches the feed")
check(render("otl-context", "") == base,
      "with an empty block the otl variant renders as base, so a missing feed cannot reshape the prompt")

# ---- every number the block prints carries a unit ----
#
# A model reading "gamma wall: 4400" cannot know whether that is a price, a
# strike index or a contract count. The desk has paid for unit-less numbers
# three times this week - pnlUsd in cents, an equity axis in dollars over a
# cent account, `since` on the wrong clock - and a block written for a model
# to read is the last place to repeat it.
import otl_context as O  # noqa: E402

FAKE = {
    "state": "ok", "as_of_ms": 1_789_000_000_000, "age_ms": 5 * 60_000,
    "fields": {
        "max_gex_strike": 4400, "alldte_poc": 4360, "alldte_vah": 4500, "alldte_val": 4280,
        "whale_sup": 4200, "whale_res": 5000, "whale_symbol": "OGV6",
        "atm_price": 4356.3, "exp_move": 55.34, "avg_iv": 0.29,
        "0dte_bull": 1, "0dte_bear": 2, "weekly_bull": 3, "weekly_bear": 4,
        "big_prints": [{"t": 1_789_000_000_000, "strike": 4500, "class": "C",
                        "side": "LONG", "premium": 1250000}],
    },
}
text = O.block(FAKE)
for label, unit in (("gamma wall", "USD/oz"), ("all-DTE POC", "USD/oz"),
                    ("whale support", "USD/oz"), ("expected move", "USD/oz"),
                    ("0DTE bull premium", "USD"), ("weekly bear premium", "USD")):
    line = next((l for l in text.splitlines() if l.strip().startswith(label)), "")
    check(unit in line, f"the block gives '{label}' a unit ({unit})")
check("positioning, not direction" in text.lower().replace("\n", " ")
      or ("POSITIONING, not direction" in text),
      "the block says these describe positioning and not direction")
check("third party" in text.lower(), "the block says a third party computed them")
check("Z" in text.splitlines()[3], "the block stamps its age in UTC")

# The three states must be distinguishable, because a silent fallback would
# put context-absent decisions in a context-present book.
check("UNAVAILABLE" in O.block({"state": "unavailable"}),
      "an unreachable feed says so in the prompt")
stale = O.block(dict(FAKE, state="stale", age_ms=42 * 60_000))
check("STALE" in stale and "42" in stale,
      "a stale feed says so, with its age")

print()
print(f"{'all checks passed' if not fails else str(len(fails)) + ' FAILED'}")
sys.exit(1 if fails else 0)
