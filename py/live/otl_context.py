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

    # Bull/bear premium by horizon. `alldte-data?tf=` is documented as the
    # snapshot aggregate for one horizon; if it does not carry the two totals
    # they stay absent and the block says so. They are NOT filled in from the
    # all-expirations totals above - a number labelled "0DTE" that is actually
    # every expiration is worse than a gap.
    for tf, label in (("daily", "0dte"), ("weekly", "weekly")):
        doc = _get(base, f"/api/options/alldte-data?tf={tf}", deadline)
        row = doc
        if isinstance(doc, dict) and isinstance(doc.get("snapshots"), list) and doc["snapshots"]:
            row = doc["snapshots"][-1]
        if isinstance(row, dict):
            f[label + "_bull"] = row.get("bull_total")
            f[label + "_bear"] = row.get("bear_total")

    # Whale support and resistance, which live only in the trade tape's
    # run-length-encoded level track and not in any summary endpoint.
    #
    # Taken from the busiest contract rather than a fixed symbol: the front
    # month rolls, and a hardcoded OGV6 would quietly describe a dead
    # contract from October onwards. `hours=8` keeps the payload small - the
    # levels are recomputed on every print, so the last pair is current
    # whatever window it was computed over.
    contracts = _get(base, "/api/options/active-contracts", deadline)
    if isinstance(contracts, list) and contracts:
        try:
            front = max((c for c in contracts if isinstance(c, dict)),
                        key=lambda c: c.get("total_premium") or 0)
        except ValueError:
            front = None
        if front and front.get("symbol"):
            f["whale_symbol"] = front["symbol"]
            chart = _get(base, f"/api/options/chart-data/{front['symbol']}?hours=8&candles=1", deadline)
            rle = (((chart or {}).get("data") or {}).get("trades") or {}).get("levels_rle") or {}
            f["whale_sup"] = _last_rle(rle.get("whale_sup"))
            f["whale_res"] = _last_rle(rle.get("whale_res"))

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
    sym = f.get("whale_symbol")
    tag = f" (from {sym})" if sym else ""
    add("whale support" + tag, f.get("whale_sup"), " USD/oz")
    add("whale resistance" + tag, f.get("whale_res"), " USD/oz")
    add("ATM (futures)", f.get("atm_price"), " USD/oz")
    add("expected move, 1 day", f.get("exp_move"), " USD/oz")
    if f.get("iv_from") is not None:
        d = f["iv_to"] - f["iv_from"]
        L.append(f"  average IV: {f['iv_to']:.4f} now, {f['iv_from']:.4f} {f.get('iv_span_h', '?')}h ago "
                 f"({d:+.4f})")
    else:
        add("average IV", f.get("avg_iv"))
    add("0DTE bull premium", f.get("0dte_bull"), " USD")
    add("0DTE bear premium", f.get("0dte_bear"), " USD")
    add("weekly bull premium", f.get("weekly_bull"), " USD")
    add("weekly bear premium", f.get("weekly_bear"), " USD")
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
