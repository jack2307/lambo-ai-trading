# Latency, whipsaw, information and anchor robustness for each bias
# definition. Read-only; prints tables the decision note pastes.
import datetime as dt
import math
import os
import random
import statistics as st
import sys

import pyarrow.parquet as pq

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from bias_defs import (UP, DOWN, FLAT, T, O, H, L, C, all_definitions, resample, atr)

# WHERE THE BARS ARE. Set FD_BARS to the directory holding XAUUSD-<tf>.parquet.
# Defaults to this repo's own data/bars, which is what a desktop checkout has.
# The 2026-09-18 study used a read-only copy of the VPS export; the note
# records that path's fingerprints so the tables can be reproduced against the
# same bytes rather than against whatever is in data/bars today.
V = os.environ.get("FD_BARS") or os.path.join(
    os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "data", "bars")


def load(path):
    t = pq.read_table(path)
    c = {k: t.column(k).to_pylist() for k in ("time", "open", "high", "low", "close")}
    rows = sorted(zip(*(c[k] for k in ("time", "open", "high", "low", "close"))))
    return [(r[0].replace(tzinfo=None), r[1], r[2], r[3], r[4]) for r in rows]


# ------------------------------------------------------------- 1. LATENCY
#
# Latency needs a definition of "a turn happened here" that none of the
# candidates supplies, or each would be graded against itself. The reference
# is an ACAUSAL zigzag: computed over the whole series with hindsight, it
# marks the turning points a chart reader would point at afterwards and says
# nothing about whether they were knowable at the time. That is exactly what
# is wanted - the question is how long each causal definition takes to agree
# with something only hindsight can see.
def reference_turns(bars, k=4.0, n=14):
    """Hindsight zigzag pivots: (index, new_direction). k x ATR reversal."""
    a = atr(bars, n)
    med = st.median([v for v in a if v is not None])
    thr = k * med
    piv, direction = [], 0
    ext_i, ext_p = 0, bars[0][C]
    for i, b in enumerate(bars):
        if direction >= 0 and b[H] > ext_p:
            ext_i, ext_p = i, b[H]
        if direction <= 0 and b[L] < ext_p:
            ext_i, ext_p = i, b[L]
        if direction >= 0 and ext_p - b[L] > thr:
            piv.append((ext_i, -1))
            direction, ext_i, ext_p = -1, i, b[L]
        elif direction <= 0 and b[H] - ext_p > thr:
            piv.append((ext_i, +1))
            direction, ext_i, ext_p = +1, i, b[H]
    return piv


def latency(labels, turns, horizon=400):
    """Bars from each reference turn to the label agreeing with it.

    A turn the label never agrees with before the NEXT turn is a miss, and is
    counted rather than dropped - dropping them would flatter a slow
    definition by deleting the turns it slept through.
    """
    want = {+1: UP, -1: DOWN}
    lat, missed = [], 0
    for j, (i, d) in enumerate(turns):
        limit = turns[j + 1][0] if j + 1 < len(turns) else min(len(labels), i + horizon)
        hit = None
        for t in range(i, min(limit, len(labels))):
            if labels[t] == want[d]:
                hit = t - i
                break
        if hit is None:
            missed += 1
        else:
            lat.append(hit)
    return lat, missed, len(turns)


# ------------------------------------------------------------- 2. WHIPSAW
def whipsaw(labels):
    """Flips per 100 bars, and how many are undone within three bars.

    FLAT is not a flip: a definition stepping aside and back has not changed
    its mind about direction. Only UP<->DOWN and X->FLAT->opposite count, so
    a definition is not punished for admitting it does not know.
    """
    directed = [(i, v) for i, v in enumerate(labels) if v in (UP, DOWN)]
    if len(directed) < 2:
        return 0.0, 0.0, 0
    flips = []
    for a, b in zip(directed, directed[1:]):
        if a[1] != b[1]:
            flips.append(b[0])
    reversed_fast = 0
    for j, f in enumerate(flips[:-1]):
        if flips[j + 1] - f <= 3:
            reversed_fast += 1
    per100 = len(flips) / len(labels) * 100
    frac = reversed_fast / len(flips) if flips else 0.0
    return per100, frac, len(flips)


# ---------------------------------------------------------- 3. INFORMATION
#
# Forward returns are taken over WALL-CLOCK time, not a bar count. This tape
# stops an hour a day and all weekend; "24 bars ahead" on H1 crosses the
# break and silently becomes 30 hours. A horizon counted in bars on a tape
# with a daily break is fault 11 in this desk's own list.
def forward_map(bars, hours, tol_hours):
    """index -> index of the bar closest to t + hours, or None if the gap is
    wrong (a weekend, a holiday, a hole)."""
    times = [b[T] for b in bars]
    out = [None] * len(bars)
    j = 0
    for i, t0 in enumerate(times):
        target = t0 + dt.timedelta(hours=hours)
        if j < i:
            j = i
        while j + 1 < len(times) and times[j + 1] <= target:
            j += 1
        if j > i and abs((times[j] - t0).total_seconds() / 3600.0 - hours) <= tol_hours:
            out[i] = j
    return out


def information(bars, labels, fwd, n_perm=400, seed=11):
    """Directional hit rate on NON-OVERLAPPING samples, against a circular
    shift null.

    Non-overlapping because 94% of overlapping windows share a day with
    another and the t would be inflated - the same trap the pair-residual
    study was closed on.

    The null CIRCULARLY SHIFTS the label series against the returns. A plain
    shuffle would destroy the label's own run structure and make the baseline
    far too tight: a definition that holds one call for forty bars would beat
    a shuffle trivially without carrying any information. A shift keeps every
    run intact and breaks only the alignment, which is the thing being
    tested.
    """
    pairs = []
    i = 0
    while i < len(bars):
        j = fwd[i]
        if j is None or labels[i] == FLAT:
            i += 1
            continue
        r = bars[j][C] - bars[i][C]
        pairs.append((i, 1 if labels[i] == UP else -1, r))
        i = j                       # non-overlapping
    if len(pairs) < 50:
        return None
    hits = [1 if (s > 0) == (r > 0) else 0 for _, s, r in pairs if r != 0]
    rate = sum(hits) / len(hits)
    signed = [s * r for _, s, r in pairs]
    mean_r = st.mean(signed)

    # circular-shift null on the same non-overlapping grid
    idx = [p[0] for p in pairs]
    rets = [p[2] for p in pairs]
    lab = [p[1] for p in pairs]
    rnd = random.Random(seed)
    worse = 0
    null_rates = []
    for _ in range(n_perm):
        k = rnd.randrange(1, len(lab))
        shifted = lab[k:] + lab[:k]
        h = [1 if (s > 0) == (r > 0) else 0 for s, r in zip(shifted, rets) if r != 0]
        nr = sum(h) / len(h)
        null_rates.append(nr)
        if nr >= rate:
            worse += 1
    pct = 100.0 * (1 - worse / n_perm)
    return {"n": len(pairs), "rate": rate, "mean_r": mean_r,
            "null_rate": st.mean(null_rates), "pct": pct}


def by_year(bars, labels, fwd):
    out = {}
    years = sorted({b[T].year for b in bars})
    for y in years:
        idx = [i for i, b in enumerate(bars) if b[T].year == y]
        if len(idx) < 200:
            continue
        lo, hi = idx[0], idx[-1]
        sub_bars = bars[lo:hi + 1]
        sub_lab = labels[lo:hi + 1]
        sub_fwd = [None if fwd[i] is None or fwd[i] > hi else fwd[i] - lo for i in range(lo, hi + 1)]
        r = information(sub_bars, sub_lab, sub_fwd, n_perm=200, seed=y)
        if r:
            out[y] = r
    return out
