"""Does a trailing stop help the Vantage gold book, out of sample?

    python scripts/trail_sweep.py                    # the registered run
    python scripts/trail_sweep.py --market=btcusd    # the same question on BTC

Read `docs/hypotheses/2026-09-16-trailing-stop.md` first. It declares the grid,
the selection rule, the falsifier and the null BEFORE any of this ran, which is
the only thing that makes the output readable.

What this is not: the exploratory comparison in commit f237c4a. That was one
cell, in sample, unguarded, with no null and no held-out window, and nothing in
it was a result. This keeps the shape of that measurement and adds the three
things it was missing.
"""

from __future__ import annotations

import argparse
import itertools
import os
import re
import statistics
import subprocess
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
SEARCH = os.path.join(ROOT, "target-b", "release", "search.exe")

# The grid, fixed here before the first run.
DISTANCE = [0.5, 0.75, 1.0, 1.5, 2.0]
ACTIVATE = [0.5, 1.0, 1.5, 2.0]

# A method is read only if it trades enough on BOTH windows to mean anything.
# Chosen from the desk's existing `min_trades` of 30, raised because a trail
# only acts on trades that go favourable first and so sees fewer of them.
MIN_TRADES = 100

ROW = re.compile(
    r"^(?P<name>[a-z0-9-]+)\s+(?P<trades>\d+)\s+(?P<win>[\d.]+)%\s+"
    r"(?P<pf>[\d.]+|NaN|inf)\s+(?P<exp>-?[\d.]+|NaN)\s+(?P<dd>\d+)"
)


def run(market: str, frm: str, to: str, trail: str) -> dict:
    """One `search --mode=compare`, parsed into {method: {...}}."""
    out = subprocess.run(
        [SEARCH, f"--market={market}", "--mode=compare", f"--from={frm}", f"--to={to}", f"--trail={trail}"],
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, cwd=ROOT, timeout=600,
    ).stdout.decode("utf-8", "replace")
    rows = {}
    for line in out.splitlines():
        m = ROW.match(line.strip())
        if not m:
            continue
        try:
            rows[m.group("name")] = {
                "trades": int(m.group("trades")),
                "win": float(m.group("win")),
                "pf": float(m.group("pf")) if m.group("pf") not in ("NaN", "inf") else float("nan"),
                "exp": float(m.group("exp")) if m.group("exp") != "NaN" else float("nan"),
                "dd": float(m.group("dd")),
            }
        except ValueError:
            continue
    return rows


def readable(off: dict) -> list[str]:
    """The method set, fixed by the CONTROL arm so a trail setting cannot
    change which methods are scored — otherwise a cell could win by silently
    dropping the methods it hurts."""
    return sorted(n for n, r in off.items() if r["trades"] >= MIN_TRADES)


def score(rows: dict, methods: list[str]) -> float:
    """Median expectancy in R across the fixed method set. Median, not mean:
    one method with nine trades and a huge expectancy must not carry a grid."""
    vals = [rows[m]["exp"] for m in methods if m in rows and rows[m]["exp"] == rows[m]["exp"]]
    return statistics.median(vals) if vals else float("nan")


def above_one(rows: dict, methods: list[str]) -> int:
    return sum(1 for m in methods if m in rows and rows[m]["pf"] > 1.0)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split(chr(10))[0])
    ap.add_argument("--market", default="xauusd")
    # The recent Vantage year is the held-out window and is primary; the owner
    # set that criterion on 2026-09-13.
    ap.add_argument("--is-from", default="2022-06-16")
    ap.add_argument("--is-to", default="2025-09-15")
    ap.add_argument("--oos-from", default="2025-09-15")
    ap.add_argument("--oos-to", default="2026-09-15")
    args = ap.parse_args()

    if not os.path.isfile(SEARCH):
        sys.exit(f"no {SEARCH}; build it: cargo build --release -p fd-backtest --bin search --target-dir target-b")

    cells = ["off"] + [f"{d},{a}" for d, a in itertools.product(DISTANCE, ACTIVATE)]
    print(f"market {args.market} | {len(cells)} cells | in-sample {args.is_from}..{args.is_to} | "
          f"held out {args.oos_from}..{args.oos_to}\n")

    print("== in sample: choosing one cell by MEDIAN expectancy, never by best ==")
    is_res = {c: run(args.market, args.is_from, args.is_to, c) for c in cells}
    methods = readable(is_res["off"])
    print(f"method set, fixed by the control arm ({MIN_TRADES}+ trades): {', '.join(methods)}\n")
    if not methods:
        sys.exit("no method trades enough on the in-sample window; nothing to read")

    base_is = score(is_res["off"], methods)
    ranked = sorted(((score(is_res[c], methods), c) for c in cells if c != "off"), reverse=True)
    print(f"{'cell':<12} {'median exp':>11} {'vs off':>9} {'PF>1':>6}")
    print(f"{'off':<12} {base_is:>11.4f} {'--':>9} {above_one(is_res['off'], methods):>6}")
    for sc, c in ranked:
        print(f"{c:<12} {sc:>11.4f} {sc - base_is:>+9.4f} {above_one(is_res[c], methods):>6}")
    chosen = ranked[0][1]
    print(f"\nchosen in sample: {chosen}\n")

    print("== held out: the chosen cell, and every other cell for the null ==")
    oos = {c: run(args.market, args.oos_from, args.oos_to, c) for c in cells}
    base_oos = score(oos["off"], methods)
    chosen_oos = score(oos[chosen], methods)
    all_oos = sorted(score(oos[c], methods) for c in cells if c != "off")
    better = sum(1 for v in all_oos if v < chosen_oos)
    pct = 100.0 * better / max(1, len(all_oos) - 1)

    print(f"{'cell':<12} {'median exp':>11} {'vs off':>9} {'PF>1':>6}")
    print(f"{'off':<12} {base_oos:>11.4f} {'--':>9} {above_one(oos['off'], methods):>6}")
    for c in cells:
        if c == "off":
            continue
        sc = score(oos[c], methods)
        mark = "  <- chosen" if c == chosen else ""
        print(f"{c:<12} {sc:>11.4f} {sc - base_oos:>+9.4f} {above_one(oos[c], methods):>6}{mark}")

    print("\n== the two declared tests ==")
    beats_off = chosen_oos > base_oos
    pf_kept = above_one(oos[chosen], methods) >= above_one(oos["off"], methods)
    print(f"1. beats `off` out of sample : {beats_off}  "
          f"({chosen_oos:+.4f} vs {base_oos:+.4f}, delta {chosen_oos - base_oos:+.4f})")
    print(f"   PF>1 count not reduced    : {pf_kept}  "
          f"({above_one(oos[chosen], methods)} vs {above_one(oos['off'], methods)} with the trail off)")
    print(f"2. selection carried signal  : {pct >= 50.0}  "
          f"(the chosen cell sits at the {pct:.0f}th percentile of all cells out of sample; "
          f"below 50 means choosing it on the first window was noise)")
    verdict = "SURVIVES" if (beats_off and pf_kept and pct >= 50.0) else "FAILS"
    print(f"\nVERDICT: {verdict}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
