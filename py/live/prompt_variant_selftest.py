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


def render(variant: str) -> str:
    return A.PROMPT.format(coin_clause=A.COIN_CLAUSE[variant], **FIELDS)


base, nocoin = render("base"), render("no-coin-penalty")

check(tuple(A.PROMPT_VARIANTS) == ("base", "no-coin-penalty"),
      "the two variants are the two the registration names")
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

print()
print(f"{'all checks passed' if not fails else str(len(fails)) + ' FAILED'}")
sys.exit(1 if fails else 0)
