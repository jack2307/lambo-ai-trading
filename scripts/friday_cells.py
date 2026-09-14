"""Is the pre-NFP hour the release, or is it the first Friday of the month?

    python scripts/friday_cells.py xauduka 2010-06-01 2026-05-31

`2026-09-14-pre-nfp-drift` found that gold falls over 07:30 -> 08:30 New York
on the day the US Employment Situation is released. Every one of those days is
also, almost always, the first Friday of a month — so "the release" and "the
first Friday" are the same column of the calendar and the permutation null in
that record cannot tell them apart. The adversary put roughly a quarter to a
third of the fall on the calendar and left the rest unexplained.

The calendar separates them itself, a few times a year. The Employment
Situation is released on the third Friday after the reference week, which is
usually the month's first Friday and sometimes its second. That gives four
cells, and the two explanations order them differently:

              release        no release
  first Fri     A               B
  later Fri     C               D

  the release  -> A and C fall, B and D do not
  the calendar -> A and B fall, C and D do not

This script measures the same 07:30 -> 08:30 New York hour on all four and
prints them side by side, with a permutation null for C drawn from D.

It is an explanatory test on windows already read for the parent question, so
it cannot confirm that claim — only explain it or weaken it. See the
registration for what each ordering is allowed to mean.

Read-only: it touches the bar store and the news calendar and nothing else.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import numpy as np
import pandas as pd
import pyarrow as pa
import pyarrow.compute as pc
import pyarrow.parquet as pq

sys.path.insert(0, str(Path(__file__).resolve().parent))
from news_drift import NEW_YORK, bar_symbol, open_at  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]

#: The window, in New York local minutes past midnight. 07:30 is an hour
#: before the release and 08:30 is the release itself; both are read as the
#: open of the first bar at or after them, the way the engine fills.
START_MIN = 7 * 60 + 30
END_MIN = 8 * 60 + 30


def load_bars(market: str, lo: pd.Timestamp, hi: pd.Timestamp) -> pd.DataFrame:
    path = ROOT / "data" / "bars" / f"{bar_symbol(market)}-15m.parquet"
    table = pq.read_table(path)
    lo_s = pa.scalar(lo, type=table["time"].type)
    hi_s = pa.scalar(hi, type=table["time"].type)
    table = table.filter(pc.and_(pc.greater_equal(table["time"], lo_s), pc.less(table["time"], hi_s)))
    return table.to_pandas().sort_values("time").reset_index(drop=True)


def load_calendar(lo: pd.Timestamp, hi: pd.Timestamp) -> tuple[pd.DatetimeIndex, set]:
    """(the NFP releases, every date carrying any scheduled high-impact event).

    The second set is what keeps a control day honest: a Friday that carried
    the ECB or a CPI print is not a quiet Friday, and putting it in the
    no-release cells would measure a different release rather than none.
    """
    events = pd.read_csv(ROOT / "data" / "news" / "events.csv")
    events["t"] = pd.to_datetime(events["time_utc"], utc=True)
    events = events[(events["t"] >= lo) & (events["t"] < hi) & (events["source"] != "forexfactory")]
    events = events[events["impact"] == 3]
    local = events["t"].dt.tz_convert(NEW_YORK)
    busy = set(local.dt.normalize())
    nfp = local[events["name"] == "US Employment Situation (NFP)"]
    return pd.DatetimeIndex(sorted(nfp.dt.normalize())), busy


def move(times: np.ndarray, opens: np.ndarray, day: pd.Timestamp) -> float:
    """The 07:30 -> 08:30 New York move on `day`, or NaN if the market was shut.

    `day` is midnight New York, so adding the minutes keeps the wall clock in
    both seasons — the fault that had to be corrected in `news_drift.py`.
    """
    lo = open_at(times, opens, (day + pd.Timedelta(minutes=START_MIN)).tz_convert("UTC").value)
    hi = open_at(times, opens, (day + pd.Timedelta(minutes=END_MIN)).tz_convert("UTC").value)
    return hi - lo


def describe(name: str, moves: np.ndarray) -> str:
    moves = moves[np.isfinite(moves)]
    if len(moves) < 3:
        return f"{name:34s} {len(moves):4d}  too few"
    mean = moves.mean()
    sd = moves.std(ddof=1)
    t = mean / sd * np.sqrt(len(moves)) if sd > 0 else 0.0
    return (
        f"{name:34s} {len(moves):4d}  {mean:+7.3f} $  t {t:+5.2f}  "
        f"up {100 * (moves > 0).mean():4.1f}%  median {np.median(moves):+7.3f}"
    )


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("market", nargs="?", default="xauduka")
    ap.add_argument("start", nargs="?", default="2010-06-01")
    ap.add_argument("end", nargs="?", default="2026-05-31")
    ap.add_argument("--draws", type=int, default=1000)
    ap.add_argument("--seed", type=int, default=7)
    args = ap.parse_args()

    lo, hi = pd.Timestamp(args.start, tz="UTC"), pd.Timestamp(args.end, tz="UTC")
    bars = load_bars(args.market, lo, hi)
    if bars.empty:
        sys.exit("no bars in that window")
    times = bars["time"].dt.tz_convert("UTC").astype("datetime64[ns, UTC]").astype("int64").to_numpy()
    opens = bars["open"].to_numpy()
    nfp, busy = load_calendar(lo, hi)

    days = pd.date_range(lo.tz_convert(NEW_YORK).normalize(), hi.tz_convert(NEW_YORK).normalize(), freq="D", tz=NEW_YORK)
    fridays = days[days.weekday == 4]
    release = set(nfp)

    cells: dict[str, list[pd.Timestamp]] = {"A": [], "B": [], "C": [], "D": []}
    for d in fridays:
        first = d.day <= 7
        if d in release:
            cells["A" if first else "C"].append(d)
        elif d not in busy:
            cells["B" if first else "D"].append(d)
    # A release that landed on a weekday other than Friday belongs with C: it
    # is a release away from the first Friday, which is the whole point of the
    # cell. Kept separate so the record can quote either.
    off_friday = [d for d in nfp if d.weekday() != 4]

    labels = {
        "A": "first Friday, release",
        "B": "first Friday, no release",
        "C": "later Friday, release",
        "D": "later Friday, no release",
    }
    print(f"{args.market} 15m {args.start} -> {args.end}: {len(bars)} bars, {len(nfp)} NFP releases")
    print(f"the 07:30 -> 08:30 New York hour, open to open\n")
    got: dict[str, np.ndarray] = {}
    for key in "ABCD":
        arr = np.array([move(times, opens, d) for d in cells[key]])
        got[key] = arr[np.isfinite(arr)]
        print(f"  {key}  {describe(labels[key], arr)}")
    arr = np.array([move(times, opens, d) for d in off_friday])
    print(f"  C' {describe('release, not a Friday at all', arr)}")
    print()

    # The two orderings, as the registration states them.
    print(f"  release reading  wants C < 0 and C close to A: A {got['A'].mean():+.3f}  C {got['C'].mean():+.3f}")
    print(f"  calendar reading wants B < 0 and B close to A: A {got['A'].mean():+.3f}  B {got['B'].mean():+.3f}")
    print(f"  the quiet baseline                            D {got['D'].mean():+.3f}")
    print()

    # Is C's fall more than a draw of the same size from D? This is the only
    # null here, and with ~33 observations it is declared underpowered in the
    # registration: it is read as a direction, not as a significance.
    rng = np.random.default_rng(args.seed)
    nulls = []
    for _ in range(args.draws):
        pick = rng.choice(len(got["D"]), size=min(len(got["C"]), len(got["D"])), replace=False)
        nulls.append(got["D"][pick].mean())
    nulls = np.array(nulls)
    pct = 100.0 * float((nulls < got["C"].mean()).mean())
    print(f"  C against {args.draws} draws of {len(got['C'])} quiet later Fridays: null mean {nulls.mean():+.3f}")
    print(f"  C sits at the {pct:.1f}th percentile of them")

    # And the same for B, which is the calendar's own claim.
    nulls_b = []
    for _ in range(args.draws):
        pick = rng.choice(len(got["D"]), size=min(len(got["B"]), len(got["D"])), replace=False)
        nulls_b.append(got["D"][pick].mean())
    nulls_b = np.array(nulls_b)
    pct_b = 100.0 * float((nulls_b < got["B"].mean()).mean())
    print(f"  B against {args.draws} draws of {len(got['B'])} quiet later Fridays: null mean {nulls_b.mean():+.3f}")
    print(f"  B sits at the {pct_b:.1f}th percentile of them")
    return 0


if __name__ == "__main__":
    sys.exit(main())
