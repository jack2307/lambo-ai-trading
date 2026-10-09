#!/usr/bin/env python3
"""F1d: split the windows four ways and look at the SIGN.

Registered as a dated note at the end of `docs/decisions/2026-10-09-lead-lag.md`,
with the prediction declared before this ran: the one cell that passed F1's
letter (`XAG->XAU` lag 8) has |t| < 2 on both windows, so it should flip sign
between adjacent quarters -- window artifact, not mechanism.

Read-only. 0 gate cells.
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import leadlag_precheck as P  # noqa: E402
from leadlag_precheck import load, ms_of  # noqa: E402

QUARTERS = [
    ("Q1 2010-06..2014-06", "2010-06-01", "2014-06-01"),
    ("Q2 2014-06..2018-06", "2014-06-01", "2018-06-01"),
    ("Q3 2018-06..2022-06", "2018-06-01", "2022-06-01"),
    ("Q4 2022-06..2026-06", "2022-06-01", "2026-06-01"),
]
HALVES = [("A 2010-06..2018-06", "2010-06-01", "2018-06-01"),
          ("B 2018-06..2026-06", "2018-06-01", "2026-06-01")]

# The two cells worth splitting: the one that passed F1's letter, and the only
# lag >= 1 cell anywhere with |t| > 2 on a full window.
CELLS = [("XAGDUKA", "XAUDUKA", 8, "passed F1's letter"),
         ("XAGDUKA", "XAUDUKA", 4, "the only |t| > 2 on a full window")]


def main() -> None:
    for s in ("XAGDUKA", "XAUDUKA"):
        P.CACHE[s] = load(s)
    for sa, sb, k, why in CELLS:
        print(f"\n## {sa}->{sb} lag {k} -- {why}")
        print(f"{'slice':<22} {'corr':>9} {'t':>7} {'n':>8} {'base':>9} {'base t':>7}")
        for name, a, b in HALVES + QUARTERS:
            lo, hi = ms_of(a), ms_of(b)
            r, t, n = P.cell(sa, sb, lo, hi, k)
            br, bt, _ = P.cell(sa, sb, lo, hi, k, baseline=True)
            if r is None:
                print(f"{name:<22}      null")
                continue
            print(f"{name:<22} {r:>+9.4f} {t:>+7.2f} {n:>8,} {br:>+9.4f} {bt:>+7.2f}")
        signs = []
        for name, a, b in QUARTERS:
            r, _, _ = P.cell(sa, sb, ms_of(a), ms_of(b), k)
            signs.append(None if r is None else (r > 0))
        flips = sum(1 for i in range(1, len(signs)) if signs[i] is not None
                    and signs[i - 1] is not None and signs[i] != signs[i - 1])
        print(f"  sign flips between adjacent quarters: {flips} of 3")


if __name__ == "__main__":
    main()
