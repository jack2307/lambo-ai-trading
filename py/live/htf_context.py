# -*- coding: utf-8 -*-
"""Higher-timeframe facts from the desk's own API, as CONTEXT for a prompt.

    py -3.9 py/live/htf_context.py                 # fetch once and print the block
    py -3.9 py/live/htf_context.py --market xauusd --api http://127.0.0.1:8138

One job: ask `GET /api/paper/htf` for the H4 and D1 facts it computes and
render them as a block a model can read. It decides nothing and it computes
nothing. Every number in the block comes off the wire exactly as the route
said it, because the whole point of that route is that the model and the Desk
panel read ONE implementation of these facts - two that each computed their
own would eventually disagree, and the disagreement would surface as a model
explaining a level the screen does not show.

WHAT THIS IS AND IS NOT. "Trend is your friend" is a claim, not a fact, and
this desk has not established it. Thirty-two registrations closed without a
survivor, and the higher-timeframe study that preceded this module found that
three of its four trend definitions changed sign or size when the bar anchor
was corrected. So the block carries FACTS - a swing label, two EMAs, ADX, an
efficiency ratio, a channel, yesterday's and last week's levels - and no
verdict. Whether they are worth anything is what the experiment measures; see
docs/hypotheses/2026-09-18-htf-context.md.

TWO AGES, AND THEY ARE DIFFERENT NUMBERS. The facts describe the newest CLOSED
higher-timeframe bar. The STRUCTURE LABEL does not: a fractal swing needs n
bars after it before it is a swing, so on H4 the newest swing is confirmed up
to eight hours after it happened. A block that showed one age would be saying
the label is as fresh as the ADX reading, and it is not. Both are printed, and
the route hands over `confirmed_at_bar_ms` for exactly this reason.

STALENESS IS MEASURED AGAINST THE BAR BEING DECIDED, NOT THE WALL CLOCK. The
route reads a parquet that `py/ingest/mt5_export.py` writes; nothing guarantees
that file is current, and a stopped export would freeze these facts while the
desk went on trading. A wall-clock threshold cannot catch it, because it cannot
tell a stopped export from a weekend - on Monday morning the newest closed
daily bar is Friday's, sixty hours old and perfectly correct. So the caller
passes the timestamp of the bar it is deciding on, and staleness is the gap
between that and the newest closed HTF bar, in HTF bars. Both freeze over a
weekend, so the gap stays small; only a stopped export opens it.

FAILING IS A RESULT, NOT AN EXCEPTION. Unreachable, absent, thin or stale, the
block says which and the caller puts that in the prompt and in the record. A
silent fallback to "no context" would put context-absent decisions into a
context-present book, and the experiment would be measuring a mixture with
nothing to separate it. This is the same rule `otl_context` runs under and it
was written for the same reason.
"""
from __future__ import annotations

import argparse
import datetime as dt
import json
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")
    except (AttributeError, ValueError):  # not a TextIOWrapper; nothing to do
        pass

REQUEST_TIMEOUT_S = 6.0

# How far the newest closed HTF bar may sit behind the bar being decided before
# this is called stale, in HTF bars.
#
# One is normal and unavoidable: at 13:05 the newest closed H4 bar is the one
# that closed at 13:00, and the 15m bar being decided is inside the H4 bar
# after it. Two is the same thing plus one missed export cycle, which is a
# desk that is slightly behind rather than one that is broken. Past that the
# export has stopped, and the model should be told rather than left to read
# three-day-old levels as current.
STALE_AFTER_BARS = 2.0

# The states a caller must be able to tell apart, worst first. `thin` is the
# one that would otherwise hide: the route answered, the object is there, and
# the facts inside it are null because a warmup is not met. That is not the
# same condition as the timeframe being absent and the two are not reported
# the same way - the route makes the distinction on purpose and throwing it
# away here would waste it.
STATES = ("unavailable", "stale", "thin", "ok")

# The facts whose absence makes an otherwise-present H4 object `thin`. Chosen
# because each needs a different warmup and between them they cover all of it:
# the swing rule needs a formed leg, ADX(14) needs its bars, EMA(55) needs
# more of them. `last_close` cannot be null on a non-empty file, and is in the
# list anyway so a future change to the route cannot make the block render a
# level against a missing price without tripping this.
CORE_H4 = ("adx14", "ema21", "ema55", "last_close")


def _unit(market: str) -> str:
    """The market's quote units, spelled out for the prompt.

    The route publishes prices "in the market's own quote units" and names no
    unit, correctly - it serves every market. A prompt has exactly one market
    in it and can therefore say which, and this desk spent 2026-09-17 removing
    five numbers whose units lived only in prose.
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


def _age(later_ms, bar_start_ms, bar_ms):
    """Age in (ms, bars) from the CLOSE of the bar starting at `bar_start_ms`.

    The route's `..._bar_ms` stamps are bar STARTS, as everywhere else in this
    repository. A bar is not knowable until it closes, so an age measured from
    its start overstates its freshness by one whole bar.
    """
    if later_ms is None or bar_start_ms is None or not bar_ms:
        return None, None
    ms = later_ms - (bar_start_ms + bar_ms)
    return ms, ms / float(bar_ms)


def gather(api: str = "http://127.0.0.1:8138", market: str = "xauusd",
           deciding_bar_ms: int | None = None, timeout: float = REQUEST_TIMEOUT_S) -> dict:
    """Ask the route, and work out which of the four states this is.

    Never raises. Always returns a dict carrying `state` (one of `STATES`),
    `h4`/`d1` exactly as the route sent them, and the two ages the block
    prints. `deciding_bar_ms` is the START of the bar the caller is deciding
    on; without it staleness cannot be judged and `state` will not be `stale`,
    which the block says out loud rather than implying freshness.
    """
    out = {
        "state": "unavailable", "market": market, "h4": None, "d1": None,
        "unavailable": None, "why": None, "deciding_bar_ms": deciding_bar_ms,
        "facts_age_ms": None, "facts_age_bars": None,
        "label_age_ms": None, "label_age_bars": None,
        "staleness_checked": deciding_bar_ms is not None,
        "thin_fields": [],
    }
    url = f"{api.rstrip('/')}/api/paper/htf?market={urllib.parse.quote(market)}"
    try:
        with urllib.request.urlopen(url, timeout=timeout) as r:
            doc = json.loads(r.read().decode("utf-8", "replace"))
    except Exception as exc:  # noqa: BLE001 - any failure is the same state
        out["why"] = f"{type(exc).__name__}: {exc}"
        return out
    if not isinstance(doc, dict):
        out["why"] = "the route did not answer with an object"
        return out

    out["h4"], out["d1"] = doc.get("h4"), doc.get("d1")
    out["unavailable"] = doc.get("unavailable")
    if not isinstance(out["h4"], dict):
        # The route's `unavailable` sentence is the join of the reasons it
        # pushed, h4's first. With h4 null it always contains h4's.
        out["why"] = out["unavailable"] or "the route returned no h4 object and gave no reason"
        return out

    h4, bar_ms = out["h4"], out["h4"].get("bar_ms")
    # Ages against the ROUTE's own clock, not this process's: both live on the
    # same machine, and using the route's removes a skew this module would
    # otherwise have to reason about.
    now_ms = h4.get("computed_at_ms")
    out["facts_age_ms"], out["facts_age_bars"] = _age(now_ms, h4.get("computed_at_bar_ms"), bar_ms)
    confirmed = ((h4.get("structure") or {}).get("confirmed_at_bar_ms"))
    out["label_age_ms"], out["label_age_bars"] = _age(now_ms, confirmed, bar_ms)

    if deciding_bar_ms is not None:
        _, behind = _age(deciding_bar_ms, h4.get("computed_at_bar_ms"), bar_ms)
        out["behind_bars"] = behind
        if behind is not None and behind > STALE_AFTER_BARS:
            out["state"] = "stale"
            out["why"] = (f"the newest closed H4 bar is {behind:.1f} H4 bars behind the bar being "
                          f"decided; the export that writes those bars has probably stopped")
            return out

    missing = [k for k in CORE_H4 if h4.get(k) is None]
    if missing:
        out["state"], out["thin_fields"] = "thin", missing
        out["why"] = "warmup not met for " + ", ".join(missing)
        return out
    out["state"] = "ok"
    return out


def block(ctx: dict) -> str:
    """The lines that go into the prompt. UTC, and a unit on every number."""
    market = ctx.get("market") or "the market"
    unit = _unit(market)
    head = [
        f"HIGHER-TIMEFRAME FACTS for {market.upper()} - computed by the desk from CLOSED H4 and D1",
        "bars only, one implementation shared with the screen. There is no verdict here and no",
        "composite score: the swing label is the only word in it, and combining the rest is your",
        "job. It is context, like everything else in this section, and not a signal.",
    ]
    if ctx.get("state") == "unavailable":
        return "\n".join([
            f"HIGHER-TIMEFRAME FACTS for {market.upper()}: UNAVAILABLE for this bar.",
            f"  reason: {ctx.get('why') or 'not stated'}",
            "You are deciding without them. Do not guess at what they would have said.",
        ])
    if ctx.get("state") == "stale":
        return "\n".join([
            f"HIGHER-TIMEFRAME FACTS for {market.upper()}: STALE, and therefore withheld.",
            f"  reason: {ctx.get('why') or 'not stated'}",
            "Levels this old would read as current and are not. You are deciding without them.",
        ])

    h4 = ctx.get("h4") or {}
    d1 = ctx.get("d1") or {}
    L = list(head)

    def add(label, value, suffix="", fmt="{}"):
        # `null` is not zero. The route makes every fact an Option and always
        # sends it, so an absent number means a warmup that is not met - a
        # different thing from a measurement of zero, and rendered differently
        # here because a reader cannot tell them apart otherwise.
        if value is None:
            L.append(f"    {label}: not computed yet (warmup not met)")
        else:
            L.append(f"    {label}: {fmt.format(value)}{suffix}")

    fa, fb = ctx.get("facts_age_ms"), ctx.get("facts_age_bars")
    L.append("")
    L.append(f"  as of the H4 bar that opened {_u(h4.get('computed_at_bar_ms'))} and has since closed"
             + (f"; {fa / 3600000.0:.1f}h ({fb:.1f} H4 bars) ago" if fa is not None else ""))
    if not ctx.get("staleness_checked"):
        L.append("  (how far these bars lag the bar you are deciding on was NOT checked this run)")
    if ctx.get("state") == "thin":
        L.append(f"  THIN: {ctx.get('why')}. Those facts are absent below, not zero.")

    st = h4.get("structure") or {}
    L.append("")
    L.append("  H4 SWING STRUCTURE")
    L.append(f"    label: {st.get('label')}  (rule: {st.get('rule')}; UP needs a higher high AND a "
             f"higher low, DOWN a lower high and a lower low, anything else RANGE)")
    la, lb = ctx.get("label_age_ms"), ctx.get("label_age_bars")
    if st.get("confirmed_at_bar_ms") is None:
        L.append("    confirmed: never - no swing leg has formed yet, so this label is the "
                 "default and not a reading")
    else:
        # THE SECOND AGE. A fractal is not a fractal until n bars have printed
        # after it, so this label can be two bars older than every other
        # number in this block. Told "structure is UP" without it, a reader is
        # being told something stronger than is known.
        L.append(f"    confirmed: {_u(st.get('confirmed_at_bar_ms'))}"
                 + (f", which is {la / 3600000.0:.1f}h ({lb:.1f} H4 bars) ago - OLDER than the "
                    f"facts below, because a swing is only a swing once the bars after it have "
                    f"printed" if la is not None else ""))
    for name, key in (("last swing high", "last_high"), ("prior swing high", "prior_high"),
                      ("last swing low", "last_low"), ("prior swing low", "prior_low")):
        sw = st.get(key)
        if not isinstance(sw, dict):
            L.append(f"    {name}: none formed yet")
        else:
            L.append(f"    {name}: {sw.get('price')} {unit} on the bar opening {_u(sw.get('bar_ms'))}")
    if st.get("break_level") is None:
        L.append("    break level: none - a RANGE has no single price that changes the label")
    else:
        side = "below" if str(st.get("break_side")).upper() == "BELOW" else "above"
        L.append(f"    break level: trading {side} {st.get('break_level')} {unit} ends this structure")

    L.append("")
    L.append("  H4 TREND AND VOLATILITY MEASURES")
    add("last H4 close", h4.get("last_close"), f" {unit}")
    add("EMA(21)", h4.get("ema21"), f" {unit}")
    add("EMA(55)", h4.get("ema55"), f" {unit}")
    for label, key in (("EMA(21) slope", "ema21_slope_sign"), ("EMA(55) slope", "ema55_slope_sign")):
        v = h4.get(key)
        if v is None:
            L.append(f"    {label}: not computed yet (warmup not met)")
        else:
            word = {1: "rising", 0: "flat", -1: "falling"}.get(int(v), str(v))
            L.append(f"    {label}: {int(v):+d} ({word}) over the last three closed H4 bars"
                     if int(v) else f"    {label}: 0 ({word}) over the last three closed H4 bars")
    add("ATR(14) on H4", h4.get("atr14"), f" {unit}")
    add("close minus EMA(21)", h4.get("dist_ema21_atr"), " ATR(14), signed: positive is above",
        fmt="{:+.2f}" if isinstance(h4.get("dist_ema21_atr"), (int, float)) else "{}")
    add("ADX(14)", h4.get("adx14"), " (unitless; conventionally 25 is the trending threshold, "
                                   "which is a convention and not a result of this desk)")
    add("+DI(14)", h4.get("plus_di14"))
    add("-DI(14)", h4.get("minus_di14"))
    add("efficiency ratio, 20 bars", h4.get("efficiency_20"),
        " (unitless: 1.0 is a straight line, 0.0 is pure churn)")
    dc = h4.get("donchian20") or {}
    add("Donchian(20) upper", dc.get("upper"), f" {unit}")
    add("Donchian(20) lower", dc.get("lower"), f" {unit}")
    for label, key in (("bars since a new 20-bar high", "bars_since_new_high"),
                       ("bars since a new 20-bar low", "bars_since_new_low")):
        v = dc.get(key)
        if v is None:
            L.append(f"    {label}: window not full yet")
        else:
            L.append(f"    {label}: {v} H4 bars" + (" - this bar made one" if v == 0 else ""))

    L.append("")
    if not isinstance(ctx.get("d1"), dict):
        # Reported on its own line, because the route can serve H4 and not D1.
        # Saying "higher-timeframe facts unavailable" over a present H4 object
        # would throw away everything above.
        why = ctx.get("unavailable") or "not stated"
        L.append(f"  DAILY LEVELS: unavailable ({why}). The H4 facts above are unaffected.")
        return "\n".join(L)
    L.append("  DAILY LEVELS - the prices a trader names out loud. This broker's day runs "
             "21:00 UTC to 21:00 UTC.")
    add("prior day high", d1.get("prior_day_high"), f" {unit}")
    add("prior day low", d1.get("prior_day_low"), f" {unit}")
    L.append(f"    that day's bar opened: {_u(d1.get('prior_day_bar_ms'))}")
    add("prior week high", d1.get("prior_week_high"), f" {unit}")
    add("prior week low", d1.get("prior_week_low"), f" {unit}")
    add("prior week midpoint", d1.get("prior_week_mid"), f" {unit}")
    L.append(f"    that week's first bar opened: {_u(d1.get('prior_week_start_ms'))}")
    pct = d1.get("close_pct_of_prior_week_range")
    if pct is None:
        L.append("    last close within the prior week's range: not computed yet")
    else:
        # NOT a 0-100 percentage, and the block must not imply that it is.
        # Above 100 means price has left the prior week's range upward and
        # below 0 downward, which is the most informative thing this number
        # ever says. A renderer that clamped or capped it would read wrong
        # exactly when it mattered most.
        where = ("ABOVE the whole prior week's range" if pct > 100 else
                 "BELOW the whole prior week's range" if pct < 0 else
                 "inside the prior week's range")
        L.append(f"    last close sits at {pct:.1f}% of the prior week's range, measured from its "
                 f"low - {where}. This figure is not capped: over 100 and under 0 are real "
                 f"readings and mean the range was left.")
    add("last daily close", d1.get("last_close"), f" {unit}")
    return "\n".join(L)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--api", default="http://127.0.0.1:8138")
    ap.add_argument("--market", default="xauusd")
    ap.add_argument("--deciding-bar-ms", type=int, default=None,
                    help="start of the bar being decided, so staleness can be judged")
    args = ap.parse_args()
    ctx = gather(args.api, args.market, args.deciding_bar_ms)
    print(f"state={ctx['state']}  why={ctx.get('why')}")
    print(f"facts_age_bars={ctx.get('facts_age_bars')}  label_age_bars={ctx.get('label_age_bars')}")
    print()
    print(block(ctx))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
