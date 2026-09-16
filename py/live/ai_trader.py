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

**The coin is not decoration and is not optional.** Every decision drives two
books: the model's side into `--run`, and a seeded coin flip into
`--control`. If the control is unreachable the model's trade is NOT posted
either — a campaign that quietly loses its control is a campaign that can only
produce a number nobody can read.
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

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))

PROMPT = """You are trading one paper book on {market} {tf} bars. You are being measured against a
coin that takes the same trades at the same bars with a random side, so a trade you are not
actually confident in is worse than no trade: it hands the coin a free sample.

You may only answer in one of two ways: propose ONE trade, or stand aside.

You cannot choose a size — the desk sizes every book identically at one percent of equity against
the trailing range. You cannot act on the bar below; whatever you propose fills at the OPEN of the
next bar. The desk owns the maximum hold, and it will close the position if your stop or target is
not hit first.

{position}

THE DESK'S STATE — these are the rules you are already playing under, not advice
{desk}

MARKET CONTEXT — computed from the same bars, for convenience; none of it is a signal
{context}

LAST {n} BARS of {market}:{tf}, oldest first, times UTC
{bars}

Answer with JSON and nothing else:

  {{"side": "LONG" | "SHORT" | "NONE", "stop": <price or null>, "target": <price or null>,
    "reason": "<one sentence, under 200 characters, naming what in the bars above you are acting on>"}}

"NONE" is a real answer and is often the right one. If you propose a trade, the stop must be on the
losing side of the last close and the target on the winning side, or it will be refused."""


def read_limits() -> dict:
    """The guard numbers, from the config the engine actually reads.

    Parsed rather than hard-coded: a limit quoted to the model that does not
    match the one enforced would be worse than saying nothing, because the
    model would plan around a rule that is not the rule.
    """
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


def parse(text: str) -> dict:
    """The decision, or a stand-aside.

    An unreadable reply is NOT a trade. Every other failure mode in this
    repository resolves toward doing nothing, and a parser that guessed a side
    would be the one place a bug could put on a position nobody chose.
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
    return {
        "side": side,
        "stop": obj.get("stop"),
        "target": obj.get("target"),
        "reason": str(obj.get("reason", ""))[:200],
    }


def sane(decision: dict, last_close: float) -> tuple[bool, str]:
    """A stop on the losing side and a target on the winning side, or no trade.

    Not a judgement about the trade — a check that the numbers mean what the
    words say. A LONG whose stop is above the price is not a bold trade, it is
    a reply that did not understand the question, and posting it would put a
    position on the book for a reason nobody held.
    """
    stop, target = decision.get("stop"), decision.get("target")
    if stop is None:
        return False, "no stop"
    try:
        stop = float(stop)
        target = float(target) if target is not None else None
    except (TypeError, ValueError):
        return False, "stop or target not a number"
    long = decision["side"] == "LONG"
    if (long and stop >= last_close) or (not long and stop <= last_close):
        return False, f"stop {stop} is on the winning side of {last_close} for a {decision['side']}"
    if target is not None and ((long and target <= last_close) or (not long and target >= last_close)):
        return False, f"target {target} is on the losing side of {last_close}"
    return True, ""


def log(run: str, record: dict) -> None:
    """Prompt and reply, whole, beside the book they drove.

    Same shape and same reason as the advisor's log: a summary cannot be
    replayed against a changed prompt, and replay is the only way a mistake
    gets fixed rather than counted.
    """
    path = os.path.join(ROOT, "data", "paper", run, "decisions.jsonl")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "a", encoding="utf-8") as f:
        f.write(json.dumps(record) + chr(10))


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split(chr(10))[0])
    ap.add_argument("--api", default="http://127.0.0.1:8138")
    ap.add_argument("--run", default="ai-xau", help="the run that takes the model's side")
    ap.add_argument("--control", default="ai-xau-coin", help="the run that takes a coin's side")
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
    ap.add_argument("--dry-run", action="store_true", help="decide and log; post nothing")
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

    limits = read_limits()
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
    print(
        f"ai trader: {args.model} on {args.market}:{args.tf} -> {args.run}, "
        f"coin -> {args.control}{' (dry run)' if args.dry_run else ''}",
        flush=True,
    )

    decided_on: int | None = recall()
    if decided_on is not None:
        print(f"resuming: bar {decided_on} was already decided", flush=True)
    while True:
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
        prompt = PROMPT.format(
            market=args.market, tf=args.tf, n=len(shown), bars=rows,
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
        if held:
            hold_prompt = HOLD_PROMPT.format(
                market=args.market, tf=args.tf, n=len(shown), bars=rows,
                position_detail=describe_open(detail, atr_now),
                context=context_block(bars),
            )
            usage = {}
            try:
                text, ms = ask(hold_prompt, args.model, provider, key, args.timeout, usage)
                verdict = parse_hold(text)
            except Exception as e:  # noqa: BLE001
                text, ms, usage = f"ERROR: {type(e).__name__}: {e}", 0, {}
                verdict = {"action": "HOLD", "reason": f"the model was unreachable; NOT an opinion: {type(e).__name__}"}

            decided_on = last_time
            remember(last_time)
            from advisor import cost_of
            stamp = dt.datetime.now(dt.timezone.utc).strftime("%H:%M:%SZ")
            print(f"{stamp} bar {dt.datetime.utcfromtimestamp(last_time/1000):%H:%MZ}  "
                  f"{verdict['action']:5s} (advisory)  {verdict['reason'][:64]}", flush=True)
            log(args.run, {
                "at": int(time.time() * 1000), "bar_time": last_time, "model": args.model,
                "prompt": hold_prompt, "response": text, "latency_ms": ms,
                "usage": usage, "cost_usd": cost_of(args.model, usage) if usage else None,
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

        try:
            usage = {}
            text, ms = ask(prompt, args.model, provider, key, args.timeout, usage)
            decision = parse(text)
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
            flip = "LONG" if coin.random() < 0.5 else "SHORT"
            # Each book says who drove it. The desk reads this back as a
            # badge, so the model's book and the coin's are never mistaken
            # for one another or for a rule — and the API records it only
            # when the intent is actually accepted.
            body = dict(bar_time=last_time, stop=decision.get("stop"),
                        target=decision.get("target"), reason=decision["reason"][:200])
            try:
                a = post_json(f"{args.api}/api/paper/intent",
                              dict(run=args.run, side=decision["side"], decider=args.model, **body))
                # The control's stop must be the same DISTANCE on its own side,
                # or the two books are not sized alike and the comparison dies.
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
                print(f"{stamp} bar {dt.datetime.utcfromtimestamp(last_time/1000):%H:%MZ}  "
                      f"{decision['side']:5s} vs coin {flip:5s}  "
                      f"{'posted' if posted else 'REFUSED: ' + str(a.get('reason')) + ' / ' + str(b.get('reason'))}"
                      f"  {decision['reason'][:60]}", flush=True)
            except Exception as e:  # noqa: BLE001
                print(f"{stamp} post failed: {type(e).__name__}: {e}", flush=True)
        else:
            print(f"{stamp} would post {decision['side']} (dry run)  {decision['reason'][:70]}", flush=True)

        from advisor import cost_of
        cost = cost_of(args.model, usage) if usage else None
        log(args.run, {
            "at": int(time.time() * 1000), "bar_time": last_time, "model": args.model,
            "prompt": prompt, "response": text, "latency_ms": ms,
            # What the call actually spent. `cost_usd` is null for a plan: that
            # call is not free, it draws on a quota, and printing $0.00 beside
            # it would claim something untrue.
            "usage": usage, "cost_usd": cost,
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
