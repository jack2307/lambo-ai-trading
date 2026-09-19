# -*- coding: utf-8 -*-
"""Structural price levels from the desk's own API, as CONTEXT for a prompt.

    py -3.9 py/live/smc_context.py                  # fetch once and print the block
    py -3.9 py/live/smc_context.py --market xauusd --api http://127.0.0.1:8138
    py -3.9 py/live/smc_context.py --sample docs/api-samples/paper-levels.json

One job: ask `GET /api/paper/levels` for the levels it computes from CLOSED
bars and render them as a block a model can read. It decides nothing and it
computes nothing except the distance from the last close, which is the only
arithmetic the ordering needs. Everything else comes off the wire as the route
said it, for the reason `htf_context` gives: the model and the Desk panel read
ONE implementation of these levels, and two that each computed their own would
eventually disagree in front of a person.

WHAT THIS IS AND IS NOT. Every mechanical use of levels of this family that
this desk has tested has FAILED out of sample - the full ICT chain
(2026-09-13-ict-sweep-mss-fvg, PF 0.75-0.77 over 613-1,428 trades), yesterday's
high and low (2026-09-13-pdhl, 1st-6th percentile of its own null), and the
twenty-five options-tape registrations behind `otl_context`. So this block
carries FACTS - a price or a band, an age, a state - and NO verdict: no score,
no ranking, no confluence count, no zone labelled with a word. What it asks is
the different question registered at docs/hypotheses/2026-09-18-smc-context.md:
whether a model that can SEE these levels decides differently. That can be true
whether or not they predict anything.

THE ROUTE IS SINGLE-TIMEFRAME AND THE BLOCK SAYS SO ONCE. `timeframe` and
`bar_ms` are on the response and no level carries its own, so every `age_bars`
on it is bars of that one timeframe. The block names it in the header and then
prints bare bar counts. This file used to name a timeframe per line, which was
right while the contract was expected to be multi-timeframe and became noise
the moment the real one landed (2026-09-19); the per-line naming is gone and
the header carries the timeframe once.

AGE AND STATE ARE THE CONTENT, NOT DECORATION. An UNTESTED order block four
bars old and an UNTESTED order block two hundred bars old are different facts,
and a block that printed only the price would be saying they are the same one.
Both are on every line. The live sample makes the point at scale: of 155
liquidity pools 135 are already SWEPT and of 76 order blocks 62 are BROKEN, so
a reader given prices alone would be looking at a tape of 252 live levels when
in fact most of them are spent.

STALENESS IS MEASURED AGAINST THE BAR BEING DECIDED, NOT AGAINST NOW. Same
reason as `htf_context`: a threshold against the current time cannot tell a
stopped export from a weekend, and on Monday morning the newest closed bar is
correctly Friday's. The caller passes the timestamp of the bar it is deciding
on and staleness is the gap between that and `computed_at_bar_ms` - start to
start, since both are bars of the same series, and reported in minutes of
clock rather than in bars. See `STALE_AFTER_BARS` for the two measurements
that forced both of those choices.

THE ROUTE'S OWN TEXT NEVER REACHES THE PROMPT, IN ANY STATE. This is the
lesson of `9a5dbfb`, pre-committed in the registration rather than discovered
here, and the route's own doc comment on `unavailable` now says the same thing
from the other side: `htf_context` printed a route's `unavailable` sentence
verbatim, and that sentence is a join written in another crate, so adding a
timeframe to the route would have changed the wording inside a REGISTERED
campaign's prompt with nobody editing the book and no diff to notice.

It costs more here than it did there, and the cost is deliberate:

* the `rule` string - "order block: last down candle before a body > 1 x
  ATR(14) at its own bar" and the like - is exactly the field the registration
  asked the route for, and it is another crate's prose. RECORDED in `gather`
  for the operator, never rendered.
* `kind`, `state`, `side` and `direction` go through this file's own
  vocabularies below. Anything outside them renders in this file's words
  ("not in this block's vocabulary"), never as the wire's token.
* `swing_ids` are not printed at all. They exist to join a pool to the swing
  `/api/paper/htf` reports, and on this response they are all this response's
  own 15m swings - nothing the model can see in this prompt joins to them, the
  forming bar already dates the pool, and each is another crate's string. What
  IS printed is how many swings a pool is made of and how far apart they sit
  in ATR, which are numbers.

`direction` is rendered as what it is - which way the move that left the level
went - and never as BULLISH or BEARISH. The engine's own doc comment says the
name "says which side of price the imbalance is on and NOT what price will do
next"; printing the word in a prompt would invite exactly that reading, which
is the refuted claim wearing the route's label.

FAILING IS A RESULT, NOT AN EXCEPTION. Unreachable, thin or stale, the block
says which and the caller puts that in the prompt and in the record. A silent
fallback to "no levels" would put context-absent decisions into a
context-present book, and the experiment would be measuring a mixture with
nothing to separate it. This is the rule `otl_context` and `htf_context` run
under and it is written here for the same reason.

The route's own distinction between `null` and `[]` is kept and mapped onto
the four registered states: `null` blocks mean there were no bars to look at
and are UNAVAILABLE, empty lists mean the window produced nothing and are
THIN. Its module doc insists the two must render differently, and they do -
what this file adds is that neither is silently an `ok` with an empty list
under it.
"""
from __future__ import annotations

import argparse
import datetime as dt
import io
import json
import sys
import textwrap
import urllib.error
import urllib.parse
import urllib.request

for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")
    except (AttributeError, ValueError):  # not a TextIOWrapper; nothing to do
        pass

REQUEST_TIMEOUT_S = 6.0

# HOW STALE THESE LEVELS MAY BE, AND WHY THE TOLERANCE IS IN MILLISECONDS
# WHILE THE THING IT TOLERATES IS COUNTED IN BARS.
#
# Two facts, both measured against the real response of 2026-09-19 rather
# than reasoned out, and both of which broke the first version of this.
#
# ONE: this route runs on the DECISION timeframe. `htf_context` compares an
# H4 bar to a 15m decision bar, where the newest closed H4 bar is always at
# least one whole H4 bar behind, so it measures from that bar's close. Here
# `computed_at_bar_ms` and the caller's decision bar are the same series and
# on a healthy desk they are the SAME BAR. Measuring from the close reported
# a perfectly current response as minus one bar behind, which is nonsense
# wearing a unit. The gap is start to start.
#
# TWO: a gap in milliseconds DIVIDED BY `bar_ms` IS NOT A BAR COUNT, because
# the market is shut about a third of the time. Measured on the sample: the
# analysis window holds 879 stored bars across a span of 1,308 bar-lengths of
# clock, a factor of 1.49, and the route's own `age_bars` counts the 879. So
# the tolerance is expressed in clock time, which is what this can actually
# measure, and never dressed up as bars.
#
# The number: the daily hole plus two bars. `htf.rs` measured hour 21Z
# holding exactly ZERO 15m bars against 332-348 in every neighbouring hour
# (2026-09-18), so an export exactly one stored bar behind at the daily roll
# is over an hour of clock behind while being one bar behind, and a two-bar
# tolerance in clock alone would call it stopped once a night. Two bars past
# the hole is 90 minutes on 15m: an export slightly behind stays inside it,
# one that has stopped does not.
DAILY_HOLE_MS = 60 * 60 * 1000
STALE_AFTER_BARS = 2.0

# The states a caller must be able to tell apart, worst first. `thin` is the
# one that would otherwise hide: the route answered for this market and there
# is nothing to place, because a warmup is not met. That is a different
# condition from having no bars at all, and the route separates them on
# purpose - `null` blocks with a sentence versus empty lists.
STATES = ("unavailable", "stale", "thin", "ok")

# HOW MANY LEVELS THE BLOCK SHOWS, AND WHY THE NUMBER IS FIXED.
#
# Six per side. The registration pre-commits "a fixed maximum count, stated in
# the block itself", because a prompt whose length depends on how many order
# blocks happen to be unfilled is a prompt whose token cost and whose
# legibility vary with the thing being measured. The route makes that concrete
# rather than hypothetical: it deliberately caps and ranks nothing, and the
# live sample of 2026-09-19 carries 252 levels - 155 liquidity pools, 76 order
# blocks, 12 gaps, 3 profile levels, 6 period extremes. Uncapped, this block
# would be 260 lines in front of forty bars.
#
# Six because of what the block sits in front of. The prompt ends with the
# forty 15m bars the model decides on (`--bars` default 40), and the two
# context blocks already beside this one measure 43 lines (`htf_context` on
# its fixture) and 17 (`otl_context` on its own). At six a side this one
# measures 35 lines on the real sample - 12 of header and census, 9 a side,
# and 5 for the three bands the live close happens to sit inside - measured
# by `smc_context_selftest.py` against `docs/api-samples/paper-levels.json`,
# which pins the ceiling. At twelve a side the same response renders 47 lines
# - measured by raising the constant, not estimated - which is longer than
# the htf block and longer than the forty bars it sits in front of.
#
# Levels beyond the cap are COUNTED and never dropped silently, and the header
# carries a census of the whole response for the same reason: 252 levels of
# which 135 are swept pools and 62 broken blocks is the single most useful
# fact about how messy this tape is, and a model shown twelve of them without
# it would think the tape is tidy.
MAX_PER_SIDE = 6

# A level's `kind`, in this file's vocabulary, keyed on the route's
# SCREAMING_SNAKE token lowercased. A kind outside this table renders by its
# FAMILY's word - which this file also chose - and never by the wire's string.
#
# The period-extreme wording carries the route's own definitions, because they
# are not the ones a reader would assume: "session" is the trading-day run IN
# PROGRESS and "day" the last COMPLETE one, both split by the measured 45
# minute hole between runs rather than by a clock. A line reading "day high"
# with no more would be read as today's.
KIND_WORDS = {
    "poc": "point of control",
    "vah": "value area high",
    "val": "value area low",
    "fair_value_gap": "fair value gap",
    "order_block": "order block",
    "equal_highs": "equal highs",
    "equal_lows": "equal lows",
    "prior_day_high": "prior day high",
    "prior_day_low": "prior day low",
    "prior_week_high": "prior week high",
    "prior_week_low": "prior week low",
    "session_high": "high of the trading day in progress",
    "session_low": "low of the trading day in progress",
    "day_high": "high of the last complete trading day",
    "day_low": "low of the last complete trading day",
    "week_high": "high of the last complete week",
    "week_low": "low of the last complete week",
}

# A level's `state`. The route's closed set, documented on `LevelState`:
# profile levels are CURRENT, gaps UNFILLED or PARTIALLY_FILLED, blocks
# UNTESTED / TESTED / BROKEN, pools RESTING or SWEPT, extremes FORMING or
# COMPLETE.
STATE_WORDS = {
    "current": "CURRENT",
    "forming": "STILL FORMING",
    "complete": "COMPLETE",
    "unfilled": "UNFILLED",
    "partially_filled": "PARTLY FILLED",
    "untested": "UNTESTED",
    "tested": "TESTED",
    "broken": "BROKEN",
    "resting": "RESTING",
    "swept": "SWEPT",
}
UNKNOWN_STATE = "state not in this block's vocabulary"

# Which side of the book a pool sits on, in words rather than the wire's token.
SIDE_WORDS = {"buy_side": "buy-side liquidity", "sell_side": "sell-side liquidity"}

# Which way the move that left a gap or a block went. NOT "bullish" and NOT
# "bearish": see the module docstring. This says what happened, which is a
# fact, rather than naming a bias, which would be the refuted claim.
DIRECTION_WORDS = {"bullish": "left by an up move", "bearish": "left by a down move"}
OB_DIRECTION_WORDS = {"bullish": "before an up move", "bearish": "before a down move"}


def _unit(market: str) -> str:
    """The market's quote units, spelled out for the prompt.

    The same rule and the same table as `htf_context._unit`, and duplicated
    rather than imported so each context block stands on its own import graph.
    The two must agree, and `smc_context_selftest.py` checks that they do -
    a units table that drifted between two blocks in one prompt would have the
    model reading two names for one thing.
    """
    m = (market or "").lower()
    if m.startswith(("xau", "xag")):
        return "USD/oz"
    if m.startswith(("eur", "gbp", "aud", "nzd", "usd")):
        return "USD"
    return "quote units"


def _u(ms) -> str:
    if ms is None:
        return "unknown"
    return dt.datetime.utcfromtimestamp(ms / 1000).strftime("%Y-%m-%d %H:%MZ")


def _f(v):
    """A float, or None for anything that is not one.

    `bool` is refused explicitly: `True` is a float in Python and a price of
    1.0 arriving from a mis-typed field would render as a level.
    """
    if v is None or isinstance(v, bool):
        return None
    try:
        return float(v)
    except (TypeError, ValueError):
        return None


def _norm(s) -> str:
    """A wire token reduced to the form the vocabularies are keyed on."""
    return str(s or "").strip().lower().replace("-", "_").replace(" ", "_")


def _level(raw, family: str, family_word: str, close: float):
    """One wire level, in this file's own words and measured from the close.

    Returns None for anything carrying neither a price nor a two-price band:
    a level with no location cannot be ordered by distance and has nothing to
    say on a line about distance.
    """
    if not isinstance(raw, dict):
        return None
    price = _f(raw.get("price"))
    lo, hi = _f(raw.get("band_low")), _f(raw.get("band_high"))
    if lo is not None and hi is not None:
        lo, hi = min(lo, hi), max(lo, hi)
    else:
        # The route sends both or neither. One alone is not a band and is not
        # guessed at from the other.
        lo = hi = None
    if lo is None and price is None:
        return None

    # Distance is to the NEARER EDGE of a band, and zero when the last close
    # is inside it. A band's midpoint would be a number the route never said
    # and the price never has to reach.
    #
    # The POC is the one level that carries BOTH a price and a band - the
    # bucket it sits in - and the price is the one to measure from, because
    # the bucket is an artefact of the histogram's resolution and the price is
    # the level. So a price wins wherever there is one.
    if price is not None:
        dist = price - close
    else:
        dist = (lo - close) if close < lo else ((hi - close) if close > hi else 0.0)

    kind = _norm(raw.get("kind"))
    word = KIND_WORDS.get(kind, family_word)
    st = raw.get("state")
    state_word = STATE_WORDS.get(_norm(st)) if st is not None else None
    if st is not None and state_word is None:
        state_word = UNKNOWN_STATE
    side_word = SIDE_WORDS.get(_norm(raw.get("side"))) if raw.get("side") is not None else None
    dmap = OB_DIRECTION_WORDS if family == "order block" else DIRECTION_WORDS
    dir_word = dmap.get(_norm(raw.get("direction"))) if raw.get("direction") is not None else None

    age = raw.get("age_bars")
    age = None if _f(age) is None else int(_f(age))
    # `formed_at_bar_ms` and NOT `bar_ms`: on this route `bar_ms` is the
    # timeframe's LENGTH and lives at the top level. Reading a duration as a
    # timestamp would render an age of fifty thousand years with nothing
    # complaining, so the duration's name is never looked for on a level.
    formed = _f(raw.get("formed_at_bar_ms"))
    swings = raw.get("swing_ids")
    swings = len(swings) if isinstance(swings, list) else None
    # WHEN A POOL WAS SWEPT, AS A UTC STAMP AND NOT AS A BAR COUNT.
    #
    # A bar count would read better beside the bar-counted ages on the same
    # line, and it is not available: the route publishes the sweep's
    # TIMESTAMP and only the forming bar's age in bars, and a bar count
    # derived here would have to be `(newest - swept_at) / bar_ms`, which is
    # clock time wearing a bar's name. Tried, and it printed "swept 675 bars
    # ago" on a pool 594 bars old - swept before it existed - because the
    # store holds 879 bars across 1,308 bar-lengths of clock. The stamp is
    # longer and it is true.
    swept_at = _f(raw.get("swept_at_bar_ms"))

    return {
        "family": family, "word": word, "price": price, "lo": lo, "hi": hi,
        "dist": dist, "dist_abs": abs(dist),
        "state": _norm(st) if st is not None else None, "state_word": state_word,
        "side_word": side_word, "direction_word": dir_word,
        "age_bars": age, "formed_at_bar_ms": None if formed is None else int(formed),
        "filled_fraction": _f(raw.get("filled_fraction")),
        "swept": raw.get("swept") if isinstance(raw.get("swept"), bool) else None,
        "swept_at_bar_ms": None if swept_at is None else int(swept_at),
        "swings": swings, "spread_atr": _f(raw.get("spread_atr")),
        "displacement_body_atr": _f(raw.get("displacement_body_atr")),
        # RECORDED AND NEVER RENDERED. The rule's name is a string another
        # crate owns; putting it in the prompt would make this book's wording
        # depend on a file nobody editing this book would think to read. The
        # operator still needs it, so the caller can log it.
        "rule": raw.get("rule"),
    }


def _harvest(doc: dict, close: float) -> list:
    """Every level on the response, in this file's words.

    The route groups by family in five differently shaped fields - `profile`
    is an object with three optional levels, `extremes` an object of three
    periods each with a high and a low, and the other three are plain lists -
    so each is unpacked where it is rather than through one generic walk that
    would have to guess.
    """
    out = []

    def take(raw, family, word):
        lv = _level(raw, family, word, close)
        if lv is not None:
            out.append(lv)

    prof = doc.get("profile")
    if isinstance(prof, dict):
        for key in ("poc", "vah", "val"):
            take(prof.get(key), "profile level", "activity profile level")

    for field, family in (("fair_value_gaps", "fair value gap"),
                          ("order_blocks", "order block"),
                          ("liquidity", "liquidity pool")):
        rows = doc.get(field)
        if isinstance(rows, list):
            for raw in rows:
                take(raw, family, family)

    ext = doc.get("extremes")
    if isinstance(ext, dict):
        for period in ("session", "day", "week"):
            p = ext.get(period)
            if not isinstance(p, dict):
                continue
            for end in ("high", "low"):
                take(p.get(end), "period extreme", "period extreme")
    return out


def census(levels: list) -> dict:
    """How many of each family there are, and how many are already spent.

    A COUNT, not a ranking: the registration forbids this block from scoring
    or ordering by anything but distance, and it does not forbid it from
    saying how big the thing it is sampling from is. On the live sample that
    is the difference between "here are twelve levels" and "here are twelve of
    252, and 135 of the pools have already been swept".
    """
    out = {"total": len(levels), "families": {}, "spent": {}}
    for lv in levels:
        out["families"][lv["family"]] = out["families"].get(lv["family"], 0) + 1
        if lv["state"] in ("swept", "broken"):
            out["spent"][lv["family"]] = out["spent"].get(lv["family"], 0) + 1
    return out


def gather(api: str = "http://127.0.0.1:8138", market: str = "xauusd",
           bar_time: int | None = None, last_close: float | None = None,
           atr: float | None = None, timeout: float = REQUEST_TIMEOUT_S,
           doc: dict | None = None) -> dict:
    """Ask the route, and work out which of the four states this is.

    Never raises. Always returns a dict carrying `state` (one of `STATES`),
    the levels in this file's words ordered by distance from the last close,
    and the numbers the block prints.

    `bar_time` is the START of the bar the caller is deciding on; without it
    staleness cannot be judged and `state` will not be `stale`, which the
    block says out loud rather than implying freshness.

    `last_close` and `atr` are passed by the caller rather than read off the
    response ON PURPOSE, even though the response carries both. The prompt
    already ends with forty bars and the block says "the last close"; a
    distance measured from a different number than the one the model can see
    on that last line would be wrong in the one way nobody would catch. The
    ATR is the one the desk block quotes 1R against, so "0.68 ATR" here and
    "1R is 1.2 x ATR" there are one unit. The route's `last_close` and `atr14`
    are the fallback, which is what the command line below uses.

    `doc` bypasses the fetch with a response already in hand - the command
    line's `--sample`, and the selftest's real captured response.
    """
    out = {
        "state": "unavailable", "market": market, "why": None,
        "unavailable": None, "bar_time": bar_time, "last_close": last_close,
        "atr": atr, "levels": [], "above": [], "below": [], "straddling": [],
        "timeframe": None, "bar_ms": None, "computed_at_bar_ms": None,
        "behind_ms": None, "behind_min": None, "tolerance_min": None,
        "staleness_checked": bar_time is not None,
        "census": {"total": 0, "families": {}, "spent": {}},
    }
    if doc is None:
        url = f"{api.rstrip('/')}/api/paper/levels?market={urllib.parse.quote(market)}"
        try:
            with urllib.request.urlopen(url, timeout=timeout) as r:
                doc = json.loads(r.read().decode("utf-8", "replace"))
        except Exception as exc:  # noqa: BLE001 - any failure is the same state
            out["why"] = f"{type(exc).__name__}: {exc}"
            return out
    if not isinstance(doc, dict):
        out["why"] = "the route did not answer with an object"
        return out

    # Recorded for the operator, never rendered. See the module docstring and
    # the route's own doc comment on the field, which says the same thing.
    out["unavailable"] = doc.get("unavailable")
    out["timeframe"] = doc.get("timeframe")
    bar_ms = _f(doc.get("bar_ms"))
    out["bar_ms"] = bar_ms
    started = _f(doc.get("computed_at_bar_ms"))
    out["computed_at_bar_ms"] = None if started is None else int(started)

    # NULL BLOCKS ARE NOT EMPTY LISTS. The route's module doc is explicit:
    # `null` means there were no bars to look at, an empty list means this
    # market has no gaps today, and the two must render differently. So the
    # test is on the blocks being absent, not on the level count.
    blocks = [doc.get(k) for k in ("profile", "fair_value_gaps", "order_blocks",
                                   "liquidity", "extremes")]
    if all(b is None for b in blocks):
        out["why"] = out["unavailable"] or "the route returned no levels and gave no reason"
        return out

    if bar_time is not None and bar_ms and started is not None:
        # Start to start and in CLOCK TIME, never divided into bars - see the
        # two measured facts above STALE_AFTER_BARS.
        behind_ms = bar_time - started
        tol_ms = DAILY_HOLE_MS + STALE_AFTER_BARS * bar_ms
        out["behind_ms"] = behind_ms
        out["behind_min"] = behind_ms / 60000.0
        out["tolerance_min"] = tol_ms / 60000.0
        if behind_ms > tol_ms:
            tf = out["timeframe"] or "base"
            out["state"] = "stale"
            out["why"] = (f"the newest closed {tf} bar behind these levels opened "
                          f"{behind_ms / 60000.0:.0f} minutes before the bar being decided, past "
                          f"the {tol_ms / 60000.0:.0f} minute tolerance (the measured one-hour "
                          f"daily hole plus two {tf} bars); the export that writes those bars "
                          f"has probably stopped")
            return out

    close = last_close if last_close is not None else _f(doc.get("last_close"))
    out["last_close"] = close
    out["atr"] = atr if atr is not None else _f(doc.get("atr14"))
    if close is None:
        # The route answered and the levels cannot be placed. Thin rather than
        # unavailable: the distinction is what tells an operator whether to
        # look at the export or at the caller.
        out["state"] = "thin"
        out["why"] = "no last close to measure distance from, so no level can be ordered"
        return out

    levels = _harvest(doc, close)
    if not levels:
        out["state"] = "thin"
        out["why"] = "the route answered with no levels in the window yet (warmup not met)"
        return out

    levels.sort(key=lambda l: l["dist_abs"])
    out["levels"] = levels
    out["above"] = [l for l in levels if l["dist"] > 0]
    out["below"] = [l for l in levels if l["dist"] < 0]
    out["straddling"] = [l for l in levels if l["dist"] == 0]
    out["census"] = census(levels)
    out["state"] = "ok"
    return out


def _line(lv: dict, unit: str, atr) -> str:
    """One level, with a unit on every number in it."""
    where = (f"{lv['lo']:.2f} to {lv['hi']:.2f} {unit}" if lv["price"] is None
             else f"{lv['price']:.2f} {unit}")
    d = lv["dist"]
    if d == 0:
        gap = f"the last close is on it (0.00 {unit} away)"
    else:
        scale = (f" ({abs(d) / atr:.2f} ATR)" if atr
                 else " (no ATR scale this bar, so distance is in price only)")
        gap = f"{d:+.2f} {unit} away{scale}"
    age = "age not reported" if lv["age_bars"] is None else f"{lv['age_bars']} bars old"

    head = lv["word"]
    if lv["side_word"]:
        head += f" ({lv['side_word']})"
    if lv["state_word"]:
        head += f" {lv['state_word']}"
    # Kept SHORT on purpose. Rendering the real response for the first time
    # put these lines at 170 characters against 60-character bar rows, and a
    # line a reader's eye cannot finish is a fact they do not have.
    extra = []
    if lv["direction_word"]:
        extra.append(lv["direction_word"])
    if lv["filled_fraction"] is not None:
        extra.append(f"{100 * lv['filled_fraction']:.0f}% filled")
    if lv["swept"] is True:
        extra.append("swept " + _u(lv["swept_at_bar_ms"])
                     if lv["swept_at_bar_ms"] is not None else "swept")
    if lv["swings"]:
        # A pool of one is a lone swing and a pool of four is four prices
        # stacked at the same level. The route reports both as pools - 95 of
        # the sample's 155 are pools of one - and which it is changes what the
        # level means, so the count is on the line. The ids themselves are
        # not: see the module docstring.
        extra.append("1 swing" if lv["swings"] == 1 else f"{lv['swings']} swings")
    if lv["spread_atr"] is not None:
        extra.append(f"spread {lv['spread_atr']:.2f} ATR")
    if lv["displacement_body_atr"] is not None:
        extra.append(f"made by a {lv['displacement_body_atr']:.2f} ATR body")
    return f"    {head}: {where}, {gap}, {age}" + ("; " + "; ".join(extra) if extra else "")


def _census_lines(c: dict) -> list:
    """The census, wrapped into the header.

    WHY IT IS HERE AT ALL. The route caps and ranks nothing, on purpose, and
    the live sample of 2026-09-19 carries 252 levels of which 135 pools are
    already swept and 62 blocks already broken. A model shown the nearest
    twelve with no idea that they were twelve of 252, most of them spent,
    would read a tidy tape. The count is not a ranking and does not become
    one; what it says is how big the thing the cap sampled from was.
    """
    if not c.get("total"):
        return []
    parts = []
    for family, n in sorted(c["families"].items(), key=lambda kv: -kv[1]):
        spent = c["spent"].get(family, 0)
        parts.append(f"{n} {family}" + ("s" if n != 1 else "")
                     + (f" ({spent} spent)" if spent else ""))
    body = (f"The desk found {c['total']} levels in this window and ranks none of them: "
            + ", ".join(parts)
            + ". Spent means a pool already swept or a block already broken.")
    return textwrap.wrap(body, width=94, initial_indent="  ", subsequent_indent="  ")


def block(ctx: dict) -> str:
    """The lines that go into the prompt. UTC, and a unit on every number."""
    market = (ctx.get("market") or "the market").upper()
    unit = _unit(ctx.get("market") or "")
    if ctx.get("state") == "unavailable":
        # IN OUR OWN WORDS, NEVER THE ROUTE'S, here and in every branch below.
        # The route's `unavailable` field is a sentence written in another
        # crate; printing it would let that crate edit a registered campaign's
        # prompt. Its own doc comment says so at the field.
        return "\n".join([
            f"PRICE LEVELS for {market}: UNAVAILABLE for this bar.",
            "  The desk computed no structural levels for this market right now.",
            "You are deciding without them. Do not guess at what they would have said.",
        ])
    if ctx.get("state") == "stale":
        return "\n".join([
            f"PRICE LEVELS for {market}: STALE, and therefore withheld.",
            f"  reason: {ctx.get('why') or 'not stated'}",
            "Levels this old would read as current and are not. You are deciding without them.",
        ])
    if ctx.get("state") == "thin":
        return "\n".join([
            f"PRICE LEVELS for {market}: THIN for this bar.",
            f"  reason: {ctx.get('why') or 'not stated'}",
            "The route answered and had no levels to give. You are deciding without them.",
        ])

    close, atr = ctx.get("last_close"), ctx.get("atr")
    tf = ctx.get("timeframe") or "the decision"
    L = [
        f"PRICE LEVELS for {market} - computed by the desk from CLOSED {tf} bars, one",
        "implementation shared with the screen. There is no score here, no ranking and no",
        "confluence count: a level's AGE and its STATE are as much of the fact as its price,",
        "and what to make of them is your job. It is context, like everything else in this",
        "section, and not a signal.",
        "",
    ]
    L.extend(_census_lines(ctx.get("census") or {}))
    L.extend([
        f"  Below are the nearest to the last close ({close:.2f} {unit}), NEAREST FIRST, above and",
        f"  below listed separately, at most {MAX_PER_SIDE} a side so this block's length does not depend",
        f"  on how busy the tape is. Ages are in {tf} bars. Distance to a band is to its nearer edge.",
    ])
    if not ctx.get("staleness_checked"):
        L.append("  (how far these levels lag the bar you are deciding on was NOT checked this run)")
    if atr is None:
        L.append("  (no ATR was available this bar, so distances are in price only)")

    for title, key in (("ABOVE the last close", "above"),
                       ("BELOW the last close", "below"),
                       ("STRADDLING the last close", "straddling")):
        rows = ctx.get(key) or []
        if not rows:
            if key == "straddling":
                continue
            L.append("")
            L.append(f"  {title}: none.")
            continue
        L.append("")
        L.append(f"  {title}, nearest first:")
        for lv in rows[:MAX_PER_SIDE]:
            L.append(_line(lv, unit, atr))
        rest = len(rows) - MAX_PER_SIDE
        if rest > 0:
            # Counted rather than dropped silently: a truncated list that
            # looked complete would tell the model the tape is tidier than it
            # is, and how many levels there are is itself a fact about it.
            L.append(f"    ({rest} further level{'s' if rest != 1 else ''} on this side are not "
                     f"shown, being further away)")
    return "\n".join(L)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--api", default="http://127.0.0.1:8138")
    ap.add_argument("--market", default="xauusd")
    ap.add_argument("--bar-time", type=int, default=None,
                    help="start of the bar being decided, so staleness can be judged")
    ap.add_argument("--last-close", type=float, default=None,
                    help="the close distances are measured from; the route's own is the fallback")
    ap.add_argument("--atr", type=float, default=None,
                    help="ATR(14) in price; the route's own atr14 is the fallback")
    ap.add_argument("--sample", default=None,
                    help="render a saved response instead of fetching one "
                         "(docs/api-samples/paper-levels.json)")
    args = ap.parse_args()
    doc = None
    if args.sample:
        doc = json.loads(io.open(args.sample, encoding="utf-8").read())
    ctx = gather(args.api, args.market, args.bar_time, args.last_close, args.atr, doc=doc)
    print(f"state={ctx['state']}  why={ctx.get('why')}")
    print(f"levels={len(ctx.get('levels') or [])}  above={len(ctx.get('above') or [])}  "
          f"below={len(ctx.get('below') or [])}  behind_min={ctx.get('behind_min')}")
    print()
    print(block(ctx))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
