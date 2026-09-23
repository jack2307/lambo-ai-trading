"""The same cell, priced the way the engine will price it.

`evening_reversion_cell.py` measures the reversion from `close_t`. The engine
does not fill there. `config/default.toml` [backtest]: "a signal fills at the
NEXT bar's open", and `crates/fd-backtest/src/engine.rs` step 1 confirms it —
the intent produced on bar t is filled at `open_{t+1}`, with the spread applied
once at entry and once at exit.

That matters more here than for most methods, because the effect being measured
is the reversal of a move that just happened, and part of a one-bar reversal is
realised **between** `close_t` and `open_{t+1}`. A method that is paid in that
gap is paid nothing: the engine hands the gap to the market.

So: the same condition, entry at `open_{t+1}`, exit at `open_{t+1+m}`, spread
charged once as a round trip, and the difference between the two conventions
printed side by side. If the effect lives in the gap, the `openFill` column
says so and the method is dead before a line of Rust is written.

    python py/research/fill_convention_check.py --symbol=XAUUSD-15m --m=8
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


def main() -> None:
    symbol = arg("symbol", "XAUUSD-15m")
    m = int(arg("m", "8"))
    spread = float(arg("spread", "0.28"))
    df = pq.read_table(f"{DATA}/bars/{symbol}.parquet").to_pandas().sort_values("time").reset_index(drop=True)
    print(f"data root: {DATA}")
    print(f"bars:      {symbol} {len(df)} from {df.time.iloc[0]} to {df.time.iloc[-1]}")
    print(f"forward:   {m} bars held.  spread {spread} points per round trip")
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

    close_fill = np.full(n, np.nan)  # entry close_t, exit close_{t+m}
    open_fill = np.full(n, np.nan)  # entry open_{t+1}, exit open_{t+1+m}
    gap = np.full(n, np.nan)  # what close_t -> open_{t+1} alone gave
    lim = n - m - 1
    close_fill[:lim] = -s[:lim] * (c[m : m + lim] - c[:lim])
    open_fill[:lim] = -s[:lim] * (o[1 + m : 1 + m + lim] - o[1 : 1 + lim])
    gap[:lim] = -s[:lim] * (o[1 : 1 + lim] - c[:lim])

    d = np.full(n, np.nan)
    d[1:] = t[1:] - t[:-1]
    step_ok = d == STEP_MS
    clean = step_ok.copy()
    for k in range(1, m + 2):
        sh = np.zeros(n, dtype=bool)
        sh[: n - k] = step_ok[k:]
        clean &= sh

    f = pd.DataFrame(
        {
            "time": df.time,
            "hour": df.time.dt.hour,
            "year": df.time.dt.year,
            "atr": atr,
            "az": np.abs(z),
            "thin": thin,
            "closeN": close_fill / atr,
            "openN": open_fill / atr,
            "gapN": gap / atr,
            "clean": clean,
        }
    )
    cell = f.clean & f.hour.between(19, 23) & (f.az >= 0.5) & (f.az < 2.0) & (f.thin < 1.0) & np.isfinite(f.openN)

    def show(sub: pd.DataFrame, name: str) -> None:
        if len(sub) < 40:
            print(f"{name:>16} {len(sub):>7}   (too few)")
            return
        cn, on, gn = sub.closeN.to_numpy(), sub.openN.to_numpy(), sub.gapN.to_numpy()
        a = float(sub.atr.mean())
        print(
            f"{name:>16} {len(sub):>7} {cn.mean():>9.5f} {nw_t(cn, m):>6.2f} {on.mean():>9.5f} {nw_t(on, m):>6.2f} "
            f"{gn.mean():>8.5f} {spread/a:>9.5f} {on.mean()-spread/a:>9.5f} {a:>7.3f}"
        )

    print()
    print("== the cell: 19-23 UTC, |z| 0.5-2, thin < 1, forward window free of halt and weekend ==")
    print("All columns in ATR units. netN = openFill minus the round trip: what the engine can be paid.")
    print(f"{'slice':>16} {'n':>7} {'closeFil':>9} {'t':>6} {'openFill':>9} {'t':>6} {'gapOnly':>8} {'sprd/atr':>9} {'netN':>9} {'atrPts':>7}")
    show(f[cell], "whole window")
    for y in sorted(f.year.unique()):
        show(f[cell & (f.year == y)], str(y))

    print()
    print("== and on every other hour block, for contrast (openFill) ==")
    print(f"{'slice':>16} {'n':>7} {'closeFil':>9} {'t':>6} {'openFill':>9} {'t':>6} {'gapOnly':>8} {'sprd/atr':>9} {'netN':>9} {'atrPts':>7}")
    base = f.clean & (f.az >= 0.5) & (f.az < 2.0) & (f.thin < 1.0) & np.isfinite(f.openN)
    for a, b in ((0, 6), (7, 11), (12, 16), (17, 18), (19, 23)):
        show(f[base & f.hour.between(a, b)], f"h{a:02d}-{b:02d}")


if __name__ == "__main__":
    main()
