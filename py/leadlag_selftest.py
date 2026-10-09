#!/usr/bin/env python3
"""Does the precheck's ruler WORK? Four checks, before any conclusion is drawn
from `leadlag_precheck.py` saying "no lead-lag".

A falsifier that cannot fire the other way is not a falsifier. These run on
synthetic series where the answer is known by construction:

1. POSITIVE CONTROL. A series built so that A genuinely leads B by one bar must
   come back with a large lag-1 correlation and a baseline near zero. If this
   fails, "no pair beat its baseline" means the instrument is broken, not that
   the market has no lead-lag.
2. LOOK-AHEAD CONTROL. Feed the lead series SHIFTED THE WRONG WAY (A[t] = B's
   future) and the measurement must come back enormous -- proving it would have
   caught a look-ahead rather than quietly reporting it as an edge.
3. CAUSALITY. Truncating the companion at index m must not change any value
   computed from samples ending at or before m. Same property as
   `companion.rs::a_truncated_primary_is_a_prefix_and_so_is_a_truncated_companion`.
4. OVERLAP INFLATION. The same correlation sampled with step 1 instead of step
   k must inflate |t| by roughly sqrt(k) -- the desk's claim about overlapping
   windows, checked rather than repeated.
"""

from __future__ import annotations

import math
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import leadlag_precheck as P  # noqa: E402

BAR_MS = P.BAR_MS
FAIL = 0


def check(name: str, ok: bool, detail: str) -> None:
    global FAIL
    print(f"  [{'PASS' if ok else 'FAIL'}] {name}: {detail}")
    if not ok:
        FAIL += 1


def synth(n: int, lead_bars: int, strength: float, seed: int = 7):
    """B's return at t is `strength` x A's return at t-lead_bars plus noise;
    A is i.i.d. so the baseline (B on its own past) is near zero only through
    the part B inherits."""
    rng = np.random.default_rng(seed)
    t = np.arange(n, dtype=np.int64) * BAR_MS
    ra = rng.normal(0.0, 0.001, n)
    rb = rng.normal(0.0, 0.001, n)
    if lead_bars > 0:
        rb[lead_bars:] += strength * ra[:-lead_bars]
    ca = 1000.0 * np.exp(np.cumsum(ra))
    cb = 100.0 * np.exp(np.cumsum(rb))
    return t, ca, cb


def install(t, ca, cb) -> None:
    P.CACHE["SYNA"] = (t, ca)
    P.CACHE["SYNB"] = (t, cb)


def main() -> None:
    print("# leadlag selftest -- synthetic, no disk read")
    n = 60_000
    lo, hi = -1, 10**18

    print("\n## 1. positive control: A leads B by 1 bar, strength 0.8")
    t, ca, cb = synth(n, 1, 0.8)
    install(t, ca, cb)
    r1, t1, n1 = P.cell("SYNA", "SYNB", lo, hi, 1)
    b1, bt1, _ = P.cell("SYNA", "SYNB", lo, hi, 1, baseline=True)
    check("lag 1 corr is large", r1 is not None and r1 > 0.4, f"corr {r1:+.4f} t {t1:+.1f} n {n1:,}")
    check("baseline stays small", b1 is not None and abs(b1) < 0.1, f"base corr {b1:+.4f} t {bt1:+.2f}")
    check("companion beats baseline", abs(r1) > abs(b1), f"{abs(r1):.4f} > {abs(b1):.4f}")
    r2, t2, _ = P.cell("SYNA", "SYNB", lo, hi, 2)
    check("lag 2 is weaker than lag 1", abs(r2) < abs(r1), f"lag2 corr {r2:+.4f} t {t2:+.2f}")

    print("\n## 2. look-ahead control: lead series IS B's future")
    t, ca, cb = synth(n, 1, 0.8, seed=11)
    # A[t] := B[t+1]: reading it at t is reading one bar ahead.
    bad = np.concatenate([cb[1:], cb[-1:]])
    install(t, bad, cb)
    rla, tla, _ = P.cell("SYNA", "SYNB", lo, hi, 1)
    # r hits exactly 1.0 here -- A[t-1] IS B's return at t -- so `t` is None by
    # design (`pearson_t` refuses a t at |r| >= 1). That is the loudest possible
    # signal, not a missing one.
    check("look-ahead shows up as an enormous number", rla is not None and abs(rla) > 0.5,
          f"corr {rla:+.6f} t {'null (|r|=1)' if tla is None else f'{tla:+.1f}'} "
          "-- the ruler would have caught it")

    print("\n## 3. causality: truncating the companion changes nothing before the cut")
    t, ca, cb = synth(n, 1, 0.8, seed=3)
    install(t, ca, cb)
    cut = 20_000
    t_cut_ms = int(t[cut])
    full_r, full_t, full_n = P.cell("SYNA", "SYNB", lo, t_cut_ms, 4)
    P.CACHE["SYNA"] = (t[:cut], ca[:cut])  # companion truncated AT the cut
    trunc_r, trunc_t, trunc_n = P.cell("SYNA", "SYNB", lo, t_cut_ms, 4)
    check("corr unchanged", full_r is not None and abs(full_r - trunc_r) < 1e-12,
          f"{full_r:+.12f} vs {trunc_r:+.12f}")
    check("sample count unchanged", full_n == trunc_n, f"n {full_n:,} vs {trunc_n:,}")
    # And the primary side, which is the other way a two-series read can cheat.
    t2, ca2, cb2 = synth(n, 1, 0.8, seed=3)
    P.CACHE["SYNA"] = (t2, ca2)
    P.CACHE["SYNB"] = (t2[:cut], cb2[:cut])
    pr, _, pn = P.cell("SYNA", "SYNB", lo, t_cut_ms, 4)
    check("primary truncation changes nothing either", abs(full_r - pr) < 1e-12 and pn == full_n,
          f"{full_r:+.12f} vs {pr:+.12f}, n {pn:,}")

    print("\n## 4. overlap inflation: step 1 vs step k on the SAME correlation")
    t, ca, cb = synth(n, 1, 0.15, seed=5)
    install(t, ca, cb)
    for k in (2, 4, 8):
        rk, tk, nk = P.cell("SYNA", "SYNB", lo, hi, k)
        # Same quantity, overlapping samples: step 1 instead of k.
        tt, a, b = P.grid("SYNA", "SYNB", lo, hi)
        ra, rb = P.logret_k(tt, a, k), P.logret_k(tt, b, k)
        idx = np.arange(2 * k, len(tt), 1)
        ro, to, no = P.pearson_t(ra[idx - k], rb[idx])
        ratio = abs(to) / abs(tk) if tk else float("nan")
        print(f"  lag {k}: step-{k} t {tk:+.2f} (n {nk:,})  |  step-1 t {to:+.2f} (n {no:,})  "
              f"ratio {ratio:.2f}  sqrt(k) {math.sqrt(k):.2f}")
        check(f"overlap inflates |t| at lag {k}", ratio > 1.3, f"ratio {ratio:.2f}")

    print(f"\n{'ALL CHECKS PASSED' if FAIL == 0 else f'{FAIL} CHECK(S) FAILED'}")
    sys.exit(1 if FAIL else 0)


if __name__ == "__main__":
    main()
