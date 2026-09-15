"""Can `news_drift.py` find a drift you plant, and does it stay quiet on fake dates?

    python scripts/news_drift_selftest.py

`2026-09-14-pre-nfp-drift` is the only registration in this loop that survived
its falsifier: gold's 07:30 -> 08:30 New York hour before the US employment
report sits at the 3.2nd percentile of a date-permutation null on 2018-06 ->
2026-05. Every other finding here is a closure, where a bug can only bury an
effect. This one is the opposite: a bug in the permutation would **manufacture**
it, and the loop would be carrying a false positive as its single survivor.

`2026-09-15-quote-asymmetry` shipped three faults inside one file in one day,
one of which (`mean(sign * sign * raw)`) left every other column looking
perfectly reasonable. That is the fault class this test exists for.

Two tests, and the second is the one that matters:

  A  RECOVERY.  Plant a drift of exactly delta dollars into the hour before each
     real release - subtract delta from every bar stamped in [event-60m, event) -
     and nothing else. The hour-before move must rise by exactly delta and the
     percentile must move monotonically with it. delta = 0 is the real data and
     must reproduce the published number.

  B  CALIBRATION.  Leave the bars untouched and hand the null FAKE release
     dates, drawn from the same pool it draws its own controls from - same
     weekday, same New York minute, same span. Twenty draws. If the harness is
     honest these percentiles are roughly uniform. If they cluster low, the
     3.2nd percentile is a property of the instrument and not of the release.

Read-only: it loads bars and the calendar, and writes nothing.
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
from news_drift import load_bars, load_events, open_at, permutation, NEW_YORK  # noqa: E402

EVENT = "US Employment Situation (NFP)"


def hour_before_mean(times, opens, when) -> tuple:
    moves = []
    for t in pd.DatetimeIndex(when):
        lo = open_at(times, opens, (t - pd.Timedelta(minutes=60)).value)
        hi = open_at(times, opens, t.value)
        moves.append(hi - lo)
    arr = np.array(moves)
    arr = arr[np.isfinite(arr)]
    return float(arr.mean()), int(len(arr)), 100.0 * float((arr > 0).mean())


def plant(df: pd.DataFrame, when, delta: float) -> np.ndarray:
    """Subtract `delta` from every bar stamped inside [event-60m, event).

    A pulse in the hour before and nowhere else: the hour-before move is
    open(event) - open(event-60m), so lowering only the earlier end raises the
    measured move by exactly delta, and every window that does not straddle the
    hour before is left alone.
    """
    opens = df["open"].to_numpy().copy()
    # Integer nanoseconds on both sides, built exactly the way news_drift.py
    # builds them. These bar files are datetime64[ms]; taking `.to_numpy()` or
    # `.asi8` off them yields MILLIseconds, and news_drift's own tolerance is a
    # nanosecond constant. It casts explicitly before converting, and so does
    # this - the agreement is the point, not the convenience.
    t = df["time"].dt.tz_convert("UTC").astype("datetime64[ns, UTC]").astype("int64").to_numpy()
    for e in pd.DatetimeIndex(when):
        opens[(t >= (e - pd.Timedelta(minutes=60)).value) & (t < e.value)] -= delta
    return opens


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--market", default="xauduka")
    ap.add_argument("--start", default="2018-06-01")
    ap.add_argument("--end", default="2026-06-01")
    ap.add_argument("--draws", type=int, default=2000)
    ap.add_argument("--fakes", type=int, default=20)
    ap.add_argument("--seed", type=int, default=20260915)
    a = ap.parse_args()
    lo, hi = pd.Timestamp(a.start, tz="UTC"), pd.Timestamp(a.end, tz="UTC")

    df = load_bars(a.market, lo, hi)
    times = df["time"].dt.tz_convert("UTC").astype("datetime64[ns, UTC]").astype("int64").to_numpy()
    events = load_events(lo, hi)
    when = events.loc[events["name"] == EVENT, "time_utc"].reset_index(drop=True)

    print(f"news_drift self-test, {a.market} 15m, {lo.date()} -> {hi.date()}")
    print(f"{len(df):,} bars, {len(when)} '{EVENT}' releases, {a.draws:,} permutation draws")
    print(f"the window under test is the registered one: the hour before, -60 -> 0\n")

    print("A. RECOVERY - a drift planted in the hour before, and nowhere else")
    print(f"   {'planted $':>10s} {'n':>4s} {'mean move':>10s} {'recovered':>10s} {'up%':>6s} {'pctile':>7s}")
    base = None
    for delta in (0.0, -1.0, -0.5, +0.5, +1.0):
        opens = plant(df, when, delta)
        mean, n, up = hour_before_mean(times, opens, when)
        if base is None:
            base = mean
        pct, _ = permutation(times, opens, when, -60, 0, a.draws, a.seed)
        rec = mean - base
        flag = "" if delta == 0.0 else ("  ok" if abs(rec - delta) < 1e-6 else "  <-- NOT RECOVERED")
        print(f"   {delta:+10.2f} {n:4d} {mean:+10.4f} {rec:+10.4f} {up:5.1f}% {pct:7.2f}{flag}")
    print("   delta = 0 is the real data; its percentile is the published finding.")
    print("   'recovered' must equal 'planted' to the penny, and the percentile must be monotone.\n")

    print("B. CALIBRATION - real bars, FAKE release dates from the null's own pool")
    print("   if the harness is honest these are roughly uniform on 0-100.")
    stamps = pd.DatetimeIndex(when)
    local = stamps.tz_convert(NEW_YORK)
    minute = int(local[0].hour) * 60 + int(local[0].minute)
    weekday = int(local[0].weekday())
    first = pd.Timestamp(times[0], tz="UTC").tz_convert(NEW_YORK).normalize()
    last = pd.Timestamp(times[-1], tz="UTC").tz_convert(NEW_YORK).normalize()
    days = pd.date_range(first, last, freq="D", tz=NEW_YORK)
    days = days[days.weekday == weekday]
    pool = (days + pd.Timedelta(minutes=minute)).tz_convert("UTC")
    opens = df["open"].to_numpy()

    rs = np.random.default_rng(a.seed)
    pcts = []
    for i in range(a.fakes):
        pick = np.sort(rs.choice(len(pool), size=len(stamps), replace=False))
        fake = pd.Series(pool[pick])
        pct, _ = permutation(times, opens, fake, -60, 0, a.draws, a.seed + 1 + i)
        mean, n, up = hour_before_mean(times, opens, fake)
        pcts.append(pct)
        print(f"   fake set {i + 1:2d}: n {n:3d}  mean {mean:+8.4f}  up {up:5.1f}%  pctile {pct:6.2f}")
    p = np.array(pcts)
    print(f"\n   {a.fakes} fake sets: min {p.min():.2f}  median {np.median(p):.2f}  max {p.max():.2f}  "
          f"mean {p.mean():.2f}")
    print(f"   below the 5th percentile: {int((p < 5).sum())} of {a.fakes} "
          f"(expected about {0.05 * a.fakes:.1f})")
    print(f"   below the 50th: {int((p < 50).sum())} of {a.fakes} (expected about {a.fakes / 2:.0f})")
    verdict = "calibrated" if 20 <= p.mean() <= 80 and (p < 5).sum() <= max(2, 0.2 * a.fakes) else "SUSPECT"
    print(f"   verdict: {verdict}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
