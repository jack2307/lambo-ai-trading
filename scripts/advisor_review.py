"""Was the advisor worth listening to? Read the log and say.

    python scripts/advisor_review.py                 # every run
    python scripts/advisor_review.py xau-stoch       # one run
    python scripts/advisor_review.py --agents        # break it down per advisor
    python scripts/advisor_review.py --cases=20      # the worst calls, to read

A veto has no outcome of its own — the trade did not happen — so nothing here
would be answerable without the shadow book, the same strategy on the same
bars that never hears an advisor. `docs/paper/ADVISOR.md` explains why that
exists; this file is what it is for.

The headline number is the simplest one in the system:

    book net  −  shadow net  =  what the advice has cost, or saved

Everything else is an attempt to say *which* advice, and it is an attempt
rather than a proof: once the two books diverge they are in different
positions, so no single veto can be cleanly credited with a dollar figure. The
per-agent tables below are therefore evidence about a habit, not an audit of a
trade, and the script says so wherever it prints one.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PAPER = ROOT / "data" / "paper"

#: Below this many consultations, nothing here is a measurement and the script
#: refuses to summarise as though it were. Thirty registrations in this
#: repository have been closed by exactly this kind of caution.
THIN = 30


def read_jsonl(path: Path) -> list[dict]:
    if not path.exists():
        return []
    rows = []
    with path.open(encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                rows.append(json.loads(line))
            except json.JSONDecodeError:
                # A half-written last line is a process that died mid-append,
                # not a corrupt log. Everything before it is still good.
                continue
    return rows


def runs_in(only: str | None) -> list[Path]:
    if not PAPER.exists():
        sys.exit(f"no {PAPER.relative_to(ROOT)} — no paper run has ever started")
    dirs = sorted(d for d in PAPER.iterdir() if d.is_dir() and (d / "state.json").exists())
    if only:
        dirs = [d for d in dirs if d.name == only]
        if not dirs:
            sys.exit(f"no paper run `{only}`")
    return dirs


def verdict_of(factor: float) -> str:
    return "veto" if factor <= 0.0 else ("cut" if factor < 1.0 else "allow")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("run", nargs="?", help="one run id; default every run")
    ap.add_argument("--agents", action="store_true", help="break the verdicts down per advisor")
    ap.add_argument("--cases", type=int, default=0, help="print this many consultations in full")
    args = ap.parse_args()

    dirs = runs_in(args.run)
    total_consults = 0
    per_run: list[tuple[str, int, dict, float | None, float | None]] = []
    per_agent: dict[str, dict[str, float]] = defaultdict(lambda: defaultdict(float))
    cases: list[dict] = []

    for d in dirs:
        consults = [r for r in read_jsonl(d / "advice.jsonl") if r.get("kind") == "consultation"]
        fills = read_jsonl(d / "fills.jsonl")
        advice_events = [r for r in fills if r.get("kind") == "advice"]

        counts = defaultdict(int)
        for c in consults:
            counts[verdict_of(float(c.get("size_factor", 1.0)))] += 1
            if not c.get("applied"):
                counts["too late"] += 1
            for turn in c.get("transcript") or []:
                agent = turn.get("agent", "?")
                per_agent[agent][verdict_of(float(turn.get("size_factor", 1.0)))] += 1
                per_agent[agent]["turns"] += 1
                per_agent[agent]["latency_ms"] += float(turn.get("latency_ms", 0))
            cases.append({"run": d.name, **c})
        total_consults += len(consults)

        # The two books, as the last advice event saw them. Read from the log
        # rather than recomputed, so this script cannot disagree with the desk.
        book = shadow = None
        if advice_events:
            last = advice_events[-1]
            book = last.get("book_net_usd")
            shadow = last.get("shadow_net_usd")
        per_run.append((d.name, len(consults), dict(counts), book, shadow))

    print(f"advisor review — {len(dirs)} run(s), {total_consults} consultation(s)\n")

    if total_consults == 0:
        print("No advisor has ever posted a verdict.")
        print()
        print("  Start one:  python py/live/advisor.py --rules-only --dry-run")
        print("  The shadow book is already running on every run, so the comparison")
        print("  starts from the moment an advisor first speaks and not from zero.")
        return 0

    print(f"{'run':18s} {'seen':>5s} {'allow':>6s} {'cut':>5s} {'veto':>5s} {'late':>5s} "
          f"{'book net':>10s} {'shadow net':>11s} {'advice cost':>12s}")
    for name, n, counts, book, shadow in per_run:
        # A run with no APPLIED advice has no pair of totals to compare, and a
        # dash says that; printing nan would read as a broken calculation
        # rather than as a question nobody has asked yet.
        both = book is not None and shadow is not None
        cost = f"{book - shadow:+.2f}" if both else "-"
        print(
            f"{name:18s} {n:5d} {counts.get('allow', 0):6d} {counts.get('cut', 0):5d} "
            f"{counts.get('veto', 0):5d} {counts.get('too late', 0):5d} "
            f"{(f'{book:.2f}' if both else '-'):>10s} {(f'{shadow:.2f}' if both else '-'):>11s} {cost:>12s}"
        )

    priced = [(b, s) for _, _, _, b, s in per_run if b is not None and s is not None]
    if priced:
        bill = sum(b - s for b, s in priced)
        print(f"\n  the whole desk: advice has {'cost' if bill < 0 else 'saved'} ${abs(bill):.2f} so far")
        print("  (book net minus shadow net — the same strategies on the same bars, unadvised)")

    if args.agents:
        print(f"\n{'agent':12s} {'turns':>6s} {'allow':>6s} {'cut':>5s} {'veto':>5s} {'mean ms':>8s}")
        for agent, c in sorted(per_agent.items()):
            turns = c["turns"] or 1
            print(
                f"{agent:12s} {int(c['turns']):6d} {int(c['allow']):6d} {int(c['cut']):5d} "
                f"{int(c['veto']):5d} {c['latency_ms'] / turns:8.0f}"
            )
        print("\n  Counts of what each advisor DID, not of what it got right. Attributing a")
        print("  dollar to one agent's objection needs the panel run with that agent removed,")
        print("  which is a replay and not a reading of this log.")

    if total_consults < THIN:
        margin = round(100 / math.sqrt(total_consults) * 0.5)
        print(
            f"\n  {total_consults} consultations is not a measurement. A veto rate over this many has a"
            f"\n  margin of roughly ±{margin} points, which is wider than any difference it could show."
            f"\n  Nothing above should be used to change a prompt yet."
        )

    if args.cases:
        worst = sorted(cases, key=lambda c: float(c.get("size_factor", 1.0)))[: args.cases]
        print(f"\n{'=' * 78}\nthe {len(worst)} most interventionist calls, in full\n{'=' * 78}")
        for c in worst:
            print(f"\n[{c['run']}] {c.get('intent_id')}  ->  {c.get('size_factor')}  "
                  f"({'applied' if c.get('applied') else 'NOT applied — the bar had filled'})")
            print(f"  verdict: {c.get('reason')}")
            for turn in c.get("transcript") or []:
                print(f"  - {turn.get('agent')} ({turn.get('model')}, {turn.get('latency_ms')}ms) "
                      f"{turn.get('size_factor')}: {turn.get('reason')}")
        print("\n  The prompts are stored whole in advice.jsonl. Replaying them against a changed")
        print("  prompt, and comparing the new verdicts to outcomes already known, is the only")
        print("  experiment here that costs model calls and no market risk.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
