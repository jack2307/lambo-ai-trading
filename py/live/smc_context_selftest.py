# -*- coding: utf-8 -*-
"""The structural-levels block says what the registration says it says.

    py -3.9 py/live/smc_context_selftest.py

WHY THIS EXISTS. `ai-xau-ds-smc` is an experiment whose entire content is one
added block, and the block was written against a CONTRACT before the route
serving it existed. Two things can therefore go wrong without anything
failing loudly: the block can quietly stop being the only difference between
this book and its control, and the route can hand over a shape this file
reads differently than intended. The first is checked the way
`prompt_variant_selftest.py` checks the other four variants - strip the block
and base must return, byte for byte. The second is checked against a mocked
`urlopen`, because the route is not on `main` yet and a test that needed it
would not run at all.

The pre-commitments this file exists to hold, from
docs/hypotheses/2026-09-18-smc-context.md:

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

import io
import json
import os
import sys
import urllib.error
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ai_trader as A  # noqa: E402
import htf_context as H  # noqa: E402
import smc_context as S  # noqa: E402

fails = []


def check(ok: bool, what: str) -> None:
    print(("ok   " if ok else "FAIL ") + what)
    if not ok:
        fails.append(what)


# ---- a mocked route, because the real one is not on `main` yet ----

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
    """Make the next `gather` see `payload`, or raise it if it is an exception."""
    def _open(url, timeout=None):
        _SEEN_URLS.append(url)
        if isinstance(payload, Exception):
            raise payload
        return _Resp(payload)
    urllib.request.urlopen = _open


def unserve():
    urllib.request.urlopen = _REAL_URLOPEN


M15 = 900_000
H4MS = 4 * 3_600_000
D1MS = 86_400_000
BAR = 1_758_232_800_000          # the bar being decided, a 15m bar start
NEWEST = BAR - M15               # the newest CLOSED 15m bar behind the levels
CLOSE = 4710.50
ATR = 18.40

PROV = {
    "15m": {"bar_ms": M15, "computed_at_bar_ms": NEWEST, "computed_at_ms": BAR, "bars": 600},
    "4h": {"bar_ms": H4MS, "computed_at_bar_ms": BAR - H4MS, "computed_at_ms": BAR, "bars": 500},
    "1d": {"bar_ms": D1MS, "computed_at_bar_ms": BAR - D1MS, "computed_at_ms": BAR, "bars": 400},
}


def lv(**kw):
    d = {"tf": "15m", "rule": "RULE-NAME-FROM-ANOTHER-CRATE"}
    d.update(kw)
    return d


# A realistic full book: eight levels above, eight below, one the close sits
# inside. The two order blocks below are the registration's own example - an
# UNTESTED block four bars old and an UNTESTED block two hundred and twelve
# bars old are different facts, and the block has to say which is which.
DOC = {
    "market": "xauusd",
    "unavailable": None,
    "last_close": CLOSE,
    "provenance": PROV,
    "levels": {
        "profile": [
            lv(kind="poc", price=4702.80, state="intact", age_bars=40),
            lv(kind="vah", price=4728.40, state="intact", age_bars=40),
            lv(kind="val", price=4688.10, state="intact", age_bars=40),
        ],
        "fvg": [
            lv(kind="fvg", band=[4713.20, 4717.60], state="unfilled",
               filled_frac=0.25, age_bars=12),
            lv(kind="fvg", band=[4694.00, 4696.50], state="unfilled",
               filled_frac=0.0, age_bars=3, tf="4h"),
        ],
        "order_blocks": [
            lv(kind="ob", band=[4719.00, 4722.50], state="untested", age_bars=4),
            lv(kind="ob", band=[4684.00, 4687.00], state="untested", age_bars=212),
            lv(kind="ob", band=[4744.00, 4748.00], state="tested", age_bars=88),
            lv(kind="ob", band=[4708.00, 4712.00], state="untested", age_bars=7),
        ],
        "liquidity_buy": [
            lv(kind="bsl", price=4735.00, state="unswept", swept=False, age_bars=31,
               ref="h4-hi-%d" % (BAR - 8 * H4MS)),
        ],
        "liquidity_sell": [
            lv(kind="ssl", price=4681.20, state="swept", swept=True,
               swept_by_bar_ms=BAR - 6 * M15, age_bars=58,
               ref="h4-lo-%d" % (BAR - 20 * H4MS)),
        ],
        "extremes": [
            lv(kind="session_high", price=4726.10, age_bars=18),
            lv(kind="session_low", price=4699.30, age_bars=22),
            lv(kind="day_high", price=4740.00, age_bars=1, tf="1d"),
            lv(kind="day_low", price=4672.00, age_bars=1, tf="1d"),
            lv(kind="week_high", price=4765.00, age_bars=7, tf="1d"),
            lv(kind="week_low", price=4640.00, age_bars=7, tf="1d"),
        ],
    },
}


def gather(doc=None, **over):
    serve(DOC if doc is None else doc)
    try:
        kw = dict(api="http://mock", market="xauusd", bar_time=BAR,
                  last_close=CLOSE, atr=ATR)
        kw.update(over)
        return S.gather(**kw)
    finally:
        unserve()


# ---- the four states ----

ctx = gather()
check(ctx["state"] == "ok", "a full response is ok")
text = S.block(ctx)
print()
print(text)
print()

check(_SEEN_URLS and _SEEN_URLS[-1].endswith("/api/paper/levels?market=xauusd"),
      "the route asked for is /api/paper/levels?market=<market>")

# An unreachable route must produce a block that SAYS SO, not an empty one.
# A silent fallback would put context-absent decisions into a context-present
# book, and the disagreement analysis would be reading a mixture.
una = gather(urllib.error.URLError("connection refused"))
check(una["state"] == "unavailable", "an unreachable route is state unavailable")
utext = S.block(una)
check("UNAVAILABLE" in utext and "deciding without them" in utext,
      "and the block says so rather than rendering empty")
check(len(utext.strip()) > 0 and len(utext.splitlines()) >= 3,
      "the unavailable block is a real block, not a blank line")

# Stale is judged against the DECISION BAR, never the wall clock: a wall-clock
# threshold cannot tell a stopped export from a weekend. Six 15m bars behind
# is a stopped export.
stale_prov = dict(PROV, **{"15m": dict(PROV["15m"], computed_at_bar_ms=NEWEST - 6 * M15)})
st = gather(dict(DOC, provenance=stale_prov))
check(st["state"] == "stale", "levels six 15m bars behind the decision bar are stale")
check(st["behind_bars"] is not None and abs(st["behind_bars"] - 6.0) < 1e-9,
      f"and the gap is measured in bars ({st['behind_bars']})")
stext = S.block(st)
check("STALE" in stext and "6.0" in stext and "15m bars behind" in stext,
      "the stale block says how far behind, in bars")
# Freshness must NOT be implied when it was never checked.
noc = gather(bar_time=None)
check(noc["state"] == "ok" and not noc["staleness_checked"],
      "without a decision bar the state is not stale")
check("NOT checked this run" in S.block(noc),
      "and the block says the lag was not checked rather than implying freshness")

# Thin: the route answered for this market and has no levels yet. A different
# condition from having no bars, and reported differently on purpose.
thin = gather(dict(DOC, levels={}))
check(thin["state"] == "thin", "a route with no levels yet is thin, not unavailable")
ttext = S.block(thin)
check("THIN" in ttext and "UNAVAILABLE" not in ttext,
      "and the block says THIN rather than claiming the route is down")

# No bars for the market at all: the route's `unavailable` string with nothing
# behind it.
none = gather({"market": "xauusd", "unavailable": "no 15m bars for xauusd", "levels": {}})
check(none["state"] == "unavailable", "a market with no bars is unavailable")

# ---- no route-supplied string reaches the prompt, in ANY state ----
#
# The lesson of 9a5dbfb, pre-committed in the registration rather than
# discovered here. The route carries the NAME OF THE RULE that produced each
# level, and that name is a string another crate owns: printing it would let
# that crate edit a registered campaign's prompt with no diff to notice.
MARKER = "ROUTE-PROSE-THAT-MUST-NOT-REACH-THE-PROMPT"
marked = {
    "market": "xauusd", "unavailable": MARKER, "last_close": CLOSE,
    "provenance": PROV,
    "levels": {
        "order_blocks": [lv(kind=MARKER, band=[4719.0, 4722.5], state=MARKER,
                            age_bars=4, rule=MARKER, tf=MARKER)],
        "liquidity_buy": [lv(kind="bsl", price=4735.0, state="unswept", swept=False,
                             age_bars=31, ref=MARKER, rule=MARKER)],
    },
}
for label, c in (
    ("ok", gather(marked)),
    ("unavailable", gather(urllib.error.URLError(MARKER))),
    ("thin", gather({"market": "xauusd", "last_close": CLOSE, "provenance": PROV,
                     "levels": {}, "unavailable": MARKER})),
    ("stale", gather(dict(marked, provenance=stale_prov))),
):
    check(MARKER not in S.block(c),
          f"no route-supplied string reaches the prompt ({label})")
check(gather(marked)["levels"][0]["rule"] == MARKER,
      "the rule's name is still RECORDED for the operator, just never rendered")
mtext = S.block(gather(marked))
check(S.UNKNOWN_STATE in mtext,
      "an unrecognised state renders in this block's own words")
check("order block" in mtext,
      "and an unrecognised kind falls back to the group's word, which this file chose")
# A swing id is the one route string allowed to shape the text, and only
# through a strict pattern whose parts are re-rendered in our words.
check("H4 swing high" in text, "a parsed swing id renders as our own words")
check("h4-hi-" not in text, "and the id itself never appears")

# ---- ordering: nearest first, above and below separate ----

def dists(t, title):
    """The `+x.xx`/`-x.xx` distance off each line of one section, in order."""
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
check(ctx["above"][0]["dist"] == min(l["dist"] for l in ctx["above"]),
      "the nearest level above really is the nearest")
check(text.index("ABOVE the last close") < text.index("BELOW the last close"),
      "above and below are listed separately, above first")
check("STRADDLING the last close" in text and "4708.00 to 4712.00" in text,
      "a band the last close sits inside is reported as straddling, not as a distance")
# The distance to a band is to its NEARER EDGE and not its midpoint: a
# midpoint is a number the route never said and the price never has to reach.
line = next(l for l in text.splitlines() if "4713.20 to 4717.60" in l)
check("+2.70" in line, f"the distance to a band is to its nearer edge ({line.strip()})")

# ---- the cap ----

check(S.MAX_PER_SIDE == 6, "the cap is six a side, as the block and the doc say")
n_above = len([l for l in ctx["above"]])
check(n_above > S.MAX_PER_SIDE, f"the fixture has more levels above than the cap ({n_above})")
check(len(above) == S.MAX_PER_SIDE, f"and only {S.MAX_PER_SIDE} are rendered ({len(above)})")
check(f"({n_above - S.MAX_PER_SIDE} further levels" in text,
      "the levels beyond the cap are COUNTED, not dropped silently")
check(str(S.MAX_PER_SIDE) in text and "per side" in text,
      "the cap is stated in the block itself, as the registration pre-commits")
check("NEAREST FIRST" in text, "and so is the ordering rule")

# A block whose length grew with the tape's mess would have a token cost that
# varied with the thing being measured. Twenty times the levels must not make
# a longer block than the fixture's.
flood = {"market": "xauusd", "last_close": CLOSE, "provenance": PROV, "levels": {
    "order_blocks": [lv(kind="ob", price=CLOSE + 1.0 * i, state="untested", age_bars=i)
                     for i in range(1, 61)]
             + [lv(kind="ob", price=CLOSE - 1.0 * i, state="untested", age_bars=i)
                for i in range(1, 61)]}}
fctx = gather(flood)
ftext = S.block(fctx)
check(len(fctx["levels"]) == 120, "the fixture floods the route with 120 levels")
check(len(ftext.splitlines()) <= len(text.splitlines()),
      f"and the block does not grow with them ({len(ftext.splitlines())} lines "
      f"vs {len(text.splitlines())})")
# The measured length the cap was chosen against, pinned so a later hand
# cannot double it without this saying so. `htf_context` renders 43 lines on
# its own fixture and `otl_context` 17; the prompt's own forty bars follow.
check(len(text.splitlines()) <= 36,
      f"the full block is {len(text.splitlines())} lines, under the htf block's 43")

# ---- a unit on every number ----
#
# The desk spent 2026-09-17 removing five numbers whose units lived only in
# prose. A model reading "order block: 4719" cannot know whether that is a
# price, an index or a count.
unit_lines = [l for l in text.splitlines()
              if l.startswith("    ") and not l.strip().startswith("(")]
check(len(unit_lines) >= 12, f"there are level lines to check ({len(unit_lines)})")
for l in unit_lines:
    check("USD/oz" in l, f"the level line carries its price unit: {l.strip()[:60]}")
    check(" ATR)" in l or "on it (0.00" in l,
          f"and its distance in ATR as well as price: {l.strip()[:60]}")
    check("bars old" in l or "age not reported" in l,
          f"and its age in bars: {l.strip()[:60]}")
check(S._unit("xauusd") == H._unit("xauusd") and S._unit("eurusd") == H._unit("eurusd")
      and S._unit("btcusd") == H._unit("btcusd"),
      "this block names the market's units exactly as htf_context does")

# AGE AND STATE ARE THE CONTENT. The registration's own example: two UNTESTED
# order blocks, four bars old and two hundred and twelve, are different facts.
young = next(l for l in text.splitlines() if "4719.00 to 4722.50" in l)
old = next(l for l in text.splitlines() if "4684.00 to 4687.00" in l)
check("UNTESTED" in young and "4 bars old" in young, f"the young block says both: {young.strip()[:70]}")
check("UNTESTED" in old and "212 bars old" in old, f"and the old one says both: {old.strip()[:70]}")
check(young != old, "so two levels in the same state read as different facts")
swept = next(l for l in text.splitlines() if "4681.20" in l)
check("SWEPT on the bar opening" in swept, f"swept liquidity says when: {swept.strip()[:70]}")
check("25% filled" in text, "an unfilled gap says what fraction of it has gone")

# Without an ATR the distances are still printed, and the block says the scale
# is missing rather than dropping half of each number silently.
noatr = S.block(gather(atr=None))
check("no ATR" in noatr and "USD/oz away" in noatr,
      "with no ATR the block says so and keeps the price distances")

# ---- no verdict ----
#
# The pre-commitment: if the route ever grows a score this book must not read
# it, and the block must not invent one either.
low = " ".join(l.lower() for l in unit_lines)
for word in ("score", "confluence", "bullish", "bearish", "you should", "recommend",
             "strong", "weak", "important", "key level"):
    check(word not in low, f"no level line passes a verdict ({word!r})")
head = text.lower()
check("no score here" in head and "no ranking" in head and "no confluence count" in head,
      "and the header disclaims all three by name")
check("not a signal" in head, "and says out loud that it is not one")
check("you should" not in head and "recommend" not in head, "the block advises nothing")

# ---- the variant: base plus the block, and nothing else ----
#
# The same idiom `prompt_variant_selftest.py` uses for the other four
# additions. If a later edit moves a word while adding the block, the book
# measures the block and the word together and nothing says so.
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
check(V["coin"] == "base", "smc-context runs the BASE coin clause, so it changes one thing and not two")
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
# previous request at one fiftieth of the fresh price, so the text that
# changes hourly at most has to sit ahead of the text that changes every bar;
# a levels block after the bars would be re-read at full price every call.
OMARK, HMARK = "\n\nOTL-LINE", "\n\nHTF-LINE"
both = render("smc-context", otl_block=OMARK, htf_block=HMARK, smc_block=SMARK)
check(both.index("SMC-LINE-ONE") < both.index("MARKET CONTEXT"),
      "the levels block sits before the market context, inside the cacheable prefix")
check(both.index("HTF-LINE") < both.index("SMC-LINE-ONE")
      and both.index("OTL-LINE") < both.index("SMC-LINE-ONE"),
      "and beside the htf and otl blocks, not among the per-bar material")
check(both.index("SMC-LINE-ONE") < both.index("LAST 40 BARS"),
      "well ahead of the forty bars, which are the per-bar material")

# The launcher and the book, so the three-file rule has something to catch.
# `campaign_wiring_selftest.py` checks they AGREE; this checks the names the
# registration's amendment records.
launcher = io.open(os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                "start_ai_traders.ps1"), encoding="utf-8").read()
check("'ai-xau-ds-smc'" in launcher and "'ai-xau-ds-smc-coin'" in launcher,
      "the launcher names the book and its coin")
check("seed = 53" in launcher, "with seed 53, which no other campaign uses")
check("'ai_trader_ds_smc'" in launcher, "and log ai_trader_ds_smc")
check("'smc-context'" in launcher, "and prompt variant smc-context")

print()
print(f"{'all checks passed' if not fails else str(len(fails)) + ' FAILED'}")
sys.exit(1 if fails else 0)
