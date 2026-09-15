"""Plant an edge of known size in the harness and see whether it is found.

    python scripts/quote_asymmetry_selftest.py

A null result is only worth as much as the instrument that produced it, and
this instrument has now shipped three faults of its own: a null that permuted
an already-signed return, a cooldown counted in rows instead of minutes, and a
gap guard that silently emptied two of four cells. Arguing that the fourth fault
is not there is weaker than testing for it.

So: keep every real thing — the real minutes, the real mid prices, the real fill
quotes, the real broker-clock hours, the real volatility — and replace ONLY the
signal column with

    os_synth = noise + beta * (the k-minute forward mid return)

which plants a directional edge of a size we choose. Then run the registered
machinery unchanged. If the harness cannot recover a planted edge the size of
the one the study reported, the study's "no effect" means "no power" and the
record must say so. If it recovers it easily, the failure is about the market.

beta = 0 is run too, and repeatedly: a harness that scores pure noise above the
gate is broken in the other direction.
"""

from __future__ import annotations

import argparse
import sys
import warnings
from pathlib import Path

import numpy as np
import pandas as pd

warnings.filterwarnings("ignore")
sys.path.insert(0, str(Path(__file__).resolve().parent))
from quote_asymmetry import load, zscore, fires, matched_null, cluster_t  # noqa: E402


def forward_mid_bp(df: pd.DataFrame, k: int, lag: int) -> np.ndarray:
    """The same forward move the measurement scores, in bp, aligned to the
    signal minute — this is what gets planted into the signal."""
    bid, ask = df[f"bid_{lag}s"].to_numpy(), df[f"ask_{lag}s"].to_numpy()
    mid = (bid + ask) / 2.0
    out = np.full(len(df), np.nan)
    out[:-k] = 1e4 * (mid[k:] - mid[:-k]) / mid[:-k]
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--symbol", default="XAUUSD.sc")
    ap.add_argument("--from", dest="start", default="2026-01-20")
    ap.add_argument("--to", dest="end", default="2026-06-16")
    ap.add_argument("--lag", type=int, default=1)
    ap.add_argument("--draws", type=int, default=20_000)
    ap.add_argument("--seed", type=int, default=20260915)
    a = ap.parse_args()
    lo, hi = pd.Timestamp(a.start, tz="UTC"), pd.Timestamp(a.end, tz="UTC")
    df = load(a.symbol, lo, hi)

    print(f"harness self-test, {a.symbol}, {lo.date()} -> {hi.date()}, fill lag {a.lag}s")
    print(f"real minutes, real quotes, real hours; ONLY the signal column is synthetic")
    print(f"{a.draws:,} permutation draws per row\n")
    print("the study's own measured gross edges, for comparison:")
    print("   W=1 k=5 +0.214 bp   W=1 k=30 +0.538 bp   W=5 k=5 +0.531 bp   W=5 k=30 +1.023 bp\n")

    rs = np.random.default_rng(a.seed)
    noise_sd = float(df["os_sum"].std())

    for w, k in ((1, 5), (5, 30)):
        fwd = forward_mid_bp(df, k, a.lag)
        fwd0 = np.nan_to_num(fwd, nan=0.0)
        print(f"--- W={w} k={k} ---")
        print(f"   {'planted':>9s} {'n':>6s} {'gross bp':>9s} {'net bp':>8s} {'t(day)':>7s} "
              f"{'pctile':>7s}   verdict")
        for beta_mult in (0.0, 0.0, 0.0, 0.02, 0.05, 0.10, 0.20):
            noise = rs.normal(0.0, noise_sd, len(df))
            synth = pd.Series(noise + beta_mult * noise_sd * fwd0 / max(np.std(fwd0), 1e-9),
                              index=df.index)
            z = zscore(synth, df["contig"], w)
            f = fires(df, z, 2.0, k, a.lag)
            n = len(f["sign"])
            if n < 30:
                print(f"   {beta_mult:9.2f} {n:6d}   too few")
                continue
            _, pct, _ = matched_null(f["sign"], f["long"], f["short"], f["vol"], f["hour"],
                                     a.draws, a.seed)
            g, net = f["gross"].mean(), f["ret"].mean()
            verdict = "FOUND" if pct >= 97.5 else "missed"
            if beta_mult == 0.0:
                verdict = "ok (noise below gate)" if pct < 97.5 else "BROKEN: noise cleared the gate"
            print(f"   {beta_mult:9.2f} {n:6d} {g:+9.4f} {net:+8.4f} "
                  f"{cluster_t(f['gross'], f['day']):+7.2f} {pct:7.2f}   {verdict}")
        print()

    print("read it this way: find the planted row whose gross bp is closest to the study's")
    print("own measured gross for that cell, and look at its percentile. That is the power")
    print("the instrument actually had at the effect size the data actually showed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
