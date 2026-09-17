# -*- coding: utf-8 -*-
"""Options-flow positioning from live.otldata.com, as CONTEXT for a prompt.

    py -3.9 py/live/otl_context.py            # fetch once and print the block

One job: fetch what a third party computes from the COMEX gold options tape,
and render it as a block a model can read. It decides nothing, it is not a
signal, and nothing else in this repository reads it.

WHAT IT IS AND IS NOT. These numbers describe POSITIONING - where option
premium sits, which strike carries the most gamma, what the tape's own whale
levels are. They are not a forecast and this desk has no evidence they predict
direction: twenty-five registrations tested levels of exactly this kind as
mechanical rules and none survived out of sample. The experiment this module
exists for tests something different - whether the same numbers, as CONTEXT
for a model rather than as a rule, change what it decides. See
docs/hypotheses/2026-09-17-otl-context.md.

THE CLOCK. The feed renders every time in the account's zone, UTC+7, including
strings carrying a "+00:00" suffix that is not true. Measured 2026-09-12 and
pinned by a test in fd-ingest; see docs/decisions/2026-09-12-otl-timestamps.md.
The offset is read from `config/default.toml` rather than written here so that
one file owns it, and everything this module prints is UTC.

FAILING IS A RESULT, NOT AN EXCEPTION. If the feed is unreachable or its data
is older than a bar, this says so and the caller puts that in the prompt and in
the record. A silent fallback to "no context" would put context-absent
decisions into a context-present book, and the experiment would be measuring a
mixture with nothing to separate it.
"""
from __future__ import annotations

import json
import os
import sys
import time
import urllib.error
import urllib.request

for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")
    except (AttributeError, ValueError):
        pass

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))

# One request's patience, and the whole gather's. The decision loop runs every
# fifteen minutes and must not be held by a third party: past the budget the
# context is simply absent, which is a state the prompt and the record can
# both express.
REQUEST_TIMEOUT_S = 6.0
TOTAL_BUDGET_S = 20.0

# How old the feed's own newest number may be before this is called stale. One
# 15-minute bar: context from the bar before the one being decided is still
# about the same market, context from an hour ago is describing a different
# one and the model should be told rather than left to assume.
STALE_AFTER_MS = 15 * 60 * 1000


def _config() -> tuple:
    """`(base_url, utc_offset_ms)` from config/default.toml.

    Read rather than hardcoded: the offset is a property of the feed that one
    file already owns, and a second copy here is a second thing to be wrong
    the day the feed changes zone.
    """
    base, hours = "https://live.otldata.com", 7.0
    try:
        import tomli
        with open(os.path.join(ROOT, "config", "default.toml"), "rb") as f:
            src = (tomli.load(f).get("sources") or {}).get("reference") or {}
        base = str(src.get("base_url") or base)
        hours = float(src.get("utc_offset_hours", hours))
    except Exception:  # noqa: BLE001 - a missing config must not stop a decision
        pass
    return base.rstrip("/"), int(hours * 3600 * 1000)


def _get(base: str, path: str, deadline: float):
    if time.time() >= deadline:
        return None
    try:
        req = urllib.request.Request(base + path, headers={"Accept": "application/json"})
        with urllib.request.urlopen(req, timeout=min(REQUEST_TIMEOUT_S, deadline - time.time())) as r:
            return json.loads(r.read().decode("utf-8", "replace"))
    except Exception:  # noqa: BLE001 - every failure here is "no context"
        return None


def _to_utc_ms(stamp, offset_ms: int):
    """Feed timestamp -> UTC epoch ms, with the offset removed.

    The suffix on the string is ignored on purpose. The feed writes "+00:00"
    on times that are UTC+7, so trusting the suffix is how this was wrong for
    a day; the only correct reading is "naive local, minus the offset".
    """
    if stamp is None:
        return None
    if isinstance(stamp, (int, float)):
        ms = float(stamp) * (1000.0 if float(stamp) < 1e11 else 1.0)
        return int(ms) - offset_ms
    s = str(stamp).strip().replace("Z", "")
    for cut in ("+", ):
        if cut in s[10:]:
            s = s[:10] + s[10:].split(cut, 1)[0]
    s = s.replace("T", " ").strip()
    for fmt in ("%Y-%m-%d %H:%M:%S.%f", "%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M"):
        try:
            import datetime as dt
            naive = dt.datetime.strptime(s, fmt).replace(tzinfo=dt.timezone.utc)
            return int(naive.timestamp() * 1000) - offset_ms
        except ValueError:
            continue
    return None


def _last_rle(rle):
    """The current value of a run-length-encoded level.

    `levels_rle` holds `[tradeIndex, value]` pairs, appended only when the
    level changes, so the last pair is the level as of the newest print.
    Verified against a cached OGV6 payload: whale_sup 3 pairs ending
    [4485, 4200], whale_res 1 pair [0, 5000], over 7,698 trades.
    """
    if not rle:
        return None
    try:
        return rle[-1][1]
    except (IndexError, TypeError):
        return None


def gather(now_ms: int | None = None) -> dict:
    """Everything the block needs, or as much of it as the feed gave.

    Never raises. Returns a dict always carrying `state`, one of `ok`,
    `stale`, `unavailable` - the three the caller must be able to tell apart.
    """
    base, offset_ms = _config()
    now_ms = int(time.time() * 1000) if now_ms is None else now_ms
    deadline = time.time() + TOTAL_BUDGET_S
    out = {"state": "unavailable", "base": base, "fields": {}, "age_ms": None, "as_of_ms": None}

    snap_doc = _get(base, "/api/options/snapshots?hours=6&limit=200", deadline)
    rows = (snap_doc or {}).get("snapshots") if isinstance(snap_doc, dict) else snap_doc
    rows = [r for r in (rows or []) if isinstance(r, dict)]
    if not rows:
        return out

    rows.sort(key=lambda r: _to_utc_ms(r.get("timestamp"), offset_ms) or 0)
    last = rows[-1]
    as_of = _to_utc_ms(last.get("timestamp"), offset_ms)
    out["as_of_ms"] = as_of
    out["age_ms"] = None if as_of is None else now_ms - as_of
    out["state"] = "ok" if (out["age_ms"] is not None and out["age_ms"] <= STALE_AFTER_MS) else "stale"

    f = out["fields"]
    for k in ("atm_price", "avg_iv", "bull_total", "bear_total", "poc", "above_poc",
              "under_poc", "max_pain", "exp_move", "min_1d", "max_1d"):
        f[k] = last.get(k)

    # Whether average IV rose or fell across the series we just read. Reported
    # as the pair rather than a word, so "rose" cannot outlive the numbers.
    first_iv = next((r.get("avg_iv") for r in rows if r.get("avg_iv") is not None), None)
    if first_iv is not None and last.get("avg_iv") is not None:
        span = as_of - (_to_utc_ms(rows[0].get("timestamp"), offset_ms) or as_of)
        f["iv_from"], f["iv_to"] = first_iv, last.get("avg_iv")
        f["iv_span_h"] = round(max(span, 0) / 3_600_000.0, 1)

    greeks = _get(base, "/api/options/analysis/greeks", deadline)
    if isinstance(greeks, dict):
        f["max_gex_strike"] = ((greeks.get("summary") or {}).get("max_gex_strike"))

    prof = _get(base, "/api/options/alldte-profile?tf=24h", deadline)
    if isinstance(prof, dict):
        for k in ("poc", "vah", "val"):
            f["alldte_" + k] = prof.get(k)

    # ONE fetch for three things: the bull/bear split, the whale levels and
    # the window they cover.
    #
    # `alldte-data?tf=` is NOT a snapshot aggregate, whatever the endpoint
    # table suggests - measured against a real response on 2026-09-17. It is
    # a LOOKBACK WINDOW over the whole tape: `tf=weekly` returned 7,915 prints
    # spanning 3.8 days across 39 contracts of every type, each print carrying
    # `symbol`, a `contracts` list mapping symbol to daily/weekly/monthly, and
    # the level columns FLAT rather than run-length encoded.
    #
    # So this one call replaces the two it used to take - active-contracts
    # plus a per-contract chart-data - and costs less than they did together.
    # 1.2 MB, parsed in 17 ms; the cost is the download and it sits well
    # inside the budget.
    #
    # `tf=weekly` and not something shorter BECAUSE one of the two lines is a
    # WEEKLY figure, and a 24-hour window cannot produce one. Quoting a day's
    # premium under a weekly label is the mislabelling this desk spent the
    # week removing. `tf=daily` is also a bare `[]` at some hours, which is a
    # second reason not to build on it.
    alldte = _get(base, "/api/options/alldte-data?tf=weekly", deadline)
    tape = (alldte or {}).get("trades") if isinstance(alldte, dict) else None
    if isinstance(tape, dict) and isinstance(tape.get("x"), list) and tape["x"]:
        kinds = {}
        for c in (alldte.get("contracts") or []):
            if isinstance(c, dict) and c.get("symbol"):
                kinds[c["symbol"]] = c.get("type")
        cls = tape.get("class") or []
        side = tape.get("side") or []
        prem = tape.get("premium") or []
        sym = tape.get("symbol") or []
        sums = {"daily": [0.0, 0.0], "weekly": [0.0, 0.0]}
        unclassified = 0
        for i in range(len(tape["x"])):
            kind = kinds.get(sym[i]) if i < len(sym) else None
            if kind is None:
                unclassified += 1
                continue
            if kind not in sums:
                continue
            # The feed's own four buckets (methodology 2.2): bull is a call
            # bought or a put sold, bear is a put bought or a call sold.
            # `side` is the AGGRESSOR and not open/close.
            #
            # `premium` is used as the feed publishes it rather than
            # recomputed as price x size x 100. The two agree on 173 of 200
            # sampled prints, and where they differ the feed's own number is
            # the one its other endpoints are consistent with.
            bull = ((cls[i] == "C" and side[i] == "LONG")
                    or (cls[i] == "P" and side[i] == "SHORT"))
            sums[kind][0 if bull else 1] += (prem[i] or 0)
        f["daily_bull"], f["daily_bear"] = sums["daily"]
        f["weekly_bull"], f["weekly_bear"] = sums["weekly"]
        f["flow_unclassified"] = unclassified
        f["flow_prints"] = len(tape["x"])
        a, b = _to_utc_ms(tape["x"][0], offset_ms), _to_utc_ms(tape["x"][-1], offset_ms)
        if a is not None and b is not None:
            f["flow_window_h"] = round(max(b - a, 0) / 3_600_000.0, 1)

        # The whale levels come from the same payload as flat per-print
        # columns, across every contract in the window rather than one
        # symbol's tape. That is what a positioning block should show, and it
        # removes the rolling-front-month problem instead of working around
        # it.
        for k in ("whale_sup", "whale_res"):
            col = tape.get(k)
            if isinstance(col, list) and col:
                f[k] = col[-1]

    big = _get(base, "/api/options/big-trades?min_premium=250000&limit=40", deadline)
    prints = big.get("trades") if isinstance(big, dict) else big
    cut = now_ms - 8 * 3600 * 1000
    keep = []
    for p in (prints or []):
        if not isinstance(p, dict):
            continue
        t = _to_utc_ms(p.get("x") or p.get("timestamp") or p.get("time"), offset_ms)
        if t is None or t < cut:
            continue
        keep.append({"t": t, "strike": p.get("strike"), "class": p.get("class"),
                     "side": p.get("side"), "premium": p.get("premium")})
    keep.sort(key=lambda p: -(p.get("premium") or 0))
    f["big_prints"] = keep[:5]
    return out


def _u(ms):
    import datetime as dt
    if ms is None:
        return "?"
    return dt.datetime.utcfromtimestamp(ms / 1000).strftime("%H:%MZ")


def block(ctx: dict) -> str:
    """The lines that go into the prompt. UTC, units on every number."""
    if ctx.get("state") == "unavailable":
        return ("OPTIONS POSITIONING (third-party, COMEX gold options tape): UNAVAILABLE for this bar.\n"
                "You are deciding without it. Do not guess at what it would have said.")
    f = ctx.get("fields") or {}
    age_min = None if ctx.get("age_ms") is None else int(ctx["age_ms"] / 60000)
    head = [
        "OPTIONS POSITIONING - computed by a third party (live.otldata.com) from the COMEX gold",
        "options tape. It describes POSITIONING, not direction: where premium and gamma sit, not",
        "where price will go. It is context, like everything else in this section, and not a signal.",
        f"As of {_u(ctx.get('as_of_ms'))} ({age_min if age_min is not None else '?'} min old).",
    ]
    if ctx.get("state") == "stale":
        head.append(f"STALE: this is older than one bar ({age_min} min). Weigh it accordingly.")
    L = []

    def add(label, value, unit=""):
        if value is None:
            L.append(f"  {label}: not reported")
        else:
            L.append(f"  {label}: {value}{unit}")

    add("gamma wall (max GEX strike)", f.get("max_gex_strike"), " USD/oz")
    add("all-DTE POC", f.get("alldte_poc"), " USD/oz")
    add("all-DTE value area high", f.get("alldte_vah"), " USD/oz")
    add("all-DTE value area low", f.get("alldte_val"), " USD/oz")
    add("whale support (all contracts)", f.get("whale_sup"), " USD/oz")
    add("whale resistance (all contracts)", f.get("whale_res"), " USD/oz")
    add("ATM (futures)", f.get("atm_price"), " USD/oz")
    add("expected move, 1 day", f.get("exp_move"), " USD/oz")
    if f.get("iv_from") is not None:
        d = f["iv_to"] - f["iv_from"]
        L.append(f"  average IV: {f['iv_to']:.4f} now, {f['iv_from']:.4f} {f.get('iv_span_h', '?')}h ago "
                 f"({d:+.4f})")
    else:
        add("average IV", f.get("avg_iv"))
    # Labelled by the feed's OWN contract class - daily-expiry, weekly-expiry
    # - and never "0DTE", which would be a claim about days to expiry that
    # this is not measuring. A daily-expiry contract is usually but not always
    # today's. The window is on the line because a premium total without one
    # is a number nobody can compare to anything.
    win = f.get("flow_window_h")
    span = f" over the last {win}h" if win is not None else ""
    if f.get("daily_bull") is None:
        L.append("  daily-expiry premium: not reported")
    else:
        L.append(f"  daily-expiry contracts{span}: bull {f['daily_bull']:,.0f} USD, "
                 f"bear {f['daily_bear']:,.0f} USD")
    if f.get("weekly_bull") is None:
        L.append("  weekly-expiry premium: not reported")
    else:
        L.append(f"  weekly-expiry contracts{span}: bull {f['weekly_bull']:,.0f} USD, "
                 f"bear {f['weekly_bear']:,.0f} USD")
    if f.get("flow_unclassified"):
        L.append(f"    ({f['flow_unclassified']} of {f.get('flow_prints')} prints had no "
                 f"contract class and are in neither total)")
    prints = f.get("big_prints") or []
    if prints:
        L.append("  largest prints, last 8h (strike USD/oz, C/P, aggressor side, premium USD):")
        for p in prints:
            L.append(f"    {_u(p['t'])}  {p.get('strike')}  {p.get('class')}  "
                     f"{p.get('side')}  {p.get('premium')}")
    else:
        L.append("  largest prints, last 8h: none reported")
    return "\n".join(head + L)


def main() -> int:
    ctx = gather()
    print(f"state={ctx['state']} age_ms={ctx['age_ms']}")
    print()
    print(block(ctx))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
