#!/usr/bin/env python3
"""Compare the Rust sweep benchmark against the JavaScript prototype.

The migration plan's Phase 2 gate is a number: the sweep must be at least 50x
faster than the prototype. This script is what produces that number, and it is
deliberately the only place the comparison is made, so it cannot be quoted from
two runs that measured different things.

    node research/bench-sweep.js > baseline.json     # in the Node repo
    cargo bench -p fd-backtest
    python scripts/bench-report.py baseline.json

Criterion reports nanoseconds per iteration; the harness reports milliseconds
for the same unit of work (every strategy swept once on one market).
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CRITERION = ROOT / "target" / "criterion"
TARGET_SPEEDUP = 50.0


def criterion_estimates() -> dict[str, float]:
    """Map criterion's full benchmark id to its median time in milliseconds."""
    out: dict[str, float] = {}
    for estimates in CRITERION.rglob("new/estimates.json"):
        bench = estimates.parent / "benchmark.json"
        if not bench.exists():
            continue
        full_id = json.loads(bench.read_text(encoding="utf-8")).get("full_id")
        if not full_id:
            continue
        data = json.loads(estimates.read_text(encoding="utf-8"))
        # The median is the right estimator here: a sweep's cost is dominated by
        # work, not by tail latency, and the mean is dragged by OS scheduling.
        out[full_id] = data["median"]["point_estimate"] / 1e6
    return out


def row(label: str, js_ms: float | None, rs_ms: float | None) -> str:
    if js_ms is None or rs_ms is None or rs_ms <= 0:
        return f"  {label:<34} {'-':>12} {'-':>12} {'-':>10}"
    return f"  {label:<34} {js_ms:>10.3f}ms {rs_ms:>10.3f}ms {js_ms / rs_ms:>8.1f}x"


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__)
        return 2
    baseline = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    rust = criterion_estimates()
    if not rust:
        print("No criterion results found. Run `cargo bench -p fd-backtest` first.")
        return 1

    print(f"JavaScript: Node {baseline.get('node')}, best of {baseline.get('repeats')} runs")
    print(f"Rust:       criterion median, {len(rust)} benchmarks")
    print()
    print("  The prototype's own grid - every strategy, default axes.")
    print()
    print(f"  {'benchmark':<34} {'JS':>12} {'Rust':>12} {'speedup':>10}")
    print(f"  {'-' * 34} {'-' * 12} {'-' * 12} {'-' * 10}")

    verdicts: list[tuple[str, float]] = []
    for market, entry in baseline.get("markets", {}).items():
        for suffix, js_key, label in (
            ("all-strategies", "totalSweepMs", "sweep"),
            ("walk-forward-all", "totalWalkForwardMs", "walk-forward"),
        ):
            full_id = f"sweep/oracle-grid/{suffix}/{market}"
            js_ms = entry.get(js_key)
            rs_ms = rust.get(full_id)
            print(row(f"{label} - {market} ({entry.get('totalCells', '?')} cells)", js_ms, rs_ms))
            if js_ms and rs_ms and label == "sweep":
                verdicts.append((market, js_ms / rs_ms))

    js_dense = baseline.get("markets", {}).get("btc", {}).get("dense", {})
    crossover = None
    if js_dense:
        print()
        print("  Dense grid - one strategy, refined axes (btc). The speedup is not a")
        print("  constant: it grows with the search, which is the whole argument.")
        print()
        print(f"  {'cells':>8} {'JS':>12} {'Rust':>12} {'speedup':>10}")
        print(f"  {'-' * 8} {'-' * 12} {'-' * 12} {'-' * 10}")
        for cells in sorted(js_dense, key=int):
            js_ms = js_dense[cells]["ms"]
            rs_ms = rust.get(f"sweep/dense-grid/ema-cross/{cells}")
            if rs_ms is None:
                continue
            speedup = js_ms / rs_ms
            if crossover is None and speedup >= TARGET_SPEEDUP:
                crossover = int(cells)
            print(f"  {cells:>8} {js_ms:>10.3f}ms {rs_ms:>10.3f}ms {speedup:>8.1f}x")

    print()
    if not verdicts:
        print("GATE: no comparable sweep measurements.")
        return 1
    worst_market, worst = min(verdicts, key=lambda kv: kv[1])
    print(f"On the prototype's own 82-cell grid the sweep is {worst:.0f}x faster ({worst_market}),")
    print(f"not the {TARGET_SPEEDUP:.0f}x the plan asked for.")
    if crossover:
        print(f"It reaches {TARGET_SPEEDUP:.0f}x at about {crossover} cells and keeps climbing.")
        print("So the gate's number describes a search the prototype cannot practically")
        print("run, not the grid it runs today. Both rows are the result; neither alone is.")
    else:
        print(f"It does not reach {TARGET_SPEEDUP:.0f}x at any measured grid size.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
