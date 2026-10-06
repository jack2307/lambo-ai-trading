"""Amendment (1) of docs/decisions/2026-10-07-intrabar-resolution.md.

Section 8 of the registration claims the main table is a FLOOR on the artifact
rate for a mechanism that enters on expanding bars, because wider bars reach
both levels more often. This checks that claim instead of asserting it: the
same cells, split by the signal bar's true range over its ATR.

Descriptive, no gate. 8 stop sizes x 3 terciles x 2 sides, rr pooled, 4 h cap.
"""
import numpy as np

import intrabar_resolution as M

STOPS = [0.30, 0.378, 0.45, 0.50, 0.60, 0.714, 1.00, 1.50]
CAP_MS, HMAX = 14_400_000, 18

ratio = M.signal_range_over_atr()
q33, q67 = np.nanpercentile(ratio, [33.3, 66.7])
TERCILES = [("quiet   (TR/ATR < %.2f)" % q33, ratio < q33),
            ("middle", (ratio >= q33) & (ratio <= q67)),
            ("expansion (TR/ATR > %.2f)" % q67, ratio > q67)]


def cell(side, k, rr, mask):
    half = M.SPREAD / 2.0
    entry = M.o15[M.fills] + (half if side == "LONG" else -half)
    risk = k * M.sig_atr
    if side == "LONG":
        stop, target = entry - risk, entry + risk * rr
    else:
        stop, target = entry + risk, entry - risk * rr
    v15, exit_h, both, _raw = M.walk15(side, stop, target, CAP_MS, HMAX)
    resolved = np.isin(v15, (M.STOP, M.TARGET)) & mask
    idx = np.flatnonzero(resolved)
    which = M.fills[idx] + exit_h[idx]
    v1, _r1, same, neither = M.walk1m(side, which, stop[idx], target[idx])
    a = v15[idx]
    ok = ~same & ~neither
    dis = int((((a == M.STOP) & (v1 == M.TARGET)) | ((a == M.TARGET) & (v1 == M.STOP))).sum())
    return int(mask.sum()), int(resolved.sum()), int(both[mask].sum()), dis, int(same.sum())


print(f"signal bar TR/ATR terciles over {len(ratio)} entries: "
      f"33.3% = {q33:.3f}, 66.7% = {q67:.3f}, max = {np.nanmax(ratio):.2f}")
print(f"\n{'='*104}")
print("DISAGREEMENT RATE BY THE SIGNAL BAR'S RANGE  (4 h cap, both sides, "
      "rr 1.0/1.5/1.8 pooled, % of all trades in the tercile)")
print(f"{'='*104}")
hdr = "  ".join(f"{n.split(' ')[0]:>12}" for n, _ in TERCILES)
print(f"{'stop':>6}  {hdr}   floor claim")
for k in STOPS:
    out = []
    for _name, mask in TERCILES:
        tr = dis = 0
        for side in ("LONG", "SHORT"):
            for rr in M.RRS:
                t, _res, _b, d, _u = cell(side, k, rr, mask)
                tr += t
                dis += d
        out.append(100.0 * dis / tr)
    verdict = "HOLDS" if out[2] >= out[0] else "FAILS"
    print(f"{k:>6.3f}  " + "  ".join(f"{v:>11.2f}%" for v in out) +
          f"   expansion vs quiet: {out[2]-out[0]:+.2f} pts -> {verdict}")
print("\nThe claim under test: a mechanism that enters on expansion bars sits "
      "ABOVE the pooled table, so the pooled table is a floor for it.")
