"""Is the shape of the forward path anything but the volatility profile?

`conditional_structure.py effratio` measures |net move over h bars| / total
travel and compares it to the 1/sqrt(h) line a Gaussian random walk sits on.
That line is the wrong null and the record should say so before any number is
read from it: by Cauchy-Schwarz, sqrt(sum sigma_j^2) / sum sigma_j >= 1/sqrt(h)
with equality only when every bar in the window has the same volatility. Gold's
volatility rises through the London-New York handover and falls into the Asian
lull, so a four-hour window that straddles either one has dispersed sigma and
an efficiency ratio above 1/sqrt(h) **with no serial dependence whatsoever.**

The null that does not have that defect is the **sign flip**: give every
fifteen-minute return a fresh random sign and keep its magnitude where it is.
That preserves the volatility of every bar, the clustering, the fat tails, the
hour-of-day profile and the total travel **exactly** — travel is a function of
|r| alone — and destroys serial dependence in direction and nothing else. The
only column that moves is the net move.

So: efficiency ratio of the real path, against 200 sign-flipped paths, per
hour-of-day bucket and per state. A percentile inside the flip distribution
means the hour's apparent persistence was its volatility profile.

    python py/research/path_shape_null.py --symbol=XAUDUKA-15m --h=16 --seeds=200
"""

from __future__ import annotations

import sys

import numpy as np
import pandas as pd
import pyarrow.parquet as pq

DATA = "E:/rust/flowdesk/data-sealed"
SPREAD = 0.28


def arg(name: str, fallback: str) -> str:
    for a in sys.argv[1:]:
        if a.startswith(f"--{name}="):
            return a.split("=", 1)[1]
    return fallback


def main() -> None:
    symbol = arg("symbol", "XAUDUKA-15m")
    h = int(arg("h", "16"))
    seeds = int(arg("seeds", "200"))
    lo, hi = arg("from", ""), arg("to", "")
    df = pq.read_table(f"{DATA}/bars/{symbol}.parquet").to_pandas().sort_values("time").reset_index(drop=True)
    print(f"data root: {DATA}")
    print(f"bars:      {symbol} {len(df)} from {df.time.iloc[0]} to {df.time.iloc[-1]}")
    if lo:
        df = df[df.time >= lo]
    if hi:
        df = df[df.time < hi]
    df = df.reset_index(drop=True)
    if lo or hi:
        print(f"window:    {len(df)} bars, {df.time.iloc[0]} to {df.time.iloc[-1]}")
    print(f"h:         {h} bars = {15*h} minutes.  seeds: {seeds}.  spread {SPREAD} points round trip")

    c = df.close.to_numpy()
    n = len(c)
    r = np.zeros(n)
    r[1:] = c[1:] - c[:-1]
    mag = np.abs(r)
    hour = df.time.dt.hour.to_numpy()

    # travel over bars t+1..t+h — identical under every sign flip
    cs_mag = np.concatenate([[0.0], mag.cumsum()])
    trav = np.full(n, np.nan)
    trav[: n - h] = cs_mag[1 + h :] - cs_mag[1 : n - h + 1]

    def nets(signed: np.ndarray) -> np.ndarray:
        cs = np.concatenate([[0.0], signed.cumsum()])
        out = np.full(n, np.nan)
        out[: n - h] = np.abs(cs[1 + h :] - cs[1 : n - h + 1])
        return out

    ok = np.isfinite(trav) & (trav > 0)
    real_net = nets(r)

    rng = np.random.default_rng(20260923)
    buckets = [("h%02d" % k, hour == k) for k in range(24)]
    flips = np.empty((seeds, len(buckets)))
    for s in range(seeds):
        signs = rng.integers(0, 2, size=n) * 2 - 1
        net = nets(mag * signs)
        for j, (_, mask) in enumerate(buckets):
            m = ok & mask
            flips[s, j] = net[m].sum() / trav[m].sum()

    print()
    print("== path efficiency by hour of day against 200 sign-flipped paths ==")
    print("effRatio = sum|net| / sum travel over the bucket. The flip keeps every |r| in place,")
    print("so travel, volatility clustering and the hour profile are identical and only direction moves.")
    print(f"{'bucket':>8} {'n':>7} {'effRatio':>9} {'flipP50':>9} {'flipP95':>9} {'pct':>6} {'walk':>8}")
    for j, (name, mask) in enumerate(buckets):
        m = ok & mask
        if m.sum() < 200:
            continue
        er = real_net[m].sum() / trav[m].sum()
        col = np.sort(flips[:, j])
        pct = 100.0 * (col < er).sum() / len(col)
        print(
            f"{name:>8} {int(m.sum()):>7} {er:>9.4f} {np.median(col):>9.4f} "
            f"{col[int(0.95 * (len(col) - 1))]:>9.4f} {pct:>5.0f}% {1/np.sqrt(h):>8.4f}"
        )

    # Whole window, one number, and the same for a few named slices.
    named = {
        "all bars": np.ones(n, dtype=bool),
        "h09-13 UTC": (hour >= 9) & (hour <= 13),
        "h12-16 UTC": (hour >= 12) & (hour <= 16),
        "h00-06 UTC": hour <= 6,
        "h17-23 UTC": hour >= 17,
    }
    print()
    print("== the same for named slices ==")
    print(f"{'slice':>14} {'n':>8} {'effRatio':>9} {'flipP50':>9} {'flipP95':>9} {'pct':>6}")
    for name, mask in named.items():
        m = ok & mask
        col = np.empty(seeds)
        rng2 = np.random.default_rng(20260923)
        for s in range(seeds):
            signs = rng2.integers(0, 2, size=n) * 2 - 1
            net = nets(mag * signs)
            col[s] = net[m].sum() / trav[m].sum()
        col.sort()
        er = real_net[m].sum() / trav[m].sum()
        pct = 100.0 * (col < er).sum() / len(col)
        print(f"{name:>14} {int(m.sum()):>8} {er:>9.4f} {np.median(col):>9.4f} {col[int(0.95*(seeds-1))]:>9.4f} {pct:>5.0f}%")


if __name__ == "__main__":
    main()
