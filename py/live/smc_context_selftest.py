# -*- coding: utf-8 -*-
"""The structural-levels block says what the registration says it says.

    py -3.9 py/live/smc_context_selftest.py

WHY THIS EXISTS. `ai-xau-ds-smc` is an experiment whose entire content is one
added block. Two things can go wrong without anything failing loudly: the
block can quietly stop being the only difference between this book and its
control, and the route can change shape under it. The first is checked the way
`prompt_variant_selftest.py` checks the other variants - strip the block and
base must return, byte for byte.

THE SECOND IS CHECKED AGAINST THE ROUTE'S OWN CAPTURED RESPONSES, and that
matters more than it sounds. This file was first written against a fixture
invented from a prose description of the route, and everything passed. When
`docs/api-samples/paper-levels.json` landed on 2026-09-19 - 120 KB off the
live store - it disagreed with that fixture in eight field names, in its
grouping, in being single-timeframe, and in two units. Two of those were bugs
a hand-made fixture could never have caught:

  * the response's newest closed bar and the decision bar are the SAME bar on
    a healthy desk, so `htf_context`'s close-to-start staleness arithmetic
    reported a perfectly current response as minus one bar behind;
  * the store holds 879 bars across 1,308 bar-lengths of clock, so a bar
    count divided out of a millisecond delta is out by half - it printed a
    pool as swept eighty-one bars before it formed.

So the fixtures here are the samples on disk, not a paraphrase of them. If d1
changes the route, this fails on the next sample refresh rather than on the
first live bar of a registered campaign.

The pre-commitments it holds, from docs/hypotheses/2026-09-18-smc-context.md:

  1. `smc-context` IS base plus the block, and nothing else moved;
  2. every level line carries a unit;
  3. a fixed maximum count, stated in the block itself;
  4. nearest-first by distance from the last close, above and below separate;
  5. route-unreachable, thin and stale each SAY SO rather than rendering an
     empty block or falling back to base;
  6. no route-supplied string reaches the prompt, in any state.

It does NOT check that the levels are worth anything. That is what the book
measures, and the registration says the prior is that they are not.
"""
from __future__ import annotations

import copy
import io
import json
import os
import sys
import urllib.error
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, HERE)
import ai_trader as A  # noqa: E402
import htf_context as H  # noqa: E402
import smc_context as S  # noqa: E402

fails = []


def check(ok: bool, what: str) -> None:
    print(("ok   " if ok else "FAIL ") + what)
    if not ok:
        fails.append(what)


def sample(name: str) -> dict:
    path = os.path.join(ROOT, "docs", "api-samples", name)
    return json.loads(io.open(path, encoding="utf-8").read())


REAL = sample("paper-levels.json")
NONE_AT_ALL = sample("paper-levels-unavailable.json")

# The bar the desk would be deciding on when this response was served: the
# route's own newest closed bar. On a healthy desk they are the same bar, and
# that identity is the thing the first version of the staleness rule got
# wrong.
BAR = REAL["computed_at_bar_ms"]
CLOSE = REAL["last_close"]
ATR = REAL["atr14"]


# ---- a mocked route, because a selftest must not need a running desk ----

class _Resp:
    def __init__(self, payload):
        self._b = json.dumps(payload).encode("utf-8")

    def read(self):
        return self._b

    def __enter__(self):
        return self

    def __exit__(self, *a):
        return False


_REAL_URLOPEN = urllib.request.urlopen
_SEEN_URLS = []


def serve(payload):
    def _open(url, timeout=None):
        _SEEN_URLS.append(url)
        if isinstance(payload, Exception):
            raise payload
        return _Resp(payload)
    urllib.request.urlopen = _open


def gather(doc=None, **over):
    serve(REAL if doc is None else doc)
    try:
        kw = dict(api="http://mock", market="xauusd", bar_time=BAR,
                  last_close=CLOSE, atr=ATR)
        kw.update(over)
        return S.gather(**kw)
    finally:
        urllib.request.urlopen = _REAL_URLOPEN


# ---- the real response ----

ctx = gather()
text = S.block(ctx)
# The header wraps, so a phrase can straddle a line break. Checked against a
# whitespace-flattened copy, the same idiom prompt_variant_selftest uses on
# HTF_RULE for the same reason.
flat = " ".join(text.split())
print()
print(text)
print()

check(ctx["state"] == "ok", "the captured live response is ok")
check(_SEEN_URLS and _SEEN_URLS[-1].endswith("/api/paper/levels?market=xauusd"),
      "the route asked for is /api/paper/levels?market=<market>")
check(ctx["timeframe"] == "15m" and ctx["bar_ms"] == 900_000,
      "the response's single timeframe is read off the top level")
check(ctx["census"]["total"] == 252 and len(ctx["levels"]) == 252,
      f"every level on the response is harvested ({ctx['census']['total']})")
# The five families, unpacked from three differently shaped fields. A family
# silently dropped would be invisible: the block would just be shorter.
fam = ctx["census"]["families"]
check(fam.get("liquidity pool") == 155, f"155 liquidity pools ({fam.get('liquidity pool')})")
check(fam.get("order block") == 76, f"76 order blocks ({fam.get('order block')})")
check(fam.get("fair value gap") == 12, f"12 fair value gaps ({fam.get('fair value gap')})")
check(fam.get("profile level") == 3, "the profile OBJECT yields its three levels")
check(fam.get("period extreme") == 6,
      "the extremes OBJECT yields session, day and week, high and low")


# ---- staleness, which the real sample is the only way to get right ----

check(ctx["behind_min"] == 0.0,
      f"a response whose newest bar IS the decision bar is 0 minutes behind ({ctx['behind_min']})")
check(ctx["state"] == "ok", "and is therefore not stale")

# The regression the measured daily hole bought. `htf.rs` measured hour 21Z
# holding ZERO 15m bars, so a desk exactly ONE STORED BAR behind across that
# hole is over an hour of clock behind while being one bar behind. A tolerance
# of two bars of clock would withhold the block once a night.
hole = gather(dict(REAL, computed_at_bar_ms=BAR - 75 * 60 * 1000))
check(hole["state"] == "ok",
      f"one stored bar behind across the daily hole (75 min) is NOT stale ({hole['behind_min']} min)")
check(hole["tolerance_min"] == 90.0, "the tolerance is the measured hour plus two 15m bars")

stale = gather(dict(REAL, computed_at_bar_ms=BAR - 4 * 3600 * 1000))
check(stale["state"] == "stale", "four hours behind IS stale")
stext = S.block(stale)
check("STALE" in stext and "240 minutes" in stext and "90 minute tolerance" in stext,
      "and the block says how far behind and against what tolerance, in minutes")
check("bars behind" not in stext,
      "a clock gap is never reported as a bar count; the store is 879 bars over 1,308 of clock")

noc = gather(bar_time=None)
check(noc["state"] == "ok" and not noc["staleness_checked"],
      "without a decision bar the state is not stale")
check("NOT checked this run" in S.block(noc),
      "and the block says the lag was not checked rather than implying freshness")


# ---- the other three states ----

una = gather(NONE_AT_ALL)
check(una["state"] == "unavailable",
      "the captured no-bars response - every block null - is unavailable")
utext = S.block(una)
check("UNAVAILABLE" in utext and "deciding without them" in utext,
      "and the block says so rather than rendering empty")
check(len(utext.splitlines()) >= 3, "the unavailable block is a real block, not a blank line")

unreachable = gather(urllib.error.URLError("connection refused"))
check(unreachable["state"] == "unavailable", "an unreachable route is unavailable too")

# NULL IS NOT EMPTY, and the route's module doc insists on the difference:
# `null` means there were no bars to look at, an empty list means this market
# has no gaps today. Null blocks are UNAVAILABLE above; all-empty blocks are a
# market whose window produced nothing, which is THIN.
empty = gather(dict(REAL, profile=None, fair_value_gaps=[], order_blocks=[],
                    liquidity=[], extremes=None))
check(empty["state"] == "thin", "empty lists are thin, not unavailable")
ttext = S.block(empty)
check("THIN" in ttext and "UNAVAILABLE" not in ttext,
      "and the block says THIN rather than claiming the route is down")

noclose = gather(dict(REAL, last_close=None), last_close=None)
check(noclose["state"] == "thin", "no close to measure from is thin, not a crash")


# ---- no route-supplied string reaches the prompt, in ANY state ----
#
# The route's own doc comment on `unavailable` says this from the other side,
# and it applies to more than that field here. The rule strings are the ones
# the registration asked the route to carry, and they are another crate's
# prose: "activity profile over 429 bars, time-at-price, bucket = ATR(14)/4"
# is a sentence nobody editing this book would think to go and read.
for phrase in ("bucket = ATR(14)/4", "last down candle before a body",
               "3-bar imbalance", "fractal(2,2)", "htf::weeks_of",
               "split by the broker's daily hole", "TIME_AT_PRICE"):
    check(phrase not in text, f"the route's rule prose does not reach the prompt ({phrase!r})")
check("15m-lo-" not in text and "15m-hi-" not in text,
      "and neither do the swing ids, which join nothing the model can see here")
check(NONE_AT_ALL["unavailable"][:30] not in S.block(gather(NONE_AT_ALL)),
      "the route's unavailable SENTENCE does not reach the prompt")

MARKER = "ROUTE-PROSE-THAT-MUST-NOT-REACH-THE-PROMPT"
marked = copy.deepcopy(REAL)
marked["unavailable"] = MARKER
for row in marked["liquidity"]:
    row["rule"], row["kind"], row["state"], row["side"] = MARKER, MARKER, MARKER, MARKER
    row["swing_ids"] = [MARKER]
for row in marked["order_blocks"]:
    row["rule"], row["direction"] = MARKER, MARKER
for label, c in (
    ("ok", gather(marked)),
    ("unavailable", gather(urllib.error.URLError(MARKER))),
    ("thin", gather(dict(marked, profile=None, fair_value_gaps=[], order_blocks=[],
                         liquidity=[], extremes=None))),
    ("stale", gather(dict(marked, computed_at_bar_ms=BAR - 4 * 3600 * 1000))),
):
    check(MARKER not in S.block(c), f"no route-supplied string reaches the prompt ({label})")
mctx = gather(marked)
mtext = S.block(mctx)
check(any(l["rule"] == MARKER for l in mctx["levels"]),
      "the rule's name is still RECORDED for the operator, just never rendered")
check(S.UNKNOWN_STATE in mtext, "an unrecognised state renders in this block's own words")
check("liquidity pool" in mtext, "and an unrecognised kind falls back to the family's own word")

# `direction` is BULLISH/BEARISH on the wire and must not be either in the
# prompt. The engine's doc comment says the name "says which side of price the
# imbalance is on and NOT what price will do next"; the word in a prompt
# invites exactly the reading forty closed registrations already refuted.
check("bullish" not in text.lower() and "bearish" not in text.lower(),
      "no level is labelled bullish or bearish")
check("left by an up move" in text or "left by a down move" in text,
      "a gap says which way the move that left it went, which is the fact")
check("before an up move" in text or "before a down move" in text,
      "and a block says which way the move it preceded went")


# ---- ordering: nearest first, above and below separate ----

def dists(t, title):
    out, on = [], False
    for line in t.splitlines():
        if line.strip().startswith(title):
            on = True
            continue
        if on:
            if not line.startswith("    ") or line.strip().startswith("("):
                break
            piece = [p for p in line.split(",") if "away" in p]
            if piece:
                out.append(float(piece[0].strip().split()[0]))
    return out


above = dists(text, "ABOVE the last close")
below = dists(text, "BELOW the last close")
check(above == sorted(above), f"ABOVE is nearest first ({above})")
check(below == sorted(below, key=lambda d: -d), f"BELOW is nearest first ({below})")
check(all(d > 0 for d in above) and all(d < 0 for d in below),
      "and nothing is filed on the wrong side of the last close")
check(text.index("ABOVE the last close") < text.index("BELOW the last close"),
      "above and below are listed separately, above first")
check(ctx["above"][0]["dist_abs"] == min(l["dist_abs"] for l in ctx["above"]),
      "the nearest level above really is the nearest")
check(len(ctx["straddling"]) == 3 and "STRADDLING the last close" in text,
      "the three bands the live close sits inside are reported as straddling, not as a distance")

# The POC is the one level carrying BOTH a price and a band - the bucket it
# sits in - and the price is the level. The bucket is an artefact of the
# histogram's resolution.
poc = next(l for l in ctx["levels"] if l["word"] == "point of control")
check(poc["price"] is not None and poc["lo"] is not None
      and abs(poc["dist"] - (poc["price"] - CLOSE)) < 1e-9,
      "the POC carries both and is measured from its price, not from its bucket's edge")
# A band with no price is measured to its NEARER EDGE and not its midpoint.
band = next(l for l in ctx["levels"] if l["price"] is None and l["dist"] > 0)
check(abs(band["dist"] - (band["lo"] - CLOSE)) < 1e-9,
      "a band above the close is measured to its lower edge")


# ---- the cap, against a tape the route deliberately does not cap ----

check(S.MAX_PER_SIDE == 6, "the cap is six a side, as the block and the doc say")
check(len(ctx["above"]) == 124 and len(ctx["below"]) == 125,
      f"the live tape has {len(ctx['above'])} above and {len(ctx['below'])} below the close")
check(len(ctx["above_groups"]) == 120 and len(ctx["below_groups"]) == 122,
      "which sit at fewer distinct prices than that, once the duplicates are collapsed")
check(len(above) == S.MAX_PER_SIDE and len(below) == S.MAX_PER_SIDE,
      "and six a side are rendered")
check("(117 further levels, at 114 further prices, on this side are not shown" in text
      and "(119 further levels, at 116 further prices, on this side are not shown" in text,
      "the levels beyond the cap are COUNTED, and so are the prices, which now differ")
check(str(S.MAX_PER_SIDE) in flat and "a side" in flat,
      "the cap is stated in the block itself, as the registration pre-commits")
check("Nearest to the last close" in flat and "first, above and below listed separately" in flat,
      "and so is the ordering rule")

# THE CENSUS. The route ranks and caps nothing on purpose, so most of what it
# serves is spent: 135 of 155 pools already swept, 62 of 76 blocks broken. A
# model shown twelve levels with no idea they were twelve of 252, most of them
# spent, would read a tidy tape. A count is not a ranking.
check("252 levels" in flat, "the block says how many levels there were in all")
check("7 of them sit at a price another level already names" in flat,
      "and how many it folded into another line, the same honesty the cap's overflow owes")
check("155 liquidity pools (135 spent)" in flat, "and how many pools are already swept")
check("76 order blocks (62 spent)" in flat, "and how many blocks are already broken")
check("ranks none of them" in flat, "and that neither the route nor the block ranks them")

# A block whose length grew with the tape's mess would have a token cost that
# varied with the thing being measured.
# 36 lines on the busiest response this desk has captured: 13 of header and
# census, 9 a side, 5 for the three bands the close sits inside. Under
# htf_context's 43 and under the forty bars that follow it, which is the
# property the cap was chosen for. The ceiling is pinned so a later hand
# cannot double it silently.
lines = len(text.splitlines())
check(lines <= 38, f"the whole block is {lines} lines against the prompt's forty bars")
small = gather(dict(REAL, liquidity=REAL["liquidity"][:3], order_blocks=[],
                    fair_value_gaps=[]))
check(len(S.block(small).splitlines()) <= lines,
      "a quiet tape is not longer than a busy one, so 252 levels cost the same as 9")


# ---- one price, one line, every fact kept ----
#
# THE CASE THIS EXISTS FOR, from the live route on 2026-09-19: three of the
# six slots above the close were one number said three ways - an equal-highs
# pool, the PRIOR_DAY_HIGH pool and `extremes.day.high`, all at 4381.20, all
# +2.88 away, all 126 bars old - and the cap then hid 88 other levels behind
# them. The captured sample has the two-way version of the same thing at
# 4367.48; the third is added below so the three-way case is pinned rather
# than remembered.
#
# NOT the ranking the registration forbids, and the comment on
# `smc_context.COLLAPSE_TOL_PRICE` says so in those terms: ranking would be
# choosing which of two facts matters, and this keeps both and only stops
# repeating a price.
check(len([l for l in ctx["levels"] if l["price"] == 4367.48]) == 2,
      "the captured sample really does carry one price under two objects")
pair = next(g for g in ctx["above_groups"] if len(g) > 1)
check(len(pair) == 2 and {m["word"] for m in pair} == {
          "prior day high", "high of the last complete trading day"},
      "the prior day's high and the day extreme are one line, not two")
pline = next(l for l in text.splitlines() if "4367.48" in l)
check(pline.count("4367.48") == 1, f"the price is printed once: {pline.strip()[:70]}")
check("prior day high SWEPT and high of the last complete trading day COMPLETE" in pline,
      "both kinds and BOTH STATES are on it - they answer different questions")
check("buy-side liquidity" in pline and "swept 2026-09-17 12:15Z" in pline,
      "and every other fact the two carried survives: the side and the sweep with its stamp")
check(pline.count("72 bars old") == 1, "an age the two agree on is hoisted and said once")

three = copy.deepcopy(REAL)
three["liquidity"].append(dict(three["liquidity"][0], kind="EQUAL_HIGHS", side="BUY_SIDE",
                               price=4367.48, band_low=None, band_high=None,
                               state="SWEPT", swept=True, swept_at_bar_ms=1789501500000,
                               age_bars=444, swing_ids=["15m-hi-1", "15m-hi-2"],
                               spread_atr=0.0131))
tctx = gather(three)
ttext3 = S.block(tctx)
tgroup = next(g for g in tctx["above_groups"] if len(g) > 2)
check(len(tgroup) == 3, "the live three-at-one-price case collapses to one line")
tline = next(l for l in ttext3.splitlines() if "4367.48" in l)
check(tline.count("4367.48") == 1, f"one price: {tline.strip()[:70]}")
check("prior day high SWEPT" in tline and "high of the last complete trading day COMPLETE" in tline
      and "equal highs SWEPT" in tline, "all three kinds, each with its own state")
check("2 swings" in tline and "spread 0.01 ATR" in tline,
      "and the pool facts only the third one carried")
# Ages differ here - 72 and 444 bars - so they are NOT hoisted, because two
# levels at one price need not have formed on the same bar.
check("(72 bars old)" in tline and "(444 bars old)" in tline,
      "ages that differ stay attached to their own member rather than being averaged away")
check(len([l for l in ttext3.splitlines() if "4367.48" in l]) == 1,
      "three objects at one price occupy ONE of the six slots above the close")
check(tctx["census"]["collapsed"] == 8,
      f"and the census counts the extra fold ({tctx['census']['collapsed']})")

# THE TOLERANCE, which was measured wrong once and is pinned here. A
# hundredth of an ATR is 0.125 USD/oz at the sample's ATR of 12.46, and it
# folded the equal-highs pool at 4367.60 into the prior day's high at
# 4367.48 - two levels the route calls separate, twelve cents apart, under
# one price and one distance. The rule is the reader's instead: only levels
# this block WOULD HAVE PRINTED IDENTICALLY are folded.
check(S.COLLAPSE_TOL_PRICE == 0.005, "the tolerance is half of the last digit the block prints")
check(any(l["price"] == 4367.60 for l in ctx["levels"]),
      "the sample carries a level twelve cents from the collapsed pair")
check("4367.60" in text and "4367.48" in text,
      "and it keeps its own line, because a reader can tell twelve cents apart")
check(0.01 * ATR > 20 * S.COLLAPSE_TOL_PRICE,
      "a hundredth of an ATR would have been more than twenty times wider here")

# LIKE WITH LIKE. A band and a price are not one level because the price sits
# inside the band, and two bands agree only when BOTH edges do.
p_at = {"price": 4367.48, "lo": None, "hi": None}
b_over = {"price": None, "lo": 4360.0, "hi": 4370.0}
b_same = {"price": None, "lo": 4360.0, "hi": 4370.0}
b_wide = {"price": None, "lo": 4360.0, "hi": 4372.0}
check(not S._same_place(p_at, b_over, 0.005), "a price inside a band is not the same level")
check(S._same_place(b_over, b_same, 0.005), "two bands with both edges equal are")
check(not S._same_place(b_over, b_wide, 0.005), "and two sharing only one edge are not")

# A GROUP OF ONE MUST RENDER EXACTLY AS IT DID BEFORE COLLAPSING EXISTED, or
# the collapse is silently reformatting lines it did not merge.
solo = next(g for g in ctx["below_groups"] if len(g) == 1)
check(S._group_line(solo, "USD/oz", ATR) == S._line(solo[0], "USD/oz", ATR),
      "a level with nothing to collapse renders through the unchanged single-level path")

# Collapsing never reaches across the close: two levels a cent apart in
# DISTANCE on opposite sides are two prices two cents apart.
check(all(all(m["dist"] > 0 for m in g) for g in ctx["above_groups"])
      and all(all(m["dist"] < 0 for m in g) for g in ctx["below_groups"]),
      "no group mixes a level above the close with one below it")


# ---- a unit on every number ----

unit_lines = [l for l in text.splitlines()
              if l.startswith("    ") and not l.strip().startswith("(")]
check(len(unit_lines) == 15, f"there are level lines to check ({len(unit_lines)})")
for l in unit_lines:
    short = l.strip()[:58]
    check("USD/oz" in l, f"the level line carries its price unit: {short}")
    check(" ATR)" in l or "on it (0.00" in l, f"and its distance in ATR too: {short}")
    check("bars old" in l or "age not reported" in l, f"and its age in bars: {short}")
check(" ATR body" in text, "a displacement is in ATR, with its denominator published by the route")
check("spread 0.01 ATR" in text, "a pool's spread is in ATR")
check("% filled" in text, "a gap's fill is a percentage of the gap")
check(S._unit("xauusd") == H._unit("xauusd") and S._unit("eurusd") == H._unit("eurusd")
      and S._unit("btcusd") == H._unit("btcusd"),
      "this block names the market's units exactly as htf_context does")
check("Ages are in 15m bars" in flat,
      "the timeframe is named ONCE, because the route serves one and no level carries its own")

# The route's own definitions of session and day are not the ones a reader
# assumes - session is the run IN PROGRESS, day the last COMPLETE one, both
# split by the measured hole and not by a clock. A bare "day high" reads as
# today's.
check("high of the trading day in progress" in " ".join(l["word"] for l in ctx["levels"]),
      "the session extreme says it is the day in progress")
check("high of the last complete trading day" in text,
      "and the day extreme says it is the last complete one")

# AGE AND STATE ARE THE CONTENT. On the real tape the nearest levels are
# mostly spent, and a line showing only the price would hide that.
check("SWEPT" in text and "BROKEN" in text and "PARTLY FILLED" in text,
      "the states the live tape actually carries all render")
check("swept 2026-" in text, "a swept pool says when, as a stamp and not as a bogus bar count")


# ---- caller-passed close and ATR win; the route's are the fallback ----

check(ctx["last_close"] == CLOSE and abs(ctx["atr"] - ATR) < 1e-9,
      "the caller's close and ATR are what the block measures against")
off = gather(last_close=4400.0, atr=20.0)
check(off["last_close"] == 4400.0 and off["atr"] == 20.0,
      "a caller passing its own numbers overrides the response's")
fb = gather(last_close=None, atr=None)
check(fb["last_close"] == CLOSE and abs(fb["atr"] - ATR) < 1e-9,
      "and the response's last_close and atr14 are the fallback when it passes none")
noatr = S.block(gather(dict(REAL, atr14=None), atr=None))
check("no ATR" in noatr and "USD/oz away" in noatr,
      "with no ATR anywhere the block says so and keeps the price distances")


# ---- no verdict ----

low = " ".join(l.lower() for l in unit_lines)
for word in ("score", "confluence", "you should", "recommend", "strong", "weak",
             "important", "key level", "support", "resistance"):
    check(word not in low, f"no level line passes a verdict ({word!r})")
head = flat.lower()
check("no score here" in head and "no ranking" in head and "no confluence count" in head,
      "the header disclaims all three by name")
check("not a signal" in head, "and says out loud that it is not one")


# ---- the variant: base plus the block, and nothing else ----

FIELDS = dict(market="xauusd", tf="15m", n=40, bars="<BARS>",
              position="<POS>", desk="<DESK>", context="<CTX>")


def render(variant: str, otl_block: str = "", htf_block: str = "",
           htf_rule_block: str = "", plan_block: str = "", smc_block: str = "") -> str:
    return A.PROMPT.format(coin_clause=A.COIN_CLAUSE[A.VARIANTS[variant]["coin"]],
                           otl_block=otl_block, htf_block=htf_block,
                           htf_rule_block=htf_rule_block, plan_block=plan_block,
                           smc_block=smc_block, **FIELDS)


base = render("base")
SMARK = "\n\nSMC-LINE-ONE\nSMC-LINE-TWO"
smc = render("smc-context", smc_block=SMARK)
V = A.VARIANTS["smc-context"]
check(V["coin"] == "base",
      "smc-context runs the BASE coin clause, so it changes one thing and not two")
check(not V["otl"] and not V["htf"] and not V["htf_rule"] and not V["plan"] and not V["trigger"],
      "and carries no other variant's block")
check(V["smc"] and not any(A.VARIANTS[v]["smc"] for v in A.VARIANTS if v != "smc-context"),
      "only smc-context fetches the levels route")
check(smc.replace(SMARK, "") == base,
      "smc-context IS base plus the block - remove the block and base returns exactly")
check(render("smc-context", smc_block="") == base,
      "with an empty block smc-context renders as base, so a missing route cannot reshape the prompt")
check(len(smc) > len(base), "the variant is longer, so the change was an addition")

# The block belongs in the SLOW part of the prompt, beside the htf and otl
# blocks and before the market context. DeepSeek serves an EXACT prefix of a
# previous request at one fiftieth of the fresh price, so what changes hourly
# at most must sit ahead of what changes every bar.
OMARK, HMARK = "\n\nOTL-LINE", "\n\nHTF-LINE"
both = render("smc-context", otl_block=OMARK, htf_block=HMARK, smc_block=SMARK)
check(both.index("SMC-LINE-ONE") < both.index("MARKET CONTEXT"),
      "the levels block sits before the market context, inside the cacheable prefix")
check(both.index("HTF-LINE") < both.index("SMC-LINE-ONE")
      and both.index("OTL-LINE") < both.index("SMC-LINE-ONE"),
      "and beside the htf and otl blocks, not among the per-bar material")
check(both.index("SMC-LINE-ONE") < both.index("LAST 40 BARS"),
      "well ahead of the forty bars, which are the per-bar material")

launcher = io.open(os.path.join(HERE, "start_ai_traders.ps1"), encoding="utf-8").read()
row = [l for l in launcher.splitlines() if "'ai-xau-ds-smc'" in l]
check(len(row) == 1, "the launcher carries exactly one row for this book")
check("'ai-xau-ds-smc-coin'" in row[0] and "seed = 53" in row[0]
      and "'ai_trader_ds_smc'" in row[0] and "'smc-context'" in row[0],
      "naming its coin, seed 53, log ai_trader_ds_smc and prompt variant smc-context")
# COMMENTED, and this is an assertion rather than an observation. The books
# and the trader exist only when the owner decides to spend on them; a
# launcher row that starts a campaign nobody chose is how a desk acquires a
# cost it cannot explain. Uncommenting it is a deliberate act and this test
# is what makes it a visible one.
check(row[0].strip().startswith("#"),
      "and the row is COMMENTED, so nothing starts until someone decides it should")

print()
print(f"{'all checks passed' if not fails else str(len(fails)) + ' FAILED'}")
sys.exit(1 if fails else 0)
