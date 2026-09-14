"""The drift of a New York-clock window, detrended by its share of the day.

    python scripts/fx_window_drift.py eurduka 0300 1100 2010-06-01 2018-06-15

Prints, for the weekday sessions in [from, to) UTC dates: the window's mean
open-to-open change per hold in pips (gross, no spread), the mean of 8/24
(the window's share of the day) times the same days' full-day change, and
the difference — the effect size a sign-only registration names in advance
(`2026-09-14-fx-local-hours.md`, adversary: "window minus trend share").
Also the per-year split and the fraction of days with the window negative.

Read-only; reads the 15-minute bar file for the market's `bar_symbol`. The
day is the New York calendar day; the window is [from, to) in New York
minutes; a hold is one weekday's window with at least four bars. "Pips" are
1e-4 of price, so the number is in the pair's own units for anything but a
JPY cross. Nothing here is a receipt: the record quotes `search`'s files
for the nulls and this script's output for the effect size, both named in
the registration before the run.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

import numpy as np
import pandas as pd
import pyarrow.compute as pc
import pyarrow.parquet as pq

ROOT = Path(__file__).resolve().parents[1]


def hhmm(v: str) -> int:
    v = int(v)
    return (v // 100) * 60 + v % 100


def main() -> int:
    if len(sys.argv) != 6:
        sys.exit(__doc__)
    market, w_from, w_to, d_from, d_to = sys.argv[1:]
    # The market's bar_symbol, read from the config without a TOML parser
    # (the interpreter here is 3.9): the `[markets.<id>]` table's first
    # `bar_symbol = "..."` line.
    text = (ROOT / "config" / "default.toml").read_text(encoding="utf-8")
    table = re.search(r"\[markets\." + re.escape(market) + r"\]\s*(.*?)(?=\n\[)", text, re.S)
    if table is None:
        sys.exit(f"no [markets.{market}] in config/default.toml")
    m = re.search(r'bar_symbol\s*=\s*"([^"]+)"', table.group(1))
    if m is None:
        sys.exit(f"no bar_symbol for {market}")
    symbol = m.group(1)
    t = pq.read_table(ROOT / "data" / "bars" / f"{symbol}-15m.parquet")
    lo = pd.Timestamp(d_from, tz="UTC")
    hi = pd.Timestamp(d_to, tz="UTC")
    import pyarrow as pa

    lo_s, hi_s = pa.scalar(lo, type=t["time"].type), pa.scalar(hi, type=t["time"].type)
    t = t.filter(pc.and_(pc.greater_equal(t["time"], lo_s), pc.less(t["time"], hi_s)))
    df = t.to_pandas().sort_values("time").reset_index(drop=True)
    ny = df["time"].dt.tz_convert("America/New_York")
    df["m"] = ny.dt.hour * 60 + ny.dt.minute
    df["date"] = pd.to_datetime(ny.dt.date)
    df["year"] = ny.dt.year
    df["d"] = df["open"].shift(-1) - df["open"]  # open to next open: the engine's fills
    df = df.iloc[:-1]
    a, b = hhmm(w_from), hhmm(w_to)
    inside = (df["m"] >= a) & (df["m"] < b) if a < b else (df["m"] >= a) | (df["m"] < b)
    # For a window that wraps midnight, bars before `to` belong to the previous day's hold.
    entry = df["date"].where(~(inside & (df["m"] < b) & (a >= b)), df["date"] - pd.Timedelta(days=1))
    df["entry"] = entry
    wk = df["entry"].dt.weekday <= 4
    span_hours = ((b - a) if a < b else (1440 - a + b)) / 60.0
    win = df[inside & wk].groupby("entry")["d"].agg(["sum", "count"])
    win = win[win["count"] >= 4]
    day = df.groupby("date")["d"].sum()
    share = day.reindex(win.index).fillna(0.0) * (span_hours / 24.0)
    drift = win["sum"] * 1e4
    trend = share * 1e4
    excess = drift - trend
    print(f"{market} {w_from}-{w_to} New York, {d_from} -> {d_to}: {len(win)} weekday holds, window {span_hours:.1f} h")
    print(f"drift per hold {drift.mean():+.2f} pips (median {drift.median():+.2f}, negative on {100.0 * (drift < 0).mean():.1f}% of days, t = {drift.mean() / drift.std() * np.sqrt(len(drift)):+.2f})")
    print(f"trend share per hold {trend.mean():+.2f} pips; EXCESS (drift minus share) {excess.mean():+.2f} pips per hold, t = {excess.mean() / excess.std() * np.sqrt(len(excess)):+.2f}")
    print(f"sum: window {drift.sum():+.0f} pips, share {trend.sum():+.0f}, excess {excess.sum():+.0f}")
    print("per year: holds, drift/hold, share/hold, excess/hold (pips)")
    years = pd.Series(win.index.year, index=win.index)
    for y in sorted(years.unique()):
        k = years == y
        print(f"  {y} {int(k.sum()):4d} {drift[k].mean():+7.2f} {trend[k].mean():+7.2f} {excess[k].mean():+7.2f}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
