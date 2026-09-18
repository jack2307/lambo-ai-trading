# -*- coding: utf-8 -*-
"""The plan answer, its sanity rules, the coin's mirror, and the fast loop.

    py -3.9 py/live/plan_entry_selftest.py

Nothing here reaches a model or the desk. `ai_trader.ask` is replaced with a
scripted reply, `urllib.request.urlopen` with a fake that answers the six
routes the trader uses, and `ai_trader.ROOT` points at a temporary directory,
so no file under `data/` is touched.

What it pins, and why each is here:

  1  parse() reads the plan shape and refuses nothing; an omitted entry is
     kept as an omission (`type: None`) for sane() to name, never quietly
     turned into a market fill. The market books' rows keep their old shape.
  2  sane() puts a limit on the pullback side of the last close and a stop
     on the breakout side, judges stop and target against the ENTRY price
     for those two, and against the last close for a market entry exactly
     as before. valid_bars outside 1..4 is refused; a malformed zone is
     refused; null zone and invalidations are fine.
  3  coin_plan() keeps the entry TYPE and valid_bars, reflects every price
     about the last close when the coin lands on the other side, keeps them
     as-is when it lands on the same side, and swaps the two invalidation
     levels on reflection. The market books' coin arithmetic is untouched
     and asserted against the old formula written out here.
  4  the fast loop, driven through main(): a limit plan posts two intents
     with the plan fields; the run's `pending_order` then draws a fast call
     whose prompt BEGINS with the decision prompt byte for byte and is sent
     with thinking off; WAIT posts nothing, TRIGGER posts `pending/act
     trigger`, CANCEL posts `cancel` with the reason; every fast call is a
     `trigger_check` row carrying usage and cached_input; the coin control
     is never acted on; the `plan` variant never makes a fast call; a new
     bar while an order waits is a `pending_skip` row and posts no intent.
"""
from __future__ import annotations

import io
import json
import os
import random
import shutil
import sys
import tempfile
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ai_trader as A  # noqa: E402

fails = []


def check(ok: bool, what: str, detail: str = "") -> None:
    print(("ok   " if ok else "FAIL ") + what + (f"   -- {detail}" if detail and not ok else ""))
    if not ok:
        fails.append(what)


LAST = 4311.85


def plan(side="LONG", kind="limit", price=4305.0, stop=4295.0, target=4325.0, valid=2,
         zone=None, above=None, below=None) -> dict:
    return {"side": side, "entry": {"type": kind, "price": price}, "stop": stop, "target": target,
            "valid_bars": valid, "zone": zone, "invalidate_above": above, "invalidate_below": below,
            "reason": "r"}


# ---------------------------------------------------------------------------
# 1 - parse
# ---------------------------------------------------------------------------
print("parse")
full = json.dumps(plan(zone=[4303, 4307], below=4290))
d = A.parse(full, plan=True)
check(d["entry"] == {"type": "limit", "price": 4305.0} and d["valid_bars"] == 2
      and d["zone"] == [4303, 4307] and d["invalidate_below"] == 4290 and d["invalidate_above"] is None,
      "the full plan shape is read as written", str(d))
check("entry" not in A.parse(full), "without plan=True the row keeps the market books' shape")
d = A.parse('{"side": "SHORT", "stop": 4320, "target": 4300, "reason": "r"}', plan=True)
check(d["entry"] == {"type": None, "price": None}, "an omitted entry is kept as an omission, not a market fill")
check(d["valid_bars"] == 2, "an omitted valid_bars is the API's default of 2, and the row says 2")
d = A.parse('{"side": "LONG", "entry": "stop", "price": 4320, "stop": 4310, "reason": "r"}', plan=True)
check(d["entry"] == {"type": "stop", "price": 4320}, "a bare-string entry with the price beside it is read")
d = A.parse('{"side": "LONG", "entry": {"type": "LIMIT", "price": 4305}, "stop": 4295, "valid_bars": 3.0, "reason": "r"}', plan=True)
check(d["entry"]["type"] == "limit" and d["valid_bars"] == 3, "case and an integral float are normalised")
d = A.parse('{"side": "LONG", "entry": {"type": "limit", "price": 4305}, "stop": 4295, "valid_bars": "two", "reason": "r"}', plan=True)
check(d["valid_bars"] == "two", "an unreadable valid_bars is kept as said, for sane() to refuse")
check(A.parse("garbage", plan=True)["side"] == "NONE", "an unreadable reply is a stand-aside")

# ---------------------------------------------------------------------------
# 2 - sane
# ---------------------------------------------------------------------------
print("sane")
CASES = [
    # (what, decision, expected ok, fragment of the refusal)
    ("LONG limit below the close", plan(), True, ""),
    ("LONG limit AT the close", plan(price=LAST), False, "pullback side"),
    ("LONG limit above the close", plan(price=4315.0), False, "pullback side"),
    ("SHORT limit above the close", plan("SHORT", price=4318.0, stop=4328.0, target=4298.0), True, ""),
    ("SHORT limit below the close", plan("SHORT", price=4305.0, stop=4315.0, target=4290.0), False, "pullback side"),
    ("LONG stop above the close", plan(kind="stop", price=4318.0, stop=4308.0, target=4338.0), True, ""),
    ("LONG stop below the close", plan(kind="stop", price=4305.0, stop=4295.0, target=4325.0), False, "breakout side"),
    ("SHORT stop below the close", plan("SHORT", kind="stop", price=4305.0, stop=4315.0, target=4285.0), True, ""),
    ("SHORT stop above the close", plan("SHORT", kind="stop", price=4318.0, stop=4328.0, target=4298.0), False, "breakout side"),
    # exits judged against the ENTRY, not the last close
    ("LONG limit 4305, stop 4308: above the entry, below the close -> refused", plan(stop=4308.0), False, "winning side of 4305"),
    ("LONG limit 4305, target 4309: above the entry, below the close -> allowed", plan(target=4309.0), True, ""),
    ("LONG stop-entry 4318, stop 4314: below the entry, above the close -> allowed",
     plan(kind="stop", price=4318.0, stop=4314.0, target=4330.0), True, ""),
    ("LONG stop-entry 4318, target 4316: below the entry -> refused",
     plan(kind="stop", price=4318.0, stop=4308.0, target=4316.0), False, "losing side of 4318"),
    # market keeps today's rules, against the last close
    ("market LONG, stop below the close", plan(kind="market", price=None, stop=4300.0, target=4320.0), True, ""),
    ("market LONG, stop above the close", plan(kind="market", price=None, stop=4315.0, target=4320.0), False, f"winning side of {LAST}"),
    ("market with a stray price and valid_bars 9: both ignored", plan(kind="market", price=4000.0, valid=9), True, ""),
    # the plan fields
    ("no entry type", dict(plan(), entry={"type": None, "price": None}), False, "not market, limit or stop"),
    ("entry type 'buy'", plan(kind="buy"), False, "not market, limit or stop"),
    ("limit with no price", plan(price=None), False, "names no price"),
    ("limit with a price that is not a number", plan(price="4305ish"), False, "not a number"),
    ("valid_bars 0", plan(valid=0), False, "valid_bars 0"),
    ("valid_bars 5", plan(valid=5), False, "valid_bars 5"),
    ("valid_bars 4", plan(valid=4), True, ""),
    ("valid_bars 1", plan(valid=1), True, ""),
    ("valid_bars 'two'", plan(valid="two"), False, "valid_bars 'two'"),
    ("valid_bars True", plan(valid=True), False, "valid_bars True"),
    ("zone [lo, hi]", plan(zone=[4303.0, 4307.0]), True, ""),
    ("zone null", plan(zone=None), True, ""),
    ("zone reversed", plan(zone=[4307.0, 4303.0]), False, "zone"),
    ("zone of one number", plan(zone=[4303.0]), False, "zone"),
    ("zone of words", plan(zone=["a", "b"]), False, "zone"),
    ("invalidation levels null", plan(above=None, below=None), True, ""),
    ("invalidation levels numeric", plan(above=4330.0, below=4290.0), True, ""),
    ("invalidate_above a word", plan(above="high"), False, "invalidate_above"),
    ("no stop at all", dict(plan(), stop=None), False, "no stop"),
]
for what, dec, want_ok, frag in CASES:
    ok, why = A.sane(dec, LAST)
    check(ok == want_ok and (frag in why), f"sane: {what}", f"got ok={ok} why={why!r}")

# The market books' path is untouched: a decision with no `entry` key is
# judged exactly as it always was.
ok, why = A.sane({"side": "LONG", "stop": 4300.0, "target": 4320.0}, LAST)
check(ok, "a market-book decision (no entry key) still passes as before", why)
ok, why = A.sane({"side": "SHORT", "stop": 4300.0, "target": 4320.0}, LAST)
check(not ok and "winning side" in why, "and is still refused as before", why)

# ---------------------------------------------------------------------------
# 3 - the coin's mirror
# ---------------------------------------------------------------------------
print("coin_plan")
book = plan(zone=[4303.0, 4307.0], above=4315.0, below=4290.0)
same = A.coin_plan(book, "LONG", LAST)
check(same["entry"] == {"type": "limit", "price": 4305.0} and same["stop"] == 4295.0
      and same["target"] == 4325.0 and same["zone"] == [4303.0, 4307.0]
      and same["invalidate_above"] == 4315.0 and same["invalidate_below"] == 4290.0
      and same["valid_bars"] == 2 and same["side"] == "LONG",
      "coin on the SAME side: the plan as is, every field", str(same))
other = A.coin_plan(book, "SHORT", LAST)
r = lambda p: A.reflect(p, LAST)  # noqa: E731
check(other["side"] == "SHORT" and other["entry"]["type"] == "limit" and other["valid_bars"] == 2,
      "coin on the OTHER side: same entry type, same valid_bars")
check(abs(other["entry"]["price"] - (LAST + (LAST - 4305.0))) < 1e-9,
      "a limit 6.85 below the close on the book is a limit 6.85 above on the coin", str(other["entry"]))
check(abs(other["stop"] - r(4295.0)) < 1e-9 and abs(other["target"] - r(4325.0)) < 1e-9,
      "stop and target reflected about the last close")
check(abs(other["stop"] - other["entry"]["price"]) - abs(4295.0 - 4305.0) < 1e-9,
      "so the coin's stop distance from ITS entry equals the book's - the two are sized alike")
check(other["zone"] == [r(4307.0), r(4303.0)], "the zone is reflected and kept ordered lo <= hi", str(other["zone"]))
check(abs(other["invalidate_above"] - r(4290.0)) < 1e-9 and abs(other["invalidate_below"] - r(4315.0)) < 1e-9,
      "the invalidation levels swap roles on reflection: the book's below is the coin's above")
sok, swhy = A.sane(dict(other, reason="coin"), LAST)
check(sok, "the reflected plan passes sane() for the coin's own side", swhy)

# The case the market books' arithmetic gets wrong, which is why coin_plan
# reflects instead: a LONG stop-entry above the close with its stop ALSO
# above the close.
above = plan(kind="stop", price=LAST + 6, stop=LAST + 2, target=LAST + 20)
c = A.coin_plan(above, "SHORT", LAST)
check(abs(c["entry"]["price"] - (LAST - 6)) < 1e-9 and abs(c["stop"] - (LAST - 2)) < 1e-9,
      "a stop above the close mirrors to a stop below the close, 4 from the coin's entry as the book's is",
      str(c))
old_style = LAST + abs((LAST + 2) - LAST)  # "same distance on the coin's losing side" - the market formula
check(abs(c["stop"] - old_style) > 1, "and NOT the market formula's answer, which would sit 4 points further out")

market = plan(kind="market", price=None, stop=4300.0, target=4320.0)
c = A.coin_plan(market, "SHORT", LAST)
check(c["entry"] == {"type": "market", "price": None} and c["valid_bars"] is None,
      "a market plan mirrors to a market plan with null price and null valid_bars")
c = A.coin_plan(dict(plan(), target=None), "SHORT", LAST)
check(c["target"] is None, "a null target stays null")

# reflect() is the market books' own two expressions, so the same input
# lands on the same float.
for p in (4295.0, 4325.0, 4301.85, 4321.85):
    d_ = abs(p - LAST)
    old = LAST - d_ if p > LAST else LAST + d_
    check(A.reflect(p, LAST) == old, f"reflect({p}) is bit-identical to the market coin's expression")

# ---------------------------------------------------------------------------
# 4 - the fast loop, through main()
# ---------------------------------------------------------------------------
print("the fast loop")

T0 = 1_758_232_800_000  # 2026-09-18 22:00Z, a bar open
BAR = 900_000


def bars(n: int, last_open: int) -> list:
    out = []
    for i in range(n):
        t = last_open - (n - 1 - i) * BAR
        o = 4300.0 + i * 0.25
        out.append([t, o, o + 2.0, o - 1.5, o + 0.5])
    out[-1][4] = LAST
    return out


def detail(last_open: int, pending=None, open_pos=None) -> dict:
    return {
        "run": {"id": "r", "tf": "15m", "equity": 10_000.0, "net_usd": 0.0, "trades": 0,
                "open": open_pos, "pending_order": pending, "last_bar_time": last_open,
                "last_bar_close": LAST},
        "live": {"time": last_open + BAR, "open": LAST, "high": LAST + 0.4, "low": LAST - 0.3,
                 "close": LAST + 0.1, "bid": LAST + 0.05, "ask": LAST + 0.25, "at": last_open + BAR + 90_000},
        "bars": bars(45, last_open),
        "series": {},
    }


PENDING = {"type": "limit", "price": 4305.0, "side": "LONG", "stop": 4295.0, "target": 4325.0,
           "zone": None, "valid_until_bar_ms": T0 + 3 * BAR, "decided_at": T0 + BAR + 5_000,
           "invalidate_above": None, "invalidate_below": None, "lots": 0.1}

M1 = {"bars": [
    {"time": T0 + BAR - 60_000, "open": 4311.0, "high": 4311.9, "low": 4310.8, "close": LAST, "ticks": 40, "spread_mean": 0.21},
    {"time": T0 + BAR, "open": LAST, "high": 4312.3, "low": 4311.4, "close": 4312.0, "ticks": 33, "spread_mean": 0.20},
    {"time": T0 + BAR + 60_000, "open": 4312.0, "high": 4312.1, "low": 4309.9, "close": 4310.2, "ticks": 51, "spread_mean": 0.22},
], "forming": {"time": T0 + BAR + 120_000, "open": 4310.2, "high": 4310.5, "low": 4309.7, "close": 4309.9},
    "unavailable": False}

LIMIT_REPLY = json.dumps(plan(zone=[4303.0, 4307.0], below=4290.0))


class FakeAPI:
    """The six routes the trader uses, answered from a script."""

    def __init__(self, details: list):
        self.details, self.i = list(details), 0
        self.posts, self.gets = [], []

    def urlopen(self, req, timeout=None):
        url = req if isinstance(req, str) else req.full_url
        data = None if isinstance(req, str) else req.data
        if data is None:
            self.gets.append(url)
        else:
            self.posts.append((url.rsplit("/api/", 1)[1], json.loads(data)))
        if "/api/paper/guards" in url:
            body = {"effective": {"max_trades_per_day": 10, "daily_loss_limit_usd": 300,
                                  "cooldown_min": 15, "max_hold_ms": 14_400_000,
                                  "max_notional_pct_equity": 300}}
        elif "/api/paper/run/" in url:
            body = self.details[min(self.i, len(self.details) - 1)]
            self.i += 1
        elif "/api/paper/m1" in url:
            body = M1
        elif url.endswith("/api/paper/intent"):
            body = {"accepted": True}
        elif url.endswith("/api/paper/pending/act"):
            body = {"ok": True}
        else:
            raise AssertionError(f"unexpected route {url}")
        return io.BytesIO(json.dumps(body).encode("utf-8"))


def drive(variant: str, details: list, replies: list, polls: int, extra=None) -> tuple:
    """`polls` iterations of main() against a scripted desk and model.

    Returns `(api, model calls, decision rows)`. Each model call records the
    prompt it was given and the `thinking` it was asked with.
    """
    tmp = tempfile.mkdtemp(prefix="plan-st-")
    api = FakeAPI(details)
    calls, left = [], list(replies)

    def fake_ask(prompt, model, provider, key, timeout, usage=None, thinking=None):
        calls.append({"prompt": prompt, "thinking": thinking})
        if usage is not None:
            usage.update({"input": 3200, "cached_input": 3000 if len(calls) > 1 else 0, "output": 40})
        return (left.pop(0) if left else '{"side": "NONE", "reason": "nothing"}'), 12

    n = {"polls": 0}

    def sleep(_):
        n["polls"] += 1
        if n["polls"] >= polls:
            raise KeyboardInterrupt

    real = (A.ROOT, A.ask, urllib.request.urlopen, A.time.sleep, sys.argv, os.environ.get("DEEPSEEK_API_KEY"))
    try:
        A.ROOT, A.ask, urllib.request.urlopen, A.time.sleep = tmp, fake_ask, api.urlopen, sleep
        os.environ["DEEPSEEK_API_KEY"] = "selftest"
        sys.argv = ["ai_trader.py", "--model=deepseek-flash", "--run=r", "--control=c", "--seed=43",
                    f"--prompt-variant={variant}", "--fast-poll=0", "--poll=0"] + list(extra or [])
        try:
            A.main()
        except KeyboardInterrupt:
            pass
        path = os.path.join(tmp, "data", "paper", "r", "decisions.jsonl")
        rows = []
        if os.path.exists(path):
            with io.open(path, encoding="utf-8") as f:
                rows = [json.loads(line) for line in f if line.strip()]
        return api, calls, rows
    finally:
        A.ROOT, A.ask, urllib.request.urlopen, A.time.sleep, sys.argv = real[:5]
        if real[5] is None:
            os.environ.pop("DEEPSEEK_API_KEY", None)
        else:
            os.environ["DEEPSEEK_API_KEY"] = real[5]
        shutil.rmtree(tmp, ignore_errors=True)


FLIP = "LONG" if random.Random(43).random() < 0.5 else "SHORT"

# --- WAIT, then TRIGGER ---
WAIT = '{"action": "WAIT", "reason": "price still above the limit"}'
TRIGGER = '{"action": "TRIGGER", "reason": "M1 rejected 4310 twice"}'
api, calls, rows = drive("plan-trigger",
                         [detail(T0), detail(T0, pending=PENDING), detail(T0, pending=PENDING),
                          detail(T0, pending=None, open_pos=None)],
                         [LIMIT_REPLY, WAIT, TRIGGER], polls=4)
intents = [p for p in api.posts if p[0] == "paper/intent"]
acts = [p for p in api.posts if p[0] == "paper/pending/act"]
check(len(intents) == 2, "a limit plan posts two intents: the book's and the coin's", str(api.posts))
book_body = next((b for u, b in intents if b.get("run") == "r"), {})
coin_body = next((b for u, b in intents if b.get("run") == "c"), {})
check(book_body.get("entry") == {"type": "limit", "price": 4305.0} and book_body.get("valid_bars") == 2
      and book_body.get("zone") == [4303.0, 4307.0] and book_body.get("invalidate_below") == 4290.0
      and book_body.get("stop") == 4295.0 and book_body.get("target") == 4325.0,
      "the book's intent carries the extended fields", str(book_body))
check(coin_body.get("side") == FLIP and coin_body.get("entry", {}).get("type") == "limit"
      and coin_body.get("valid_bars") == 2 and coin_body.get("decider") == "coin",
      f"the coin's intent is a {FLIP} limit with the same valid_bars", str(coin_body))
want_coin = A.coin_plan(A.parse(LIMIT_REPLY, plan=True), FLIP, LAST)
check(all(abs(coin_body.get(k) - want_coin[k]) < 1e-9 for k in ("stop", "target"))
      and abs(coin_body["entry"]["price"] - want_coin["entry"]["price"]) < 1e-9,
      "at the mirrored distances coin_plan() gives", str(coin_body))
check(len(calls) == 3, "three model calls: the decision and two fast questions", str(len(calls)))
check(calls[0]["thinking"] is None, "the decision goes with the provider's default thinking")
check(all(c["thinking"] == "off" for c in calls[1:]), "every fast call goes with thinking off")
check(all(c["prompt"].startswith(calls[0]["prompt"]) for c in calls[1:]),
      "every fast prompt BEGINS with the decision prompt, byte for byte")
check(all(c["prompt"] != calls[0]["prompt"] for c in calls[1:]), "and is longer than it")
tail = calls[1]["prompt"][len(calls[0]["prompt"]):]
check("SINCE THAT DECISION" in tail and "22:15Z" in tail and "22:16Z" in tail and "22:14Z" not in tail,
      "the tail carries only the M1 bars after the decision bar (22:15Z on), oldest first", tail[:400])
check("quote now: bid 4311.9  ask 4312.1" in tail, "and the latest quote", tail)
check("forming minute" in tail, "and the forming minute")
check(len(acts) == 1 and acts[0][1] == {"run": "r", "action": "trigger", "reason": "M1 rejected 4310 twice"},
      "WAIT posts nothing; TRIGGER posts pending/act trigger with the reason, on the BOOK only", str(acts))
tc = [r for r in rows if r.get("kind") == "trigger_check"]
check(len(tc) == 2, "every fast call is a trigger_check row", str([r.get("kind") for r in rows]))
check([r["action"] for r in tc] == ["WAIT", "TRIGGER"], "with its action", str([r.get("action") for r in tc]))
check(all(r.get("cached_input") == 3000 and r["usage"]["input"] == 3200 and "latency_ms" in r for r in tc),
      "and usage, cached_input and latency")
check(tc[0]["acted"] is False and tc[1]["acted"] is True, "and whether it acted")
check(all(r["bar_time"] == T0 for r in tc), "stamped with the decision bar, so they join to their decision row")
dec = next((r for r in rows if r.get("kind") is None and r.get("posted")), {})
check(dec.get("entry") == {"type": "limit", "price": 4305.0}, "the decision row logs `entry`", str(dec.get("entry")))
check(dec.get("prompt") == calls[0]["prompt"], "and the prompt the fast calls repeat")

# --- CANCEL ---
CANCEL = '{"action": "CANCEL", "reason": "the level broke without a fill"}'
api, calls, rows = drive("plan-trigger", [detail(T0), detail(T0, pending=PENDING)],
                         [LIMIT_REPLY, CANCEL], polls=2)
acts = [p for p in api.posts if p[0] == "paper/pending/act"]
check(acts == [("paper/pending/act", {"run": "r", "action": "cancel",
                                      "reason": "the level broke without a fill"})],
      "CANCEL posts pending/act cancel with the reason", str(acts))

# --- an unreadable fast reply is WAIT ---
api, calls, rows = drive("plan-trigger", [detail(T0), detail(T0, pending=PENDING)],
                         [LIMIT_REPLY, "I would probably trigger here"], polls=2)
check(not [p for p in api.posts if p[0] == "paper/pending/act"]
      and [r["action"] for r in rows if r.get("kind") == "trigger_check"] == ["WAIT"],
      "an unreadable fast reply is WAIT and acts on nothing")

# --- after the order resolves, no more fast calls; a memory-less pending is left alone ---
api, calls, rows = drive("plan-trigger",
                         [detail(T0), detail(T0, pending=None), detail(T0, pending=PENDING)],
                         [LIMIT_REPLY, WAIT], polls=3)
check(len(calls) == 1, "once the desk reads no pending order the memory goes; a later order this "
                       "process did not post gets no fast call", str(len(calls)))

# --- a market plan keeps no memory ---
MARKET_REPLY = json.dumps(plan(kind="market", price=None, stop=4300.0, target=4320.0))
api, calls, rows = drive("plan-trigger", [detail(T0), detail(T0, pending=PENDING)],
                         [MARKET_REPLY, WAIT], polls=2)
check(len(calls) == 1, "a market entry has nothing to wait for: no fast call even if the desk shows a pending")
book_body = next((b for u, b in api.posts if u == "paper/intent" and b.get("run") == "r"), {})
check(book_body.get("entry") == {"type": "market", "price": None} and book_body.get("valid_bars") is None,
      "a market plan posts entry market with null price and null valid_bars", str(book_body))

# --- the plan variant NEVER runs the fast loop, and a new bar while pending is a skip ---
api, calls, rows = drive("plan", [detail(T0), detail(T0, pending=PENDING), detail(T0 + BAR, pending=PENDING)],
                         [LIMIT_REPLY, WAIT, LIMIT_REPLY], polls=3)
check(len(calls) == 1, "plan: one decision call and no fast call while the order waits", str(len(calls)))
check(not [p for p in api.posts if p[0] == "paper/pending/act"], "plan: nothing is triggered or cancelled")
skip = [r for r in rows if r.get("kind") == "pending_skip"]
check(len(skip) == 1 and skip[0]["bar_time"] == T0 + BAR and skip[0]["pending"] == PENDING,
      "a new bar while an order waits is a pending_skip row carrying the order", str([r.get("kind") for r in rows]))
check(len([p for p in api.posts if p[0] == "paper/intent"]) == 2,
      "and posts no intent - a NONE would replace the waiting order")

# --- the coin control is never the target of an act ---
api, calls, rows = drive("plan-trigger", [detail(T0), detail(T0, pending=PENDING), detail(T0, pending=PENDING)],
                         [LIMIT_REPLY, TRIGGER, TRIGGER], polls=3)
check(all(b.get("run") == "r" for u, b in api.posts if u == "paper/pending/act"),
      "every act names the book, never the coin")

# --- the market books are untouched: base posts the old body and the old coin arithmetic ---
OLD_REPLY = '{"side": "LONG", "stop": 4301.85, "target": 4331.85, "reason": "old shape"}'
api, calls, rows = drive("base", [detail(T0)], [OLD_REPLY], polls=1)
book_body = next((b for u, b in api.posts if u == "paper/intent" and b.get("run") == "r"), {})
coin_body = next((b for u, b in api.posts if u == "paper/intent" and b.get("run") == "c"), {})
check("entry" not in book_body and "valid_bars" not in book_body, "base posts no plan fields", str(book_body))
d_ = abs(4301.85 - LAST)
old_stop = LAST - d_ if FLIP == "LONG" else LAST + d_
td = abs(4331.85 - LAST)
old_target = LAST + td if FLIP == "LONG" else LAST - td
check(coin_body.get("stop") == old_stop and coin_body.get("target") == old_target and "entry" not in coin_body,
      "base's coin uses the market formula, bit for bit", str(coin_body))
check(rows and rows[0].get("entry") is None and "prompt_variant" in rows[0],
      "a base row carries entry: null")
check("PLAN" not in calls[0]["prompt"] and "THIS BOOK ANSWERS WITH A PLAN" not in calls[0]["prompt"],
      "and base's prompt has no plan block")

# --- dry run: decides and logs, posts and acts on nothing ---
api, calls, rows = drive("plan-trigger", [detail(T0), detail(T0, pending=PENDING)],
                         [LIMIT_REPLY, TRIGGER], polls=2, extra=["--dry-run"])
check(not api.posts, "dry run posts nothing at all", str(api.posts))
check(len(calls) == 1, "and, having posted no order, asks no fast question")

print()
print(f"{'all checks passed' if not fails else str(len(fails)) + ' FAILED'}")
sys.exit(1 if fails else 0)
