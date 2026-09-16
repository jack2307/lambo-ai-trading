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

LAST {n} BARS of {market}:{tf}, oldest first, times UTC
{bars}

Answer with JSON and nothing else:

  {{"side": "LONG" | "SHORT" | "NONE", "stop": <price or null>, "target": <price or null>,
    "reason": "<one sentence, under 200 characters, naming what in the bars above you are acting on>"}}

"NONE" is a real answer and is often the right one. If you propose a trade, the stop must be on the
losing side of the last close and the target on the winning side, or it will be refused."""


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
    ap.add_argument("--bars", type=int, default=40, help="bars shown to the model")
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
    env = PROVIDERS[provider]["env"]
    key = os.environ.get(env) if env else ""
    if env and not key:
        sys.exit(f"no {env} in the environment for {args.model}")

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
            detail = get_json(f"{args.api}/api/paper/run/{args.run}?bars={args.bars}")
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

        rows = chr(10).join(
            f"  {dt.datetime.utcfromtimestamp(t / 1000):%Y-%m-%d %H:%MZ}  "
            f"O {o:g}  H {h:g}  L {lo:g}  C {c:g}"
            for t, o, h, lo, c in bars
        )
        prompt = PROMPT.format(
            market=args.market, tf=args.tf, n=len(bars), bars=rows,
            position=describe_position(detail),
        )

        try:
            text, ms = ask(prompt, args.model, provider, key, args.timeout)
            decision = parse(text)
        except Exception as e:  # noqa: BLE001
            text, ms = f"ERROR: {type(e).__name__}: {e}", 0
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

        log(args.run, {
            "at": int(time.time() * 1000), "bar_time": last_time, "model": args.model,
            "prompt": prompt, "response": text, "latency_ms": ms,
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
