"""Is the 20:00-22:00 UTC reversion a market, or the daily halt?

`thinness_reversion.py` finds the reversion of a fifteen-minute move
concentrated at 19:00-22:00 UTC and finds that book thinness does **not**
explain it: holding thinness fixed, the evening block reverts four to eight
times as much as every other hour. That leaves two readings and they have
opposite consequences.

1. A real property of that part of the day.
2. The **daily halt**. A CFD on gold stops for an hour every weekday evening —
   21:00-22:00 UTC in northern winter, 20:00-21:00 in summer. The last bar
   before a halt is quoted into a book that is closing, and the first bar after
   it is a fresh mark. A four-bar forward window opened at 20:45 does not
   measure four bars of trading; it measures one bar, a halt, and a re-mark, and
   "the move reverted" is the re-mark. Nothing can trade that: the position is
   held through a period with no quotes and reopened at the widest spread of the
   day, which `2026-09-15-intraday-frontier` measured at 0.26-0.27 points
   against 0.19-0.20 in the hours either side.

The test is mechanical. A bar is **clean** when the m bars after it are all
consecutive fifteen-minute bars — no gap, no halt, no weekend — and the bar
itself follows its predecessor by exactly fifteen minutes. Reading 1 survives
that filter; reading 2 does not.

    python py/research/halt_artifact.py --symbol=XAUDUKA-15m --m=4
"""

from __future__ import annotations

import sys

import numpy as np
import pandas as pd
import pyarrow.parquet as pq

DATA = "E:/rust/flowdesk/data-sealed"
SPREAD = 0.28
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


def main() -> None:
    symbol, m = arg("symbol", "XAUDUKA-15m"), int(arg("m", "4"))
    df = pq.read_table(f"{DATA}/bars/{symbol}.parquet").to_pandas().sort_values("time").reset_index(drop=True)
    print(f"data root: {DATA}")
    print(f"bars:      {symbol} {len(df)} from {df.time.iloc[0]} to {df.time.iloc[-1]}")
    print(f"forward:   {m} bars.  spread {SPREAD} points round trip")
    c, hi, lo = df.close.to_numpy(), df.high.to_numpy(), df.low.to_numpy()
    t = df.time.astype("int64").to_numpy()  # dtype is datetime64[ms]: already milliseconds
    n = len(c)
    prev = np.concatenate([[np.nan], c[:-1]])
    tr = np.maximum(hi - lo, np.maximum(np.abs(hi - prev), np.abs(lo - prev)))
    atr_prev = pd.Series(tr).rolling(96).mean().shift(1).to_numpy()
    z = (c - prev) / atr_prev
    absr = np.abs(c - prev)
    roll4 = pd.Series(absr).rolling(4).sum().shift(1).to_numpy()
    roll96 = pd.Series(absr).rolling(96).sum().shift(1).to_numpy()
    thin = roll4 / (roll96 / 24.0)
    fwd = np.full(n, np.nan)
    fwd[: n - m] = c[m:] - c[: n - m]
    give = -np.sign(z) * fwd

    # clean = the entry bar follows its predecessor by one step AND the next m
    # bars are consecutive steps.
    d = np.full(n, np.nan)
    d[1:] = t[1:] - t[:-1]
    step_ok = d == STEP_MS
    clean = step_ok.copy()
    for k in range(1, m + 1):
        shifted = np.zeros(n, dtype=bool)
        shifted[: n - k] = step_ok[k:]
        clean &= shifted

    f = pd.DataFrame(
        {
            "hour": df.time.dt.hour,
            "year": df.time.dt.year,
            "z": z,
            "thin": thin,
            "giveN": give / atr_prev,
            "give": give,
            "atr": atr_prev,
            "clean": clean,
        }
    )
    az = f.z.abs()
    core = (az >= 0.5) & (az < 2.0) & np.isfinite(f.giveN)

    print()
    print("== by hour: all bars against only those whose whole forward window is consecutive ==")
    print("|z| in 0.5-2. giveN in ATR units. 'dropped' is the share of that hour's bars the filter removes.")
    print(f"{'hourUTC':>8} {'nAll':>7} {'giveNall':>9} {'tAll':>7} {'nClean':>7} {'giveNcln':>9} {'tCln':>7} {'dropped':>8}")
    for hr in range(24):
        a = f[core & (f.hour == hr)]
        b = a[a.clean]
        if len(a) < 200:
            continue
        ga, gb = a.giveN.to_numpy(), b.giveN.to_numpy()
        print(
            f"{hr:>8} {len(a):>7} {ga.mean():>9.5f} {nw_t(ga, m):>7.2f} {len(b):>7} "
            f"{(gb.mean() if len(gb) > 30 else float('nan')):>9.5f} {nw_t(gb, m):>7.2f} {1 - len(b)/len(a):>7.1%}"
        )

    print()
    print("== the evening block 19-22 UTC, clean bars only, with thinness bands ==")
    ev = core & f.hour.between(19, 22) & f.clean
    print(f"{'thin':>10} {'n':>7} {'giveN':>9} {'tNW':>7} {'give pts':>9}")
    for a, b in ((0.0, 0.75), (0.75, 1.0), (1.0, 1.5), (1.5, 1e9)):
        sub = f[ev & (f.thin >= a) & (f.thin < b)]
        if len(sub) < 100:
            continue
        g = sub.giveN.to_numpy()
        print(f"{f'{a:g}-{b:g}' if b < 1e8 else f'>{a:g}':>10} {len(sub):>7} {g.mean():>9.5f} {nw_t(g, m):>7.2f} {float(sub.give.mean()):>9.4f}")

    print()
    print("== per year, clean bars, 19-22 UTC, |z| 0.5-2, any thinness ==")
    print(f"{'year':>6} {'n':>6} {'giveN':>9} {'tNW':>7} {'give pts':>9} {'atr':>7}")
    for y in range(2010, 2026):
        sub = f[ev & (f.year == y)]
        if len(sub) < 60:
            continue
        g = sub.giveN.to_numpy()
        print(f"{y:>6} {len(sub):>6} {g.mean():>9.5f} {nw_t(g, m):>7.2f} {float(sub.give.mean()):>9.4f} {float(sub.atr.mean()):>7.3f}")


if __name__ == "__main__":
    main()
