"""The candidate cell, measured once, with everything that qualifies it.

The condition, stated before the numbers below it and derived in this order
from `path_shape_null.py`, `thinness_reversion.py` and `halt_artifact.py`:

  entry hour  in 19:00-23:00 UTC, the hours after the COMEX pit close and
              before Asia, where the four-hour path is 10-18% LESS directional
              than a sign-flipped copy of itself at the 0th percentile of a
              200-seed flip null;
  |z_t|       the last bar's move divided by ATR96 through t-1, in a band: a
              moderate move, not a tiny one and not a huge one;
  thin_t      travel over t-4..t-1 against the average hour of t-96..t-1,
              below a ceiling: a quiet book;
  clean       the forward window does not cross the daily halt or the weekend,
              because `halt_artifact.py` shows the halt manufactures **half**
              of hour 20's apparent reversion;
  side        against the last bar. There is no fixed direction anywhere in
              this: the side is whatever the market just did, reversed.

Reported in ATR units first. The effect is a fraction of the move being
reverted, so it scales with volatility while the 0.28-point spread does not,
and gold's fifteen-minute true range ran 1.12 points in 2018 against 4.66 in
2025. A figure in points is a figure about one volatility regime.

`expR` is the honest trading arithmetic: (giveN - spread/atr) / stopAtr, the
expectancy in R of a fixed-time hold with a stop at `stopAtr` ATR, charging the
configured round trip once. It is not a backtest — the guards, the stop and the
daily caps live in the Rust engine — but it is the number that says whether a
backtest is worth running.

    python py/research/evening_reversion_cell.py --symbol=XAUDUKA-15m
    python py/research/evening_reversion_cell.py --symbol=XAGDUKA-15m
    python py/research/evening_reversion_cell.py --symbol=EURDUKA-15m --spread=0.00014
    python py/research/evening_reversion_cell.py --symbol=XAUUSD-15m
"""

from __future__ import annotations

import sys

import numpy as np
import pandas as pd
import pyarrow.parquet as pq

DATA = "E:/rust/flowdesk/data-sealed"
STEP_MS = 15 * 60 * 1000


def arg(name: str, fallback: str) -> str:
    for a in sys.argv[1:]:
        if a.startswith(f"--{name}="):
            return a.split("=", 1)[1]
    return fallback


def nw_t(x: np.ndarray, lag: int) -> float:
    x = x[np.isfinite(x)]
    n = len(x)
    if n < 30:
        return float("nan")
    d = x - x.mean()
    var = (d @ d) / n
    for k in range(1, lag + 1):
        if k >= n:
            break
        var += 2.0 * (1.0 - k / (lag + 1.0)) * ((d[k:] @ d[:-k]) / n)
    return float("nan") if var <= 0 else x.mean() / np.sqrt(var / n)


def build(symbol: str, m: int) -> pd.DataFrame:
    df = pq.read_table(f"{DATA}/bars/{symbol}.parquet").to_pandas().sort_values("time").reset_index(drop=True)
    print(f"data root: {DATA}")
    print(f"bars:      {symbol} {len(df)} from {df.time.iloc[0]} to {df.time.iloc[-1]}")
    c, hi, lo = df.close.to_numpy(), df.high.to_numpy(), df.low.to_numpy()
    t = df.time.astype("int64").to_numpy()
    n = len(c)
    prev = np.concatenate([[np.nan], c[:-1]])
    tr = np.maximum(hi - lo, np.maximum(np.abs(hi - prev), np.abs(lo - prev)))
    atr = pd.Series(tr).rolling(96).mean().shift(1).to_numpy()
    absr = np.abs(c - prev)
    thin = pd.Series(absr).rolling(4).sum().shift(1).to_numpy() / (
        pd.Series(absr).rolling(96).sum().shift(1).to_numpy() / 24.0
    )
    z = (c - prev) / atr
    fwd = np.full(n, np.nan)
    fwd[: n - m] = c[m:] - c[: n - m]
    s = np.sign(z)
    give = -s * fwd
    mfe = np.full(n, np.nan)
    mae = np.full(n, np.nan)
    for i in range(n - m):
        if not np.isfinite(s[i]) or s[i] == 0:
            continue
        w_hi, w_lo = hi[i + 1 : i + 1 + m].max(), lo[i + 1 : i + 1 + m].min()
        if s[i] > 0:
            mfe[i], mae[i] = c[i] - w_lo, w_hi - c[i]
        else:
            mfe[i], mae[i] = w_hi - c[i], c[i] - w_lo
    d = np.full(n, np.nan)
    d[1:] = t[1:] - t[:-1]
    step_ok = d == STEP_MS
    clean = step_ok.copy()
    for k in range(1, m + 1):
        sh = np.zeros(n, dtype=bool)
        sh[: n - k] = step_ok[k:]
        clean &= sh
    return pd.DataFrame(
        {
            "time": df.time,
            "hour": df.time.dt.hour,
            "year": df.time.dt.year,
            "atr": atr,
            "z": z,
            "thin": thin,
            "giveN": give / atr,
            "give": give,
            "mfeN": mfe / atr,
            "maeN": mae / atr,
            "clean": clean,
        }
    )


def row(sub: pd.DataFrame, m: int, spread: float, stop_atr: float, name: str) -> None:
    if len(sub) < 60:
        print(f"{name:>26} {len(sub):>7}   (too few)")
        return
    g = sub.giveN.to_numpy()
    atr = float(sub.atr.mean())
    exp_r = (g.mean() - spread / atr) / stop_atr
    print(
        f"{name:>26} {len(sub):>7} {g.mean():>9.5f} {nw_t(g, m):>7.2f} {atr:>8.3f} "
        f"{float(sub.give.mean()):>9.4f} {spread/atr:>9.5f} {exp_r:>8.4f} {float(sub.mfeN.mean()):>7.4f} {float(sub.maeN.mean()):>7.4f}"
    )


HEAD = f"{'cell':>26} {'n':>7} {'giveN':>9} {'tNW':>7} {'atrPts':>8} {'givePts':>9} {'sprd/atr':>9} {'expR':>8} {'mfeN':>7} {'maeN':>7}"


def main() -> None:
    m = int(arg("m", "4"))
    spread = float(arg("spread", "0.28"))
    stop_atr = float(arg("stopAtr", "1.0"))
    symbol = arg("symbol", "XAUDUKA-15m")
    f = build(symbol, m)
    print(f"forward:   {m} bars = {15*m} minutes.  spread {spread} per round trip.  stopAtr {stop_atr}")
    print("expR = (giveN - spread/atr) / stopAtr — a fixed-time hold's expectancy in R, cost charged once.")
    az = f.z.abs()

    print()
    print("== the hour block, holding the rest of the condition open ==")
    print(HEAD)
    base = f.clean & np.isfinite(f.giveN) & (az >= 0.5) & (az < 2.0)
    for lo, hi in ((0, 6), (7, 11), (12, 16), (17, 18), (19, 23)):
        row(f[base & f.hour.between(lo, hi)], m, spread, stop_atr, f"h{lo:02d}-{hi:02d} UTC")

    print()
    print("== inside 19-23 UTC: the move-size band ==")
    print(HEAD)
    ev = f.clean & np.isfinite(f.giveN) & f.hour.between(19, 23)
    for a, b in ((0.25, 0.5), (0.5, 0.75), (0.75, 1.0), (1.0, 1.5), (1.5, 2.0), (2.0, 3.0), (3.0, 1e9)):
        row(f[ev & (az >= a) & (az < b)], m, spread, stop_atr, f"|z| {a:g}-{b:g}" if b < 1e8 else f"|z| >{a:g}")

    print()
    print("== inside 19-23 UTC and |z| 0.5-2: the thinness ceiling ==")
    print(HEAD)
    ev2 = ev & (az >= 0.5) & (az < 2.0)
    for b in (0.5, 0.75, 1.0, 1.5, 1e9):
        row(f[ev2 & (f.thin < b)], m, spread, stop_atr, f"thin < {b:g}" if b < 1e8 else "thin any")

    print()
    print("== the cell, per calendar year: 19-23 UTC, |z| 0.5-2, thin < 1.0, clean ==")
    print(HEAD)
    cell = ev2 & (f.thin < 1.0)
    for y in sorted(f.year.unique()):
        row(f[cell & (f.year == y)], m, spread, stop_atr, str(y))
    print()
    row(f[cell], m, spread, stop_atr, "whole window")
    fired = int(cell.sum())
    yrs = (f.time.iloc[-1] - f.time.iloc[0]).days / 365.25
    print()
    print(f"firing rate: {fired} entries over {yrs:.2f} years = {fired/yrs:.1f} a year")
    print("             (before the engine's guards, which refuse some of them)")

    print()
    print("== holding period: the same cell at other m ==")
    print(HEAD)
    for mm in (1, 2, 4, 6, 8, 12):
        g = build(symbol, mm)
        azz = g.z.abs()
        c2 = g.clean & np.isfinite(g.giveN) & g.hour.between(19, 23) & (azz >= 0.5) & (azz < 2.0) & (g.thin < 1.0)
        row(g[c2], mm, spread, stop_atr, f"m = {mm} bars")


if __name__ == "__main__":
    main()
