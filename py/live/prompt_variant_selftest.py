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


def render(variant: str, otl_block: str = "", htf_block: str = "",
           htf_rule_block: str = "") -> str:
    return A.PROMPT.format(coin_clause=A.COIN_CLAUSE[A.VARIANTS[variant]["coin"]],
                           otl_block=otl_block, htf_block=htf_block,
                           htf_rule_block=htf_rule_block, **FIELDS)


base, nocoin = render("base"), render("no-coin-penalty")

check(tuple(A.PROMPT_VARIANTS) == ("base", "no-coin-penalty", "otl-context",
                                   "htf-context", "htf-filter"),
      "the variants are the five the registrations name")
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
        "whale_sup": 4200, "whale_res": 5000,
        "atm_price": 4356.3, "exp_move": 55.34, "avg_iv": 0.29,
        "daily_bull": 12321257.0, "daily_bear": 19885617.0,
        "weekly_bull": 11068826.0, "weekly_bear": 11912388.0,
        "flow_window_h": 90.8, "flow_prints": 7915, "flow_unclassified": 0,
        "big_prints": [{"t": 1_789_000_000_000, "strike": 4500, "class": "C",
                        "side": "LONG", "premium": 1250000}],
    },
}
text = O.block(FAKE)
for label, unit in (("gamma wall", "USD/oz"), ("all-DTE POC", "USD/oz"),
                    ("whale support", "USD/oz"), ("expected move", "USD/oz"),
                    ("daily-expiry contracts", "USD"), ("weekly-expiry contracts", "USD")):
    line = next((l for l in text.splitlines() if l.strip().startswith(label)), "")
    check(unit in line, f"the block gives '{label}' a unit ({unit})")

# The premium lines must carry the window they cover. A total with no window
# is a number nobody can compare to anything, and both lines are sums over a
# span the feed chooses rather than one this desk fixed.
for label in ("daily-expiry contracts", "weekly-expiry contracts"):
    line = next((l for l in text.splitlines() if l.strip().startswith(label)), "")
    check("over the last" in line and "h:" in line,
          f"'{label}' states the window it covers")

# Never the words 0DTE: the block reports the feed's contract CLASS and is not
# measuring days to expiry. A daily-expiry contract is usually but not always
# today's, and labelling it 0DTE would be a claim this does not check.
check("0DTE" not in text, "the block does not claim 0DTE, which it does not measure")
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


# ---- htf-context and htf-filter: one addition, then one sentence ----
#
# Registered at docs/hypotheses/2026-09-18-htf-context.md. Two variants, and
# the pair only means anything if each differs from its own control by exactly
# one thing: htf-context is base plus a facts block, and htf-filter is
# htf-context plus one sentence. If a later edit moves a word while adding
# either, the books measure the change and the word together and nothing says
# so.
HMARK = "\n\nHTF-LINE-ONE\nHTF-LINE-TWO"
htf_ctx = render("htf-context", "", HMARK)
check(A.VARIANTS["htf-context"]["coin"] == "base"
      and not A.VARIANTS["htf-context"]["otl"],
      "htf-context runs the BASE coin clause and no options block, so it changes one thing")
check(htf_ctx.replace(HMARK, "") == base,
      "htf-context IS base plus the block - remove the block and base returns exactly")
check(render("htf-context", "", "") == base,
      "with an empty block htf-context renders as base, so a missing route cannot reshape the prompt")

rule_block = "\n" + A.HTF_RULE + "\n"
htf_filt = render("htf-filter", "", HMARK, rule_block)
check(htf_filt.replace(rule_block, "") == htf_ctx,
      "htf-filter IS htf-context plus the rule - remove the rule and htf-context returns exactly")
check(len(htf_filt) > len(htf_ctx), "the filter variant is longer, so the change was an addition")
check(A.VARIANTS["htf-filter"]["htf"] and A.VARIANTS["htf-filter"]["htf_rule"]
      and A.VARIANTS["htf-context"]["htf"] and not A.VARIANTS["htf-context"]["htf_rule"],
      "only htf-filter carries the rule, and both carry the block")
check(not any(A.VARIANTS[v]["htf"] for v in ("base", "no-coin-penalty", "otl-context")),
      "no existing book starts fetching the route because these were added")

# THE RULE KEYS ON STRUCTURE AND ON NOTHING ELSE. a5's decision of
# 2026-09-18: ADX and the efficiency ratio stay in the block as facts and out
# of the rule, because the study behind the registration found the structure
# label was the one definition that did not change sign or size when the bar
# anchor was corrected. A later hand widening the rule to ADX would make the
# variant a different experiment under the same book id.
check("structure" in A.HTF_RULE.lower(), "the rule names the structure label")
for word in ("adx", "efficiency", "ema", "donchian", "atr", "di("):
    check(word not in A.HTF_RULE.lower(), f"the rule does not key on {word}")

# A rule that left RANGE and a missing route undefined would have the model
# resolve them silently, differently on different bars.
# The rule is wrapped for the prompt, so a phrase can straddle a line break.
low = " ".join(A.HTF_RULE.lower().split())
check("range" in low, "the rule says what RANGE means for it")
for word in ("absent", "stale", "unavailable"):
    check(word in low, f"the rule says what a {word} label means for it")
check("both sides stay open" in low, "and says both sides stay open in those states")

# ---- the block: two ages, four states, a unit on every number ----
import htf_context as H  # noqa: E402

H4MS = 4 * 3600 * 1000
BAR = 1_758_232_800_000
FAKE_H4 = {
    "computed_at_bar_ms": BAR, "computed_at_ms": BAR + H4MS + 900_000,
    "timeframe": "4h", "bar_ms": H4MS,
    "structure": {"label": "UP", "rule": "fractal(2)",
                  "confirmed_at_bar_ms": BAR - 2 * H4MS,
                  "last_high": {"price": 3712.4, "bar_ms": BAR - 3 * H4MS},
                  "prior_high": {"price": 3690.0, "bar_ms": BAR - 9 * H4MS},
                  "last_low": {"price": 3655.1, "bar_ms": BAR - 5 * H4MS},
                  "prior_low": {"price": 3631.2, "bar_ms": BAR - 11 * H4MS},
                  "break_level": 3655.1, "break_side": "BELOW"},
    "ema21": 3688.2, "ema55": 3661.9, "ema21_slope_sign": 1, "ema55_slope_sign": 0,
    "atr14": 18.4, "dist_ema21_atr": 0.83, "adx14": 28.6,
    "plus_di14": 31.2, "minus_di14": 14.9, "efficiency_20": 0.41,
    "donchian20": {"upper": 3718.0, "lower": 3612.5,
                   "bars_since_new_high": 0, "bars_since_new_low": 37},
    "last_close": 3703.5,
}
FAKE_D1 = {
    "computed_at_bar_ms": BAR - 6 * H4MS, "computed_at_ms": BAR + H4MS,
    "timeframe": "1d", "bar_ms": 86_400_000,
    "prior_day_high": 3701.0, "prior_day_low": 3648.0, "prior_day_bar_ms": BAR - 12 * H4MS,
    "prior_week_high": 3720.0, "prior_week_low": 3600.0, "prior_week_mid": 3660.0,
    "prior_week_start_ms": BAR - 60 * H4MS,
    "close_pct_of_prior_week_range": 86.3, "last_close": 3703.5,
}


def ctx(**over):
    c = {"state": "ok", "market": "xauusd", "h4": FAKE_H4, "d1": FAKE_D1,
         "unavailable": None, "why": None, "staleness_checked": True,
         "facts_age_ms": H4MS + 900_000 - H4MS, "facts_age_bars": 0.1,
         "label_age_ms": 3 * H4MS + 900_000, "label_age_bars": 2.1,
         "thin_fields": []}
    c.update(over)
    return c


htext = H.block(ctx())
for label, unit in (("last H4 close", "USD/oz"), ("EMA(21)", "USD/oz"),
                    ("ATR(14) on H4", "USD/oz"), ("Donchian(20) upper", "USD/oz"),
                    ("prior day high", "USD/oz"), ("prior week midpoint", "USD/oz"),
                    ("close minus EMA(21)", "ATR(14)")):
    line = next((l for l in htext.splitlines() if l.strip().startswith(label)), "")
    check(unit in line, f"the htf block gives '{label}' a unit ({unit})")
for label in ("ADX(14)", "efficiency ratio, 20 bars"):
    line = next((l for l in htext.splitlines() if l.strip().startswith(label)), "")
    check("unitless" in line, f"'{label}' is declared unitless rather than left bare")

# TWO AGES, AND THE BLOCK MUST SAY THEY ARE DIFFERENT. A fractal is confirmed
# up to n bars after the swing, so the label can be older than every other
# number in the block. One age would tell the reader the label is as fresh as
# the ADX reading.
check("has since closed" in htext, "the block ages the facts")
check("confirmed:" in htext, "the block ages the structure label separately")
conf = next(l for l in htext.splitlines() if l.strip().startswith("confirmed:"))
check("OLDER than the facts" in conf, "and says which of the two is older, and why")

# The percentage is NOT a 0-100 percentage and the block must not imply it is.
out = H.block(ctx(d1=dict(FAKE_D1, close_pct_of_prior_week_range=137.4)))
line = next(l for l in out.splitlines() if "% of the prior week" in l)
check("137.4" in line and "ABOVE" in line and "not capped" in line,
      "a close above the prior week's range reads as above it, uncapped")
out = H.block(ctx(d1=dict(FAKE_D1, close_pct_of_prior_week_range=-12.8)))
line = next(l for l in out.splitlines() if "% of the prior week" in l)
check("-12.8" in line and "BELOW" in line,
      "and a close below it reads as below, not as zero")

# The four states must be distinguishable, and `thin` must not read as
# `unavailable`: the route separates them on purpose.
una = H.block(ctx(state="unavailable", h4=None, d1=None, why="no 4h bars for xauusd"))
check("UNAVAILABLE" in una and "no 4h bars" in una, "an absent timeframe says so, with the reason")
stale = H.block(ctx(state="stale", why="17.0 H4 bars behind"))
check("STALE" in stale and "17.0" in stale, "a stale route says so, with how far behind")
thin = H.block(ctx(state="thin", why="warmup not met for adx14",
                   h4=dict(FAKE_H4, adx14=None), thin_fields=["adx14"]))
check("THIN" in thin and "UNAVAILABLE" not in thin,
      "a thin answer says THIN and is not reported as unavailable")
check("not computed yet" in thin and "absent below, not zero" in thin,
      "and its missing facts read as absent rather than as zero")

# Zero IS a measurement. A flat slope and a bar that just made a new high are
# readings, and rendering either as absent would lose them.
check("0 (flat)" in htext, "a zero slope reads as a flat measurement")
check("this bar made one" in htext, "zero bars since a new high reads as a measurement")

# H4 present and D1 absent is not a global outage, and the route can serve
# exactly that. Saying "unavailable" over a full H4 object would throw it away.
half = H.block(ctx(d1=None, unavailable="no 1d bars for xauusd"))
check("H4 SWING STRUCTURE" in half and "DAILY LEVELS: unavailable" in half,
      "a missing D1 is reported on its own line and the H4 facts survive it")
check(not half.startswith("HIGHER-TIMEFRAME FACTS for XAUUSD: UNAVAILABLE"),
      "and the block does not declare a global outage over a present H4 object")

# No verdict. The route publishes facts and the block must not add a word the
# route did not say - a composite "trend: UP" here would be this module
# deciding, which is the one thing it must not do.
check("trend is your friend" not in htext.lower(), "the block asserts no trend doctrine")
check("you should" not in htext.lower() and "recommend" not in htext.lower(),
      "the block advises nothing")

print()
print(f"{'all checks passed' if not fails else str(len(fails)) + ' FAILED'}")
sys.exit(1 if fails else 0)
