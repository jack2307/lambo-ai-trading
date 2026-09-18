# -*- coding: utf-8 -*-
"""Adding a campaign takes three files. This fails when only two were edited.

    py -3.9 py/live/campaign_wiring_selftest.py

WHY THIS EXISTS. On 2026-09-18 `ai-xau-ds-ctx-htf` and `-htf-filter` were
added to `ai_trader.py` (the prompt variant) and to `start_ai_traders.ps1`
(the row that launches a process), and NOT to `start_ai_runs.py` (the book
the process talks to). Both traders ran for hours logging
`cannot read ...: HTTP 404` and decided nothing.

Nothing failed loudly, which is the whole reason for this file:

  * `start_ai_runs.py` prints "409 already there" for every book that
    exists and says NOTHING about a book it has never heard of, so its
    output on the broken state was indistinguishable from a clean re-run.
  * `start_ai_traders.ps1` starts a process per row and reports success
    when the process starts. A trader that comes up, polls, and 404s is a
    started process.
  * The trader itself cannot tell "this book does not exist" from "the API
    is not up yet" strongly enough to refuse, because the second is normal
    at boot.

So three files have to agree and no single one of them can notice. The
disagreement is only visible from outside, which is here.

WHAT IS CHECKED, and deliberately only what is mechanical:

  1. every campaign the launcher would start has a book to talk to
  2. every campaign's coin control has a book too
  3. every prompt variant the launcher names is one `ai_trader` accepts
  4. no two campaigns share a run id, a control id, a seed or a log name

It does NOT check that a book is the right SHAPE - market, timeframe and
strategy live in `start_ai_runs.COMMON` and are one object shared by every
book, so there is nothing per-campaign to get wrong there yet. If that ever
becomes per-book, this is where the check belongs.

Reading the launcher's table needs PowerShell, because it is a PowerShell
literal and a regex over it would be a second parser that agrees until it
does not. The array is sliced out and evaluated on its own - the launcher is
never RUN, because running it stops and starts live traders.
"""
from __future__ import annotations

import io
import json
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, HERE)

fails: list[str] = []


def check(ok: bool, what: str) -> None:
    print(("ok   " if ok else "FAIL ") + what)
    if not ok:
        fails.append(what)


def launcher_campaigns() -> list[dict]:
    """The `$campaigns` array from start_ai_traders.ps1, as data.

    Sliced from `$campaigns = @(` to the first line that is exactly `)`, then
    evaluated by PowerShell with nothing else around it. If the launcher is
    ever restructured so that slice stops being the whole array, this raises
    rather than quietly checking a subset - a partial list would make the
    missing campaign invisible, which is the failure being guarded against.
    """
    path = os.path.join(HERE, "start_ai_traders.ps1")
    lines = io.open(path, encoding="utf-8").read().splitlines()
    start = next(i for i, l in enumerate(lines) if l.strip().startswith("$campaigns = @("))
    end = next(i for i in range(start + 1, len(lines)) if lines[i].rstrip() == ")")
    body = "\n".join(lines[start:end + 1])
    if body.count("@{") < 2:
        raise SystemExit(f"{path}: the $campaigns slice holds {body.count('@{')} entries; "
                         "the array's shape has changed and this test would check a subset")
    probe = os.path.join(tempfile.gettempdir(), "fd_campaigns_probe.ps1")
    io.open(probe, "w", encoding="utf-8", newline="\r\n").write(
        body + "\n"
        "$campaigns | ForEach-Object { [pscustomobject]@{ run=$_.run; control=$_.control; "
        "seed=$_.seed; log=$_.log; "
        "variant=$(if ($_.promptVariant) { $_.promptVariant } else { 'base' }) } } | "
        "ConvertTo-Json -Depth 3\n"
    )
    out = subprocess.run(["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", probe],
                         capture_output=True, text=True)
    if out.returncode != 0:
        raise SystemExit(f"could not read the launcher's table: {out.stderr.strip()}")
    doc = json.loads(out.stdout)
    return doc if isinstance(doc, list) else [doc]


def main() -> int:
    import ai_trader
    import start_ai_runs

    campaigns = launcher_campaigns()
    books = {b[0] for b in start_ai_runs.BOOKS} | {b[1] for b in start_ai_runs.BOOKS}
    print(f"{len(campaigns)} campaigns in the launcher, "
          f"{len(start_ai_runs.BOOKS)} book pairs in start_ai_runs\n")

    # 1 and 2: the thing that was actually broken.
    for c in campaigns:
        check(c["run"] in books,
              f"{c['run']}: the launcher starts it and start_ai_runs creates it")
        check(c["control"] in books,
              f"{c['control']}: its coin control has a book too")

    # 3: an unknown variant is an argparse error raised INSIDE the trader,
    # after the launcher has already reported the campaign started.
    for c in campaigns:
        check(c["variant"] in ai_trader.PROMPT_VARIANTS,
              f"{c['run']}: prompt variant {c['variant']!r} is one ai_trader accepts")

    # 4: two books sharing a seed share a coin's luck on every bar where both
    # traded, which is the one thing a control may not do.
    for field in ("run", "control", "seed", "log"):
        vals = [c[field] for c in campaigns]
        dupes = sorted({v for v in vals if vals.count(v) > 1})
        check(not dupes, f"every {field} is distinct" + (f" (duplicates: {dupes})" if dupes else ""))

    # A book with no campaign is not an error - a campaign can be stopped and
    # its book deliberately left in place so its record stays readable, which
    # is what happened to ai-xau-sol-ctx. Reported, never failed.
    launched = {c["run"] for c in campaigns} | {c["control"] for c in campaigns}
    orphans = sorted(b for b in books if b not in launched)
    if orphans:
        print("\nbooks with no campaign (stopped runs keep their books on purpose):")
        for o in orphans:
            print(f"  {o}")

    print()
    print("all checks passed" if not fails else f"{len(fails)} FAILED")
    return 1 if fails else 0


if __name__ == "__main__":
    raise SystemExit(main())
