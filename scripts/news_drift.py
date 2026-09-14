"""What gold does around a scheduled release, measured before anything is claimed.

    python scripts/news_drift.py xauduka 2010-06-01 2018-06-15

For each class of event in `data/news/events.csv` (FOMC, US CPI, US Employment
Situation, ECB) and each window relative to the release, this prints the mean
move, its t-statistic, how often it was positive, and the same figure on
**non-event days at the same clock time** — the control that says whether a
number is the release or the hour.

Windows, in minutes relative to the release: (-1440, 0) the day before,
(-60, 0) the hour before, (0, +15) the print itself, (0, +60) the hour after,
(0, +480) the rest of the session. A move is open-to-open on the bars, the way
the engine fills, and is quoted in the instrument's own units and in units of
the trailing 20-day range so two instruments can be compared.

This is a measurement, not a receipt. Nothing here is a strategy and nothing
here is evidence for one: run it on the exploratory half of the data, decide
what is worth pre-registering, and test that on the half this never touched.
The loop has closed 26 registrations whose first window looked like this one.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

import numpy as np
import pandas as pd
import pyarrow as pa
import pyarrow.compute as pc
import pyarrow.parquet as pq

ROOT = Path(__file__).resolve().parents[1]

# Minutes relative to the release; the label is what the record will quote.
WINDOWS: list[tuple[str, int, int]] = [
    ("day before", -1440, 0),
    ("hour before", -60, 0),
    ("the print", 0, 15),
    ("hour after", 0, 60),
    ("session after", 0, 480),
]


def bar_symbol(market: str) -> str:
    text = (ROOT / "config" / "default.toml").read_text(encoding="utf-8")
    table = re.search(r"\[markets\." + re.escape(market) + r"\]\s*(.*?)(?=\n\[)", text, re.S)
    if table is None:
        sys.exit(f"no [markets.{market}] in config/default.toml")
    found = re.search(r'bar_symbol\s*=\s*"([^"]+)"', table.group(1))
    if found is None:
        sys.exit(f"no bar_symbol for {market}")
    return found.group(1)


def load_bars(market: str, lo: pd.Timestamp, hi: pd.Timestamp) -> pd.DataFrame:
    path = ROOT / "data" / "bars" / f"{bar_symbol(market)}-15m.parquet"
    table = pq.read_table(path)
    lo_s = pa.scalar(lo, type=table["time"].type)
    hi_s = pa.scalar(hi, type=table["time"].type)
    table = table.filter(pc.and_(pc.greater_equal(table["time"], lo_s), pc.less(table["time"], hi_s)))
    df = table.to_pandas().sort_values("time").reset_index(drop=True)
    # The trailing 20-day range, so a move can be read in the unit the engine
    # sizes with rather than only in price.
    day = df["time"].dt.floor("D")
    span = df.groupby(day)["high"].max() - df.groupby(day)["low"].min()
    df["range20"] = day.map(span.rolling(20, min_periods=5).mean().shift(1))
    return df


def load_events(lo: pd.Timestamp, hi: pd.Timestamp) -> pd.DataFrame:
    events = pd.read_csv(ROOT / "data" / "news" / "events.csv")
    events["time_utc"] = pd.to_datetime(events["time_utc"], utc=True)
    events = events[(events["time_utc"] >= lo) & (events["time_utc"] < hi)]
    # Only the scheduled history, not the live ForexFactory layer: it covers
    # one week and would put a thumb on whichever class it happens to hold.
    events = events[events["source"] != "forexfactory"]
    return events[events["impact"] == 3].sort_values("time_utc").reset_index(drop=True)


def open_at(times: np.ndarray, opens: np.ndarray, when_ns: int) -> float:
    """The open of the first bar at or after `when_ns`; NaN past the end.

    Times are integer nanoseconds on both sides: numpy's `searchsorted` has no
    notion of a timezone and pandas refuses to compare an aware stamp with a
    naive one, so the comparison is done in the one unit neither can mistake.

    The engine fills at an open, so a window measured open-to-open is the one
    a trade could have had.
    """
    i = int(np.searchsorted(times, when_ns, side="left"))
    return float(opens[i]) if i < len(opens) else float("nan")


def permutation(
    times: np.ndarray,
    opens: np.ndarray,
    when: pd.Series,
    a: int,
    b: int,
    draws: int,
    seed: int,
) -> tuple[float, float]:
    """Where the events' mean move sits among `draws` fake event sets.

    A fake set is the same number of days, drawn from the weekdays of the same
    window at the same clock minute and the same weekday as the real releases —
    so the null holds the hour and the day of the week fixed and varies only
    *which* dates were announcements. That is the question: is it the release,
    or is it Friday at half past eight.

    Returns (percentile of the actual mean, the null's own mean).
    """
    stamps = pd.DatetimeIndex(when)
    minute = stamps[0].hour * 60 + stamps[0].minute
    weekday = stamps[0].weekday()
    # Every candidate day in the bars' span with that weekday, stamped at the
    # release minute; the real release days are left in, because removing them
    # would make the null a sample of "days that were not announcements" and
    # bias it by exactly the effect being measured.
    first = pd.Timestamp(times[0], tz="UTC").normalize()
    last = pd.Timestamp(times[-1], tz="UTC").normalize()
    days = pd.date_range(first, last, freq="D", tz="UTC")
    days = days[days.weekday == weekday]
    candidates = days + pd.Timedelta(minutes=minute)

    def mean_move(points: pd.DatetimeIndex) -> float:
        moves = []
        for t in points:
            lo = open_at(times, opens, (t + pd.Timedelta(minutes=a)).value)
            hi = open_at(times, opens, (t + pd.Timedelta(minutes=b)).value)
            moves.append(hi - lo)
        arr = np.array(moves)
        arr = arr[np.isfinite(arr)]
        return float(arr.mean()) if len(arr) else float("nan")

    actual = mean_move(stamps)
    rng = np.random.default_rng(seed)
    nulls = []
    for _ in range(draws):
        pick = rng.choice(len(candidates), size=min(len(stamps), len(candidates)), replace=False)
        value = mean_move(candidates[np.sort(pick)])
        if np.isfinite(value):
            nulls.append(value)
    if not nulls:
        return float("nan"), float("nan")
    nulls = np.array(nulls)
    return 100.0 * float((nulls < actual).mean()), float(nulls.mean())


def describe(moves: np.ndarray, ranges: np.ndarray, unit: str) -> str:
    keep = np.isfinite(moves)
    moves, ranges = moves[keep], ranges[keep]
    if len(moves) < 5:
        return f"{len(moves):5d}  too few"
    mean = moves.mean()
    t = mean / moves.std(ddof=1) * np.sqrt(len(moves)) if moves.std(ddof=1) > 0 else 0.0
    in_r = moves / ranges
    in_r = in_r[np.isfinite(in_r)]
    r_mean = in_r.mean() if len(in_r) else float("nan")
    return f"{len(moves):5d}  {mean:+9.3f} {unit}  t {t:+5.2f}  up {100 * (moves > 0).mean():4.1f}%  {r_mean:+6.3f} R"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("market", nargs="?", default="xauduka")
    ap.add_argument("start", nargs="?", default="2010-06-01")
    ap.add_argument("end", nargs="?", default="2018-06-15")
    ap.add_argument("--unit", default="$", help="what one price point is called")
    ap.add_argument("--permute", default="", help="'NAME:a:b' — run the date-permutation null on that class and window")
    ap.add_argument("--draws", type=int, default=1000)
    ap.add_argument("--seed", type=int, default=7)
    args = ap.parse_args()

    lo, hi = pd.Timestamp(args.start, tz="UTC"), pd.Timestamp(args.end, tz="UTC")
    bars = load_bars(args.market, lo, hi)
    events = load_events(lo, hi)
    if bars.empty or events.empty:
        sys.exit("no bars or no events in that window")
    times = bars["time"].dt.tz_convert("UTC").astype("datetime64[ns, UTC]").astype("int64").to_numpy()
    opens = bars["open"].to_numpy()
    ranges = bars.set_index("time")["range20"]

    print(f"{args.market} 15m {args.start} -> {args.end}: {len(bars)} bars, {len(events)} high-impact events")
    print("a move is open-to-open; R is the trailing 20-day range\n")

    if args.permute:
        name, a, b = args.permute.rsplit(":", 2)
        when = events.loc[events["name"] == name, "time_utc"]
        if when.empty:
            sys.exit(f"no events named {name!r}; have {sorted(events['name'].unique())}")
        pct, null_mean = permutation(times, opens, when, int(a), int(b), args.draws, args.seed)
        moves = np.array([
            open_at(times, opens, (t + pd.Timedelta(minutes=int(b))).value)
            - open_at(times, opens, (t + pd.Timedelta(minutes=int(a))).value)
            for t in when
        ])
        moves = moves[np.isfinite(moves)]
        t_stat = moves.mean() / moves.std(ddof=1) * np.sqrt(len(moves))
        print(f"{name}  {a}..{b} min  {len(moves)} events")
        print(f"  actual mean {moves.mean():+.3f} {args.unit}  t {t_stat:+.2f}  up {100 * (moves > 0).mean():.1f}%")
        print(f"  {args.draws} date permutations (same weekday, same minute): mean {null_mean:+.3f} {args.unit}")
        print(f"  actual sits at the {pct:.1f}th percentile of them")
        return 0

    classes = sorted(events["name"].unique())
    for name in classes:
        when = events.loc[events["name"] == name, "time_utc"]
        print(f"{name}  ({len(when)} events)")
        for label, a, b in WINDOWS:
            moves, rs, placebo, placebo_rs = [], [], [], []
            for t in when:
                start = t + pd.Timedelta(minutes=a)
                end = t + pd.Timedelta(minutes=b)
                first = open_at(times, opens, start.value)
                last = open_at(times, opens, end.value)
                moves.append(last - first)
                r = ranges.asof(start)
                rs.append(r if pd.notna(r) else np.nan)
                # The control: the same clock window seven days earlier, which
                # is a weekday with no release of this class by construction.
                p_start, p_end = start - pd.Timedelta(days=7), end - pd.Timedelta(days=7)
                if (p_start - pd.Timedelta(minutes=1)) in when.values:
                    continue
                pf, pl = open_at(times, opens, p_start.value), open_at(times, opens, p_end.value)
                placebo.append(pl - pf)
                pr = ranges.asof(p_start)
                placebo_rs.append(pr if pd.notna(pr) else np.nan)
            print(f"  {label:14s} {describe(np.array(moves), np.array(rs), args.unit)}")
            print(f"  {'  (7 days earlier)':14s} {describe(np.array(placebo), np.array(placebo_rs), args.unit)}")
        print()
    return 0


if __name__ == "__main__":
    sys.exit(main())
