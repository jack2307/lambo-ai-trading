#!/usr/bin/env python3
"""Lagged cross-correlation precheck for `agent/lead-lag`.

Registered in `docs/decisions/2026-10-09-lead-lag.md` section 4. This script is
READ-ONLY on the data directory: it opens parquet files and writes nothing.

The question it answers, and nothing else: at lag `k >= 1` bars of 15m, does
`corr(rA[t-k], rB[t])` carry MORE than the baseline `corr(rB[t-k], rB[t])` --
instrument B's own past. If it does not, the companion adds no information and
the axis closes here.

Three properties that make the number honest, each one a line of code below:

1. EXACT timestamp match. Never nearest, never forward-fill. Same rule as
   `fd_indicators::companion::aligned_change`, which matches `bar.time`
   exactly and `continue`s on a miss rather than taking the previous bar.
2. CONTIGUITY. A k-bar log return is accepted only when
   `time[t] - time[t-k] == k * 900_000 ms` in BOTH series. A change straddling
   a weekend or a daily break is refused, not reported.
3. NON-OVERLAPPING samples. The sample index steps by `k`, so neither series
   overlaps itself and `t = r * sqrt(n-2) / sqrt(1-r^2)` is not inflated by
   ~sqrt(overlap). `n` is printed beside every `t`.

Lag 0 is printed for reference and is NOT a lead-lag claim: two instruments
correlated at the same instant is ordinary and not tradable.
"""

from __future__ import annotations

import math
import sys
from pathlib import Path

import numpy as np
import pyarrow.parquet as pq

BAR_MS = 900_000  # 15m
DATA = Path(sys.argv[1] if len(sys.argv) > 1 else "/e/rust/flowdesk/data")

# Windows: exactly the desk's two, as `docs/decisions/2026-10-06-long-horizon-swapfree.md`
# spells them.
WINDOWS = [
    ("A 2010-06-01..2018-06-01", "2010-06-01", "2018-06-01"),
    ("B 2018-06-01..2026-06-01", "2018-06-01", "2026-06-01"),
]

# Declared before the first number, section 3 of the registration.
PAIRS = [
    ("XAG->XAU", "XAGDUKA", "XAUDUKA"),
    ("EUR->XAU", "EURDUKA", "XAUDUKA"),
    ("XAU->XAG", "XAUDUKA", "XAGDUKA"),
    ("BTC->XAU", "BTCUSDT", "XAUUSD"),
]
LAGS = [0, 1, 2, 4, 8]


def days_from_civil(y: int, m: int, d: int) -> int:
    y -= m <= 2
    era = (y if y >= 0 else y - 399) // 400
    yoe = y - era * 400
    doy = (153 * (m + (-3 if m > 2 else 9)) + 2) // 5 + d - 1
    doe = yoe * 365 + yoe // 4 - yoe // 100 + doy
    return era * 146097 + doe - 719468


def ms_of(iso: str) -> int:
    y, m, d = (int(x) for x in iso.split("-"))
    return days_from_civil(y, m, d) * 86_400_000


def load(symbol: str) -> tuple[np.ndarray, np.ndarray]:
    path = DATA / "bars" / f"{symbol}-15m.parquet"
    tbl = pq.read_table(path, columns=["time", "close"])
    t = np.asarray(tbl.column("time").to_numpy(), dtype=np.int64)
    c = np.asarray(tbl.column("close").to_numpy(), dtype=np.float64)
    order = np.argsort(t, kind="stable")
    t, c = t[order], c[order]
    keep = np.ones(len(t), dtype=bool)
    keep[1:] = t[1:] != t[:-1]  # drop duplicate stamps, keep the first
    return t[keep], c[keep]


def logret_k(t: np.ndarray, c: np.ndarray, k: int) -> np.ndarray:
    """k-bar log return ending at each bar, NaN where the span is not exactly
    k intervals (a break) or a price is not positive."""
    out = np.full(len(t), np.nan)
    if k == 0 or k >= len(t):
        return out
    span_ok = (t[k:] - t[:-k]) == k * BAR_MS
    a, b = c[:-k], c[k:]
    good = span_ok & (a > 0) & (b > 0)
    vals = np.full(len(t) - k, np.nan)
    vals[good] = np.log(b[good] / a[good])
    out[k:] = vals
    return out


def pearson_t(x: np.ndarray, y: np.ndarray) -> tuple[float | None, float | None, int]:
    m = np.isfinite(x) & np.isfinite(y)
    n = int(m.sum())
    if n < 30:
        return None, None, n
    xs, ys = x[m], y[m]
    xs = xs - xs.mean()
    ys = ys - ys.mean()
    den = math.sqrt(float((xs * xs).sum()) * float((ys * ys).sum()))
    if den <= 0.0:
        return None, None, n
    r = float((xs * ys).sum()) / den
    r = max(-1.0, min(1.0, r))
    tstat = None if abs(r) >= 1.0 else r * math.sqrt(n - 2) / math.sqrt(1 - r * r)
    return r, tstat, n


def grid(sa: str, sb: str, lo: int, hi: int):
    """The EXACT-match intersection of two series' timestamps inside [lo, hi),
    with each series' close on it. No nearest, no fill."""
    ta, ca = CACHE[sa]
    tb, cb = CACHE[sb]
    inter, ia, ib = np.intersect1d(ta, tb, assume_unique=True, return_indices=True)
    sel = (inter >= lo) & (inter < hi)
    return inter[sel], ca[ia[sel]], cb[ib[sel]]


def cell(sa: str, sb: str, lo: int, hi: int, k: int, baseline: bool = False):
    """corr(rA ending at t-k, rB ending at t), non-overlapping step k.

    Both returns are k-bar log returns on the SHARED grid, so `t-k` means k
    shared bars back. A shared-grid step is not always exactly k*BAR_MS -- the
    contiguity mask in `logret_k` refuses every sample where it is not, in both
    series and for both the A leg and the B leg.

    `baseline=True` replaces the LEAD series with B itself, on the SAME grid,
    the SAME step and the SAME contiguity rule: that is B against its own past,
    the number the companion has to beat."""
    tt, ca, cb = grid(sa, sb, lo, hi)
    if baseline:
        ca = cb
    if len(tt) < 4 * max(k, 1) + 40:
        return None, None, 0
    ra = logret_k(tt, ca, k if k > 0 else 1)
    rb = logret_k(tt, cb, k if k > 0 else 1)
    if k == 0:
        # Reference only: same-instant correlation of 1-bar returns.
        idx = np.arange(1, len(tt))
        return pearson_t(ra[idx], rb[idx])
    # A's return ends at index i-k; B's ends at index i. Both spans must be
    # exactly k intervals -- logret_k already NaNs the ones that are not.
    idx = np.arange(2 * k, len(tt), k)  # step k => non-overlapping
    return pearson_t(ra[idx - k], rb[idx])


CACHE: dict[str, tuple[np.ndarray, np.ndarray]] = {}


def main() -> None:
    symbols = sorted({s for _, a, b in PAIRS for s in (a, b)})
    print("# leadlag precheck -- read-only, writes nothing")
    print(f"# data: {DATA}")
    for s in symbols:
        t, c = load(s)
        CACHE[s] = (t, c)
        print(f"series {s}-15m: {len(t):,} bars {t[0]} .. {t[-1]}")
    print()

    header = f"{'pair':<10} {'window':<26} {'lag':>4} {'corr':>9} {'t':>9} {'n':>8} " \
             f"{'base_corr':>10} {'base_t':>9} {'base_n':>8} {'verdict':>10}"
    print(header)
    print("-" * len(header))

    rows = []
    for name, sa, sb in PAIRS:
        for wname, wlo, whi in WINDOWS:
            lo, hi = ms_of(wlo), ms_of(whi)
            for k in LAGS:
                r, tst, n = cell(sa, sb, lo, hi, k)
                # BASELINE: B against its own past, SAME script, SAME sample
                # grid (the A/B intersection), SAME step. Anything else is not
                # a comparison.
                br, bt, bn = cell(sa, sb, lo, hi, k, baseline=True)
                if r is None or br is None:
                    verdict = "null"
                elif k == 0:
                    verdict = "ref-only"
                elif abs(r) > abs(br):
                    verdict = "OVER"
                else:
                    verdict = "under"
                fmt = lambda v, w, p: ("null".rjust(w) if v is None else f"{v:>{w}.{p}f}")
                print(
                    f"{name:<10} {wname:<26} {k:>4} {fmt(r,9,4)} {fmt(tst,9,2)} {n:>8,} "
                    f"{fmt(br,10,4)} {fmt(bt,9,2)} {bn:>8,} {verdict:>10}"
                )
                rows.append((name, wname, k, r, br, verdict))
        print()

    # F1, exactly as registered: a pair-and-lag must beat its own baseline on
    # BOTH windows with the SAME sign.
    print("## F1 -- over baseline on BOTH windows, same sign")
    any_pass = False
    for name, sa, sb in PAIRS:
        for k in LAGS:
            if k == 0:
                continue
            got = [x for x in rows if x[0] == name and x[2] == k]
            if len(got) != 2 or any(x[3] is None or x[4] is None for x in got):
                print(f"  {name} lag {k}: null (not measurable on both windows)")
                continue
            over = all(abs(x[3]) > abs(x[4]) for x in got)
            same_sign = (got[0][3] > 0) == (got[1][3] > 0)
            if over and same_sign:
                any_pass = True
                print(f"  {name} lag {k}: PASS  corr {got[0][3]:+.4f} / {got[1][3]:+.4f} "
                      f"vs base {got[0][4]:+.4f} / {got[1][4]:+.4f}")
    if not any_pass:
        print("  no pair at any lag >= 1 beat its own baseline on both windows "
              "with the same sign => F1 FIRED, 0 gate cells.")


if __name__ == "__main__":
    main()
