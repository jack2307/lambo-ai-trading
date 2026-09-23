"""Is the evening reversion a clock, or a measurable state of the book?

The 2026-09-14 synthesis closed a family of findings that were real on a first
window and gone on a second, and every one of them was a **wall clock**: gold
long across the 16:30 New York close, the euro short through European hours.
A clock is a label for whatever was happening at that hour when the window was
measured; when the thing behind the label moves, the clock keeps pointing at the
old place.

This file asks whether the reversion of a fifteen-minute move — measured in
`displacement_giveback.py` to be about three times larger in 17:00-20:00 UTC
than in 12:00-16:00 UTC — is a property of the hour or a property of **how thin
the book is at that moment**, which the hour only proxies. If it is the state,
a rule can read the state and will follow it when the hour moves; if it is the
hour, the rule is the thing the synthesis already closed.

Two state variables, both from bars strictly before t so that neither can see
the bar it is about to judge:

  thin_t  = travel over bars t-4..t-1  /  (travel over bars t-96..t-1 / 24)
            the last hour's travel against the average hour of the last day.
            Below 1 the book is quieter than its own recent normal.
  z_t     = (close_t - close_{t-1}) / atr96_{t-1}
            the size of the move to be reverted, scale-free.

Everything is reported **normalised by ATR** as well as in points, because the
configured spread is fixed at 0.28 points while gold's fifteen-minute true
range ran 1.12 points in 2018 and 4.66 in 2025. An effect averaged in points
over fifteen years is an average over cost regimes that differ fourfold, and
reading it as the cost regime of the year a method will trade is a mistake this
file refuses to make for the reader.

    python py/research/thinness_reversion.py --symbol=XAUDUKA-15m --m=4
    python py/research/thinness_reversion.py --symbol=XAUUSD-15m  --m=4
"""

from __future__ import annotations

import sys

import numpy as np
import pandas as pd
import pyarrow.parquet as pq

DATA = "E:/rust/flowdesk/data-sealed"
SPREAD = 0.28
ERAS = [("2010-06", "2015-06"), ("2015-06", "2020-06"), ("2020-06", "2025-09-23")]


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
    n = len(c)
    prev = np.concatenate([[np.nan], c[:-1]])
    tr = np.maximum(hi - lo, np.maximum(np.abs(hi - prev), np.abs(lo - prev)))
    absr = np.abs(c - prev)
    # trailing sums that END AT t-1: shift the rolling window by one bar
    roll4 = pd.Series(absr).rolling(4).sum().shift(1).to_numpy()
    roll96 = pd.Series(absr).rolling(96).sum().shift(1).to_numpy()
    atr_prev = pd.Series(tr).rolling(96).mean().shift(1).to_numpy()
    thin = roll4 / (roll96 / 24.0)
    z = (c - prev) / atr_prev
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
    return pd.DataFrame(
        {
            "time": df.time,
            "hour": df.time.dt.hour,
            "atr": atr_prev,
            "z": z,
            "thin": thin,
            "give": give,
            "giveN": give / atr_prev,
            "mfeN": mfe / atr_prev,
            "maeN": mae / atr_prev,
        }
    )


THIN_EDGES = [(0.0, 0.5), (0.5, 0.75), (0.75, 1.0), (1.0, 1.5), (1.5, 2.5), (2.5, 1e9)]
Z_EDGES = [(0.5, 1.0), (1.0, 2.0), (2.0, 1e9)]


def label(a: float, b: float) -> str:
    return f"{a:g}-{b:g}" if b < 1e8 else f">{a:g}"


def by_thin(f: pd.DataFrame, m: int) -> None:
    az = f.z.abs()
    print()
    print("== reversion of the last bar over the next m bars, by book thinness and move size ==")
    print("giveN is in ATR units; give is points at this window's own volatility.")
    for za, zb in Z_EDGES:
        print()
        print(f"-- |z| {label(za, zb)}")
        print(f"{'thin':>10} {'n':>8} {'giveN':>8} {'tNW':>7} {'give pts':>9} {'atr':>7} {'giveN*atr25':>11} {'mfeN':>7} {'maeN':>7}")
        for a, b in THIN_EDGES:
            sub = f[(az >= za) & (az < zb) & (f.thin >= a) & (f.thin < b) & np.isfinite(f.giveN)]
            if len(sub) < 200:
                print(f"{label(a,b):>10} {len(sub):>8}   (too few)")
                continue
            gn = sub.giveN.to_numpy()
            print(
                f"{label(a,b):>10} {len(sub):>8} {gn.mean():>8.5f} {nw_t(gn, m):>7.2f} {float(sub.give.mean()):>9.4f} "
                f"{float(sub.atr.mean()):>7.3f} {gn.mean()*4.662:>11.4f} {float(sub.mfeN.mean()):>7.4f} {float(sub.maeN.mean()):>7.4f}"
            )


def thin_vs_clock(f: pd.DataFrame, m: int) -> None:
    """Does the hour still matter once thinness is held fixed, and vice versa?"""
    az = f.z.abs()
    core = (az >= 0.5) & (az < 2.0) & np.isfinite(f.giveN)
    evening = (f.hour >= 17) & (f.hour <= 20)
    print()
    print("== the clock against the state: |z| in 0.5-2, giveN in ATR units ==")
    print("If the state carries it, the thin rows agree across the two clock blocks.")
    print(f"{'thin':>10} {'block':>14} {'n':>8} {'giveN':>9} {'tNW':>7}")
    for a, b in THIN_EDGES:
        for name, mask in (("17-20 UTC", evening), ("all other h", ~evening)):
            sub = f[core & (f.thin >= a) & (f.thin < b) & mask]
            if len(sub) < 200:
                continue
            gn = sub.giveN.to_numpy()
            print(f"{label(a,b):>10} {name:>14} {len(sub):>8} {gn.mean():>9.5f} {nw_t(gn, m):>7.2f}")
    print()
    print("== and by hour, holding thinness in its quietest band (thin < 0.75) ==")
    print(f"{'hourUTC':>8} {'n':>8} {'giveN':>9} {'tNW':>7}")
    for hr in range(24):
        sub = f[core & (f.thin < 0.75) & (f.hour == hr)]
        if len(sub) < 200:
            continue
        gn = sub.giveN.to_numpy()
        print(f"{hr:>8} {len(sub):>8} {gn.mean():>9.5f} {nw_t(gn, m):>7.2f}")


def eras(f: pd.DataFrame, m: int) -> None:
    az = f.z.abs()
    print()
    print("== era stability, in ATR units: |z| 0.5-2 and thin < 0.75 ==")
    print("This is the cell a method would act on. A cell that lives in one era is the closed shape.")
    print(f"{'era':>20} {'n':>8} {'giveN':>9} {'tNW':>7} {'give pts':>9} {'atr':>7}")
    cell = (az >= 0.5) & (az < 2.0) & (f.thin < 0.75) & np.isfinite(f.giveN)
    for a, b in ERAS:
        sub = f[cell & (f.time >= a) & (f.time < b)]
        if len(sub) < 200:
            print(f"{a+'..'+b:>20} {len(sub):>8}   (too few)")
            continue
        gn = sub.giveN.to_numpy()
        print(
            f"{a+'..'+b:>20} {len(sub):>8} {gn.mean():>9.5f} {nw_t(gn, m):>7.2f} "
            f"{float(sub.give.mean()):>9.4f} {float(sub.atr.mean()):>7.3f}"
        )
    print()
    print("== and per calendar year, same cell ==")
    print(f"{'year':>6} {'n':>7} {'giveN':>9} {'tNW':>7} {'give pts':>9} {'atr':>7}")
    for y in range(2010, 2026):
        sub = f[cell & (f.time.dt.year == y)]
        if len(sub) < 100:
            continue
        gn = sub.giveN.to_numpy()
        print(
            f"{y:>6} {len(sub):>7} {gn.mean():>9.5f} {nw_t(gn, m):>7.2f} "
            f"{float(sub.give.mean()):>9.4f} {float(sub.atr.mean()):>7.3f}"
        )


def main() -> None:
    m = int(arg("m", "4"))
    f = build(arg("symbol", "XAUDUKA-15m"), m)
    print(f"forward:   {m} bars = {15*m} minutes.  spread {SPREAD} points round trip")
    print("giveN*atr25 rescales the effect to 2025's own volatility (mean 15m true range 4.662 points).")
    by_thin(f, m)
    thin_vs_clock(f, m)
    eras(f, m)


if __name__ == "__main__":
    main()
