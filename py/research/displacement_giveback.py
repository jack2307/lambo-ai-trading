"""How much of a displacement is given back, and in which hours.

`path_shape_null.py` establishes the conditional difference this file measures
in trade-sized units: over four hours, gold's fifteen-minute path is **less**
directional than a sign-flipped copy of itself everywhere, and the size of that
deficit varies by a factor of thirty across the hour of day — 0.6% of the flip
null's efficiency at 11:00-13:00 UTC against 17.8% at 19:00 UTC. A path less
efficient than a coin flip means net moves are partly retraced inside the
window. This file asks how big the retrace is in points, conditional on how big
the displacement was, and whether 0.28 points of spread fits inside it.

State at bar t, from bars <= t only:
  disp_k = (close_t - close_{t-k}) / atr96_t     signed displacement, scale-free
Forward, over the next m bars:
  give   = -sign(disp_k) * (close_{t+m} - close_t)      points given back
  mfe    = the best the give-back reached (a target)
  mae    = the worst it went against (a stop)

Every figure is in points (US dollars an ounce). The configured round trip is
0.28 points, so a column of 0.28 is a column of nothing.

    python py/research/displacement_giveback.py --symbol=XAUDUKA-15m --k=8 --m=8
    python py/research/displacement_giveback.py --symbol=XAUDUKA-15m --k=8 --m=8 --eras
"""

from __future__ import annotations

import sys

import numpy as np
import pandas as pd
import pyarrow.parquet as pq

DATA = "E:/rust/flowdesk/data-sealed"
SPREAD = 0.28
HOUR_BLOCKS = {
    "asia 00-06": lambda h: h <= 6,
    "london 07-11": lambda h: (h >= 7) & (h <= 11),
    "ny am 12-16": lambda h: (h >= 12) & (h <= 16),
    "ny pm 17-20": lambda h: (h >= 17) & (h <= 20),
    "late 21-23": lambda h: h >= 21,
}
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


def build(symbol: str, k: int, m: int) -> pd.DataFrame:
    df = pq.read_table(f"{DATA}/bars/{symbol}.parquet").to_pandas().sort_values("time").reset_index(drop=True)
    print(f"data root: {DATA}")
    print(f"bars:      {symbol} {len(df)} from {df.time.iloc[0]} to {df.time.iloc[-1]}")
    c, hi, lo = df.close.to_numpy(), df.high.to_numpy(), df.low.to_numpy()
    n = len(c)
    prev = np.concatenate([[np.nan], c[:-1]])
    tr = np.maximum(hi - lo, np.maximum(np.abs(hi - prev), np.abs(lo - prev)))
    atr = pd.Series(tr).rolling(96).mean().to_numpy()
    disp = np.full(n, np.nan)
    disp[k:] = c[k:] - c[:-k]
    z = disp / atr
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
        if s[i] > 0:  # displaced up, the give-back is down
            mfe[i], mae[i] = c[i] - w_lo, w_hi - c[i]
        else:
            mfe[i], mae[i] = w_hi - c[i], c[i] - w_lo
    return pd.DataFrame(
        {"time": df.time, "hour": df.time.dt.hour, "atr": atr, "z": z, "give": give, "mfe": mfe, "mae": mae}
    )


ZED = [(0.5, 1.0), (1.0, 1.5), (1.5, 2.0), (2.0, 3.0), (3.0, 1e9)]


def table(f: pd.DataFrame, m: int, label: str) -> None:
    print()
    print(f"-- {label}")
    print(f"{'|disp|/atr':>11} {'n':>8} {'give':>8} {'tNW':>7} {'give/sprd':>10} {'mfe':>7} {'mae':>7} {'atr':>7}")
    az = f.z.abs()
    for a, b in ZED:
        sub = f[(az >= a) & (az < b) & np.isfinite(f.give)]
        if len(sub) < 100:
            print(f"{(f'{a:g}-{b:g}' if b < 1e8 else f'>{a:g}'):>11} {len(sub):>8}   (too few)")
            continue
        g = sub.give.to_numpy()
        print(
            f"{(f'{a:g}-{b:g}' if b < 1e8 else f'>{a:g}'):>11} {len(sub):>8} {g.mean():>8.4f} {nw_t(g, m):>7.2f} "
            f"{g.mean()/SPREAD:>10.2f} {float(sub.mfe.mean()):>7.3f} {float(sub.mae.mean()):>7.3f} {float(sub.atr.mean()):>7.3f}"
        )


def main() -> None:
    symbol, k, m = arg("symbol", "XAUDUKA-15m"), int(arg("k", "8")), int(arg("m", "8"))
    f = build(symbol, k, m)
    print(f"state:     displacement over the last {k} bars / ATR96, both from bars <= t")
    print(f"forward:   {m} bars = {15*m} minutes.  give = points of the displacement handed back")
    print(f"spread:    {SPREAD} points round trip; give/sprd below ~3 is not worth a method")
    for name, pick in HOUR_BLOCKS.items():
        table(f[pick(f.hour)], m, f"{name} UTC")
    table(f, m, "all hours")
    if "--eras" in sys.argv:
        print()
        print("== era stability of the two extreme blocks ==")
        for name in ("ny pm 17-20", "ny am 12-16"):
            pick = HOUR_BLOCKS[name]
            for a, b in ERAS:
                sub = f[pick(f.hour) & (f.time >= a) & (f.time < b)]
                table(sub, m, f"{name} UTC  {a}..{b}")


if __name__ == "__main__":
    main()
