"""Exercise the watch's alerting logic on synthetic payloads.

    C:\\Python39\\python.exe py/live/telegram_notify_selftest.py

No token is read, no message is sent and no route is called. Every function
under test is pure over a status/accounts payload, which is the only way this
file can be checked at all: the channel is a real group and the bot's token is
a credential this has no business touching.

What it pins, in the order the gaps were reported on 2026-09-17:

  1  a funded book is read at three bars and a paper book at four, and the
     60-minute outage that went unreported this morning now fires on the
     funded side and still does not on the paper side
  2  an account not mirroring a book it is configured for is announced BY NAME,
     once, on the transition, and the recovery is announced too
  3  a real-money gap is announced with the market shut; a paper gap is not
  4  an executor refusal is announced once, a repeat is silent, and an event
     older than the freshness window on first sight is learned rather than
     replayed into the channel
"""

from __future__ import annotations

import datetime as dt
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telegram_notify as T  # noqa: E402

FAIL = 0


def check(name: str, got, want) -> None:
    global FAIL
    ok = got == want
    if not ok:
        FAIL += 1
    print(f"  {'ok  ' if ok else 'FAIL'} {name}")
    if not ok:
        print(f"       got  {got!r}")
        print(f"       want {want!r}")


def at(y, m, d, hh, mm=0) -> float:
    """A UTC instant in milliseconds."""
    return dt.datetime(y, m, d, hh, mm, tzinfo=dt.timezone.utc).timestamp() * 1000


def book(rid, tf="15m", market="xauusd", last=None, **kw) -> dict:
    r = {"id": rid, "tf": tf, "market": market, "last_bar_time": last,
         "net_usd": 0.0, "trades": 0, "open": None, "brokers": []}
    r.update(kw)
    return r


def main() -> int:
    # A Wednesday at 14:00 UTC: metals open, nowhere near the 21:00 halt.
    now = at(2026, 9, 16, 14, 0)

    print("1. the feed threshold, funded against paper")
    sixty = book("xau-ema", last=now - 60 * 60_000)
    check("60 min on a paper 15m book is NOT dead (4 bars, strict >)",
          T.is_dead(sixty, now, T.STALE_BARS), False)
    check("60 min on a FUNDED 15m book is dead (3 bars)",
          T.is_dead(sixty, now, T.STALE_BARS_REAL), True)
    check("45 min is the funded boundary and is not dead either (strict >)",
          T.is_dead(book("x", last=now - 45 * 60_000), now, T.STALE_BARS_REAL), False)
    check("46 min on a funded book is dead",
          T.is_dead(book("x", last=now - 46 * 60_000), now, T.STALE_BARS_REAL), True)
    check("the default is still the paper threshold",
          T.is_dead(sixty, now), False)
    # The whole point of the parameter: the same book, same instant, two answers.
    check("a 5m funded book is dead after 16 min",
          T.is_dead(book("x", tf="5m", last=now - 16 * 60_000), now, T.STALE_BARS_REAL), True)

    print("\n2. which books count as funded")
    accounts = [
        {"id": "vantage-cent", "real_money": True, "dry_run": True,
         "runs": ["xau-ema", "xau-close"], "mirroring": ["xau-ema", "xau-close"]},
        {"id": "vantage-demo", "real_money": False, "dry_run": False,
         "runs": ["xau-box-5m"], "mirroring": ["xau-box-5m"]},
    ]
    check("real_money wins even while the account is dry",
          T.funded_books(accounts), {"xau-ema", "xau-close"})
    check("no accounts payload means no funded books, not a crash",
          T.funded_books([]), set())

    print("\n3. changes() applies the tighter threshold to a funded book")
    runs = [book("xau-ema", last=now - 60 * 60_000),
            book("xau-box-5m", last=now - 60 * 60_000)]
    state = {"runs": {r["id"]: {"trades": 0, "net": 0.0, "open": None, "dead": False,
                                "mute": False, "mirrors": 0, "refused": None}
                      for r in runs}}
    lines = T.changes(runs, state, now, {"xau-ema"})
    check("the funded book is reported dead", any("xau-ema" in l and "feed dead" in l for l in lines), True)
    check("the paper book is not", any("xau-box-5m" in l for l in lines), False)

    print("\n4. an account that stops mirroring, by name")
    st: dict = {}
    gap = [{"id": "vantage-cent", "real_money": True, "runs": ["a", "b", "c"],
            "mirroring": ["a", "b", "c"]}]
    check("a fresh state learns and says nothing", T.mirror_gaps(gap, st, now), [])
    gap[0]["mirroring"] = ["a"]
    out = T.mirror_gaps(gap, st, now)
    check("the drop is announced once", len(out), 1)
    check("and it names the books", "<code>b</code>" in out[0] and "<code>c</code>" in out[0], True)
    check("and says it is real money", "REAL MONEY" in out[0], True)
    check("and counts them", "2 of 3" in out[0], True)
    check("the same gap on the next poll is silent", T.mirror_gaps(gap, st, now), [])
    gap[0]["mirroring"] = ["a", "b", "c"]
    back = T.mirror_gaps(gap, st, now)
    check("the recovery is announced", len(back) == 1 and "again" in back[0], True)

    print("\n5. a PARTIAL loss is what the per-book line cannot see")
    st2: dict = {}
    two = [{"id": "vantage-cent", "real_money": True, "runs": ["x"], "mirroring": ["x"]},
           {"id": "vantage-demo", "real_money": False, "runs": ["x"], "mirroring": ["x"]}]
    T.mirror_gaps(two, st2, now)
    two[0]["mirroring"] = []          # the funded mirror dies; the demo one lives
    out2 = T.mirror_gaps(two, st2, now)
    check("the funded account's loss is announced on its own",
          len(out2) == 1 and "vantage-cent" in out2[0], True)

    print("\n6. market suppression, and the judgement it encodes")
    shut = at(2026, 9, 19, 12, 0)     # a Saturday: metals closed
    st3: dict = {}
    both = [{"id": "vantage-cent", "real_money": True, "runs": ["g"], "mirroring": ["g"]},
            {"id": "vantage-demo", "real_money": False, "runs": ["g"], "mirroring": ["g"]}]
    markets = {"g": "xauusd"}
    T.mirror_gaps(both, st3, shut, markets)
    both[0]["mirroring"] = []
    both[1]["mirroring"] = []
    out3 = T.mirror_gaps(both, st3, shut, markets)
    check("with the market shut only the real-money account speaks",
          len(out3) == 1 and "vantage-cent" in out3[0], True)
    check("btc is never suppressed",
          T.mirror_gaps(
              [{"id": "d", "real_money": False, "runs": ["b1"], "mirroring": []}],
              {"accounts": {"d": {"missing": []}}}, shut, {"b1": "btc"}) != [], True)

    print("\n7. the executor refusing to start")
    st4: dict = {}
    fresh = [{"at": now - 60_000, "kind": "autotrading-off"}]
    out4 = T.broker_alarms("xau-ema", "vantage-cent", fresh, st4, now)
    check("a fresh refusal is announced", len(out4), 1)
    check("it names the book and the account",
          "xau-ema" in out4[0] and "vantage-cent" in out4[0], True)
    check("the same event next poll is silent",
          T.broker_alarms("xau-ema", "vantage-cent", fresh, st4, now), [])
    newer = fresh + [{"at": now - 10_000, "kind": "refused",
                      "reason": "AutoTrading is disabled in the terminal"}]
    out5 = T.broker_alarms("xau-ema", "vantage-cent", newer, st4, now)
    check("a NEWER refusal is announced", len(out5), 1)
    check("and carries the reason verbatim",
          "AutoTrading is disabled in the terminal" in out5[0], True)
    check("a sizing refusal is not alarming",
          T.broker_alarms("b", "acc", [{"at": now, "kind": "refused-size"}], {}, now), [])
    old = [{"at": now - 3 * 24 * 3_600_000, "kind": "autotrading-off"}]
    st5: dict = {}
    check("an old event on FIRST sight is learned, not replayed",
          T.broker_alarms("b", "acc", old, st5, now), [])
    check("and having been learned, it stays quiet",
          T.broker_alarms("b", "acc", old, st5, now), [])

    print("\n8. the watch noticing it has gone blind")
    st6: dict = {"had_accounts": True}
    full = [{"id": "a", "runs": [], "mirroring": [], "real_money": False}]
    check("a complete payload says nothing", T.blind_spots(full, st6), [])
    thin = [{"id": "a", "runs": [], "mirroring": []}]          # real_money dropped
    out6 = T.blind_spots(thin, st6)
    check("a dropped field is announced once", len(out6), 1)
    check("and names the field", "real_money" in out6[0], True)
    check("the same payload next poll is silent", T.blind_spots(thin, st6), [])
    back2 = T.blind_spots(full, st6)
    check("the recovery is announced", len(back2) == 1 and "complete again" in back2[0], True)
    check("an empty registry where there were accounts is announced",
          len(T.blind_spots([], st6)) == 1, True)
    check("and not repeated", T.blind_spots([], st6), [])
    check("a desk that never had accounts says nothing",
          T.blind_spots([], {}), [])

    print("\n9. a STOP that did not get flat")
    st7: dict = {}
    clean = [{"at": now - 30_000, "kind": "stopped", "reason": "STOP file present"}]
    out7 = T.stop_report("xau-ema", "vantage-cent", clean, st7, now)
    check("a clean stop is announced, because the gap line alone is ambiguous",
          len(out7) == 1 and "closed flat" in out7[0], True)
    check("and not repeated", T.stop_report("xau-ema", "vantage-cent", clean, st7, now), [])
    st8: dict = {}
    dirty = [
        {"at": now - 31_000, "kind": "order-failed", "action": "close",
         "retcode": 10027, "comment": "AutoTrading disabled by client"},
        {"at": now - 30_000, "kind": "stopped", "reason": "STOP file present"},
    ]
    out8 = T.stop_report("xau-ema", "vantage-cent", dirty, st8, now)
    check("a stop whose close was refused is announced as a failure", len(out8), 1)
    check("and says the position may still be open",
          "REFUSED" in out8[0] and "still be open" in out8[0], True)
    check("and carries the retcode that decides what to do",
          "10027" in out8[0], True)
    far = [
        {"at": now - 10 * 60_000, "kind": "order-failed", "action": "open", "retcode": 10019},
        {"at": now - 30_000, "kind": "stopped", "reason": "STOP file present"},
    ]
    out9 = T.stop_report("b", "acc", far, {}, now)
    check("an unrelated older failure is not blamed on the stop",
          len(out9) == 1 and "closed flat" in out9[0], True)
    check("an old stop on first sight is learned, not replayed",
          T.stop_report("b", "acc",
                        [{"at": now - 3 * 24 * 3_600_000, "kind": "stopped"}], {}, now), [])

    print("\n10. order-failed is alarming on its own")
    outA = T.broker_alarms("b", "acc", [{"at": now, "kind": "order-failed",
                                         "action": "open", "retcode": 10027,
                                         "comment": "AutoTrading disabled by client"}], {}, now)
    check("it is announced", len(outA), 1)
    check("with the retcode and the comment, not just the word failed",
          "10027" in outA[0] and "AutoTrading" in outA[0], True)

    print("\n11. the broker resizing a healthy book")
    st9: dict = {}
    few = [{"at": now, "kind": "clipped", "asked": 0.004, "sending": 0.01}]
    check("one clip is not a condition", T.clipping("b", "acc", few, st9, now), [])
    many = [{"at": now - i * 60_000, "kind": "clipped", "asked": 0.004, "sending": 0.01}
            for i in range(4)]
    outB = T.clipping("b", "acc", many, st9, now)
    check("a sustained clip is announced once", len(outB), 1)
    check("and says which way it was resized", "resized UP" in outB[0], True)
    check("and gives both sizes", "0.004" in outB[0] and "0.01" in outB[0], True)
    check("the same condition next check is silent", T.clipping("b", "acc", many, st9, now), [])
    backC = T.clipping("b", "acc", [], st9, now)
    check("the recovery is announced", len(backC) == 1 and "no longer" in backC[0], True)
    down = [{"at": now - i * 60_000, "kind": "clipped", "asked": 90.0, "sending": 50.0}
            for i in range(4)]
    outD = T.clipping("c", "acc", down, {}, now)
    check("a maximum clamp reads as resized down", "resized down" in outD[0], True)

    print("\n12. a model that lost its session, and a book nobody drives")
    live = now - 10 * 60_000          # bars arriving: 10 min behind on a 15m book
    silent = book("ai-xau-opus-ctx", last=live, strategy="external",
                  decider={"last": "claude-opus-5", "last_at": now - 50 * 60_000,
                           "decisions": {}, "stood_aside": 4})
    check("a decider quiet for 50 min on a live feed is gone (3 bars = 45)",
          T.decider_gone(silent, now), True)
    check("a coin decider is never alarmed on",
          T.decider_gone(book("c", last=live, decider={"last": "coin", "last_at": 0}), now), False)
    dead_feed = book("x", last=now - 10 * 3_600_000, strategy="external",
                     decider={"last": "m", "last_at": now - 10 * 3_600_000})
    check("a dead FEED suppresses it - one outage, one alarm",
          T.decider_gone(dead_feed, now), False)
    check("and the suppression uses the SAME threshold the caller did",
          T.decider_gone(book("x", tf="15m", last=now - 55 * 60_000, strategy="external",
                              decider={"last": "m", "last_at": now - 55 * 60_000}),
                         now, T.STALE_BARS_REAL), False)
    holding = book("h", last=live, strategy="external", open={"side": "LONG"},
                   decider={"last": "m", "last_at": now - 3 * 3_600_000})
    check("a book holding a position is not asked, so it is not alarmed on",
          T.decider_gone(holding, now), False)

    check("the cause is read out of the trader's own log",
          T.cause_of([{"at": now, "response": "ERROR: RuntimeError: claude exited 1"}]),
          "RuntimeError: claude exited 1")
    check("and out of the decision reason when there is no response",
          T.cause_of([{"at": now, "reason": "the model was unreachable; NOT a decision: TimeoutError"}]),
          "TimeoutError")
    check("a healthy decision yields no cause",
          T.cause_of([{"at": now, "response": '{"side":"NONE"}', "reason": "chop"}]), "")
    check("the NEWEST entry wins, not the first",
          T.cause_of([{"at": now - 9_999, "response": "ERROR: OldError: x"},
                      {"at": now, "response": "ERROR: NewError: y"}]), "NewError: y")

    never = book("ai-xau-terra-ctx", last=live, strategy="external",
                 started_at=now - 3 * 3_600_000, decider=None)
    check("an external book nobody has ever posted to is announced",
          T.undriven(never, now), True)
    check("decider_gone CANNOT see that case - which is why undriven exists",
          T.decider_gone(never, now), False)
    check("a rule-based book with no decider is normal, not undriven",
          T.undriven(book("xau-ema", last=live, strategy="ema-cross",
                          started_at=now - 3 * 3_600_000), now), False)
    check("a book created a minute ago is waiting, not undriven",
          T.undriven(book("new", last=live, strategy="external",
                          started_at=now - 60_000), now), False)
    check("a driven book is not undriven",
          T.undriven(book("d", last=live, strategy="external", started_at=now - 3 * 3_600_000,
                          decider={"last": "m", "last_at": now}), now), False)

    # The first pass must see a HEALTHY decider, or it records mute=True while
    # learning and the second pass is no longer a transition — which is what
    # the first version of this check got wrong, not the code.
    st10: dict = {}
    healthy = book("ai-xau-opus-ctx", last=live, strategy="external",
                   decider={"last": "claude-opus-5", "last_at": now, "decisions": {}, "stood_aside": 4})
    T.changes([healthy], st10, now)                  # learns, announces nothing
    out12 = T.changes([silent], st10, now, set(), {"ai-xau-opus-ctx": "RuntimeError: claude exited 1"})
    check("the alert carries the cause when one was fetched",
          any("RuntimeError" in l for l in out12), True)

    print("\n13. the account holding a different shape from the book")
    st11: dict = {}
    def with_broker(**kw):
        return book("xau-ema", last=live, brokers=[dict({"account": "vantage-cent",
                                                         "at": now - 5_000}, **kw)])
    check("no drift field at all is inert - it is not on the wire yet",
          T.drifts(with_broker(), st11, now), [])
    check("and an explicit null is inert too",
          T.drifts(with_broker(drift=None), st11, now), [])
    d1 = T.drifts(with_broker(drift="2 positions open on one book (tickets 51, 52)"), st11, now)
    check("a drift is announced once", len(d1), 1)
    check("quoting the executor's own sentence, tickets and all",
          "tickets 51, 52" in d1[0], True)
    check("and carrying the bound rather than an alarm",
          "clears when the book next goes flat" in d1[0], True)
    check("and the thing the bound implies",
          "running twice" in d1[0], True)
    check("the same drift next poll is silent",
          T.drifts(with_broker(drift="2 positions open on one book (tickets 51, 52)"), st11, now), [])
    d2 = T.drifts(with_broker(drift="3 positions open on one book (tickets 51, 52, 53)"), st11, now)
    check("a drift that CHANGES shape is announced again", len(d2), 1)
    back3 = T.drifts(with_broker(), st11, now)
    check("and the recovery is announced when it clears",
          len(back3) == 1 and "matches the book again" in back3[0], True)
    stale_only = book("x", last=live, brokers=[{"account": "a", "at": now - 10 * 60_000,
                                                "drift": "holding 0.03 lots"}])
    check("a mirror that stopped reporting is not read for drift",
          T.drifts(stale_only, {}, now), [])

    print("\n14. entry lag, measured rather than assumed")
    bar = now - 20 * 60_000
    evs = [
        {"kind": "opened", "time": bar, "learned_at": bar + 900_000},      # one bar
        {"kind": "opened", "time": bar, "learned_at": bar + 2_000},        # after the fix
        {"kind": "trade"}, {"kind": "gap"},                                # not entries
        {"kind": "opened", "time": bar},                                   # no learned_at
        {"kind": "opened", "learned_at": bar},                             # no bar time
    ]
    got = T.lags(evs, now)
    check("only opened lines with BOTH clocks are counted", sorted(got), [2.0, 900.0])
    check("a line missing a clock is skipped, not counted as zero", 0.0 in got, False)
    old_entry = [{"kind": "opened", "time": now - 40 * 3_600_000,
                  "learned_at": now - 40 * 3_600_000 + 900_000}]
    check("an entry older than the window is out", T.lags(old_entry, now), [])

    check("no entries reads as no entries, not as zero lag",
          "no entries in 24h" in T.lag_line("b", []), True)
    line = T.lag_line("ai-xau-terra-ctx", [2.0, 900.0, 910.0])
    check("the line carries the count, the median and the max",
          "3 entries" in line and "median 900s" in line and "max 910s" in line, True)
    check("the median is the middle and not the mean",
          "median 900s" in T.lag_line("b", [1.0, 900.0, 1000.0]), True)

    st12: dict = {}
    check("the alarm is SILENT at the shipped default of 0",
          T.lag_alarms("b", evs, st12, now, 0), [])
    a1 = T.lag_alarms("b", evs, st12, now, 60)
    check("at 60s the slow entry alarms and the fast one does not", len(a1), 1)
    check("and it says how late and against what threshold",
          "900s later" in a1[0] and "60s threshold" in a1[0], True)
    check("the same entry is not announced twice",
          T.lag_alarms("b", evs, st12, now, 60), [])
    check("the default constant really is off", T.LAG_ALARM_DEFAULT_S, 0)

    print(f"\n{'all checks passed' if not FAIL else str(FAIL) + ' CHECK(S) FAILED'}")
    return 1 if FAIL else 0


if __name__ == "__main__":
    sys.exit(main())
