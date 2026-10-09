#!/usr/bin/env python3
"""Does the lagged correlation carry enough GROSS to pay a spread?

Second half of the `agent/lead-lag` precheck, registered as a dated note at the
end of `docs/decisions/2026-10-09-lead-lag.md`. Still a disk measurement: 0
gate cells, nothing written.

`leadlag_precheck.py` found every lag >= 1 correlation inside |0.019|. A
correlation that small can still be "over the baseline" while carrying no
tradable gross, so this script converts it into the only unit the gate reads:
R per trade, where R = 1.5 x ATR(15m) of the traded instrument -- the same
ruler the desk's published cost/R numbers use.

The rule measured is the simplest honest expression of a lead-lag claim:

    at shared bar i, sign(A's k-bar log return ENDING AT i) -> hold B for the
    next k bars.

A ends at i and B's move is [i, i+k], so nothing is read before it closes.
Samples step by k, so no sample overlaps another. The contiguity rule from
`leadlag_precheck.py` applies to both legs: a span that is not exactly
k * 900_000 ms is refused.

Drift is subtracted, not assumed away: the same book's long share times the
desk's measured per-session gold drift is printed beside the gross, because an
edge equal to that is exposure, not lead-lag.
"""

from __future__ import annotations

import math
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from leadlag_precheck import BAR_MS, CACHE, WINDOWS, grid, load, logret_k, ms_of  # noqa: E402

# Round-trip spread, USD per oz / per unit, read from `config/default.toml`
# READ from `config/default.toml`: [markets.xauduka] 0.28, [markets.xagduka] 0.021,
# [markets.eurduka] 0.00014. Not assumed.
SPREAD = {"XAUDUKA": 0.28, "XAGDUKA": 0.021, "EURDUKA": 0.00014}
# Desk-measured gold drift per NY session, both halves (addendum 5 section B).
DRIFT_R = {"A 2010-06-01..2018-06-01": -0.0040, "B 2018-06-01..2026-06-01": +0.0205}

PAIRS = [("XAG->XAU", "XAGDUKA", "XAUDUKA"), ("EUR->XAU", "EURDUKA", "XAUDUKA")]
LAGS = [1, 2, 4, 8]


def rma(x: np.ndarray, period: int) -> np.ndarray:
    """Wilder's smoothing, matching `fd_indicators::rma` -- the first value is
    the simple mean of the first `period`, then recursive."""
    out = np.full(len(x), np.nan)
    if period == 0 or len(x) < period:
        return out
    acc = float(np.nansum(x[:period])) / period
    out[period - 1] = acc
    for i in range(period, len(x)):
        acc = (acc * (period - 1) + x[i]) / period
        out[i] = acc
    return out


def atr_on(t: np.ndarray, h: np.ndarray, l: np.ndarray, c: np.ndarray, period: int = 14) -> np.ndarray:
    tr = np.empty(len(c))
    tr[0] = h[0] - l[0]
    tr[1:] = np.maximum(h[1:] - l[1:], np.maximum(np.abs(h[1:] - c[:-1]), np.abs(l[1:] - c[:-1])))
    return rma(tr, period)


def load_hlc(symbol: str):
    import pyarrow.parquet as pq

    path = Path(sys.argv[1] if len(sys.argv) > 1 else "/e/rust/flowdesk/data") / "bars" / f"{symbol}-15m.parquet"
    tbl = pq.read_table(path, columns=["time", "high", "low", "close"])
    t = np.asarray(tbl.column("time").to_numpy(), dtype=np.int64)
    order = np.argsort(t, kind="stable")
    t = t[order]
    h = np.asarray(tbl.column("high").to_numpy(), dtype=np.float64)[order]
    lo = np.asarray(tbl.column("low").to_numpy(), dtype=np.float64)[order]
    c = np.asarray(tbl.column("close").to_numpy(), dtype=np.float64)[order]
    keep = np.ones(len(t), dtype=bool)
    keep[1:] = t[1:] != t[:-1]
    return t[keep], h[keep], lo[keep], c[keep]


def main() -> None:
    print("# leadlag gross ceiling -- read-only, 0 gate cells")
    hlc: dict[str, tuple] = {}
    for _, sa, sb in PAIRS:
        for s in (sa, sb):
            if s not in CACHE:
                CACHE[s] = load(s)
            if s not in hlc:
                hlc[s] = load_hlc(s)

    # Calibration: 1.5 ATR(15m) must reproduce the desk's published cost/R
    # before any number below is believed.
    print("\n## ruler calibration -- spread / (1.5 x ATR(15m)), vs published")
    for sym, pub in (("XAUDUKA", 4.04), ("XAGDUKA", 17.32), ("EURDUKA", 14.81)):
        t, h, l, c = hlc[sym]
        a = atr_on(t, h, l, c)
        for wname, wlo, whi in WINDOWS:
            m = (t >= ms_of(wlo)) & (t < ms_of(whi)) & np.isfinite(a)
            stop = 1.5 * float(np.median(a[m]))
            print(f"  {sym:<9} {wname:<26} median 1.5xATR = {stop:.4f}  "
                  f"cost/R = {100 * SPREAD[sym] / stop:6.2f}%   (published {pub:.2f}%)")

    header = (f"{'pair':<10} {'window':<26} {'lag':>4} {'n':>7} {'grossR':>9} {'t':>7} "
              f"{'long%':>6} {'driftR':>9} {'grossR-drift':>13} {'costR':>7} {'netR':>9}")
    print("\n## gross per trade, R = 1.5 x ATR(15m) of the TRADED instrument")
    print(header)
    print("-" * len(header))

    for name, sa, sb in PAIRS:
        for wname, wlo, whi in WINDOWS:
            lo, hi = ms_of(wlo), ms_of(whi)
            tt, ca, cb = grid(sa, sb, lo, hi)
            # ATR of the traded instrument, on the shared grid.
            tb, hb, lb, cbf = hlc[sb]
            idx_b = np.searchsorted(tb, tt)
            atr_b = atr_on(tb, hb, lb, cbf)[idx_b]
            for k in LAGS:
                ra = logret_k(tt, ca, k)  # A's return ENDING at i
                # B's forward move over the next k shared bars, same contiguity
                # rule: shift the backward return forward by k.
                rb_back_pts = np.full(len(tt), np.nan)
                ok = (tt[k:] - tt[:-k]) == k * BAR_MS
                d = np.full(len(tt) - k, np.nan)
                d[ok] = cb[k:][ok] - cb[:-k][ok]
                rb_back_pts[k:] = d
                fwd_pts = np.full(len(tt), np.nan)
                fwd_pts[:-k] = rb_back_pts[k:]  # move over [i, i+k]
                idx = np.arange(k, len(tt) - k, k)  # non-overlapping, A warm
                sig = np.sign(ra[idx])
                stop = 1.5 * atr_b[idx]
                gross = sig * fwd_pts[idx] / stop
                m = np.isfinite(gross) & (sig != 0) & (stop > 0)
                n = int(m.sum())
                if n < 40:
                    print(f"  {name:<10} {wname:<26} {k:>4} {n:>7,}  null (under 40 samples)")
                    continue
                g = gross[m]
                mean = float(g.mean())
                sd = float(g.std(ddof=1))
                tstat = mean / (sd / math.sqrt(n)) if sd > 0 else float("nan")
                long_share = float((sig[m] > 0).mean())
                # Exposure, not lead-lag: the same book's long share against the
                # window's own measured drift. Each sample holds k bars of 15m;
                # a NY session is 24 bars of 15m on this feed's trading day.
                sessions = k / 24.0
                drift = (2.0 * long_share - 1.0) * DRIFT_R[wname] * sessions
                cost = SPREAD[sb] / float(np.median(stop[m]))
                print(f"{name:<10} {wname:<26} {k:>4} {n:>7,} {mean:>+9.5f} {tstat:>+7.2f} "
                      f"{100 * long_share:>5.1f}% {drift:>+9.5f} {mean - drift:>+13.5f} "
                      f"{cost:>7.4f} {mean - drift - cost:>+9.5f}")
        print()


if __name__ == "__main__":
    main()
