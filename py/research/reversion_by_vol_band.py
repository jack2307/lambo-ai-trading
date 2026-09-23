"""The measurement that closed this angle: the reversion inverts where the cost becomes affordable.

The candidate condition — revert a moderate fifteen-minute move made in
19:00-23:00 UTC in a quiet book, hold eight bars, fill at the next bar's open
the way the engine fills — measures a gross +0.153 ATR on both gold feeds. The
configured round trip is 0.28 points, which is 0.136 ATR at the fifteen-year
mean volatility and 0.065 ATR at 2025's. So the whole case for the method rests
on one claim: **the effect is a fraction of the move, so it scales with
volatility while the fixed spread does not, and it therefore becomes affordable
in exactly the high-volatility regime the withheld year is in.**

This file tests that claim by measuring the effect inside bands of trailing
volatility, on both vendors independently. It is the pre-statable form of the
question — does the effect scale with ATR? — and not a filter added to rescue a
number: whichever way it comes out, it is reported.

It comes out against. The effect is flat at +0.15 to +0.18 ATR in every band up
to an ATR of 4 points and **negative above it**, on both feeds, and above 4
points is where the withheld year lives. The cost advantage and the effect are
anti-correlated, so there is no band where the method is paid.

    python py/research/reversion_by_vol_band.py --m=8
"""

from __future__ import annotations

import sys

import numpy as np
import pandas as pd
import pyarrow.parquet as pq

DATA = "E:/rust/flowdesk/data-sealed"
STEP_MS = 15 * 60 * 1000
BANDS = [(0.0, 1.25), (1.25, 1.75), (1.75, 2.25), (2.25, 3.0), (3.0, 4.0), (4.0, 1e9)]


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


def cell(symbol: str, m: int) -> pd.DataFrame:
    df = pq.read_table(f"{DATA}/bars/{symbol}.parquet").to_pandas().sort_values("time").reset_index(drop=True)
    print(f"data root: {DATA}")
    print(f"bars:      {symbol} {len(df)} from {df.time.iloc[0]} to {df.time.iloc[-1]}")
    c, o, hi, lo = (df[k].to_numpy() for k in ("close", "open", "high", "low"))
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
    s = np.sign(z)
    lim = n - m - 1
    fill = np.full(n, np.nan)
    fill[:lim] = -s[:lim] * (o[1 + m : 1 + m + lim] - o[1 : 1 + lim])
    d = np.full(n, np.nan)
    d[1:] = t[1:] - t[:-1]
    ok = d == STEP_MS
    clean = ok.copy()
    for k in range(1, m + 2):
        sh = np.zeros(n, dtype=bool)
        sh[: n - k] = ok[k:]
        clean &= sh
    return pd.DataFrame(
        {"year": df.time.dt.year, "hour": df.time.dt.hour, "atr": atr, "az": np.abs(z), "thin": thin, "openN": fill / atr, "clean": clean}
    )


def main() -> None:
    m = int(arg("m", "8"))
    spread = float(arg("spread", "0.28"))
    print(f"forward:   {m} bars held, entry at open[t+1], exit at open[t+1+{m}] — the engine's own fill")
    print(f"spread:    {spread} points per round trip (configured)")
    print("netN = gross reversion in ATR units minus the round trip in ATR units.")
    for symbol in ("XAUDUKA-15m", "XAUUSD-15m"):
        f = cell(symbol, m)
        mask = f.clean & f.hour.between(19, 23) & (f.az >= 0.5) & (f.az < 2.0) & (f.thin < 1.0) & np.isfinite(f.openN)
        print()
        print(f"== {symbol}: the cell inside bands of trailing ATR96, n = {int(mask.sum())} ==")
        print(f"{'atr band pts':>14} {'n':>7} {'grossN':>9} {'tNW':>7} {'sprd/atr':>9} {'netN':>9} {'netPts':>8}")
        for a, b in BANDS:
            sub = f[mask & (f.atr >= a) & (f.atr < b)]
            name = f"{a:g}-{b:g}" if b < 1e8 else f">{a:g}"
            if len(sub) < 40:
                print(f"{name:>14} {len(sub):>7}   (too few)")
                continue
            g = sub.openN.to_numpy()
            sa = float((spread / sub.atr).mean())
            print(
                f"{name:>14} {len(sub):>7} {g.mean():>9.5f} {nw_t(g, m):>7.2f} {sa:>9.5f} "
                f"{g.mean()-sa:>9.5f} {(g.mean()-sa)*float(sub.atr.mean()):>8.4f}"
            )
        print()
        print(f"   where the withheld year sits: mean ATR96 by year, {symbol}")
        for y in sorted(f.year.unique()):
            sub = f[(f.year == y) & np.isfinite(f.atr)]
            print(f"      {y}  mean ATR96 {float(sub.atr.mean()):.3f} points   spread/atr {spread/float(sub.atr.mean()):.4f}")


if __name__ == "__main__":
    main()
