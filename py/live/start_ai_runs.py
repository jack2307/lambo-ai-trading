"""Create the paper books the AI campaigns post into (idempotent).

    python py/live/start_ai_runs.py [--api=http://127.0.0.1:8138]
    python py/live/start_ai_runs.py --only=ai-xau-ds-ctx,ai-xau-sol-ctx
    python py/live/start_ai_runs.py --no-control

Each book is one `POST /api/paper/start` with strategy `external`, which is
the only strategy that takes a side from outside; a run that already exists
comes back 409 and is left exactly as it is, so re-running this is safe.

WHY THIS FILE EXISTS
--------------------
It did not, until 2026-09-17. The rule-based books had start_runs.py from the
beginning and the AI books were created by hand, once, on one machine - so
the record of what they are lived only in their own state.json. Standing up a
second machine is what made that expensive: there was nothing to run, and the
configuration had to be read back out of the first machine's book files.

The rows below were read from those files rather than invented, so a book
created here matches the one created by hand: `external`, atrPeriod 14,
guards on, no filters, 600 bars.

The coin books are listed with the models they control. --no-control skips
them - see ai_trader.py's docstring for what that costs.
"""

from __future__ import annotations

import argparse
import json
import sys
import urllib.error
import urllib.request

for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")
    except (AttributeError, ValueError):
        pass

# run id, control id, label
BOOKS = [
    ("ai-xau-ds-ctx", "ai-xau-ds-ctx-coin",
     "AI trader - deepseek-flash (API, ~$1-2/thang), prompt co desk state + market context"),
    # `ai-xau-sol-ctx` is deliberately NOT in this list. It was retired
    # 2026-09-17 when gpt-5.6-sol stopped being reachable from the VPS, and its
    # book was stopped rather than repointed - see start_ai_traders.ps1.
    #
    # Leaving it here would be worse than untidy. Stopping writes final.json
    # and removes state.json, so a later run of this idempotent script would
    # NOT come back 409 - it would create a second, empty book under the same
    # id, and that id would then name two campaigns by two models. Keeping the
    # id meaning one thing is the entire reason the campaign was retired.
    ("ai-xau-terra-ctx", "ai-xau-terra-ctx-coin",
     "AI trader - gpt-5.6-terra (plan, Codex CLI), prompt co desk state + market context"),
    ("ai-xau-opus-ctx", "ai-xau-opus-ctx-coin",
     "AI trader - claude-opus-5, prompt co desk state + market context"),
]

# Read from the books the hand-made campaigns are still running on.
COMMON = {"market": "xauusd", "tf": "15m", "strategy": "external",
          "params": {"atrPeriod": 14.0}, "filters": [], "guards": True, "window": 600}


def post(api: str, path: str, payload: dict) -> tuple[int, str]:
    body = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(f"{api}{path}", data=body,
                                 headers={"Content-Type": "application/json"}, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            return r.status, r.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode("utf-8", "replace")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--api", default="http://127.0.0.1:8138")
    ap.add_argument("--only", default="", help="comma-separated run ids; default all")
    ap.add_argument("--no-control", action="store_true",
                    help="do not create the coin books")
    args = ap.parse_args()
    only = {s for s in args.only.split(",") if s}

    bad = 0
    for run_id, control_id, label in BOOKS:
        if only and run_id not in only:
            continue
        wanted = [(run_id, label)]
        if not args.no_control:
            wanted.append((control_id, label + " - coin control"))
        for book, book_label in wanted:
            payload = dict(id=book, label=book_label, **COMMON)
            status, text = post(args.api, "/api/paper/start", payload)
            note = "created" if status == 200 else ("already there" if status == 409 else text[:120])
            print(f"{book:24s} {status}  {note}")
            if status not in (200, 409):
                bad += 1
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
