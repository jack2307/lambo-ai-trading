"""Is the path-shape deficit a property of the volatility regime?

`path_shape_null.py` establishes the one large, precisely measured conditional
difference this angle found: over four hours, gold's fifteen-minute path is
**less** directional than a sign-flipped copy of itself — 0.2607 against a flip
median of 0.2731 over the whole window, at the 0th percentile of 200 seeds — and
the size of the deficit varies thirtyfold across the hour of day.

`evening_reversion_cell.py` and the volatility-band table then kill the only
first-moment consequence of it: the evening reversion is +0.15 ATR at every
volatility level up to an ATR of 4 points and **negative above it**, on two
independent vendors, and an ATR above 4 points is the regime the withheld year
is in.

Those two facts together make one question worth asking before this angle is
declared exhausted: **does the path-shape deficit itself vanish, or invert, when
volatility is high?** If it does, the sign flip above is not an anomaly of one
year but the same structure seen from the other side, and the condition is a
measured state — the volatility regime — rather than a wall clock.

The trap is obvious and is stated before the run: the high-ATR bars of this
design window are concentrated in 2011, 2020 and 2024-25, and 2024-25 is the
part adjacent to the hold-out. A regime claim has to appear in **2011 and 2020
as well**, on their own, or it is a claim about the most recent months wearing a
regime's clothes. Both are printed separately and neither is dropped.

The state is the trailing ATR96 through t-1, in absolute points, so it is
causal; the bands are read off the per-year volatility table in the design note
and not tuned.

    python py/research/path_shape_by_regime.py --symbol=XAUDUKA-15m --h=16 --seeds=200
"""

from __future__ import annotations

import sys

import numpy as np
import pandas as pd
import pyarrow.parquet as pq

DATA = "E:/rust/flowdesk/data-sealed"
BANDS = [(0.0, 1.25), (1.25, 1.75), (1.75, 2.25), (2.25, 3.0), (3.0, 4.0), (4.0, 1e9)]


def arg(name: str, fallback: str) -> str:
    for a in sys.argv[1:]:
        if a.startswith(f"--{name}="):
            return a.split("=", 1)[1]
    return fallback


def main() -> None:
    symbol, h, seeds = arg("symbol", "XAUDUKA-15m"), int(arg("h", "16")), int(arg("seeds", "200"))
    df = pq.read_table(f"{DATA}/bars/{symbol}.parquet").to_pandas().sort_values("time").reset_index(drop=True)
    print(f"data root: {DATA}")
    print(f"bars:      {symbol} {len(df)} from {df.time.iloc[0]} to {df.time.iloc[-1]}")
    print(f"h:         {h} bars = {15*h} minutes.  seeds: {seeds}")
    c, hi, lo = (df[k].to_numpy() for k in ("close", "high", "low"))
    n = len(c)
    r = np.zeros(n)
    r[1:] = c[1:] - c[:-1]
    mag = np.abs(r)
    prev = np.concatenate([[np.nan], c[:-1]])
    tr = np.maximum(hi - lo, np.maximum(np.abs(hi - prev), np.abs(lo - prev)))
    atr = pd.Series(tr).rolling(96).mean().shift(1).to_numpy()
    year = df.time.dt.year.to_numpy()

    cs_mag = np.concatenate([[0.0], mag.cumsum()])
    trav = np.full(n, np.nan)
    trav[: n - h] = cs_mag[1 + h :] - cs_mag[1 : n - h + 1]

    def nets(signed: np.ndarray) -> np.ndarray:
        cs = np.concatenate([[0.0], signed.cumsum()])
        out = np.full(n, np.nan)
        out[: n - h] = np.abs(cs[1 + h :] - cs[1 : n - h + 1])
        return out

    ok = np.isfinite(trav) & (trav > 0) & np.isfinite(atr)
    real = nets(r)

    def report(mask: np.ndarray, name: str) -> None:
        m = ok & mask
        if m.sum() < 300:
            print(f"{name:>22} {int(m.sum()):>8}   (too few)")
            return
        er = real[m].sum() / trav[m].sum()
        rng = np.random.default_rng(20260923)
        col = np.empty(seeds)
        for s in range(seeds):
            signs = rng.integers(0, 2, size=n) * 2 - 1
            col[s] = nets(mag * signs)[m].sum() / trav[m].sum()
        col.sort()
        pct = 100.0 * (col < er).sum() / seeds
        print(
            f"{name:>22} {int(m.sum()):>8} {er:>9.4f} {np.median(col):>9.4f} {col[int(0.95*(seeds-1))]:>9.4f} "
            f"{pct:>5.0f}% {er/np.median(col)-1:>+9.2%}"
        )

    head = f"{'slice':>22} {'n':>8} {'effRatio':>9} {'flipP50':>9} {'flipP95':>9} {'pct':>6} {'vs flip':>9}"
    print()
    print("== path efficiency by trailing volatility band, against 200 sign-flipped paths ==")
    print("The flip keeps every |r| where it is, so travel and the volatility profile are identical.")
    print(head)
    for a, b in BANDS:
        report((atr >= a) & (atr < b), f"atr {a:g}-{b:g} pts" if b < 1e8 else f"atr >{a:g} pts")

    print()
    print("== the high-volatility band, one episode at a time: is it a regime or is it 2025? ==")
    print(head)
    high = atr >= 3.0
    for lo_y, hi_y, tag in ((2010, 2012, "2010-2012"), (2013, 2019, "2013-2019"), (2020, 2021, "2020-2021"), (2022, 2023, "2022-2023"), (2024, 2024, "2024"), (2025, 2025, "2025")):
        report(high & (year >= lo_y) & (year <= hi_y), f"atr>=3  {tag}")

    print()
    print("== and the whole window per year, any volatility, for the base rate ==")
    print(head)
    for y in range(2010, 2026):
        report(year == y, str(y))


if __name__ == "__main__":
    main()
