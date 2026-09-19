# -*- coding: utf-8 -*-
"""Structural price levels from the desk's own API, as CONTEXT for a prompt.

    py -3.9 py/live/smc_context.py                  # fetch once and print the block
    py -3.9 py/live/smc_context.py --market xauusd --api http://127.0.0.1:8138

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

AGE AND STATE ARE THE CONTENT, NOT DECORATION. An UNTESTED order block four
bars old and an UNTESTED order block two hundred bars old are different facts,
and a block that printed only the price would be saying they are the same one.
Both numbers are on every line, with the timeframe whose bars they are counted
in, because "4 bars" means forty minutes on 15m and sixteen hours on 4h.

STALENESS IS MEASURED AGAINST THE BAR BEING DECIDED, NOT THE WALL CLOCK. Same
reason as `htf_context`: a wall-clock threshold cannot tell a stopped export
from a weekend, and on Monday morning the newest closed bar is correctly
Friday's. The caller passes the timestamp of the bar it is deciding on, and
staleness is the gap between that and the newest closed bar behind the FINEST
timeframe the route reports provenance for - the one that must be current. A
coarse timeframe lagging is normal; the fine one lagging is a stopped export.

THE ROUTE'S OWN TEXT NEVER REACHES THE PROMPT, IN ANY STATE. This is the
lesson of `9a5dbfb`, pre-committed in the registration rather than discovered
here: `htf_context` printed the route's `unavailable` sentence verbatim, and
that sentence is a join written in another crate - so adding a timeframe to
the route would have changed the wording inside a REGISTERED campaign's prompt
with nobody editing the book and no diff to notice.

It costs more here than it did there, and the cost is deliberate. The route
carries **the name of the rule that produced each level**, and that name is a
string another crate owns, so it is RECORDED in `gather` and never rendered. A
level's `kind` and `state` are likewise mapped through this file's own
vocabulary (`KIND_WORDS`, `STATE_WORDS`); anything outside it renders as "not
in this block's vocabulary" rather than echoing what the wire said. The one
route string that does influence the text is a swing id, and only through a
strict `SWING_ID` pattern whose parts are re-rendered in our words - a marker
fed through it does not match and does not print.

FAILING IS A RESULT, NOT AN EXCEPTION. Unreachable, thin or stale, the block
says which and the caller puts that in the prompt and in the record. A silent
fallback to "no levels" would put context-absent decisions into a
context-present book, and the experiment would be measuring a mixture with
nothing to separate it. This is the rule `otl_context` and `htf_context` run
under and it is written here for the same reason.
"""
from __future__ import annotations

import argparse
import datetime as dt
import json
import re
import sys
import urllib.error
import urllib.parse
import urllib.request

for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")
    except (AttributeError, ValueError):  # not a TextIOWrapper; nothing to do
        pass

REQUEST_TIMEOUT_S = 6.0

# How far the newest closed bar behind these levels may sit behind the bar
# being decided before this is called stale, in bars of that same timeframe.
#
# Two, and the same two `htf_context` uses, for the same arithmetic: one is
# normal and unavoidable (the newest CLOSED bar is always at least one behind
# the bar being decided), two is that plus one missed export cycle. Past that
# the export has stopped and the model should be told rather than left to read
# levels that formed three days ago as if they were this morning's.
STALE_AFTER_BARS = 2.0

# The states a caller must be able to tell apart, worst first. `thin` is the
# one that would otherwise hide: the route answered for this market and
# computed no levels at all, because a warmup is not met. That is a different
# condition from having no bars for the market, and the two are not reported
# the same way.
STATES = ("unavailable", "stale", "thin", "ok")

# HOW MANY LEVELS THE BLOCK SHOWS, AND WHY THE NUMBER IS FIXED.
#
# Six per side. The registration pre-commits "a fixed maximum count, stated in
# the block itself", because a prompt whose length depends on how many order
# blocks happen to be unfilled is a prompt whose token cost and whose
# legibility vary with the thing being measured.
#
# Six because of what the block sits in front of. The prompt ends with the
# forty 15m bars the model actually decides on (`--bars` default 40), and the
# two context blocks already beside this one measure 43 lines (`htf_context`
# on the selftest's fixture) and 17 (`otl_context` on its own). At six a side
# this one measures 30 lines on a full book with something straddling the
# close and 27 without - measured by `smc_context_selftest.py`, which pins
# the ceiling - so it is the smaller of the two big blocks and the bars stay
# the longest thing in the prompt. At twelve a side the arithmetic is 9 header
# lines plus 2 x (2 + 12 + 1) plus a straddling section: 42, level with the
# htf block and longer than the forty bars it sits in front of.
#
# Levels beyond the cap are COUNTED on their own line rather than dropped
# silently: "9 further levels above are not shown" is a fact about the market,
# and a truncated list that looked complete would tell the model the tape is
# tidier than it is.
MAX_PER_SIDE = 6

# THE GROUPS THIS BLOCK READS, AND THE WORD IT CALLS EACH ONE BY.
#
# Our words, never the route's, for the reason in the module docstring. The
# aliases exist because this was written against the contract before the route
# was on `main`: a group the route names differently is read, a group nobody
# named is simply absent, and neither can put another crate's prose in the
# prompt.
GROUPS = (
    ("profile", ("profile", "activity_profile", "volume_profile"), "activity profile level"),
    ("fvg", ("fvg", "fvgs", "fair_value_gaps"), "fair value gap"),
    ("order_blocks", ("order_blocks", "order_block", "obs"), "order block"),
    ("liquidity_buy", ("liquidity_buy", "buyside_liquidity", "buy_side_liquidity", "bsl"),
     "buy-side liquidity"),
    ("liquidity_sell", ("liquidity_sell", "sellside_liquidity", "sell_side_liquidity", "ssl"),
     "sell-side liquidity"),
    ("extremes", ("extremes", "session_extremes", "range_extremes"), "session/day/week extreme"),
)

# A level's `kind`, in this file's vocabulary. A kind outside it is rendered by
# its GROUP's word - which this file chose - and never by the wire's string.
KIND_WORDS = {
    "poc": "point of control", "vah": "value area high", "val": "value area low",
    "fvg": "fair value gap", "bisi": "fair value gap (up)", "sibi": "fair value gap (down)",
    "ob": "order block", "order_block": "order block",
    "bullish_ob": "bullish order block", "bearish_ob": "bearish order block",
    "bsl": "buy-side liquidity", "buyside_liquidity": "buy-side liquidity",
    "ssl": "sell-side liquidity", "sellside_liquidity": "sell-side liquidity",
    "equal_highs": "equal highs", "equal_lows": "equal lows",
    "session_high": "session high", "session_low": "session low",
    "day_high": "day high", "day_low": "day low",
    "week_high": "week high", "week_low": "week low",
    "prior_day_high": "prior day high", "prior_day_low": "prior day low",
    "prior_week_high": "prior week high", "prior_week_low": "prior week low",
}

# A level's `state`, likewise. The contract names untested / tested / broken
# for order blocks and `swept` for liquidity; the rest are here because a
# route that grows one should not silently print it.
STATE_WORDS = {
    "untested": "UNTESTED", "tested": "TESTED", "broken": "BROKEN",
    "unfilled": "UNFILLED", "partial": "PARTIALLY FILLED", "filled": "FILLED",
    "mitigated": "MITIGATED", "intact": "INTACT",
    "swept": "SWEPT", "unswept": "NOT SWEPT",
}
UNKNOWN_STATE = "state not in this block's vocabulary"

# The one route string allowed to shape the text, and only through this.
#
# The contract says liquidity references HTF swing ids like `h4-hi-<bar_ms>`,
# which is the handle tying a level to the swing the `htf-context` block
# prints. It is worth carrying, and it is carried by PARSING it and
# re-rendering its three parts in our words - never by printing it. A marker
# fed through the field does not match this pattern and reaches nothing.
SWING_ID = re.compile(r"^(1m|5m|15m|h1|h4|d1|w1)-(hi|lo)-(\d{10,16})$")
SWING_TF = {"1m": "1m", "5m": "5m", "15m": "15m", "h1": "H1", "h4": "H4", "d1": "D1", "w1": "W1"}

# A timeframe token, validated before it is printed, for the same reason and
# by the same means. `tf` is another crate's string too, and it names the
# bars an age is counted in - "4 bars" is forty minutes on 15m and sixteen
# hours on 4h, so it has to be on the line, and it gets there only by
# matching a shape this file fixed. An unrecognised token is dropped and the
# age prints without it, which loses a fact rather than importing prose.
TF_TOKEN = re.compile(r"^\d{1,3}(m|h|d|w)$")


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


def _swing_words(ref) -> str | None:
    """`h4-hi-1758232800000` -> "the H4 swing high from 2026-09-18 12:00Z".

    Parsed and re-rendered rather than printed: see SWING_ID. Anything that is
    not exactly that shape returns None and nothing is said about it, which is
    the safe direction - an unreadable handle is less than no handle.
    """
    m = SWING_ID.match(str(ref or "").strip().lower())
    if not m:
        return None
    tf, side, stamp = m.group(1), m.group(2), int(m.group(3))
    return (f"the {SWING_TF[tf]} swing {'high' if side == 'hi' else 'low'} "
            f"from {_u(stamp)}")


def _groups(doc: dict) -> list:
    """`[(group_key, group_word, [raw levels])]`, from wherever the route put them.

    The contract groups the levels by kind; whether that mapping sits under a
    `levels` key or at the top of the document is not something this file
    should care about, and reading both costs one line.
    """
    holder = doc.get("levels") if isinstance(doc.get("levels"), dict) else doc
    out = []
    for key, aliases, word in GROUPS:
        rows = None
        for alias in aliases:
            v = holder.get(alias)
            if isinstance(v, list):
                rows = v
                break
        out.append((key, word, rows or []))
    return out


def _provenance(doc: dict) -> dict:
    """`{tf: {"bar_ms":, "computed_at_bar_ms":, ...}}`, per timeframe."""
    prov = doc.get("provenance")
    return prov if isinstance(prov, dict) else {}


def _base_tf(prov: dict):
    """The FINEST timeframe the route reported, and its bar duration in ms.

    Staleness is judged on this one. A 1d level being a day old is what a 1d
    level is; the 15m provenance falling behind is the export having stopped,
    and that is the failure this is looking for.
    """
    best = (None, None)
    for tf, p in prov.items():
        if not isinstance(p, dict):
            continue
        ms = _f(p.get("bar_ms"))
        if ms and (best[1] is None or ms < best[1]):
            best = (tf, ms)
    return best


def _level(raw, group_word: str, close: float, prov: dict):
    """One wire level, in this file's own words and measured from the close.

    Returns None for anything carrying neither a price nor a two-price band:
    a level with no location cannot be ordered by distance and has nothing to
    say on a line about distance.
    """
    if not isinstance(raw, dict):
        return None
    price = _f(raw.get("price"))
    lo = hi = None
    band = raw.get("band")
    if isinstance(band, (list, tuple)) and len(band) == 2:
        a, b = _f(band[0]), _f(band[1])
        if a is not None and b is not None:
            lo, hi = min(a, b), max(a, b)
    if lo is None and price is None:
        return None

    # Distance is to the NEARER EDGE of a band, and zero when the last close
    # is inside it. A band's midpoint would be a number the route never said
    # and the price never has to reach.
    if lo is not None:
        dist = (lo - close) if close < lo else ((hi - close) if close > hi else 0.0)
    else:
        dist = price - close

    tf = raw.get("tf") or raw.get("timeframe")
    tf = str(tf).strip() if tf else None
    tf_word = tf.lower() if tf and TF_TOKEN.match(tf.lower()) else None
    # `formed_at_bar_ms` / `formed_bar_ms` only, and deliberately NOT `bar_ms`:
    # everywhere else in this repository `bar_ms` is a bar's DURATION, and
    # reading a duration as a timestamp is a unit bug that would render an age
    # of fifty thousand years without anything complaining.
    formed = raw.get("formed_at_bar_ms")
    if formed is None:
        formed = raw.get("formed_bar_ms")
    formed = None if _f(formed) is None else int(_f(formed))
    age_bars = _f(raw.get("age_bars"))

    kind = _norm(raw.get("kind"))
    word = KIND_WORDS.get(kind, group_word)
    st = raw.get("state")
    state_word = STATE_WORDS.get(_norm(st)) if st is not None else None
    if st is not None and state_word is None:
        state_word = UNKNOWN_STATE

    filled = _f(raw.get("filled_frac"))
    if filled is None:
        filled = _f(raw.get("fraction_filled"))
    swept = raw.get("swept")
    swept = bool(swept) if isinstance(swept, bool) else None
    swept_bar = raw.get("swept_by_bar_ms")
    if swept_bar is None:
        swept_bar = raw.get("swept_at_bar_ms")
    swept_bar = None if _f(swept_bar) is None else int(_f(swept_bar))

    return {
        "word": word, "price": price, "lo": lo, "hi": hi,
        "dist": dist, "dist_abs": abs(dist),
        "state_word": state_word, "age_bars": age_bars, "tf": tf, "tf_word": tf_word,
        "formed_at_bar_ms": formed,
        "filled_frac": filled, "swept": swept, "swept_by_bar_ms": swept_bar,
        "swing": _swing_words(raw.get("ref") or raw.get("swing_id")),
        # RECORDED AND NEVER RENDERED. The rule's name is a string another
        # crate owns; putting it in the prompt would make this book's wording
        # depend on a file nobody editing this book would think to read. The
        # operator still needs it, so the caller can log it.
        "rule": raw.get("rule"),
    }


def gather(api: str = "http://127.0.0.1:8138", market: str = "xauusd",
           bar_time: int | None = None, last_close: float | None = None,
           atr: float | None = None, timeout: float = REQUEST_TIMEOUT_S) -> dict:
    """Ask the route, and work out which of the four states this is.

    Never raises. Always returns a dict carrying `state` (one of `STATES`),
    the levels in this file's words ordered by distance from the last close,
    and the numbers the block prints.

    `bar_time` is the START of the bar the caller is deciding on; without it
    staleness cannot be judged and `state` will not be `stale`, which the
    block says out loud rather than implying freshness.

    `last_close` is passed by the caller rather than read off the route ON
    PURPOSE. The prompt already ends with forty bars and says "the last
    close"; a distance measured from a different number than the one the model
    can see on the last line would be wrong in the one way nobody would catch.
    The route's own value is the fallback for the command line below.

    `atr` is the volatility scale the distances are also quoted in - the
    caller passes the ATR(14) the desk sizes on, so "0.5 ATR" here and "1R is
    1.2 x ATR" in the desk block are the same unit. Without it the distances
    are still printed in price and the block says the scale was missing rather
    than quietly dropping half of each number.
    """
    out = {
        "state": "unavailable", "market": market, "why": None,
        "unavailable": None, "bar_time": bar_time, "last_close": last_close,
        "atr": atr, "levels": [], "above": [], "below": [], "straddling": [],
        "provenance": {}, "base_tf": None, "base_bar_ms": None,
        "behind_bars": None, "staleness_checked": bar_time is not None,
    }
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

    # Recorded for the operator, never rendered. See the module docstring.
    out["unavailable"] = doc.get("unavailable")
    prov = _provenance(doc)
    out["provenance"] = prov
    groups = _groups(doc)
    if out["unavailable"] and not any(rows for _, _, rows in groups):
        out["why"] = "the route has no bars for this market"
        return out
    if not prov and not any(rows for _, _, rows in groups):
        out["why"] = "the route returned neither levels nor provenance"
        return out

    tf, bar_ms = _base_tf(prov)
    out["base_tf"], out["base_bar_ms"] = tf, bar_ms
    if bar_time is not None and bar_ms:
        # The gap from the newest CLOSED bar behind the levels to the bar being
        # decided, in bars of that timeframe. A bar is not knowable until it
        # closes, so the close is `start + bar_ms` - measuring from the start
        # would overstate freshness by one whole bar.
        p = prov.get(tf) or {}
        start = _f(p.get("computed_at_bar_ms"))
        if start is not None:
            behind = (bar_time - (start + bar_ms)) / float(bar_ms)
            out["behind_bars"] = behind
            if behind > STALE_AFTER_BARS:
                out["state"] = "stale"
                out["why"] = (f"the newest closed {tf} bar behind these levels is {behind:.1f} "
                              f"{tf} bars behind the bar being decided; the export that writes "
                              f"those bars has probably stopped")
                return out

    close = last_close if last_close is not None else _f(doc.get("last_close"))
    out["last_close"] = close
    if close is None:
        # The route answered and the levels cannot be placed. Thin rather than
        # unavailable: the distinction is what tells an operator whether to
        # look at the export or at the caller.
        out["state"] = "thin"
        out["why"] = "no last close to measure distance from, so no level can be ordered"
        return out

    levels = []
    for _key, word, rows in groups:
        for raw in rows:
            lv = _level(raw, word, close, prov)
            if lv is not None:
                levels.append(lv)
    if not levels:
        out["state"] = "thin"
        out["why"] = "the route computed no levels for this market yet (warmup not met)"
        return out

    # Ages the route did not state, from the bar it formed on and that
    # timeframe's own bar length. Left as None when neither is knowable, and
    # the line then says the age was not reported rather than printing a zero.
    for lv in levels:
        if lv["age_bars"] is None and lv["formed_at_bar_ms"] is not None and bar_time is not None:
            p = prov.get(lv["tf"]) if lv["tf"] else None
            ms = _f((p or {}).get("bar_ms")) or bar_ms
            if ms:
                lv["age_bars"] = (bar_time - lv["formed_at_bar_ms"]) / float(ms)

    levels.sort(key=lambda l: l["dist_abs"])
    out["levels"] = levels
    out["above"] = [l for l in levels if l["dist"] > 0]
    out["below"] = [l for l in levels if l["dist"] < 0]
    out["straddling"] = [l for l in levels if l["dist"] == 0]
    out["state"] = "ok"
    return out


def _line(lv: dict, unit: str, atr) -> str:
    """One level, with a unit on every number in it."""
    where = (f"{lv['lo']:.2f} to {lv['hi']:.2f} {unit}" if lv["lo"] is not None
             else f"{lv['price']:.2f} {unit}")
    d = lv["dist"]
    if d == 0:
        gap = f"the last close is on it (0.00 {unit} away)"
    else:
        scale = (f" ({abs(d) / atr:.2f} ATR)" if atr
                 else " (no ATR scale this bar, so distance is in price only)")
        gap = f"{d:+.2f} {unit} away{scale}"
    if lv["age_bars"] is None:
        age = "age not reported"
    else:
        age = f"{lv['age_bars']:.0f} bars old" + (f" ({lv['tf_word']})" if lv["tf_word"] else "")
    head = lv["word"] + (f" {lv['state_word']}" if lv["state_word"] else "")
    extra = []
    if lv["filled_frac"] is not None:
        extra.append(f"{100 * lv['filled_frac']:.0f}% filled")
    if lv["swept"] is True:
        extra.append("SWEPT on the bar opening " + _u(lv["swept_by_bar_ms"]))
    elif lv["swept"] is False:
        extra.append("not swept")
    if lv["swing"]:
        extra.append("at " + lv["swing"])
    return f"    {head}: {where}, {gap}, {age}" + ("; " + "; ".join(extra) if extra else "")


def block(ctx: dict) -> str:
    """The lines that go into the prompt. UTC, and a unit on every number."""
    market = (ctx.get("market") or "the market").upper()
    unit = _unit(ctx.get("market") or "")
    if ctx.get("state") == "unavailable":
        # IN OUR OWN WORDS, NEVER THE ROUTE'S, here and in every branch below.
        # The route's `unavailable` field is a join written in another crate;
        # printing it would let that crate edit a registered campaign's prompt.
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
    L = [
        f"PRICE LEVELS for {market} - computed by the desk from CLOSED bars, one implementation",
        "shared with the screen. There is no score here, no ranking and no confluence count: a",
        "level's AGE and its STATE are as much of the fact as its price, and what to make of them",
        "is your job. It is context, like everything else in this section, and not a signal.",
        "",
        f"  Ordered by distance from the last close ({close:.2f} {unit}), NEAREST FIRST, above and below",
        f"  listed separately. At most {MAX_PER_SIDE} are shown per side and the rest are counted, so this",
        "  block's length does not depend on how busy the tape is. Distance to a band is to its nearer",
        "  edge; ages are counted in bars of the timeframe named on the line.",
    ]
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
                    help="ATR(14) in price, so distances can also be quoted in ATR")
    args = ap.parse_args()
    ctx = gather(args.api, args.market, args.bar_time, args.last_close, args.atr)
    print(f"state={ctx['state']}  why={ctx.get('why')}")
    print(f"levels={len(ctx.get('levels') or [])}  above={len(ctx.get('above') or [])}  "
          f"below={len(ctx.get('below') or [])}  behind_bars={ctx.get('behind_bars')}")
    print()
    print(block(ctx))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
