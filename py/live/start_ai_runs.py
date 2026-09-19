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
    # The same model and the same bars as the row above, with one sentence
    # removed from the prompt: the clause saying that a trade you are not
    # confident in is worse than no trade. The book above has answered NONE
    # on every bar it has ever seen - 79 real answers out of 99 rows on
    # 2026-09-17 - and that clause is the suspect.
    #
    # A SECOND BOOK AND NOT AN EDIT TO THE FIRST. The rule this file already
    # applies to models applies to prompts: change a running book's prompt
    # and its record becomes two campaigns wearing one id, intact and
    # meaningless. Here the comparison between the two IS the experiment, so
    # both have to exist and the first has to keep running untouched.
    #
    # Registered before its first bar at
    # docs/hypotheses/2026-09-17-prompt-coin-penalty.md.
    ("ai-xau-opus-ctx-b", "ai-xau-opus-ctx-b-coin",
     "AI trader - claude-opus-5, prompt no-coin-penalty (coin named, penalty clause removed)"),
    # deepseek-flash again, with one block ADDED to the prompt's market
    # context: options-flow positioning from the COMEX tape. `ai-xau-ds-ctx`
    # keeps running unchanged and is the control.
    #
    # The same second-book rule as the pair above, and for a second reason
    # here: this book's bars divide into ones where the feed answered and
    # ones where it did not, and only a book of its own can carry that
    # distinction in its record.
    #
    # Registered before its first bar at
    # docs/hypotheses/2026-09-17-otl-context.md, which also states why this is
    # not a re-run of the twenty-five registrations that found nothing in
    # levels of this kind.
    ("ai-xau-ds-ctx-otl", "ai-xau-ds-ctx-otl-coin",
     "AI trader - deepseek-flash, prompt otl-context (base + COMEX options positioning block)"),
    # deepseek-flash a third and fourth time, for the higher-timeframe pair.
    # `ai-xau-ds-ctx` is the control for both and keeps running unchanged.
    #
    # `htf-context` is the base prompt plus one facts block: H4 swing
    # structure, EMAs, ADX, efficiency, Donchian, and the daily and weekly
    # levels, all from CLOSED bars on the broker's 21Z anchor. `htf-filter`
    # is that book plus exactly one sentence, which refuses a side against
    # the H4 structure label and nothing else. Two books because they are two
    # claims and the second is only worth reading if the first survives.
    #
    # Both must be books of their own for the reason the -otl pair is: their
    # bars divide into ones where the route answered and ones where it did
    # not, the `htf` field on every decision row records which, and only a
    # book of its own can carry that distinction in its record.
    #
    # Registered before their first bar at
    # docs/hypotheses/2026-09-18-htf-context.md, which states the kill
    # conditions in advance - a disagreement rate under 10% closes the first
    # claim, and htf-filter taking under a third of the control's entries is
    # recorded as the rule closing the book.
    ("ai-xau-ds-ctx-htf", "ai-xau-ds-ctx-htf-coin",
     "AI trader - deepseek-flash, prompt htf-context (base + H4/D1 facts block)"),
    ("ai-xau-ds-ctx-htf-filter", "ai-xau-ds-ctx-htf-filter-coin",
     "AI trader - deepseek-flash, prompt htf-filter (htf-context + one structure-only rule)"),
    # deepseek-flash again, with one more block ADDED to the prompt: the
    # structural price levels GET /api/paper/levels computes from closed bars
    # - activity profile, unfilled fair value gaps, order blocks, buy- and
    # sell-side liquidity, session/day/week extremes - each with its price or
    # band, its AGE and its STATE, nearest first and capped. `ai-xau-ds-ctx`
    # keeps running unchanged and is the control.
    #
    # The same second-book rule as every pair above, and for the same second
    # reason the -otl and -htf books needed one: this book's bars divide into
    # ones where the route answered and ones where it did not, the `smc`
    # field on every decision row records which, and only a book of its own
    # can carry that distinction in its record.
    #
    # Registered before its first bar - and before the route existed - at
    # docs/hypotheses/2026-09-18-smc-context.md, which states the kill
    # condition in advance (a disagreement rate under 10% over the first 100
    # shared warm `ok` bars closes the claim and stops both books) and states
    # that this is the THIRD variant on one control, so a positive result is
    # to be read against three tests and not one.
    ("ai-xau-ds-smc", "ai-xau-ds-smc-coin",
     "AI trader - deepseek-flash, prompt smc-context (base + structural price levels block)"),
    # deepseek-flash a fifth and sixth time, for the staged-entry pair
    # (docs/plans/2026-09-18-staged-ai-entry.md, stages 1 and 2).
    # `ai-xau-ds-ctx` is the control for the first and keeps running unchanged.
    #
    # `plan` answers with an entry TYPE - market, limit or stop - a price, a
    # zone, a validity in bars and two invalidation levels, instead of a side
    # that fills at the next open. The order fills from the desk's own tick
    # feed and is cancelled unfilled after the bars the model named, and that
    # cancellation is a ROW in fills.jsonl, so a missed trade is counted rather
    # than absent. `plan-trigger` is the same prompt with a fast question on
    # top: every minute while an order waits the model is asked TRIGGER, WAIT
    # or CANCEL, with thinking off, on a prompt whose first six kilobytes are
    # the decision prompt again so the provider serves them from cache.
    #
    # Two books because they are two claims and the second is only worth
    # reading against the first: does a limit entry beat a market entry at
    # all, and if it does, does a model at the trigger add anything to a rule
    # that fills by itself. Each has its own coin, which takes the same entry
    # type at mirrored distances so the two share the fill mechanics.
    #
    # Registered before their first bar at
    # docs/hypotheses/2026-09-18-plan-entry.md and 2026-09-18-plan-trigger.md,
    # with the kill conditions in advance: net R of `plan` not above the
    # market book's over 30 trades and two windows drops limit entries, and
    # no difference between `plan-trigger` and `plan` drops the model at the
    # trigger. Nothing here reaches the funded account: the executor's
    # --mirror-pending is off until the stage-1 comparison has 30 trades.
    ("ai-xau-ds-plan", "ai-xau-ds-plan-coin",
     "AI trader - deepseek-flash, prompt plan (base + plan answer: entry type/price, zone, valid_bars)"),
    ("ai-xau-ds-plan-trigger", "ai-xau-ds-plan-trigger-coin",
     "AI trader - deepseek-flash, prompt plan-trigger (plan + TRIGGER/WAIT/CANCEL every minute while waiting)"),
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
