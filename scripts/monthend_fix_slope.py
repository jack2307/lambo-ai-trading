"""Does a month of US equity outperformance sign the euro's move through the fix?

    python scripts/monthend_fix_slope.py --draws=100000

The registration is `docs/hypotheses/2026-09-15-monthend-fix-slope.md` and it
was committed before this file had a first line. Everything gated here was
declared there: the window, the variable, the direction, the null, the three
placebos, the number of draws, and the band that is to be called underpowered
rather than refuted. **Nothing in this script may be tuned to make a gate
pass.** If a number disappoints, the number is the result.

The claim, in one line: a euro-based holder of US equities hedges with short
dollar forwards sized to the portfolio, its mandate says to resize at month end
at the benchmark, so US outperformance through the month forces it to sell more
dollars into the 16:00 London fix.

What that makes this script measure is a SLOPE, not a drift. An unconditional
month-end drift would be an intercept, and `2026-09-14-fx-local-hours.md`
already established a real unconditional euro drift through European hours that
the fix window sits inside. The null permutes the equity return across
month-ends precisely so that the drift is preserved and only the conditioning
is tested.

Three placebos, each fatal on its own, each declared in advance:

  day     the penultimate business day must NOT reproduce the slope
          (or the day is not the variable and this is monthly momentum)
  window  13:45-14:15 London, inside European hours with no benchmark in it,
          must NOT reproduce it (or it is the closed drift, not the fix)
  gold    the same machinery on gold's month-to-date return must NOT
          reproduce it (or the variable is risk appetite, not hedging)
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

#: Windows in London local time, as the registration names them.
#: London because the benchmark is a London benchmark; reading it on a New York
#: or a UTC clock would put a third of the year in the wrong half hour, which is
#: the fault this repository has logged twice (6 and 8).
LONDON = "Europe/London"
FIX_WINDOW = (15 * 60 + 45, 16 * 60 + 15)
PLACEBO_WINDOW = (13 * 60 + 45, 14 * 60 + 15)

#: One round trip on `eurduka`, measured read-only against the live terminal.
#: 1.4 pips on a mean level of 1.16 is 1.21 basis points, and the registration
#: declares the band between this and the minimum detectable slope to be
#: UNDERPOWERED rather than "no".
COST_BP = 1.21


def bar_symbol(market: str) -> str:
    text = (ROOT / "config" / "default.toml").read_text(encoding="utf-8")
    table = re.search(r"\[markets\." + re.escape(market) + r"\]\s*(.*?)(?=\n\[)", text, re.S)
    if table is None:
        sys.exit(f"no [markets.{market}] in config/default.toml")
    return re.search(r'bar_symbol\s*=\s*"([^"]+)"', table.group(1)).group(1)


def load_bars(market: str, lo: pd.Timestamp, hi: pd.Timestamp) -> pd.DataFrame:
    path = ROOT / "data" / "bars" / f"{bar_symbol(market)}-15m.parquet"
    t = pq.read_table(path)
    t = t.filter(
        pc.and_(
            pc.greater_equal(t["time"], pa.scalar(lo, type=t["time"].type)),
            pc.less(t["time"], pa.scalar(hi, type=t["time"].type)),
        )
    )
    df = t.to_pandas()[["time", "open", "close"]].sort_values("time").reset_index(drop=True)
    df["time"] = pd.to_datetime(df["time"], utc=True)
    return df


def load_equity(name: str) -> pd.Series:
    """Daily closes, on the exchange's own trading days.

    Never forward-filled. The two indices keep different holiday calendars and
    a gap is information about which market was shut; filling it would invent a
    return on a day nobody could trade.
    """
    path = ROOT / "data" / "equity" / f"{name}.csv"
    if not path.exists():
        sys.exit(f"missing {path.relative_to(ROOT)} — the registration is withdrawn without it")
    df = pd.read_csv(path, comment="#")
    df["date"] = pd.to_datetime(df["date"]).dt.tz_localize("UTC")
    return df.set_index("date")["close"].sort_index()


def window_return(bars: pd.DataFrame, day: pd.Timestamp, window: tuple[int, int]) -> tuple[float, int]:
    """Log return across a London-clock window on one day, and the bars in it.

    Open-to-open across the window's own bars, the way the engine fills, and
    the bar count is returned beside it so a thin day can be seen rather than
    silently averaged in.
    """
    local = bars["time"].dt.tz_convert(LONDON)
    minute = local.dt.hour * 60 + local.dt.minute
    on_day = local.dt.normalize() == day
    inside = on_day & (minute >= window[0]) & (minute < window[1])
    rows = bars.loc[inside]
    if len(rows) < 2:
        return float("nan"), len(rows)
    return float(np.log(rows["close"].iloc[-1] / rows["open"].iloc[0])) * 1e4, len(rows)


def month_to_date(series: pd.Series, through: pd.Timestamp) -> float:
    """Return from the previous month's last close to `through`, in log points.

    `through` is the close PRIOR to the last business day — the information a
    treasurer actually has when the order is sized. A month whose index has no
    close on or before `through`, or none in the previous month, returns NaN and
    is dropped rather than approximated.
    """
    prior = series[series.index <= through]
    if prior.empty:
        return float("nan")
    last = prior.iloc[-1]
    month_start = through.replace(day=1)
    before = series[series.index < month_start]
    if before.empty:
        return float("nan")
    return float(np.log(last / before.iloc[-1]))


def ols_slope(x: np.ndarray, y: np.ndarray) -> float:
    """Slope of y on x. No intercept suppression — the drift lives there."""
    var = x.var()
    return float(np.cov(x, y, ddof=1)[0, 1] / var) if var > 0 else float("nan")


def build(lo: pd.Timestamp, hi: pd.Timestamp) -> pd.DataFrame:
    """One row per month-end: the windows, the conditioning variables, the bars."""
    eur = load_bars("eurduka", lo, hi)
    gold = load_bars("xauduka", lo, hi)
    spx = load_equity("spx")
    stoxx = load_equity("stoxx50e")

    # The trading days the euro feed actually has, on a London clock. The last
    # of each month is the month-end; the one before it is the day placebo.
    days = pd.DatetimeIndex(sorted(set(eur["time"].dt.tz_convert(LONDON).dt.normalize())))
    by_month: dict[tuple[int, int], list[pd.Timestamp]] = {}
    for d in days:
        by_month.setdefault((d.year, d.month), []).append(d)

    rows = []
    for (year, month), in_month in sorted(by_month.items()):
        if len(in_month) < 2:
            continue
        last, penult = in_month[-1], in_month[-2]
        fix, n_fix = window_return(eur, last, FIX_WINDOW)
        placebo_day, n_day = window_return(eur, penult, FIX_WINDOW)
        placebo_win, n_win = window_return(eur, last, PLACEBO_WINDOW)
        # The conditioning variable is measured through the close PRIOR to the
        # last business day, so the equity information is strictly older than
        # the window being predicted.
        through = penult
        equity = month_to_date(spx, through) - month_to_date(stoxx, through)
        gold_mtd = month_to_date(gold.set_index("time")["close"], through)
        rows.append(
            {
                "year": year,
                "month": month,
                "last": last,
                "fix_bp": fix,
                "bars_fix": n_fix,
                "placebo_day_bp": placebo_day,
                "bars_day": n_day,
                "placebo_win_bp": placebo_win,
                "bars_win": n_win,
                "equity": equity,
                "gold": gold_mtd,
            }
        )
    return pd.DataFrame(rows)


def permutation(x: np.ndarray, y: np.ndarray, draws: int, seed: int) -> tuple[float, np.ndarray]:
    """Where the slope sits among `draws` shuffles of the conditioning variable.

    Permuting x and not y is the point: every month-end keeps its own window
    return, so the unconditional drift is preserved exactly and only the pairing
    between the equity month and the fix is destroyed. A null that shuffled the
    windows would be testing whether month-ends move at all, which is a question
    `2026-09-13-london-fix.md` already answered.
    """
    rng = np.random.default_rng(seed)
    actual = ols_slope(x, y)
    nulls = np.empty(draws)
    for i in range(draws):
        nulls[i] = ols_slope(rng.permutation(x), y)
    return actual, nulls


def report(name: str, x: np.ndarray, y: np.ndarray, draws: int, seed: int, unit: str) -> float:
    actual, nulls = permutation(x, y, draws, seed)
    pct = 100.0 * float((nulls < actual).mean())
    sd = float(x.std(ddof=1))
    print(
        f"  {name:24s} n {len(x):4d}  slope {actual:+9.3f} bp per {unit}"
        f"  = {actual * sd:+7.3f} bp per 1 sd  percentile {pct:6.2f}"
    )
    return actual * sd


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--start", default="2010-06-01")
    ap.add_argument("--end", default="2026-05-31")
    ap.add_argument("--draws", type=int, default=100_000, help="declared in the registration; do not lower it")
    ap.add_argument("--seed", type=int, default=7)
    args = ap.parse_args()

    lo, hi = pd.Timestamp(args.start, tz="UTC"), pd.Timestamp(args.end, tz="UTC")
    df = build(lo, hi)
    usable = df.dropna(subset=["fix_bp", "equity"])

    print(f"month-end fix slope, eurduka 15m, {args.start} -> {args.end}")
    print(f"{len(df)} month-ends found, {len(usable)} with both a window and an equity month")
    thin = usable[usable["bars_fix"] < 2]
    print(f"window 15:45-16:15 London: mean {usable['bars_fix'].mean():.2f} bars, min {usable['bars_fix'].min()}")
    print(f"a round trip costs {COST_BP:.2f} bp; the registration calls 1.2-2.9 bp UNDERPOWERED, not 'no'\n")
    if len(thin):
        print(f"  {len(thin)} month-end(s) with fewer than two bars in the window, dropped\n")

    x = usable["equity"].to_numpy()
    print(f"the declared cell, and the three placebos ({args.draws:,} draws each):")
    headline = report("FIX (the claim)", x, usable["fix_bp"].to_numpy(), args.draws, args.seed, "1.0 log return")

    day = usable.dropna(subset=["placebo_day_bp"])
    report("placebo: day before", day["equity"].to_numpy(), day["placebo_day_bp"].to_numpy(), args.draws, args.seed, "1.0")
    win = usable.dropna(subset=["placebo_win_bp"])
    report("placebo: 13:45 window", win["equity"].to_numpy(), win["placebo_win_bp"].to_numpy(), args.draws, args.seed, "1.0")
    au = usable.dropna(subset=["gold"])
    report("placebo: gold instead", au["gold"].to_numpy(), au["fix_bp"].to_numpy(), args.draws, args.seed, "1.0")

    print()
    print(f"  the claim is worth {headline:+.3f} bp per 1 sd of the equity variable")
    if abs(headline) < COST_BP:
        print("  which is inside the round trip: the mechanism, if it exists, is not a trade")
    elif abs(headline) < 2.9:
        print("  which is inside the band this registration declared UNDERPOWERED in advance")
    return 0


if __name__ == "__main__":
    sys.exit(main())
