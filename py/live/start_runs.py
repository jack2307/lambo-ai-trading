"""Start the paper runs listed in docs/paper/CANDIDATES.md (idempotent).

    python py/live/start_runs.py [--api=http://127.0.0.1:8138] [--only=xau-ema,eur-hours]

Each run is one `POST /api/paper/start` with guards on. A run that already
exists (409) is left as it is; the table below is the source of truth for
what runs and why, mirrored from CANDIDATES.md — change both.
"""

from __future__ import annotations

import argparse
import json
import sys
import urllib.request

RUNS = [
    # id, market, tf, strategy, params, filters, label
    ("xau-ema", "xauusd", "15m", "ema-cross", {}, ["weekdays", "news:60-30"], "#1 baseline trend, all day"),
    ("xau-macd-asia", "xauusd", "15m", "macd-cross", {}, ["weekdays", "hours:1800-0200", "news:60-30"], "#2 the year's best gridded row, Asia"),
    ("xau-keltner-asia", "xauusd", "15m", "keltner-break", {}, ["weekdays", "hours:1800-0200", "news:60-30"], "#3 Asian breakout, the contrast to #2"),
    ("xau-close", "xauusd", "15m", "session-hold", {"from": 1615, "to": 1815, "side": 1, "riskDailyRanges": 1, "rangeDays": 20}, ["weekdays:MoTuWeTh", "hours:1615-1620"], "#4 long across the NY close"),
    ("xau-evening", "xauusd", "15m", "session-hold", {"from": 1800, "to": 2000, "side": 1, "riskDailyRanges": 1, "rangeDays": 20}, ["weekdays:MoTuWeTh", "hours:1800-1805"], "#5 the one row of 170"),
    ("xau-rsi2-ny", "xauusd", "15m", "rsi2-pullback", {}, ["weekdays", "hours:0800-1600", "news:60-30"], "#6 mean reversion, NY hours"),
    ("xau-stoch", "xauusd", "15m", "stoch-reversal", {}, ["weekdays", "news:60-30"], "#7 the in-sample leaderboard's top, to see what it does live"),
    ("xau-ict-5m", "xauusd", "5m", "ict-sweep-mss-fvg", {}, ["weekdays", "hours:0800-1600", "news:60-30"], "#8 the long window's best 5m row"),
    ("xau-box-5m", "xauusd", "5m", "volman-box", {"boxes": 2, "maxBoxAtr": 3.0, "emaPeriod": 18, "atrPeriod": 14, "maxBreakAtr": 1.0}, ["weekdays", "news:60-30"], "#9 the owner's three boxes"),
    ("eur-hours", "eurusd", "15m", "session-hold", {"from": 245, "to": 1045, "side": -1, "riskDailyRanges": 1, "rangeDays": 20}, ["weekdays", "hours:0245-0250", "news:480-30"], "#10 the euro's European hours"),
]


def post(api: str, path: str, payload: dict) -> tuple[int, str]:
    body = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(f"{api}{path}", data=body, headers={"Content-Type": "application/json"}, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=20) as r:
            return r.status, r.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as e:  # type: ignore[attr-defined]
        return e.code, e.read().decode("utf-8", "replace")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--api", default="http://127.0.0.1:8138")
    ap.add_argument("--only", default="", help="comma-separated run ids; default all")
    args = ap.parse_args()
    only = {s for s in args.only.split(",") if s}
    bad = 0
    for run_id, market, tf, strategy, params, filters, label in RUNS:
        if only and run_id not in only:
            continue
        payload = {"id": run_id, "market": market, "tf": tf, "strategy": strategy, "params": params, "filters": filters, "guards": True, "window": 600, "label": label}
        status, text = post(args.api, "/api/paper/start", payload)
        short = text[:140].replace("\n", " ")
        print(f"{run_id:18s} {status}  {short}")
        if status not in (200, 409):
            bad += 1
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
