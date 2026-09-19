"""A model trading its own paper book, beside a coin flipping for the same trades.

    python py/live/ai_trader.py --model=gpt-5 --market=xauusd --tf=15m
    python py/live/ai_trader.py --dry-run          # decide and log, post nothing

**Read `docs/paper/AI-TRADER.md` first.** It is short and it is the design; the
one sentence that matters is that the model is judged against a coin-flip book
taking the same trades at the same bars, never against zero.

What this process may do is bounded by the route it posts to, not by this file:
it can name a side, a stop and a target for a run whose strategy is `external`,
and nothing else. It cannot size a trade, cannot act on the bar it was shown
(the intent fills at the next open), cannot arrive late and still trade, and
cannot reach a broker. `py/live/mt5_executor.py` remains the only code in this
repository that can send an order, and it refuses any account that is not a
demo.

One exception to "cannot act on the bar it was shown", and it is narrow: the
`plan` variants (stages 1 and 2 of docs/plans/2026-09-18-staged-ai-entry.md)
let the model name an entry TYPE - market, limit or stop - and a price, so it
can say "wait for 4331" instead of taking the next open or standing aside. The
order still fills by the desk's rules, from the desk's tick feed, and expires
unfilled after the bars the model named; `plan-trigger` additionally asks the
model, once a minute while the order waits, whether to fill it now, cancel it,
or leave it - a narrow question, sent with thinking off, on a prompt whose
first six kilobytes are byte-identical to the decision so the provider serves
them from cache. The coin control takes the same entry type at mirrored
distances and never gets the fast question.

**The coin is not decoration.** By default every decision drives two books:
the model's side into `--run`, and a seeded coin flip into `--control`. If the
control is unreachable the model's trade is NOT posted either — a campaign
that quietly loses its control is a campaign that can only produce a number
nobody can read.

`--no-control` turns that off, and the owner asked for it on 2026-09-17. It
is a real loss, stated here rather than softened: without the coin there is
nothing to read the P&L against. Every long-gold book made money in a week
gold rose, and the coin is what separates "the model was right" from "the
market went that way". A campaign run this way can say what it earned; it
cannot say whether the model earned it.
"""

from __future__ import annotations

import argparse
import datetime as dt
import io
import json
import os
import random
import sys
import time
import urllib.error
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
# One model adapter, one panel, one place where a provider's quirks live.
from advisor import PROVIDERS, ask, provider_of  # noqa: E402
# Options-flow positioning, used by the `otl-context` variant only. Imported
# unconditionally so a broken import fails at start rather than at the first
# bar of a campaign that has already been launched.
import otl_context  # noqa: E402
# Higher-timeframe facts, used by the `htf-context` and `htf-filter` variants
# only. Imported unconditionally for the same reason as the line above: a
# broken import should stop a process at start, not at the first bar it was
# needed on.
import htf_context  # noqa: E402
# Structural price levels, used by the `smc-context` variant only. Imported
# unconditionally for the same reason as the two lines above.
import smc_context  # noqa: E402

# Output is UTF-8, and undisplayable characters are replaced rather than fatal.
#
# Windows hands a redirected stdout the cp1252 codec, and the text below is not
# ours - it is whatever a model wrote in its reasoning, or whatever a broker put
# in a comment. On 2026-09-17 a single arrow in a DeepSeek stand-aside raised
# UnicodeEncodeError out of the print, out of main(), and killed the trader for
# five bars. `errors="replace"` is the important half: encoding can then never
# be the thing that stops a process, whatever arrives.
for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")
    except (AttributeError, ValueError):  # not a TextIOWrapper; nothing to do
        pass

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))

# The one sentence that differs between prompt variants, quoted in full in both
# forms so the difference is readable here rather than reconstructed from a
# diff. See PROMPT_VARIANTS below for why there are two.
COIN_CLAUSE = {
    "base": (
        "You are being measured against a\n"
        "coin that takes the same trades at the same bars with a random side, so a trade you are not\n"
        "actually confident in is worse than no trade: it hands the coin a free sample."
    ),
    "no-coin-penalty": (
        "You are being measured against a\n"
        "coin that takes the same trades at the same bars with a random side."
    ),
}

# The prompt variants this trader can run, and the whole of what differs.
#
# WHY THERE IS A SECOND ONE. `ai-xau-opus-ctx` answered NONE on every bar it
# ever saw - 79 real answers out of 99 rows on 2026-09-17, the other 20 being
# CLI auth failures, with a coherent reason given each time and not one entry.
# On the same bars and the same prompt, deepseek-flash entered 7 of 77 and
# gpt-5.6-sol 6 of 94.
#
# The suspicion is the second half of the opening sentence. "A trade you are
# not actually confident in is worse than no trade" is an asymmetric penalty,
# and for a cautious instruction-following model it makes standing aside the
# literal-safe answer on every bar. A book that never trades measures nothing.
#
# `no-coin-penalty` removes that clause and adds NOTHING in its place. The coin
# is still named as the measurement, because it is; what goes is the
# instruction about what to conclude from it. No nudge toward trading, no base
# rate, no "you may be too cautious" - any of those would be a different
# experiment and the result would not say which half did the work.
#
# The base prompt is NOT edited and the running book is not touched. Changing a
# running book's prompt makes its record two campaigns wearing one id, which is
# the rule this desk already applies to models and accounts. The comparison
# between the two books IS the experiment; it is registered at
# docs/hypotheses/2026-09-17-prompt-coin-penalty.md.

# The variants, and the two axes a variant can move on.
#
# `coin` picks which form of the opening sentence it runs. `otl` adds the
# options-positioning block to MARKET CONTEXT and adds nothing else - it keeps
# the BASE coin clause on purpose, because a variant that changed two things
# would produce a result that could not say which one did the work.
#
# `otl-context` is registered at docs/hypotheses/2026-09-17-otl-context.md. It
# is a different claim from the twenty-five registrations that tested levels of
# this kind as mechanical rules and found nothing: those asked whether the
# levels predict price, this asks whether they change what a model decides.
#
# `htf` adds the higher-timeframe facts block to MARKET CONTEXT and nothing
# else. `htf_rule` adds ONE further sentence, and only on top of `htf`: the
# two differ by exactly that sentence, so whatever separates them is the
# sentence and cannot be anything else.
#
# WHY THE RULE KEYS ON THE STRUCTURE LABEL AND NOTHING ELSE. ADX and the
# efficiency ratio are in the block as facts and are deliberately NOT in the
# rule. An ADX gate measured over the last twelve months of H4 on the broker's
# own anchor is open most of the time, which makes it a filter that mostly
# does not filter, and the study behind this registration found that three of
# its four trend definitions changed sign or size when that anchor was
# corrected - the structure label being the one that did not. A rule keyed on
# the least anchor-sensitive definition is the only one whose result will mean
# something. Decision recorded by a5, 2026-09-18.
#
# `plan` changes the ANSWER SHAPE and nothing else: the same base prompt, the
# same coin clause, no options block, no higher-timeframe block, plus one
# block that replaces `{side, stop, target}` with a plan - an entry type
# (market, limit or stop), a price, a zone, a validity in bars and two
# invalidation levels. It is stage 1 of docs/plans/2026-09-18-staged-ai-entry.md
# and is registered at docs/hypotheses/2026-09-18-plan-entry.md. What it asks
# is whether a model that can say "wait for 4331" saves more at the entry than
# it loses in the trades that never fill; the market variant on the same model
# is its control, and the coin takes the same entry type at mirrored distances.
#
# `plan-trigger` is `plan` with the SAME prompt - the rendered text is
# byte-identical, and the selftest pins that - plus a second, fast question
# asked while a limit or stop is waiting: every `--fast-poll` seconds the
# decision prompt is re-sent unchanged with the one-minute bars since the
# decision appended, and the model answers TRIGGER, WAIT or CANCEL with
# thinking off. Stage 2, registered at docs/hypotheses/2026-09-18-plan-trigger.md.
# The comparison is against `plan`, whose orders fill by rule alone: does a
# model at the trigger add anything, or only cost?
#
# `smc` adds the structural price levels block to MARKET CONTEXT and adds
# nothing else - the BASE coin clause again, for the reason `otl` gives. It is
# the THIRD variant on the same control, after `otl-context` and
# `htf-context`, and its registration
# (docs/hypotheses/2026-09-18-smc-context.md) says so out loud before any of
# the three reports: three variants each given a 10% disagreement gate are
# three chances at a false positive, and the 2026-09-18 bias study measured
# the largest z across 34 tests at +1.98 where noise alone predicts +1.94.
#
# It is a different claim from the registrations that tested levels of this
# family as mechanical rules and closed - the full ICT chain at PF 0.75-0.77
# over 613-1,428 out-of-sample trades, and yesterday's high and low at the
# 1st-6th percentile of its own null. Those asked whether the levels predict
# price. This asks whether a model that can see them decides differently,
# which can be true whether or not they predict anything.
VARIANTS = {
    "base":            {"coin": "base",            "otl": False, "htf": False, "htf_rule": False, "smc": False, "plan": False, "trigger": False},
    "no-coin-penalty": {"coin": "no-coin-penalty", "otl": False, "htf": False, "htf_rule": False, "smc": False, "plan": False, "trigger": False},
    "otl-context":     {"coin": "base",            "otl": True,  "htf": False, "htf_rule": False, "smc": False, "plan": False, "trigger": False},
    "htf-context":     {"coin": "base",            "otl": False, "htf": True,  "htf_rule": False, "smc": False, "plan": False, "trigger": False},
    "htf-filter":      {"coin": "base",            "otl": False, "htf": True,  "htf_rule": True,  "smc": False, "plan": False, "trigger": False},
    "smc-context":     {"coin": "base",            "otl": False, "htf": False, "htf_rule": False, "smc": True,  "plan": False, "trigger": False},
    "plan":            {"coin": "base",            "otl": False, "htf": False, "htf_rule": False, "smc": False, "plan": True,  "trigger": False},
    "plan-trigger":    {"coin": "base",            "otl": False, "htf": False, "htf_rule": False, "smc": False, "plan": True,  "trigger": True},
}

# The whole of what `htf-filter` adds to `htf-context`, quoted here in full so
# the difference between the two books is readable in one place rather than
# reconstructed from a diff.
#
# It is one sentence and it names its own null behaviour, because a rule that
# said only "do not trade against the structure" would leave RANGE, an absent
# label and an unreachable route undefined - and a model resolving that
# silently, three different ways on three different bars, is a book measuring
# a mixture. The clause that keeps both sides open in those states is what
# makes the variant's trades attributable to the rule.
#
# It is also the only place in either prompt that contradicts the MARKET
# CONTEXT header's "none of it is a signal", which is why it says so out loud
# instead of leaving a model to reconcile the two.
HTF_RULE = (
    "ONE ADDITIONAL RULE FOR THIS BOOK - unlike the context above, this one is binding: do not\n"
    "propose LONG while the H4 swing structure label reads DOWN and do not propose SHORT while it\n"
    "reads UP; on RANGE, or whenever that label is absent, thin, stale or unavailable, both sides\n"
    "stay open."
)
# The whole of what `plan` adds to base, quoted in full for the same reason
# HTF_RULE is. It REPLACES the answer shape rather than extending it, and says
# so in its first sentence: a block that only added fields would leave the
# model two contracts to choose between, and a book measuring a mixture of
# the two is the failure every registration here is written to avoid.
#
# Every rule the shape needs is in the block and none is left to the model:
# which side of the last close a limit sits on, which side a stop sits on,
# what happens after `valid_bars`, and that a missed fill is COUNTED. That
# last one matters for the record - `cancelled_unfilled` rows are how missed
# winners are measured, and a model told its unfilled orders simply vanish
# would price them as free.
#
# No sentence about WHEN to prefer a limit over a market entry. That is the
# question the campaign asks the model; a hint here would be the answer
# leaking into the prompt.
PLAN_BLOCK = (
    "THIS BOOK ANSWERS WITH A PLAN, NOT AN ORDER. The answer shape above is replaced by the one below;\n"
    "every other rule stands, including that NONE is a real answer:\n"
    "\n"
    '  {"side": "LONG" | "SHORT" | "NONE",\n'
    '   "entry": {"type": "market" | "limit" | "stop", "price": <price or null>},\n'
    '   "zone": [<lo>, <hi>] or null, "stop": <price>, "target": <price or null>,\n'
    '   "valid_bars": <1 to 4>, "invalidate_above": <price or null>, "invalidate_below": <price or null>,\n'
    '   "reason": "<one sentence, under 200 characters, naming what in the bars above you are acting on>"}\n'
    "\n"
    'A "market" entry fills at the OPEN of the next bar as before, and its "price" is null. A "limit"\n'
    "waits for the price to come back to it: below the last close for a LONG, above it for a SHORT. A\n"
    '"stop" waits for the price to go through it: above the last close for a LONG, below it for a SHORT.\n'
    'A limit or stop still unfilled after "valid_bars" bars (1 to 4) is cancelled and counted as a missed\n'
    "trade, never as a stand-aside. For a limit or stop the stop must be on the losing side of the ENTRY\n"
    'price and the target on the winning side, or the plan is refused. "zone" is the price band inside\n'
    'which the entry is still worth taking; "invalidate_above" and "invalidate_below" are levels that\n'
    "cancel the pending entry if the price reaches them first. The zone and the invalidation levels may\n"
    "be null; the entry type may not."
)

# The fast question, asked by `plan-trigger` while a limit or stop waits. It
# is APPENDED to the decision prompt, which is sent again byte for byte: the
# provider's cache is keyed on an exact prefix (see PROMPT below), so the
# whole decision - rules, context, forty bars - costs one fiftieth the second
# time and only this tail is fresh. `{plan}` is the model's own answer, which
# it has otherwise no way of knowing: the prompt carries the question, not
# the reply, and a model asked to trigger a plan it cannot see would guess.
TRIGGER_BLOCK = """

SINCE THAT DECISION

YOUR PLAN from the bars above, as the desk holds it: {plan}
It fills by itself when the price reaches the entry, expires unfilled after the bars you named, and is
cancelled if an invalidation level is reached first. None of that needs you. This question does.

ONE-MINUTE BARS closed since the last bar above, oldest first, times UTC ({m1_state}):
{m1_bars}
{quote}

Answer with JSON and nothing else:

  {{"action": "TRIGGER" | "WAIT" | "CANCEL", "reason": "<one sentence, under 200 characters>"}}

TRIGGER fills the plan NOW at the current quote instead of waiting for the entry price. WAIT leaves the
order to its own rules. CANCEL withdraws it. WAIT is the right answer most of the time."""

PROMPT_VARIANTS = tuple(VARIANTS)

PROMPT_LAYOUT = "cache-v2"

# How stale the quote may be before the fast loop stops asking.
#
# The 15-minute decision needs no such guard: it fires on a NEW CLOSED BAR,
# and a shut market produces none. Measured on this desk 2026-09-18 over
# every row on disk - the 21:00-22:00Z daily break has exactly ZERO rows of
# any kind, every day, while its neighbours have forty to seventy.
#
# The fast loop has no such anchor. It fires while an order RESTS, on a wall
# clock, and an order's expiry is counted in closed bars - so an order left
# resting at the Friday close cannot expire until Sunday, and the loop would
# ask the model about a frozen chart every sixty seconds for forty-eight
# hours: about 2,880 calls, and 2,880 `trigger_check` rows drowning whatever
# the campaign actually measured. A poller outage mid-week does the same
# thing on a smaller scale.
#
# So the loop asks only while quotes are arriving. Three minutes is three
# missed minute bars - long enough not to fire on a thin hour, short enough
# that the weekend is caught in the first three minutes of it.
FAST_STALE_MS = 180_000

# ORDER IS DELIBERATE, AND IT IS NOT THE READING ORDER. Cost, not clarity,
# decides it: DeepSeek serves an exact prefix of a previous request from cache
# at one fiftieth of the fresh input price, and it is an EXACT prefix - the
# first byte that differs ends the hit. So the text that never changes comes
# first (the rules and the answer shape), then what changes hourly at most
# (the higher-timeframe and options blocks), then what changes every bar
# (context, desk state, position), and the bars themselves last. Under the
# old order the desk block sat third, its equity and spread differed every
# call, and the hit rate measured 1%. The sentences are the previous layout's
# sentences, unedited; rows carry `prompt_layout` so the two can be told
# apart in the record (stage 4 of docs/plans/2026-09-18-staged-ai-entry.md).
PROMPT = """You are trading one paper book on {market} {tf} bars. {coin_clause}

You may only answer in one of two ways: propose ONE trade, or stand aside.

You cannot choose a size — the desk sizes every book identically at one percent of equity against
the trailing range. You cannot act on the bar below; whatever you propose fills at the OPEN of the
next bar. The desk owns the maximum hold, and it will close the position if your stop or target is
not hit first.

Answer with JSON and nothing else:

  {{"side": "LONG" | "SHORT" | "NONE", "stop": <price or null>, "target": <price or null>,
    "reason": "<one sentence, under 200 characters, naming what in the bars above you are acting on>"}}

"NONE" is a real answer and is often the right one. If you propose a trade, the stop must be on the
losing side of the last close and the target on the winning side, or it will be refused.
{plan_block}{htf_rule_block}{htf_block}{otl_block}{smc_block}

MARKET CONTEXT — computed from the same bars, for convenience; none of it is a signal
{context}

THE DESK'S STATE — these are the rules you are already playing under, not advice
{desk}

{position}

LAST {n} BARS of {market}:{tf}, oldest first, times UTC
{bars}"""


def read_limits(api: str = "") -> dict:
    """The guard numbers, as the engine will actually apply them.

    Parsed rather than hard-coded: a limit quoted to the model that does not
    match the one enforced would be worse than saying nothing, because the
    model would plan around a rule that is not the rule.

    That is exactly what happened on 2026-09-17. This read only
    `config/default.toml` and ran once at startup, so it knew nothing about
    `config/guards.toml` - the file the desk writes when a guard is changed
    from the Settings screen. The owner raised the daily trade cap from 4 to
    10, the engine honoured it immediately, and the model went on declining
    every setup because its prompt still said four and said the four were
    spent. The guard was never reached: the model refused first.

    So the API is asked, because the API is the thing that enforces. Its
    `effective` block is the file's values with the desk's edits on top -
    the same numbers the book is checked against. The file stays as the
    fallback for a desk that is not answering, since a slightly stale limit
    beats no limit at all in the prompt.
    """
    if api:
        try:
            with urllib.request.urlopen(f"{api}/api/paper/guards", timeout=10) as r:
                view = json.loads(r.read().decode("utf-8", "replace"))
            eff = view.get("effective") or {}
            if eff:
                # Milliseconds are what the prompt builder reads for the two
                # duration limits; the API reports minutes, as a person types
                # them.
                out = {k: float(v) for k, v in eff.items() if isinstance(v, (int, float))}
                if "cooldown_min" in out:
                    out["cooldown_ms"] = out["cooldown_min"] * 60_000.0
                return out
        except Exception:  # noqa: BLE001 - any failure falls through to the file
            pass

    path = os.path.join(ROOT, "config", "default.toml")
    out = {}
    try:
        text = io.open(path, encoding="utf-8").read()
    except OSError:
        return out
    block = text.split("[trading.guards]", 1)
    if len(block) < 2:
        return out
    for line in block[1].splitlines():
        line = line.split("#", 1)[0].strip()
        if line.startswith("["):
            break
        if "=" in line:
            k, v = (x.strip() for x in line.split("=", 1))
            try:
                out[k] = float(v.replace("_", ""))
            except ValueError:
                pass
    return out


def ema(values: list, period: int):
    if len(values) < period:
        return None
    k = 2.0 / (period + 1)
    e = sum(values[:period]) / period
    for v in values[period:]:
        e = v * k + e * (1 - k)
    return e


def rsi(values: list, period: int = 14):
    if len(values) <= period:
        return None
    gains, losses = [], []
    for a, b in zip(values, values[1:]):
        d = b - a
        gains.append(max(d, 0.0))
        losses.append(max(-d, 0.0))
    ag = sum(gains[:period]) / period
    al = sum(losses[:period]) / period
    for g, l in zip(gains[period:], losses[period:]):
        ag = (ag * (period - 1) + g) / period
        al = (al * (period - 1) + l) / period
    if al == 0:
        return 100.0
    return 100.0 - 100.0 / (1.0 + ag / al)


def session_of(hour: int) -> str:
    """New York hours are what the desk's own filters are written in."""
    if 0 <= hour < 7:
        return "Asia"
    if 7 <= hour < 12:
        return "London"
    if 12 <= hour < 21:
        return "New York"
    return "late/rollover"


def desk_block(detail: dict, limits: dict, atr, run: str) -> str:
    """What the desk knows about this book and had been keeping to itself.

    Every line here is a rule already being enforced. The model was proposing
    trades into a daily cap it had already hit — four of them on 2026-09-16 —
    and setting stops without being told the volatility unit it is sized
    against. None of this is new market information and none of it adds a
    degree of freedom; it is the rulebook.
    """
    r = detail.get("run") or {}
    live = detail.get("live") or {}
    lines = []

    equity = r.get("equity")
    net = r.get("net_usd")
    lines.append(f"- equity ${equity:,.2f}, net {net:+,.2f} since this book started, {r.get('trades', 0)} closed trades")

    if atr:
        risk = atr * 1.2  # [trading] stop_atr
        lines.append(f"- ATR(14) is {atr:.2f}. The desk sizes on 1.2 x ATR, so 1R is about {risk:.2f} in price.")
        lines.append("  A stop much tighter than that will be mostly noise; much wider and your size shrinks.")

    spread = live.get("spread")
    if spread:
        lines.append(f"- spread right now {spread:.2f} (about {100 * spread / max(1e-9, atr * 1.2):.1f}% of 1R per round trip)" if atr
                     else f"- spread right now {spread:.2f}")

    cap = int(limits.get("max_trades_per_day", 0) or 0)
    refused = (r.get("skipped_by_guard") or {}).get("DAILY_TRADE_CAP", 0)
    if cap:
        lines.append(f"- the desk allows {cap} trades a day on this book."
                     + (f" It has already REFUSED {refused} of your proposals for hitting that cap." if refused else ""))
    if r.get("sized_down"):
        lines.append(f"- {r['sized_down']} of your trades were sized DOWN by the notional cap "
                     f"({int(limits.get('max_notional_pct_equity', 0))}% of equity).")
    loss = limits.get("daily_loss_limit_usd")
    if loss:
        lines.append(f"- the book stops trading for the day at ${loss:,.0f} of realised loss.")
    cooldown = limits.get("cooldown_ms")
    if cooldown:
        lines.append(f"- there is a {int(cooldown / 60000)} minute cooldown after every trade.")
    hold = limits.get("max_hold_ms") or 14_400_000
    lines.append(f"- maximum hold is {int(hold / 3600000)} hours; the desk closes the position then whatever the price.")

    news = r.get("news") or {}
    nb = news.get("next_blackout")
    if nb:
        mins = (nb["time"] - time.time() * 1000) / 60000
        when = "IN PROGRESS" if -30 <= mins <= 60 else f"in {mins:,.0f} minutes"
        lines.append(f"- next scheduled release: {nb.get('name')} ({nb.get('currency')}, impact {nb.get('impact')}) {when}.")
        lines.append("  The desk goes flat from 60 minutes before to 30 minutes after, and will refuse an entry inside that window.")
    return chr(10).join(lines)


def price_profile(bars: list, buckets: int = 24) -> list:
    """Where the session spent its activity, by price.

    **This is not a volume profile and must not be called one.** The `volume`
    column on a Vantage CFD bar is TICK volume — the number of price changes —
    and the parquet metadata says so. There are no traded contracts to profile.
    What this measures is how much price ACTIVITY happened at each level, which
    is a different and weaker thing, and the prompt says which it is.

    Each bar's activity is spread evenly across the range it covered, which is
    the honest approximation available from OHLC: the bar does not say where
    inside its range the ticks fell.
    """
    lo = min(b[3] for b in bars)
    hi = max(b[2] for b in bars)
    if not (hi > lo):
        return []
    step = (hi - lo) / buckets
    hist = [0.0] * buckets
    for b in bars:
        activity = b[5] if len(b) > 5 and b[5] else 1.0
        first = max(0, min(buckets - 1, int((b[3] - lo) / step)))
        last = max(0, min(buckets - 1, int((b[2] - lo) / step)))
        span = last - first + 1
        for i in range(first, last + 1):
            hist[i] += activity / span
    return [(lo + (i + 0.5) * step, hist[i]) for i in range(buckets)]


def profile_block(bars: list) -> str:
    """POC and the 70% value area, from the activity profile."""
    prof = price_profile(bars)
    if not prof:
        return ""
    total = sum(v for _, v in prof)
    if total <= 0:
        return ""
    poc_i = max(range(len(prof)), key=lambda i: prof[i][1])
    lo_i = hi_i = poc_i
    got = prof[poc_i][1]
    # Grow outward from the POC, always toward the busier side, until 70% is in.
    while got < 0.70 * total and (lo_i > 0 or hi_i < len(prof) - 1):
        down = prof[lo_i - 1][1] if lo_i > 0 else -1.0
        up = prof[hi_i + 1][1] if hi_i < len(prof) - 1 else -1.0
        if up >= down:
            hi_i += 1
            got += up
        else:
            lo_i -= 1
            got += down
    last = bars[-1][4]
    where = "inside" if prof[lo_i][0] <= last <= prof[hi_i][0] else ("above" if last > prof[hi_i][0] else "below")
    return (
        "- activity profile over the window (TICK activity, not traded contracts — a CFD has no\n"
        "  real volume; each bar's ticks are spread evenly across its range):\n"
        f"    point of control {prof[poc_i][0]:.2f} — the price level with the most activity\n"
        f"    70% value area {prof[lo_i][0]:.2f} to {prof[hi_i][0]:.2f}; last close is {where} it"
    )


def context_block(bars: list) -> str:
    """Ordinary technical context, computed from the same bars.

    Explicitly labelled as convenience rather than signal. The desk has closed
    thirty-two registrations and none of these indicators survived out of
    sample on this instrument at this timeframe; they are here because the
    owner asked for them, and because a forward test with a declared falsifier
    is not harmed by extra inputs the way a parameter sweep is.
    """
    closes = [b[4] for b in bars]
    highs = [b[2] for b in bars]
    lows = [b[3] for b in bars]
    last = closes[-1]
    lines = []

    for period in (21, 55):
        e = ema(closes, period)
        if e:
            lines.append(f"- EMA({period}) {e:.2f} — price is {'above' if last > e else 'below'} it by {abs(last - e):.2f}")
    r = rsi(closes)
    if r is not None:
        lines.append(f"- RSI(14) {r:.1f}")

    # Sessions and day levels, in UTC because the bars are.
    now = dt.datetime.utcfromtimestamp(bars[-1][0] / 1000)
    lines.append(f"- session: {session_of(now.hour)} (bar stamped {now:%H:%MZ})")

    today = now.date()
    td = [b for b in bars if dt.datetime.utcfromtimestamp(b[0] / 1000).date() == today]
    if td:
        lines.append(f"- today so far: high {max(b[2] for b in td):.2f}, low {min(b[3] for b in td):.2f}")
    prev = today - dt.timedelta(days=1)
    yd = [b for b in bars if dt.datetime.utcfromtimestamp(b[0] / 1000).date() == prev]
    if yd:
        lines.append(f"- previous day: high {max(b[2] for b in yd):.2f}, low {min(b[3] for b in yd):.2f}")

    lines.append(f"- range of the window shown: high {max(highs):.2f}, low {min(lows):.2f}")

    block = profile_block(bars)
    if block:
        lines.append(block)

    # A higher timeframe, aggregated from the same bars so it cannot disagree
    # with them.
    hourly = {}
    for t, o, h, l, c in bars:
        k = t - (t % 3_600_000)
        if k not in hourly:
            hourly[k] = [o, h, l, c]
        else:
            hourly[k][1] = max(hourly[k][1], h)
            hourly[k][2] = min(hourly[k][2], l)
            hourly[k][3] = c
    keys = sorted(hourly)[-8:]
    if len(keys) >= 4:
        lines.append("- last hours (1h, aggregated from these bars):")
        for k in keys:
            o, h, l, c = hourly[k]
            lines.append(f"    {dt.datetime.utcfromtimestamp(k / 1000):%m-%d %H:%MZ}  O {o:g} H {h:g} L {l:g} C {c:g}")
    return chr(10).join(lines)


HOLD_PROMPT = """You are watching ONE open position on {market} {tf} bars. You did not necessarily
open it and you cannot add to it, reverse it, or move its stop.

You are being asked one question and nothing else: **has the reason for this trade broken?**

{position_detail}

YOUR ANSWER IS RECORDED, NOT ACTED ON. The desk still owns the exit: the stop, the target and the
maximum hold close this position, not you. Nothing you say here changes the book. It is being
collected to find out whether a model can tell, ahead of the stop, that a trade has stopped
working — and if that turns out to be true it becomes a separate campaign with its own control.
So answer as you would if it counted, and do not hedge to look safe.

MARKET CONTEXT
{context}

LAST {n} BARS of {market}:{tf}, oldest first, times UTC
{bars}

Answer with JSON and nothing else:

  {{"action": "HOLD" | "CLOSE", "reason": "<one sentence, under 200 characters, naming what in the
    bars above changed your mind or did not>"}}

"HOLD" is the right answer most of the time. Say CLOSE only when the thing that made this trade
worth taking is no longer there."""


def describe_open(detail: dict, rules_atr) -> str:
    """The position as the desk sees it, for the hold-or-close question."""
    o = (detail.get("run") or {}).get("open") or {}
    if not o:
        return "No position."
    run = detail.get("run") or {}
    last = run.get("last_bar_close")
    bars_held = ""
    if o.get("entry_time") and run.get("last_bar_time"):
        step = {"1m": 60_000, "5m": 300_000, "15m": 900_000, "1h": 3_600_000}.get(run.get("tf"), 900_000)
        bars_held = f", held {max(0, (run['last_bar_time'] - o['entry_time']) // step)} bars"
    return (
        f"YOU ARE {o['side']} from {o['entry_price']}, {o.get('lots')} lots{bars_held}.\n"
        f"  stop {o.get('stop')} · target {o.get('target')} · last close {last}\n"
        f"  unrealised {o.get('unrealised_usd_at_last_close'):+.2f} USD"
        f" · worst so far {o.get('mae'):+.2f}R · best so far {o.get('mfe'):+.2f}R"
    )


def parse_hold(text: str) -> dict:
    """HOLD or CLOSE. Anything unreadable is HOLD — the conservative reading,
    and the one that matches what the desk will do anyway."""
    start, end = text.find("{"), text.rfind("}")
    if start < 0 or end <= start:
        return {"action": "HOLD", "reason": f"unreadable reply: {text[:100]!r}"}
    try:
        obj = json.loads(text[start : end + 1])
    except (ValueError, TypeError) as e:
        return {"action": "HOLD", "reason": f"unreadable reply: {e}"}
    action = str(obj.get("action", "HOLD")).upper()
    return {
        "action": "CLOSE" if action == "CLOSE" else "HOLD",
        "reason": str(obj.get("reason", ""))[:200],
    }


def post_json(url: str, payload: dict, timeout: float = 20.0) -> dict:
    req = urllib.request.Request(
        url, data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json"}, method="POST",
    )
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read().decode("utf-8"))


def get_json(url: str, timeout: float = 20.0):
    with urllib.request.urlopen(url, timeout=timeout) as r:
        return json.loads(r.read().decode("utf-8"))


def describe_position(detail: dict) -> str:
    open_pos = (detail.get("run") or {}).get("open")
    if not open_pos:
        return "YOU HAVE NO POSITION. The desk allows one at a time."
    return (
        f"YOU ARE ALREADY {open_pos['side']} at {open_pos['entry_price']}, "
        f"unrealised {open_pos['unrealised_usd_at_last_close']:+.2f}. "
        "The desk allows one position at a time, so a new trade is not possible until this one "
        "closes. Answer NONE."
    )


def parse(text: str, plan: bool = False) -> dict:
    """The decision, or a stand-aside.

    An unreadable reply is NOT a trade. Every other failure mode in this
    repository resolves toward doing nothing, and a parser that guessed a side
    would be the one place a bug could put on a position nobody chose.

    `plan` reads the plan fields too (see PLAN_BLOCK). Off by default so the
    market books' rows keep exactly the shape they have always had.
    """
    start, end = text.find("{"), text.rfind("}")
    if start < 0 or end <= start:
        return {"side": "NONE", "reason": f"unreadable reply, stood aside: {text[:120]!r}"}
    try:
        obj = json.loads(text[start : end + 1])
    except (ValueError, TypeError) as e:
        return {"side": "NONE", "reason": f"unreadable reply, stood aside: {e}"}
    side = str(obj.get("side", "NONE")).upper()
    if side not in ("LONG", "SHORT"):
        return {"side": "NONE", "reason": str(obj.get("reason", ""))[:200]}
    out = {
        "side": side,
        "stop": obj.get("stop"),
        "target": obj.get("target"),
        "reason": str(obj.get("reason", ""))[:200],
    }
    if plan:
        # The plan fields are READ here and JUDGED in `sane`: this keeps
        # what the model said, in the shape the poster needs, and refuses
        # nothing. `entry` is always present on a plan decision, even when
        # the reply had none, so `sane` can name the omission instead of a
        # market fill quietly standing in for an entry type nobody chose.
        raw = obj.get("entry")
        if isinstance(raw, dict):
            kind = raw.get("type")
            entry = {"type": str(kind).strip().lower() if kind is not None else None,
                     "price": raw.get("price")}
        elif isinstance(raw, str):
            # `"entry": "limit"` with the price beside it is a shape a model
            # produces often enough to read; the price is then wherever the
            # model put it, and `sane` refuses a limit that names none.
            entry = {"type": raw.strip().lower(), "price": obj.get("entry_price", obj.get("price"))}
        else:
            entry = {"type": None, "price": None}
        out["entry"] = entry
        out["zone"] = obj.get("zone")
        # Absent means the API's own default of two bars, and the row then
        # carries the 2 the intent carried rather than a null the reader
        # would have to resolve against the API's code. Anything the model
        # DID say is kept as said; `sane` refuses what is not 1 to 4.
        valid = obj.get("valid_bars")
        if valid is None:
            valid = 2
        else:
            try:
                valid = int(valid) if float(valid).is_integer() else valid
            except (TypeError, ValueError):
                pass
        out["valid_bars"] = valid
        out["invalidate_above"] = obj.get("invalidate_above")
        out["invalidate_below"] = obj.get("invalidate_below")
    return out


def _price(v):
    """A float, or None for null; raises on anything else."""
    return None if v is None else float(v)


def sane(decision: dict, last_close: float) -> tuple[bool, str]:
    """A stop on the losing side and a target on the winning side, or no trade.

    Not a judgement about the trade — a check that the numbers mean what the
    words say. A LONG whose stop is above the price is not a bold trade, it is
    a reply that did not understand the question, and posting it would put a
    position on the book for a reason nobody held.

    A plan decision (one carrying `entry`) is held to the same idea with the
    reference point moved: a limit sits on the pullback side of the last
    close and a stop on the breakout side, and the exit levels are then judged
    against the ENTRY price, because that is where the trade would start. A
    market entry keeps the last close as its reference, so a plan variant
    answering "market" is checked exactly as the market books are.
    """
    stop, target = decision.get("stop"), decision.get("target")
    if stop is None:
        return False, "no stop"
    try:
        stop = float(stop)
        target = float(target) if target is not None else None
    except (TypeError, ValueError):
        return False, "stop or target not a number"
    side = decision["side"]
    long = side == "LONG"
    ref = last_close
    entry = decision.get("entry")
    if entry is not None:
        kind = entry.get("type")
        if kind not in ("market", "limit", "stop"):
            return False, f"entry type {kind!r} is not market, limit or stop"
        if kind != "market":
            try:
                price = _price(entry.get("price"))
            except (TypeError, ValueError):
                return False, f"{kind} entry price {entry.get('price')!r} is not a number"
            if price is None:
                return False, f"a {kind} entry names no price"
            if kind == "limit" and ((long and price >= last_close) or (not long and price <= last_close)):
                return False, f"limit {price} is not on the pullback side of {last_close} for a {side}"
            if kind == "stop" and ((long and price <= last_close) or (not long and price >= last_close)):
                return False, f"stop entry {price} is not on the breakout side of {last_close} for a {side}"
            valid = decision.get("valid_bars")
            if isinstance(valid, bool) or not isinstance(valid, int) or not 1 <= valid <= 4:
                return False, f"valid_bars {valid!r} is not a whole number from 1 to 4"
            ref = price
        zone = decision.get("zone")
        if zone is not None:
            try:
                ok_zone = (isinstance(zone, (list, tuple)) and len(zone) == 2
                           and float(zone[0]) <= float(zone[1]))
            except (TypeError, ValueError):
                ok_zone = False
            if not ok_zone:
                return False, f"zone {zone!r} is not [lo, hi]"
        for key in ("invalidate_above", "invalidate_below"):
            try:
                _price(decision.get(key))
            except (TypeError, ValueError):
                return False, f"{key} {decision.get(key)!r} is not a price"
    if (long and stop >= ref) or (not long and stop <= ref):
        return False, f"stop {stop} is on the winning side of {ref} for a {side}"
    if target is not None and ((long and target <= ref) or (not long and target >= ref)):
        return False, f"target {target} is on the losing side of {ref}"
    return True, ""


def plan_body(decision: dict) -> dict:
    """The plan fields of an accepted decision, as the intent route takes them.

    Everything numeric is a float and `valid_bars` is null for a market entry:
    the API applies validity to limit and stop orders only, and sending a
    number it would ignore invites a reader to think it meant something.
    """
    entry = decision["entry"]
    market = entry["type"] == "market"
    zone = decision.get("zone")
    return {
        "entry": {"type": entry["type"], "price": None if market else float(entry["price"])},
        "zone": None if zone is None else [float(zone[0]), float(zone[1])],
        "valid_bars": None if market else int(decision["valid_bars"]),
        "invalidate_above": _price(decision.get("invalidate_above")),
        "invalidate_below": _price(decision.get("invalidate_below")),
    }


def reflect(price: float, ref: float) -> float:
    """The same distance from `ref`, on the other side of it.

    Written as `ref - (p - ref)` above and `ref + (ref - p)` below rather than
    `2 * ref - p`: those are the two expressions the market books' coin has
    always used for its stop, and a plan book's coin should land on the same
    float for the same input, not one that differs in the last place.
    """
    return ref - (price - ref) if price > ref else ref + (ref - price)


def coin_plan(decision: dict, flip: str, last_close: float) -> dict:
    """The coin's plan: the book's plan at mirrored distances from the last close.

    Same entry TYPE, same `valid_bars`, same zone width, same stop and target
    distances - so both books share the fill mechanics and the comparison
    stays fair. When the coin lands on the book's own side every price is
    taken as is; when it lands on the other side every price is reflected
    about the last close, and the two invalidation levels swap roles, because
    "above" for a LONG is "below" for the SHORT mirror of it.

    Not the market books' formula, on purpose. That one measures each level's
    distance from the last close and puts it on the coin's losing side, which
    is right when the entry IS the last close and wrong the moment it is not:
    a LONG stop-entry at +6 with its stop at +2 has a stop ABOVE the last
    close, and "the same distance on the losing side" would put the coin's
    stop at -2, four points further from its entry than the book's. Reflection
    keeps every distance measured from the entry, which is the one that sizes
    the trade.
    """
    same = flip == decision["side"]
    def m(p):
        return None if p is None else (float(p) if same else reflect(float(p), last_close))
    entry = decision["entry"]
    market = entry["type"] == "market"
    zone = decision.get("zone")
    if zone is None:
        c_zone = None
    else:
        lo, hi = m(zone[0]), m(zone[1])
        c_zone = [min(lo, hi), max(lo, hi)]
    above, below = decision.get("invalidate_above"), decision.get("invalidate_below")
    return {
        "side": flip,
        "stop": m(decision["stop"]),
        "target": m(decision.get("target")),
        "entry": {"type": entry["type"], "price": None if market else m(entry["price"])},
        "zone": c_zone,
        "valid_bars": None if market else int(decision["valid_bars"]),
        "invalidate_above": m(above) if same else m(below),
        "invalidate_below": m(below) if same else m(above),
    }


def describe_plan(decision: dict) -> str:
    """The plan in one line, for the fast question and the console."""
    e = decision.get("entry") or {}
    where = "at the next open" if e.get("type") == "market" else f"at {e.get('price')}"
    return (f"{decision['side']} {e.get('type')} {where}, stop {decision.get('stop')}, "
            f"target {decision.get('target')}, valid {decision.get('valid_bars')} bars, "
            f"zone {decision.get('zone')}, invalidate above {decision.get('invalidate_above')} / "
            f"below {decision.get('invalidate_below')}")


def m1_since(m1: dict, bar_time: int, bar_ms: int) -> tuple[str, str]:
    """The closed one-minute bars after the decision bar, and the feed's state.

    `bar_time` is the OPEN of the decision bar, so a minute counts once its
    own open is at or past that bar's close. The bars are rendered in the
    15m rows' own format, `:g` and all, so a number the model has already
    seen in the decision reads the same way here.
    """
    if not m1 or m1.get("unavailable"):
        return "  (the one-minute feed is unavailable)", "unavailable"
    rows = []
    for b in m1.get("bars") or []:
        t = b.get("time")
        if t is None or t < bar_time + bar_ms:
            continue
        if any(b.get(k) is None for k in ("open", "high", "low", "close")):
            # A minute the feed could not price is left out rather than
            # rendered as "None": the fast call must never die on the shape
            # of one bar, and a bar with no prices tells the model nothing.
            continue
        rows.append(
            f"  {dt.datetime.utcfromtimestamp(t / 1000):%Y-%m-%d %H:%MZ}  "
            f"O {b.get('open'):g}  H {b.get('high'):g}  L {b.get('low'):g}  C {b.get('close'):g}"
            + (f"  ({b.get('ticks')} ticks, mean spread {b.get('spread_mean'):.2f})"
               if b.get("ticks") is not None and b.get("spread_mean") is not None else "")
        )
    if not rows:
        return "  (none closed yet)", "ok, 0 bars"
    return chr(10).join(rows), f"ok, {len(rows)} bars"


def quote_line(live: dict, forming: dict) -> str:
    """The latest quote and the forming minute, or a plain statement that there is none."""
    live = live or {}
    parts = []
    if live.get("bid") is not None and live.get("ask") is not None:
        at = live.get("at")
        when = f" at {dt.datetime.utcfromtimestamp(at / 1000):%H:%M:%SZ}" if at else ""
        parts.append(f"quote now: bid {live['bid']:g}  ask {live['ask']:g}{when}")
    else:
        parts.append("quote now: none (the feed gave no bid/ask)")
    if forming and forming.get("open") is not None:
        parts.append(f"the forming minute so far: O {forming.get('open'):g}  H {forming.get('high'):g}  "
                     f"L {forming.get('low'):g}  C {forming.get('close'):g}")
    return chr(10).join(parts)


def fast_prompt(decision_prompt: str, decision: dict, m1: dict, live: dict,
                bar_time: int, bar_ms: int) -> str:
    """The decision prompt, byte for byte, then what has happened since."""
    bars, state = m1_since(m1, bar_time, bar_ms)
    return decision_prompt + TRIGGER_BLOCK.format(
        plan=describe_plan(decision), m1_state=state, m1_bars=bars,
        quote=quote_line(live, (m1 or {}).get("forming") or {}),
    )


def parse_trigger(text: str) -> dict:
    """TRIGGER, WAIT or CANCEL. Anything unreadable is WAIT - the answer that
    changes nothing, and the one the order's own rules give anyway."""
    start, end = text.find("{"), text.rfind("}")
    if start < 0 or end <= start:
        return {"action": "WAIT", "reason": f"unreadable reply: {text[:100]!r}"}
    try:
        obj = json.loads(text[start : end + 1])
    except (ValueError, TypeError) as e:
        return {"action": "WAIT", "reason": f"unreadable reply: {e}"}
    action = str(obj.get("action", "WAIT")).upper()
    return {
        "action": action if action in ("TRIGGER", "CANCEL") else "WAIT",
        "reason": str(obj.get("reason", ""))[:200],
    }


# Roll `decisions.jsonl` aside once it passes 32 MiB.
#
# The arithmetic, measured 2026-09-17:
# `data/paper/ai-xau-opus-ctx/decisions.jsonl` is 632,258 bytes over 99 lines
# — 6,386 bytes a line, because the whole prompt is in the line and that is
# the point of the file — written over 25.8 hours, so 24.5 KB an hour, 588 KB
# a day for one book at 15-minute bars. 32 MiB is therefore about 57 days.
#
# The threshold is not finely tuned and cannot be, because rolling saves no
# disk at all: nothing is ever removed, so the total on the VPS grows exactly
# as fast either way. What it bounds is the size of any ONE file — one that
# can be copied off the box in a single go, opened in an editor, or read whole
# by something that legitimately wants all of it. Anywhere between a month and
# a quarter would do; 57 days was picked because it makes the cost below
# something that happens twice a season rather than twice a month.
#
# That cost, plainly: right after a roll the Desk's reasoning panel is empty,
# because fd-api reads the tail of the live file and does not look at its
# predecessors. The first decision appears at the next bar, the default fifty
# take about half a day to come back, and for that half day the panel
# understates a campaign that has been running for months. Worth fixing on the
# Rust side if it ever annoys anyone; not worth blocking this on.
ROTATE_BYTES = 32 * 1024 * 1024


def roll_aside(path: str) -> None:
    """Move a full log out of the way, under a name that sorts by when.

    Not truncation and not pruning, and there is deliberately NO option to
    keep only the last N files. The owner's standing requirement is that the
    trading record reads as one unbroken thing from the first day, and a
    rotation that is able to delete is one that eventually will — on the night
    somebody wants the first week back. If more disk is ever genuinely needed,
    the answer is to move old files somewhere else by hand, which is a
    decision a person makes once, not a flag that quietly runs every day.

    `os.rename`, never `os.replace`. Rename refuses to overwrite on Windows;
    replace overwrites everywhere and would silently destroy an existing
    rolled file, which is the one outcome this function exists to prevent. The
    explicit `exists` check is there because POSIX `rename` DOES overwrite, so
    without it the guarantee would hold on the VPS and not on Linux, and the
    guarantee is the whole point.

    Failing to roll is acceptable; losing a line is not. So this runs BEFORE
    the append and never touches a byte of content: the new line lands in the
    fresh file, and a reader holding the old one open keeps reading it under
    its new name. If the rename is refused the file simply keeps growing and
    the next line tries again.
    """
    try:
        if os.path.getsize(path) < ROTATE_BYTES:
            return
        stamp = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
        stem, ext = os.path.splitext(path)
        rolled = f"{stem}-{stamp}{ext}"
        if os.path.exists(rolled):
            return
        os.rename(path, rolled)
    except OSError:
        # Every way this fails leaves the log exactly as it was: no file yet,
        # or a reader holding it open on Windows without FILE_SHARE_DELETE and
        # the rename refused. Swallowed as narrowly as `remember()` swallows
        # its own, and for the same reason — a decision must never depend on
        # housekeeping. Only OSError: a TypeError here would be a bug in this
        # function and should be loud.
        pass


def log(run: str, record: dict) -> None:
    """Prompt and reply, whole, beside the book they drove.

    Same shape and same reason as the advisor's log: a summary cannot be
    replayed against a changed prompt, and replay is the only way a mistake
    gets fixed rather than counted.
    """
    path = os.path.join(ROOT, "data", "paper", run, "decisions.jsonl")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    roll_aside(path)
    with open(path, "a", encoding="utf-8") as f:
        f.write(json.dumps(record) + chr(10))


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split(chr(10))[0])
    ap.add_argument("--api", default="http://127.0.0.1:8138")
    ap.add_argument("--run", default="ai-xau", help="the run that takes the model's side")
    ap.add_argument("--control", default="ai-xau-coin", help="the run that takes a coin's side")
    # Run the model's book with no coin beside it.
    #
    # The owner asked for this on 2026-09-17, having decided the control was
    # not earning its place. It is a real loss and the docstring above says so
    # rather than being quietly softened: without the coin, a campaign's P&L
    # cannot be separated from what the market did anyway - every long-gold
    # book made money in a week gold rose, and the coin is what says whether
    # the model beat that.
    #
    # It is a flag and not an empty --control because "no control" is a
    # decision that should be visible in the command line and in the log, not
    # an argument someone forgot to pass.
    ap.add_argument("--no-control", action="store_true",
                    help="drive only the model's book; no coin, and no comparison")
    ap.add_argument("--model", default="gpt-5")
    ap.add_argument("--provider", choices=sorted(PROVIDERS), default=None)
    ap.add_argument("--market", default="xauusd")
    ap.add_argument("--tf", default="15m")
    ap.add_argument("--bars", type=int, default=40, help="bars shown to the model one by one")
    ap.add_argument("--context-bars", type=int, default=240,
                    help="bars fetched for indicators, day levels and the hourly view")
    ap.add_argument("--poll", type=float, default=20.0)
    ap.add_argument("--timeout", type=float, default=90.0)
    ap.add_argument("--seed", type=int, default=7, help="the coin's seed, so the control replays")
    # Which prompt this campaign runs. `base` is the prompt every book has used
    # since the beginning, NAMED rather than left implicit: a row that does not
    # say which prompt produced it is a row nobody can compare later, and the
    # default having a name is what makes the old rows and the new ones talk to
    # each other. See PROMPT_VARIANTS for the difference and for the
    # registration.
    ap.add_argument("--prompt-variant", default="base", choices=PROMPT_VARIANTS,
                    help="which prompt wording to run; the record carries it per row")
    ap.add_argument("--dry-run", action="store_true", help="decide and log; post nothing")
    # The hold funnel (stage 3 of docs/plans/2026-09-18-staged-ai-entry.md).
    # `bar` is the old behaviour: while a position is open, the model is asked
    # "has the reason broken" on EVERY bar, each answer costing ~2k reasoning
    # tokens - 117 of them on 2026-09-18 for a quarter of the day's bill, on a
    # verdict that is recorded and never acted on. `event` asks only when
    # something has happened to the trade: price half a stop against it, a
    # full stop in its favour, or four bars since the last look as a floor so
    # a slow bleed is still seen hourly. What is skipped is logged as
    # `hold_skip` with the numbers that decided it, so the record still shows
    # every bar the position was held through.
    ap.add_argument("--hold-check", choices=("bar", "event"), default="event",
                    help="ask the hold question every bar, or only when price moved (default event)")
    ap.add_argument("--hold-thinking", choices=("default", "off", "low"), default="off",
                    help="reasoning on the hold question; off by default because the question is narrow")
    ap.add_argument("--decision-thinking", choices=("default", "off", "low", "high"), default="default",
                    help="reasoning on the entry decision; default leaves the provider's setting")
    # The fast loop's cadence, used by `plan-trigger` only. Sixty seconds
    # because that is one M1 bar - the finest thing the fast prompt shows -
    # so a faster loop would re-ask on the same bars. It cannot run faster
    # than `--poll`, which is what actually wakes the process; the check is
    # made on the first poll at or after the interval.
    ap.add_argument("--fast-poll", type=float, default=60.0,
                    help="seconds between trigger questions while a plan waits (plan-trigger only)")
    ap.add_argument("--once", action="store_true")
    args = ap.parse_args()

    try:
        provider = args.provider or provider_of(args.model)
    except ValueError as e:
        sys.exit(str(e))
    # A keyless provider reaches the model through the account's own plan
    # (the Claude Code CLI); only a metered one can be missing a key.
    # `key_for` looks in the environment and then in the gitignored
    # `config/local.toml`, so a metered provider works after a reboot without
    # anyone exporting anything.
    from advisor import key_for
    env = PROVIDERS[provider]["env"]
    key = key_for(provider)
    if env and not key:
        sys.exit(f"no {env} in the environment or config/local.toml for {args.model}")

    # Read once here only to fail loudly at startup if neither the desk nor
    # the file can be read; the prompt takes a fresh copy on every decision.
    limits = read_limits(args.api)
    if not limits:
        print("warning: no guard limits from the desk or config/default.toml; "
              "the prompt will not quote any", flush=True)
    coin = random.Random(args.seed)
    # Which bar was last decided, on disk. A restart used to forget, re-ask the
    # current bar, spend another model call on it and post a SECOND intent for
    # it — visible in the log as two entries for one bar, and it flipped the
    # coin twice for a single trade. Harmless to the books (the pending slot is
    # simply overwritten) but it pollutes the campaign's own record, which is
    # the only thing this campaign produces.
    mark_path = os.path.join(ROOT, "data", "paper", args.run, "decided_on.txt")

    def remember(t: int) -> None:
        try:
            os.makedirs(os.path.dirname(mark_path), exist_ok=True)
            with open(mark_path, "w", encoding="utf-8") as f:
                f.write(str(t))
        except OSError:
            pass  # a lost mark costs one duplicate call, never a wrong trade

    def recall() -> int | None:
        try:
            with open(mark_path, encoding="utf-8") as f:
                return int(f.read().strip())
        except (OSError, ValueError):
            return None

    def beat() -> None:
        """Say "still here" every poll, for both books this process drives.

        Until this existed, the only evidence a book was still being driven was
        a decision, and decisions arrive one per CLOSED BAR. So a trader that
        had been killed looked identical to one thinking about the current bar
        for up to two bars - half an hour on a 15m book - and the desk went on
        showing a stopped experiment as a running one.

        The poll is thirty seconds and independent of the market, so this makes
        the same question answerable in about a minute, and answerable at three
        in the morning with the market shut.

        Written for the control book too. The coin is driven by this same
        process and stops when it does; making the desk infer that from a
        naming convention works, but only the process actually knows.
        """
        now = int(time.time() * 1000)
        beats = (args.run,) if args.no_control else (args.run, args.control)
        for run in beats:
            if not run:
                continue
            path = os.path.join(ROOT, "data", "paper", run, "driver.json")
            tmp = path + ".tmp"
            try:
                os.makedirs(os.path.dirname(path), exist_ok=True)
                with open(tmp, "w", encoding="utf-8") as f:
                    json.dump({"at": now, "model": args.model, "run": args.run,
                               "pid": os.getpid(), "poll_s": args.poll}, f)
                os.replace(tmp, path)
            except (OSError, TypeError, ValueError):
                # A missed beat costs a book being called stopped for one poll.
                # It must never cost a decision.
                pass
    coin_says = "NO COIN (no control book)" if args.no_control else f"coin -> {args.control}"
    print(
        f"ai trader: {args.model} on {args.market}:{args.tf} -> {args.run}, "
        f"{coin_says}{' (dry run)' if args.dry_run else ''}",
        flush=True,
    )

    decided_on: int | None = recall()
    if decided_on is not None:
        print(f"resuming: bar {decided_on} was already decided", flush=True)
    # The hold funnel's memory: which position was last looked at, on which
    # bar, and whether its favourable trigger has already fired. Per process,
    # not persisted: a restart looks once more, which is the cheap side to err on.
    hold_state = {"last_bar": None, "entry_time": None, "favourable_seen": False, "trigger": ""}
    bar_ms = {"1m": 60_000, "5m": 300_000, "15m": 900_000, "1h": 3_600_000, "4h": 14_400_000}.get(args.tf, 900_000)
    is_plan = VARIANTS[args.prompt_variant]["plan"]
    is_trigger = VARIANTS[args.prompt_variant]["trigger"]
    # The plan this process posted and has not yet seen resolve: the exact
    # prompt string that produced it (the fast question re-sends it byte for
    # byte, so it is kept and never rebuilt), the decision, the bar, and when
    # the fast question was last asked. Per process and not persisted, on
    # purpose: a restart cannot reproduce the prompt - the desk state in it
    # has moved on - so it leaves the order to the rules, and says so once.
    pending_plan = None
    pending_seen = None  # the `decided_at` of a pending order this process did not post, warned once
    print(f"hold check: {args.hold_check}, thinking {args.hold_thinking}; decision thinking {args.decision_thinking}; "
          f"prompt layout {PROMPT_LAYOUT}"
          + (f"; plan answers, fast loop {'every %gs' % args.fast_poll if is_trigger else 'OFF (rule-only fill)'}"
             if is_plan else ""), flush=True)
    while True:
        beat()
        try:
            detail = get_json(f"{args.api}/api/paper/run/{args.run}?bars={args.context_bars}")
        except Exception as e:  # noqa: BLE001
            print(f"cannot read {args.run}: {type(e).__name__}: {e}", flush=True)
            if args.once:
                return 1
            time.sleep(args.poll)
            continue

        bars = detail.get("bars") or []
        if not bars:
            print("no bars yet; the poller has fed this run nothing", flush=True)
            if args.once:
                return 0
            time.sleep(args.poll)
            continue

        last_time, *_rest = bars[-1]
        last_close = bars[-1][4]

        # ---- the pending order, on EVERY poll and not only on a new bar ----
        #
        # `pending_order` is the desk's word on whether a limit or stop is
        # still waiting. The moment it reads null the order has filled,
        # expired, been invalidated, been cancelled or been replaced - the
        # book's fills.jsonl says which - and the memory of it goes, so a
        # later order this process did not post is never mistaken for it.
        pending = (detail.get("run") or {}).get("pending_order") if is_plan else None
        if pending_plan is not None and not pending:
            pending_plan = None
        if pending and is_trigger and pending_plan is None and pending.get("decided_at") != pending_seen:
            # An order from before this process started. Its prompt is gone
            # with the process that built it, and a rebuilt one would not be
            # the same bytes, so the fast question is not asked: the order
            # fills by rule, as the `plan` book's do, and the record shows
            # the gap as an absence of trigger_check rows for this order.
            pending_seen = pending.get("decided_at")
            print(f"pending order from before this process started ({pending.get('side')} "
                  f"{pending.get('type')} at {pending.get('price')}); rule-only fill until it resolves",
                  flush=True)
        # Quotes, not the clock, say whether there is anything to look at.
        # `live.at` is when the poller last read a tick for this stream.
        live_at = (detail.get("live") or {}).get("at")
        quote_age_ms = None if not live_at else max(0, int(time.time() * 1000) - int(live_at))
        feed_live = quote_age_ms is not None and quote_age_ms < FAST_STALE_MS
        if (pending and is_trigger and pending_plan is not None
                and not (detail.get("run") or {}).get("open")
                and not feed_live):
            # Said once per stale spell, and written once, so a shut market
            # leaves one row saying the order rested through it rather than
            # one row a minute saying nothing happened.
            if not pending_plan.get("stale_since"):
                pending_plan["stale_since"] = int(time.time() * 1000)
                age = "no quote at all" if quote_age_ms is None else f"{quote_age_ms / 1000:.0f}s old"
                print(f"{dt.datetime.now(dt.timezone.utc):%H:%M:%SZ} fast loop paused: quote is {age}; "
                      f"the order rests and fills by rule if the feed returns", flush=True)
                log(args.run, {
                    "at": int(time.time() * 1000), "bar_time": pending_plan["bar_time"], "model": args.model,
                    "prompt_variant": args.prompt_variant, "prompt_layout": PROMPT_LAYOUT,
                    "kind": "trigger_paused", "quote_age_ms": quote_age_ms, "pending": pending,
                    "decision": {"side": "NONE", "reason": "[PAUSED] the feed is stale; the model was not asked"},
                    "posted": False, "refused_locally": "", "dry_run": bool(args.dry_run),
                })
        elif (pending and is_trigger and pending_plan is not None
                and not (detail.get("run") or {}).get("open")
                and time.monotonic() - pending_plan["asked_at"] >= args.fast_poll):
            pending_plan["stale_since"] = None
            pending_plan["asked_at"] = time.monotonic()
            try:
                m1 = get_json(f"{args.api}/api/paper/m1?market={args.market}&n=60")
            except Exception as e:  # noqa: BLE001
                # Asked anyway, with the block saying the feed is down. The
                # question is still worth its cached price and the record
                # must show the call, not a silent skip.
                m1 = {"unavailable": True, "why": f"{type(e).__name__}: {e}"}
            fprompt = fast_prompt(pending_plan["prompt"], pending_plan["decision"], m1,
                                  detail.get("live") or {}, pending_plan["bar_time"], bar_ms)
            usage = {}
            try:
                text, ms = ask(fprompt, args.model, provider, key, args.timeout, usage, thinking="off")
                verdict = parse_trigger(text)
            except Exception as e:  # noqa: BLE001
                text, ms, usage = f"ERROR: {type(e).__name__}: {e}", 0, {}
                verdict = {"action": "WAIT", "reason": f"the model was unreachable; NOT an opinion: {type(e).__name__}"}
            acted, reply = False, None
            if verdict["action"] in ("TRIGGER", "CANCEL") and not args.dry_run:
                # The book only. The coin's order fills by rule alone - a
                # coin that could be triggered would need an opinion, and
                # then it would not be a coin.
                try:
                    reply = post_json(f"{args.api}/api/paper/pending/act", dict(
                        run=args.run, action=verdict["action"].lower(), reason=verdict["reason"][:200]))
                    acted = bool(reply.get("ok", reply.get("accepted", True)))
                except Exception as e:  # noqa: BLE001
                    reply = {"error": f"{type(e).__name__}: {e}"}
            from advisor import cost_of
            stamp = dt.datetime.now(dt.timezone.utc).strftime("%H:%M:%SZ")
            print(f"{stamp} trigger? {verdict['action']:7s} "
                  f"{'acted' if acted else ('dry run' if args.dry_run else 'no act')}  "
                  f"cached {usage.get('cached_input')}/{usage.get('input')}  {verdict['reason'][:60]}", flush=True)
            log(args.run, {
                "at": int(time.time() * 1000), "bar_time": pending_plan["bar_time"], "model": args.model,
                "prompt_variant": args.prompt_variant, "prompt_layout": PROMPT_LAYOUT,
                # Marked so nothing downstream reads a trigger opinion as an
                # entry decision; `decision` is the same NONE-shaped record
                # the hold check writes, for the readers that expect one.
                "kind": "trigger_check",
                "pending": pending, "m1_state": m1_since(m1, pending_plan["bar_time"], bar_ms)[1],
                "prompt": fprompt, "response": text, "latency_ms": ms,
                "usage": usage, "cached_input": usage.get("cached_input"),
                "cost_usd": cost_of(args.model, usage, provider) if usage else None,
                "action": verdict["action"], "verdict": verdict, "acted": acted, "act_reply": reply,
                "decision": {"side": "NONE", "reason": f"[{verdict['action']}] {verdict['reason']}"},
                "posted": False, "refused_locally": "", "dry_run": bool(args.dry_run),
            })
            if acted:
                # The desk reads null on its next poll; until then this is
                # the same order, and not one to warn about.
                pending_plan = None
                pending_seen = pending.get("decided_at")

        if last_time == decided_on:
            if args.once:
                return 0
            time.sleep(args.poll)
            continue

        shown = bars[-args.bars:]
        rows = chr(10).join(
            f"  {dt.datetime.utcfromtimestamp(b[0] / 1000):%Y-%m-%d %H:%MZ}  "
            f"O {b[1]:g}  H {b[2]:g}  L {b[3]:g}  C {b[4]:g}"
            for b in shown
        )
        atr_series = (detail.get("series") or {}).get("atr_14.atr") or []
        atr_now = None
        for p in reversed(atr_series):
            v = p.get("value") if isinstance(p, dict) else (p[1] if isinstance(p, (list, tuple)) and len(p) > 1 else None)
            if v is not None and v == v:
                atr_now = float(v)
                break
        # Re-read per decision, not once at startup. A guard changed from the
        # Settings screen has to reach the very next bar, or the model spends
        # the rest of the session planning around a rule that has been lifted -
        # which it did on 2026-09-17, refusing setups for a daily cap the desk
        # had already raised from 4 to 10. One local HTTP call per decided bar.
        limits = read_limits(args.api)
        # The options block, fetched per decision and never cached across
        # bars: positioning that is one bar old is a different market, and
        # `otl_context` reports its own age so the model is told which it is.
        #
        # An unreachable or stale feed does NOT fall back to the base prompt.
        # That would put context-absent decisions into a context-present
        # book's numbers, and the experiment would be measuring a mixture with
        # nothing in the record to separate it. The block says it is missing
        # and `otl` below records which bars those were.
        otl_state, otl_block = "n/a", ""
        if VARIANTS[args.prompt_variant]["otl"]:
            ctx_otl = otl_context.gather()
            otl_block = "\n\n" + otl_context.block(ctx_otl)
            age = ctx_otl.get("age_ms")
            otl_state = ctx_otl["state"]
            if otl_state == "stale" and age is not None:
                otl_state = f"stale {int(age / 60000)}m"

        # The higher-timeframe block, fetched per decision for the same reason
        # as the options block above: facts from the bar before the one being
        # decided describe the same market, facts from three days ago do not,
        # and `htf_context` judges that against THIS bar rather than against a
        # wall clock - which cannot tell a stopped export from a weekend.
        #
        # An absent, thin or stale route does NOT fall back to the base
        # prompt. The block says which it is and `htf` below records it, so
        # the bars that had the facts can be separated from the bars that did
        # not when the book is read.
        htf_state, htf_block, htf_rule_block = "n/a", "", ""
        if VARIANTS[args.prompt_variant]["htf"]:
            ctx_htf = htf_context.gather(args.api, args.market, last_time)
            htf_block = "\n\n" + htf_context.block(ctx_htf)
            htf_state = ctx_htf["state"]
            if htf_state == "stale" and ctx_htf.get("behind_bars") is not None:
                htf_state = f"stale {ctx_htf['behind_bars']:.1f} bars"
            elif htf_state == "thin":
                htf_state = "thin " + ",".join(ctx_htf.get("thin_fields") or [])
        # The rule is NOT dropped when the facts are missing. It names that
        # case itself - both sides stay open - so a bar with no facts is still
        # a bar decided under this book's rule, and the record says so. A rule
        # that vanished on the bars its input was absent would make the
        # variant two prompts sharing one book id.
        if VARIANTS[args.prompt_variant]["htf_rule"]:
            htf_rule_block = "\n" + HTF_RULE + "\n"

        # The structural levels block, fetched per decision for the reason the
        # two blocks above are: levels computed from bars three days old
        # describe a different market, and `smc_context` judges that against
        # THIS bar rather than a wall clock, which cannot tell a stopped
        # export from a weekend.
        #
        # `last_close` and `atr_now` are handed over rather than left to the
        # route. The prompt ends with forty bars and the block says "the last
        # close": a distance measured from a number the model cannot see on
        # that last line would be wrong in the one way nobody would catch.
        # `atr_now` is the same ATR(14) the desk block quotes 1R against, so
        # "0.48 ATR" there and "1R is 1.2 x ATR" here are one unit.
        #
        # An absent, thin or stale route does NOT fall back to the base
        # prompt, for the reason the two above give: the block says which it
        # is and `smc` below records it.
        smc_state, smc_block = "n/a", ""
        if VARIANTS[args.prompt_variant]["smc"]:
            ctx_smc = smc_context.gather(args.api, args.market, last_time, last_close, atr_now)
            smc_block = "\n\n" + smc_context.block(ctx_smc)
            smc_state = ctx_smc["state"]
            if smc_state == "stale" and ctx_smc.get("behind_min") is not None:
                # MINUTES, not bars. The gap is clock time and the store holds
                # 879 bars across 1,308 bar-lengths of it, so a bar count
                # divided out of a millisecond delta is out by half. See
                # smc_context.STALE_AFTER_BARS for the measurement.
                smc_state = f"stale {ctx_smc['behind_min']:.0f}m"
            elif smc_state == "ok":
                # How many levels the route FOUND, not how many were shown.
                # The block shows at most twelve and the live response of
                # 2026-09-19 carried 252; "ok" over a tape with three levels
                # and "ok" over one with 252 are not the same bar, and the
                # disagreement analysis has to be able to tell them apart.
                smc_state = f"ok {len(ctx_smc.get('levels') or [])} levels"
        # The plan block is static text, so it lands in the cached prefix
        # with the rules it amends - before the blocks that change hourly.
        plan_block = ("\n" + PLAN_BLOCK + "\n") if is_plan else ""

        prompt = PROMPT.format(
            market=args.market, tf=args.tf, n=len(shown), bars=rows,
            plan_block=plan_block,
            otl_block=otl_block, htf_block=htf_block, htf_rule_block=htf_rule_block,
            smc_block=smc_block,
            coin_clause=COIN_CLAUSE[VARIANTS[args.prompt_variant]["coin"]],
            position=describe_position(detail),
            desk=desk_block(detail, limits, atr_now, args.run),
            context=context_block(bars),
        )

        # A book already holding a position has nothing to decide. The desk
        # allows one at a time, so any entry would be refused, and the prompt
        # says so in as many words — the model was being paid to read the
        # answer back. Measured before this: 33% to 65% of every book's calls
        # were made in this state.
        #
        # The bar is still RECORDED, with no usage and no latency, so the log
        # has no unexplained gap. It is not posted to the desk, because nothing
        # was decided and a bar the model never saw must not be counted as a
        # stand-aside — the same rule that keeps a failed call from being one.
        # A held book is asked a DIFFERENT question, not the same one again.
        #
        # It used to be asked for an entry it could not have — the desk allows
        # one position at a time and the prompt said so, so the model was paid
        # to read its own constraint back, on a third to two thirds of every
        # book's calls. Then it was skipped entirely. Neither is right: the
        # owner's point is that a model watching a position might see the
        # reason for it break before the stop does.
        #
        # So it is asked whether to close early, and the answer is RECORDED AND
        # NOT ACTED ON. Granting the power would break the control — the coin
        # has no reasoning and cannot close early, so the difference between
        # the books would stop measuring direction and start measuring
        # direction and exit skill mixed together, inseparably. Measuring the
        # opinion first is what tells us whether the power is worth a campaign
        # of its own.
        held = (detail.get("run") or {}).get("open")
        if held and args.hold_check == "event":
            # Decide from the desk's own numbers whether this bar is worth a
            # question. Units are R when the stop is known, price otherwise.
            entry = held.get("entry_price")
            stop = held.get("stop")
            side = str(held.get("side") or "").upper()
            moved = None
            if isinstance(entry, (int, float)) and isinstance(last_close, (int, float)):
                moved = (last_close - entry) if side == "LONG" else (entry - last_close)
            unit = abs(float(entry) - float(stop)) if isinstance(entry, (int, float)) and isinstance(stop, (int, float)) and entry != stop else None
            moved_r = (moved / unit) if (moved is not None and unit) else None
            since = 0 if hold_state["last_bar"] is None else max(0, (last_time - hold_state["last_bar"]) // max(1, bar_ms))
            trigger = ""
            if hold_state["entry_time"] != held.get("entry_time"):
                # A new position: the first bar it is held through is always
                # looked at, so every trade has at least one verdict early.
                trigger = "first bar held"
            elif moved_r is not None and moved_r <= -0.5:
                trigger = f"adverse {moved_r:+.2f}R"
            elif moved_r is not None and moved_r >= 1.0 and not hold_state["favourable_seen"]:
                trigger = f"favourable {moved_r:+.2f}R"
            elif since >= 4:
                trigger = f"floor, {since} bars since the last look"
            if not trigger:
                decided_on = last_time
                remember(last_time)
                log(args.run, {
                    "at": int(time.time() * 1000), "bar_time": last_time, "model": args.model,
                    "prompt_variant": args.prompt_variant, "prompt_layout": PROMPT_LAYOUT,
                    "kind": "hold_skip",
                    "moved_r": None if moved_r is None else round(moved_r, 3),
                    "bars_since_check": int(since),
                    "decision": {"side": "NONE", "reason": "[HOLD] no event on this bar; not asked"},
                    "posted": False, "refused_locally": "", "dry_run": bool(args.dry_run),
                })
                if args.once:
                    return 0
                time.sleep(args.poll)
                continue
            hold_state["trigger"] = trigger
            if moved_r is not None and moved_r >= 1.0:
                hold_state["favourable_seen"] = True
        elif held:
            hold_state["trigger"] = "every bar"
        if held:
            hold_prompt = HOLD_PROMPT.format(
                market=args.market, tf=args.tf, n=len(shown), bars=rows,
                position_detail=describe_open(detail, atr_now),
                context=context_block(bars),
            )
            usage = {}
            try:
                text, ms = ask(hold_prompt, args.model, provider, key, args.timeout, usage,
                               thinking=None if args.hold_thinking == "default" else args.hold_thinking)
                verdict = parse_hold(text)
            except Exception as e:  # noqa: BLE001
                text, ms, usage = f"ERROR: {type(e).__name__}: {e}", 0, {}
                verdict = {"action": "HOLD", "reason": f"the model was unreachable; NOT an opinion: {type(e).__name__}"}

            decided_on = last_time
            remember(last_time)
            from advisor import cost_of
            stamp = dt.datetime.now(dt.timezone.utc).strftime("%H:%M:%SZ")
            print(f"{stamp} bar {dt.datetime.utcfromtimestamp(last_time/1000):%H:%MZ}  "
                  f"{verdict['action']:5s} (advisory, {hold_state['trigger']})  {verdict['reason'][:64]}", flush=True)
            hold_state["last_bar"] = last_time
            hold_state["entry_time"] = held.get("entry_time")
            log(args.run, {
                "at": int(time.time() * 1000), "bar_time": last_time, "model": args.model,
            # WHICH PROMPT PRODUCED THIS ROW. The same rule the desk
            # applies to every other unit it carries: a number whose unit
            # lives somewhere else is a number nobody can compare later.
            # The prompt TEXT is already stored per row, but text is not
            # a key - two campaigns are compared by variant name, and
            # rows written before this existed are `base` by the default.
                "prompt_variant": args.prompt_variant, "prompt_layout": PROMPT_LAYOUT,
                "hold_trigger": hold_state["trigger"],
                "prompt": hold_prompt, "response": text, "latency_ms": ms,
                # The RESOLVED provider, not the one derived from the model
                # name. `--provider anthropic` forces a CLI-named model onto
                # the metered API, and the name cannot know that: without this
                # argument a run that is spending real money logs
                # `cost_usd: null` and the campaign looks free. See
                # `advisor.cost_of`.
                "usage": usage, "cost_usd": cost_of(args.model, usage, provider) if usage else None,
                # Marked so nothing downstream mistakes an opinion about an
                # open trade for a decision about a new one.
                "kind": "hold_check", "verdict": verdict,
                "decision": {"side": "NONE", "reason": f"[{verdict['action']}] {verdict['reason']}"},
                "posted": False, "refused_locally": "", "dry_run": bool(args.dry_run),
            })
            if args.once:
                return 0
            time.sleep(args.poll)
            continue

        # A plan book with an order still waiting is NOT asked again on the
        # new bar. The model named how many bars the order lives for, and
        # asking it afresh every bar would replace that order with a new one
        # each time - the API keeps one pending order per run - so a
        # `valid_bars` of 4 would never mean four. The bar is recorded, as a
        # held bar is, so the log has no gap; nothing is posted, because a
        # NONE intent is a new intent and would replace the order too.
        # Whether to withdraw it early is the fast loop's question, and only
        # on the `plan-trigger` book.
        if pending:
            decided_on = last_time
            remember(last_time)
            log(args.run, {
                "at": int(time.time() * 1000), "bar_time": last_time, "model": args.model,
                "prompt_variant": args.prompt_variant, "prompt_layout": PROMPT_LAYOUT,
                "kind": "pending_skip", "pending": pending,
                "decision": {"side": "NONE", "reason": "[PENDING] an order is still waiting; not asked"},
                "posted": False, "refused_locally": "", "dry_run": bool(args.dry_run),
            })
            if args.once:
                return 0
            time.sleep(args.poll)
            continue

        try:
            usage = {}
            text, ms = ask(prompt, args.model, provider, key, args.timeout, usage,
                           thinking=None if args.decision_thinking == "default" else args.decision_thinking)
            decision = parse(text, plan=is_plan)
        except Exception as e:  # noqa: BLE001
            text, ms, usage = f"ERROR: {type(e).__name__}: {e}", 0, {}
            # `unreachable` is NOT a stand-aside. It is marked so the poster
            # below refuses to tell the desk this book was consulted: a model
            # that is down did not decline, and recording it as a decline is
            # the one lie this campaign cannot survive. `last_at` going quiet
            # is the true signal that a process has died, and posting on
            # failure destroyed exactly that signal for eight hours.
            decision = {"side": "NONE", "unreachable": True,
                        "reason": f"the model was unreachable; NOT a decision: {type(e).__name__}"}

        decided_on = last_time
        remember(last_time)
        posted = False
        refused = ""
        if decision["side"] != "NONE":
            ok, why = sane(decision, last_close)
            if not ok:
                refused = why
                decision = {"side": "NONE", "reason": f"refused locally: {why}"}

        stamp = dt.datetime.now(dt.timezone.utc).strftime("%H:%M:%SZ")
        if decision["side"] == "NONE":
            # Tell the desk anyway. A stand-aside sets no intent and changes no
            # book, but it is the only thing separating a model that is
            # thinking and declining from a model that stopped running an hour
            # ago — from the outside both show zero trades.
            if not args.dry_run and not decision.get("unreachable"):
                try:
                    post_json(f"{args.api}/api/paper/intent", dict(
                        run=args.run, side="NONE", bar_time=last_time,
                        reason=decision["reason"][:200], decider=args.model))
                except Exception as e:  # noqa: BLE001
                    print(f"{stamp} stand-aside not recorded: {type(e).__name__}: {e}", flush=True)
            print(f"{stamp} bar {dt.datetime.utcfromtimestamp(last_time/1000):%H:%MZ}  "
                  f"NONE  {decision['reason'][:80]}", flush=True)
        elif not args.dry_run:
            # The coin is flipped for EVERY trade the model takes, and both
            # posts must succeed. A campaign that loses its control silently
            # produces a number nobody can read.
            # The coin is still FLIPPED when there is no control book, and the
            # flip is still consumed from the same seeded stream. Skipping the
            # draw would make the sequence depend on whether a control existed,
            # so a campaign restarted with the coin switched back on would not
            # replay - and the seed exists precisely so that it does.
            flip = "LONG" if coin.random() < 0.5 else "SHORT"
            # Each book says who drove it. The desk reads this back as a
            # badge, so the model's book and the coin's are never mistaken
            # for one another or for a rule — and the API records it only
            # when the intent is actually accepted.
            body = dict(bar_time=last_time, stop=decision.get("stop"),
                        target=decision.get("target"), reason=decision["reason"][:200])
            if is_plan:
                body.update(plan_body(decision))
            try:
                a = post_json(f"{args.api}/api/paper/intent",
                              dict(run=args.run, side=decision["side"], decider=args.model, **body))
                b = {"accepted": True}
                if not args.no_control and is_plan:
                    # The whole plan at mirrored distances - same entry
                    # type, same validity - so both books share the fill
                    # mechanics. See `coin_plan` for why this is not the
                    # market books' arithmetic below.
                    c = coin_plan(decision, flip, last_close)
                    b = post_json(f"{args.api}/api/paper/intent", dict(
                        run=args.control, bar_time=last_time, reason=f"coin: {flip}",
                        decider="coin", **c))
                elif not args.no_control:
                    # The control's stop must be the same DISTANCE on its own
                    # side, or the two books are not sized alike and the
                    # comparison dies.
                    d = abs(float(decision["stop"]) - last_close)
                    c_stop = last_close - d if flip == "LONG" else last_close + d
                    c_target = None
                    if decision.get("target") is not None:
                        td = abs(float(decision["target"]) - last_close)
                        c_target = last_close + td if flip == "LONG" else last_close - td
                    b = post_json(f"{args.api}/api/paper/intent", dict(
                        run=args.control, side=flip, bar_time=last_time, stop=c_stop,
                        target=c_target, reason=f"coin: {flip}", decider="coin"))
                posted = bool(a.get("accepted")) and bool(b.get("accepted"))
                if posted and is_plan and decision["entry"]["type"] != "market":
                    # The order now waits on the desk. Kept with the exact
                    # prompt that produced it, for the fast loop; a market
                    # entry has nothing to wait for and keeps no memory.
                    pending_plan = {"prompt": prompt, "decision": decision, "bar_time": last_time,
                                    "asked_at": time.monotonic()}
                against = "no coin" if args.no_control else f"vs coin {flip:5s}"
                how = f"{decision['entry']['type']} at {decision['entry']['price']}  " if is_plan else ""
                print(f"{stamp} bar {dt.datetime.utcfromtimestamp(last_time/1000):%H:%MZ}  "
                      f"{decision['side']:5s} {against}  {how}"
                      f"{'posted' if posted else 'REFUSED: ' + str(a.get('reason')) + ' / ' + str(b.get('reason'))}"
                      f"  {decision['reason'][:60]}", flush=True)
            except Exception as e:  # noqa: BLE001
                print(f"{stamp} post failed: {type(e).__name__}: {e}", flush=True)
        else:
            print(f"{stamp} would post {decision['side']} (dry run)  {decision['reason'][:70]}", flush=True)

        from advisor import cost_of
        # The resolved provider, for the reason given at the hold-check above.
        cost = cost_of(args.model, usage, provider) if usage else None
        log(args.run, {
            "at": int(time.time() * 1000), "bar_time": last_time, "model": args.model,
            # WHICH PROMPT PRODUCED THIS ROW. The same rule the desk
            # applies to every other unit it carries: a number whose unit
            # lives somewhere else is a number nobody can compare later.
            # The prompt TEXT is already stored per row, but text is not
            # a key - two campaigns are compared by variant name, and
            # rows written before this existed are `base` by the default.
            "prompt_variant": args.prompt_variant, "prompt_layout": PROMPT_LAYOUT,
            # Which bars had the options context and which did not, so the
            # two can be separated when the book is read. "n/a" for the
            # variants that never ask for it.
            "otl": otl_state,
            # Which bars had the higher-timeframe facts, and in what
            # condition: `ok`, `thin <fields>`, `stale <n> bars`,
            # `unavailable`, or `n/a` for the variants that never ask. The
            # first analysis this book gets is disagreement rate, and a bar
            # whose facts were absent cannot be counted in it.
            "htf": htf_state,
            # Which bars had the structural levels, and in what condition:
            # `ok <n> levels`, `thin`, `stale <n> bars`, `unavailable`, or
            # `n/a` for the variants that never ask. Same rule as `htf`
            # above: stage 1 of this registration counts only the bars where
            # the block read `ok`, and a row that did not say so cannot be
            # separated out afterwards.
            "smc": smc_state,
            "prompt": prompt, "response": text, "latency_ms": ms,
            # What the call actually spent. `cost_usd` is null for a plan: that
            # call is not free, it draws on a quota, and printing $0.00 beside
            # it would claim something untrue.
            "usage": usage, "cost_usd": cost,
            # How the trade was to be entered, hoisted out of `decision` so
            # the plan books' rows can be split by entry type without
            # parsing the reply. Null on every market book: those rows never
            # carried an entry and an absent key would read as one more
            # thing to look up.
            "entry": decision.get("entry"),
            "decision": decision, "posted": posted, "refused_locally": refused,
            "dry_run": bool(args.dry_run),
        })

        if args.once:
            return 0
        time.sleep(args.poll)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        sys.exit(0)
