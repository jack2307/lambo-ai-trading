"""Is `friday_cells.py`'s 0.2nd percentile the calendar, or the harness?

    python scripts/friday_cells_selftest.py

`2026-09-14-nfp-vs-first-friday` closed, but it left one number alive and the
loop has been quoting it since: **25 releases that landed on a later Friday sit
at the 0.2nd percentile of a control drawn from quiet later Fridays, with 20% of
hours up against 53%.** Twenty-five observations and a percentile that extreme
is exactly the combination a miscalibrated null can produce on its own, and
unlike a closure a false positive here propagates - it is the seed of the next
registration.

Two tests:

  A  CALIBRATION.  Keep the bars and the cell machinery, and assign FAKE
     releases - the same number of first Fridays and the same number of later
     Fridays, drawn at random from the Fridays available. C is then "a random
     subset of later Fridays" and its percentile against D must be uniform by
     construction. If these cluster low, the instrument makes 0.2nd percentiles
     out of nothing.

  B  FAULT 9, measured rather than asserted.  `friday_cells.py` builds its cells
     as `if d in release: A or C; elif d not in busy: B or D` - so a Friday
     carrying a SECOND high-impact release is excluded from the controls and
     kept in the treatments. The screen is applied to one arm of the comparison
     only (backlog item, fault 9, `2026-09-13-instrument-faults.md`). This
     recomputes C's percentile with the screen applied symmetrically and reports
     how far the surviving number moves.

Read-only: bars and the calendar, nothing written.
"""

from __future__ import annotations

import argparse
import sys
import warnings
from pathlib import Path

import numpy as np
import pandas as pd

warnings.filterwarnings("ignore")
sys.path.insert(0, str(Path(__file__).resolve().parent))
from news_drift import NEW_YORK  # noqa: E402
import friday_cells as fc  # noqa: E402


def cells_from(fridays, release: set, busy: set, symmetric: bool, multi: set = frozenset()) -> dict:
    """The script's own cell rule, with the screen optionally applied to both arms.

    `symmetric=False` reproduces `friday_cells.main` exactly: a release day
    enters A or C whether or not it also carries another high-impact event,
    while a quiet day must clear `busy` to enter B or D.

    `symmetric=True` applies the same screen to the treatments: a release day
    that ALSO carries a second high-impact event is dropped. It is `multi`, not
    `busy`, that does this — `busy` holds every high-impact date INCLUDING the
    releases themselves, so screening the treatments on it empties the cell, as
    the first version of this test did with all 25.
    """
    out = {"A": [], "B": [], "C": [], "D": []}
    for d in fridays:
        first = d.day <= 7
        if d in release:
            if symmetric and d in multi:
                continue
            out["A" if first else "C"].append(d)
        elif d not in busy:
            out["B" if first else "D"].append(d)
    return out


def percentile_C(times, opens, cells: dict, draws: int, seed: int) -> tuple:
    """C's mean against `draws` equal-sized subsamples of D — the script's null."""
    got = {}
    for key in "ACD":
        arr = np.array([fc.move(times, opens, d) for d in cells[key]])
        got[key] = arr[np.isfinite(arr)]
    if len(got["C"]) < 3 or len(got["D"]) < len(got["C"]):
        return float("nan"), 0, 0, float("nan"), float("nan")
    rng = np.random.default_rng(seed)
    nulls = np.array([
        got["D"][rng.choice(len(got["D"]), size=len(got["C"]), replace=False)].mean()
        for _ in range(draws)
    ])
    pct = 100.0 * float((nulls < got["C"].mean()).mean())
    up = 100.0 * float((got["C"] > 0).mean())
    return pct, len(got["C"]), len(got["D"]), got["C"].mean(), up


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--market", default="xauduka")
    ap.add_argument("--start", default="2010-06-01")
    ap.add_argument("--end", default="2026-05-31")
    ap.add_argument("--draws", type=int, default=2000)
    ap.add_argument("--fakes", type=int, default=25)
    ap.add_argument("--seed", type=int, default=20260915)
    a = ap.parse_args()
    lo, hi = pd.Timestamp(a.start, tz="UTC"), pd.Timestamp(a.end, tz="UTC")

    bars = fc.load_bars(a.market, lo, hi)
    times = bars["time"].dt.tz_convert("UTC").astype("datetime64[ns, UTC]").astype("int64").to_numpy()
    opens = bars["open"].to_numpy()
    nfp, busy = fc.load_calendar(lo, hi)
    # Dates carrying TWO OR MORE scheduled high-impact events. This is the
    # screen the controls already get, expressed so it can be applied to the
    # treatments too.
    _ev = pd.read_csv(fc.ROOT / "data" / "news" / "events.csv")
    _ev["t"] = pd.to_datetime(_ev["time_utc"], utc=True)
    _ev = _ev[(_ev["t"] >= lo) & (_ev["t"] < hi) & (_ev["source"] != "forexfactory") & (_ev["impact"] == 3)]
    _per_day = _ev["t"].dt.tz_convert(NEW_YORK).dt.normalize().value_counts()
    multi = set(_per_day[_per_day >= 2].index)

    days = pd.date_range(lo.tz_convert(NEW_YORK).normalize(),
                         hi.tz_convert(NEW_YORK).normalize(), freq="D", tz=NEW_YORK)
    fridays = days[days.weekday == 4]
    release = set(nfp)

    print(f"friday_cells self-test, {a.market} 15m, {lo.date()} -> {hi.date()}")
    print(f"{len(bars):,} bars, {len(nfp)} releases, {len(fridays)} Fridays, "
          f"{a.draws:,} null draws\n")

    real = cells_from(fridays, release, busy, symmetric=False)
    pct, nC, nD, meanC, upC = percentile_C(times, opens, real, a.draws, a.seed)
    print(f"the published construction: C n={nC}, D n={nD}, C mean {meanC:+.3f} $, "
          f"up {upC:.1f}%, percentile {pct:.2f}")

    print(f"\nA. CALIBRATION - {a.fakes} fake release assignments, same cell sizes")
    print("   C is a random subset of later Fridays; its percentile must be uniform.")
    nA_real = len(real["A"])
    nC_real = len(real["C"])
    first_f = [d for d in fridays if d.day <= 7]
    later_f = [d for d in fridays if d.day > 7]
    rs = np.random.default_rng(a.seed)
    pcts = []
    for i in range(a.fakes):
        fake = set(
            [first_f[j] for j in rs.choice(len(first_f), size=min(nA_real, len(first_f)), replace=False)]
            + [later_f[j] for j in rs.choice(len(later_f), size=min(nC_real, len(later_f)), replace=False)]
        )
        cells = cells_from(fridays, fake, busy, symmetric=False)
        p, n_c, n_d, m, u = percentile_C(times, opens, cells, a.draws, a.seed + 1 + i)
        pcts.append(p)
        print(f"   fake {i + 1:2d}: C n={n_c:3d} mean {m:+7.3f} up {u:4.1f}% pctile {p:6.2f}")
    p = np.array([x for x in pcts if np.isfinite(x)])
    print(f"\n   {len(p)} fakes: min {p.min():.2f} median {np.median(p):.2f} max {p.max():.2f} "
          f"mean {p.mean():.2f}")
    print(f"   below the 5th: {int((p < 5).sum())} (expected {0.05 * len(p):.1f}); "
          f"below the 1st: {int((p < 1).sum())} (expected {0.01 * len(p):.1f})")
    verdict = "calibrated" if 25 <= p.mean() <= 75 and (p < 1).sum() <= max(1, 0.05 * len(p)) else "SUSPECT"
    print(f"   verdict: {verdict}")

    print(f"\nB. FAULT 9 - the screen applied to BOTH arms instead of one")
    sym = cells_from(fridays, release, busy, symmetric=True, multi=multi)
    pct_s, nC_s, nD_s, meanC_s, upC_s = percentile_C(times, opens, sym, a.draws, a.seed)
    print(f"   as published (screen on controls only): C n={nC} mean {meanC:+.3f} "
          f"up {upC:.1f}% pctile {pct:.2f}")
    print(f"   screen on both arms:                    C n={nC_s} mean {meanC_s:+.3f} "
          f"up {upC_s:.1f}% pctile {pct_s:.2f}")
    print(f"   later-Friday releases that also carry a second high-impact event: {nC - nC_s}")
    moved = abs(pct_s - pct)
    print(f"   the surviving number moves by {moved:.2f} percentile points.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
